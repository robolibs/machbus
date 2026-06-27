//! File Server *server* as a [`Plugin`]. Wraps [`FileServer`]: routes
//! `PGN_FILE_CLIENT_TO_SERVER`, ships responses/status on tick, and surfaces
//! client connect/file activity as [`FsServerEvent`].

use crate::isobus::fs::{FSOutbound, FileServer, FileServerConfig};
use crate::net::pgn_defs::PGN_FILE_CLIENT_TO_SERVER;
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, FsServerEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_FILE_CLIENT_TO_SERVER];

/// File Server (server role) plugin.
pub struct FsServer {
    server: FileServer,
    events: Rc<RefCell<Vec<FsServerEvent>>>,
    last_tick: Option<Instant>,
}

impl FsServer {
    /// Create with the given server config.
    #[must_use]
    pub fn new(config: FileServerConfig) -> Self {
        let mut server = FileServer::new(config);
        let events = Rc::new(RefCell::new(Vec::new()));
        wire_events(&mut server, &events);
        Self {
            server,
            events,
            last_tick: None,
        }
    }

    /// Add an in-memory file the server exposes.
    ///
    /// # Errors
    /// Propagates server errors (e.g. bad path).
    pub fn add_file(&mut self, path: impl Into<String>, data: Vec<u8>, attrs: u8) -> Result<()> {
        self.server.add_file(path, data, attrs)
    }

    /// Add an in-memory directory.
    ///
    /// # Errors
    /// Propagates server errors.
    pub fn add_directory(&mut self, path: impl Into<String>) -> Result<()> {
        self.server.add_directory(path)
    }

    /// Set the advertised volume name.
    ///
    /// # Errors
    /// Propagates server errors.
    pub fn set_volume_name(&mut self, name: impl Into<String>) -> Result<()> {
        self.server.set_volume_name(name)
    }

    /// Direct access to the underlying server.
    pub fn server_mut(&mut self) -> &mut FileServer {
        &mut self.server
    }

    fn ship(out: FSOutbound, ctx: &mut PluginCtx<'_>) {
        ctx.send(
            out.pgn,
            out.data,
            out.dest.unwrap_or(BROADCAST_ADDRESS),
            Priority::Default,
        );
    }

    fn drain_events(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.events.borrow_mut().drain(..) {
            ctx.emit(Event::FsServer(event));
        }
    }
}

impl Plugin for FsServer {
    fn name(&self) -> &'static str {
        "fs_server"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_FILE_CLIENT_TO_SERVER) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        let outbound: Vec<_> = self.server.handle_client_message(msg).into_iter().collect();
        for out in outbound {
            Self::ship(out, ctx);
        }
        self.drain_events(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        self.last_tick = Some(now);

        let outbound: Vec<_> = self.server.update(elapsed).into_iter().collect();
        for out in outbound {
            Self::ship(out, ctx);
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

fn wire_events(server: &mut FileServer, sink: &Rc<RefCell<Vec<FsServerEvent>>>) {
    let s = sink.clone();
    server
        .on_client_connected
        .subscribe(move |&addr| s.borrow_mut().push(FsServerEvent::ClientConnected(addr)));
    let s = sink.clone();
    server.on_client_disconnected.subscribe(move |&addr| {
        s.borrow_mut().push(FsServerEvent::ClientDisconnected(addr));
    });
    let s = sink.clone();
    server.on_file_opened.subscribe(move |(addr, path)| {
        s.borrow_mut().push(FsServerEvent::FileOpened {
            client: *addr,
            path: path.clone(),
        });
    });
    let s = sink.clone();
    server.on_file_closed.subscribe(move |&(addr, handle)| {
        s.borrow_mut().push(FsServerEvent::FileClosed {
            client: addr,
            handle,
        });
    });
}
