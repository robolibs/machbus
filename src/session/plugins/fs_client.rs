//! File Server client as a [`Plugin`]. Wraps the pump-style [`FileClient`]:
//! routes `PGN_FILE_SERVER_TO_CLIENT`, ships CCM/keepalive/request frames on
//! tick, and surfaces async request/response results as [`FsEvent`]. Each
//! request method returns its TAN; the matching response arrives as an event.

use crate::isobus::fs::{
    FSClientOutbound, FileClient, FileClientConfig, FileHandle, FileServerProperties,
    FileServerStatus, TAN,
};
use crate::net::pgn_defs::PGN_FILE_SERVER_TO_CLIENT;
use crate::net::{Address, BROADCAST_ADDRESS, Error, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, FsEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_FILE_SERVER_TO_CLIENT];

/// File Server client plugin.
pub struct FsClient {
    client: FileClient,
    events: Rc<RefCell<Vec<FsEvent>>>,
    pending: Vec<FSClientOutbound>,
    last_tick: Option<Instant>,
}

impl FsClient {
    /// Create with the given client config.
    #[must_use]
    pub fn new(config: FileClientConfig) -> Self {
        let mut client = FileClient::new(config);
        let events = Rc::new(RefCell::new(Vec::new()));
        wire_events(&mut client, &events);
        Self {
            client,
            events,
            pending: Vec::new(),
            last_tick: None,
        }
    }

    /// Begin the connect handshake to a server address.
    ///
    /// # Errors
    /// Propagates client connect errors.
    pub fn connect_to(&mut self, server: Address) -> Result<()> {
        let out = self.client.try_connect_to_server(server)?;
        self.pending.push(out);
        Ok(())
    }

    /// Disconnect, flushing any close-file requests.
    pub fn disconnect(&mut self) {
        self.pending.extend(self.client.disconnect());
    }

    /// Whether the CCM handshake is complete.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    /// Most recent server properties, if known.
    #[must_use]
    pub fn server_properties(&self) -> Option<FileServerProperties> {
        self.client.server_properties()
    }

    /// Most recent server status, if known.
    #[must_use]
    pub fn server_status(&self) -> Option<FileServerStatus> {
        self.client.server_status()
    }

    /// Open a file; returns the TAN (response via [`FsEvent::OpenResponse`]).
    ///
    /// # Errors
    /// Not connected, or encode error.
    pub fn open(&mut self, path: &str, flags: u8) -> Result<TAN> {
        self.ensure_connected()?;
        let out = self.client.try_open_file(path, flags)?;
        Ok(self.issue(out))
    }

    /// Close a file; returns the TAN.
    ///
    /// # Errors
    /// Not connected, or encode error.
    pub fn close(&mut self, handle: FileHandle) -> Result<TAN> {
        self.ensure_connected()?;
        let out = self.client.try_close_file(handle)?;
        Ok(self.issue(out))
    }

    /// Read up to `count` bytes; returns the TAN.
    ///
    /// # Errors
    /// Not connected, or encode error.
    pub fn read(&mut self, handle: FileHandle, count: u16) -> Result<TAN> {
        self.ensure_connected()?;
        let out = self.client.try_read_file(handle, count)?;
        Ok(self.issue(out))
    }

    /// Write `data`; returns the TAN.
    ///
    /// # Errors
    /// Not connected, or encode error.
    pub fn write(&mut self, handle: FileHandle, data: &[u8]) -> Result<TAN> {
        self.ensure_connected()?;
        let out = self.client.try_write_file(handle, data)?;
        Ok(self.issue(out))
    }

    /// Seek to absolute `position`; returns the TAN.
    ///
    /// # Errors
    /// Not connected, or encode error.
    pub fn seek(&mut self, handle: FileHandle, position: u32) -> Result<TAN> {
        self.ensure_connected()?;
        let out = self.client.try_seek_file(handle, position)?;
        Ok(self.issue(out))
    }

    /// Request the current directory; returns the TAN.
    ///
    /// # Errors
    /// Not connected, or encode error.
    pub fn current_directory(&mut self) -> Result<TAN> {
        self.ensure_connected()?;
        let out = self.client.try_get_current_directory()?;
        Ok(self.issue(out))
    }

    /// Change directory; returns the TAN.
    ///
    /// # Errors
    /// Not connected, or encode error.
    pub fn change_directory(&mut self, path: &str) -> Result<TAN> {
        self.ensure_connected()?;
        let out = self.client.try_change_directory(path)?;
        Ok(self.issue(out))
    }

    /// Delete a file; returns the TAN.
    ///
    /// # Errors
    /// Not connected, or encode error.
    pub fn delete_file(&mut self, path: &str) -> Result<TAN> {
        self.ensure_connected()?;
        let out = self.client.try_delete_file(path)?;
        Ok(self.issue(out))
    }

    /// Direct access to the underlying client.
    pub fn client_mut(&mut self) -> &mut FileClient {
        &mut self.client
    }

    fn issue(&mut self, out: FSClientOutbound) -> TAN {
        let tan = out.data.get(1).copied().unwrap_or(0);
        self.pending.push(out);
        tan
    }

    fn ensure_connected(&self) -> Result<()> {
        if self.client.is_connected() {
            Ok(())
        } else {
            Err(Error::not_connected())
        }
    }

    fn drain_events(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.events.borrow_mut().drain(..) {
            ctx.emit(Event::Fs(event));
        }
    }
}

impl Plugin for FsClient {
    fn name(&self) -> &'static str {
        "fs_client"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_FILE_SERVER_TO_CLIENT) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        self.client.handle_server_response(msg);
        self.drain_events(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        self.last_tick = Some(now);

        let updated: Vec<_> = self.client.update(elapsed).into_iter().collect();
        self.pending.extend(updated);
        for out in self.pending.drain(..) {
            ctx.send(
                out.pgn,
                out.data,
                out.dest.unwrap_or(BROADCAST_ADDRESS),
                Priority::Default,
            );
        }
        self.drain_events(ctx);
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[allow(clippy::too_many_lines)]
fn wire_events(client: &mut FileClient, sink: &Rc<RefCell<Vec<FsEvent>>>) {
    let s = sink.clone();
    client
        .on_connected
        .subscribe(move |&()| s.borrow_mut().push(FsEvent::Connected));
    let s = sink.clone();
    client
        .on_disconnected
        .subscribe(move |&()| s.borrow_mut().push(FsEvent::Disconnected));
    let s = sink.clone();
    client
        .on_error
        .subscribe(move |&e| s.borrow_mut().push(FsEvent::Error(e)));
    let s = sink.clone();
    client.on_open_response.subscribe(move |(tan, result)| {
        s.borrow_mut().push(FsEvent::OpenResponse {
            tan: *tan,
            result: *result,
        });
    });
    let s = sink.clone();
    client
        .on_properties_response
        .subscribe(move |(tan, result)| {
            s.borrow_mut().push(FsEvent::PropertiesResponse {
                tan: *tan,
                result: *result,
            });
        });
    let s = sink.clone();
    client.on_status_response.subscribe(move |(tan, result)| {
        s.borrow_mut().push(FsEvent::StatusResponse {
            tan: *tan,
            result: *result,
        });
    });
    let s = sink.clone();
    client.on_close_response.subscribe(move |(tan, result)| {
        s.borrow_mut().push(FsEvent::CloseResponse {
            tan: *tan,
            result: *result,
        });
    });
    let s = sink.clone();
    client.on_read_response.subscribe(move |(tan, result)| {
        s.borrow_mut().push(FsEvent::ReadResponse {
            tan: *tan,
            result: result.clone(),
        });
    });
    let s = sink.clone();
    client.on_write_response.subscribe(move |(tan, result)| {
        s.borrow_mut().push(FsEvent::WriteResponse {
            tan: *tan,
            result: *result,
        });
    });
    let s = sink.clone();
    client.on_seek_response.subscribe(move |(tan, result)| {
        s.borrow_mut().push(FsEvent::SeekResponse {
            tan: *tan,
            result: *result,
        });
    });
    let s = sink.clone();
    client
        .on_current_directory_response
        .subscribe(move |(tan, result)| {
            s.borrow_mut().push(FsEvent::CurrentDirectoryResponse {
                tan: *tan,
                result: result.clone(),
            });
        });
    let s = sink.clone();
    client
        .on_change_directory_response
        .subscribe(move |(tan, result)| {
            s.borrow_mut().push(FsEvent::ChangeDirectoryResponse {
                tan: *tan,
                result: result.clone(),
            });
        });
    let s = sink.clone();
    client.on_move_response.subscribe(move |(tan, result)| {
        s.borrow_mut().push(FsEvent::MoveResponse {
            tan: *tan,
            result: *result,
        });
    });
    let s = sink.clone();
    client.on_delete_response.subscribe(move |(tan, result)| {
        s.borrow_mut().push(FsEvent::DeleteResponse {
            tan: *tan,
            result: *result,
        });
    });
    let s = sink.clone();
    client
        .on_file_attributes_response
        .subscribe(move |(tan, result)| {
            s.borrow_mut().push(FsEvent::FileAttributesResponse {
                tan: *tan,
                result: *result,
            });
        });
    let s = sink.clone();
    client
        .on_set_file_attributes_response
        .subscribe(move |(tan, result)| {
            s.borrow_mut().push(FsEvent::SetFileAttributesResponse {
                tan: *tan,
                result: *result,
            });
        });
    let s = sink.clone();
    client
        .on_file_date_time_response
        .subscribe(move |(tan, result)| {
            s.borrow_mut().push(FsEvent::FileDateTimeResponse {
                tan: *tan,
                result: *result,
            });
        });
    let s = sink.clone();
    client
        .on_initialize_volume_response
        .subscribe(move |(tan, result)| {
            s.borrow_mut().push(FsEvent::InitializeVolumeResponse {
                tan: *tan,
                result: *result,
            });
        });
    let s = sink.clone();
    client
        .on_volume_status
        .subscribe(move |&state| s.borrow_mut().push(FsEvent::VolumeStatus { state }));
}
