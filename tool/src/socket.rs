//! Raw Linux SocketCAN socket: open, send, and receive classical CAN
//! frames directly through `libc`, mirroring how the upstream `can-utils`
//! tools work. No dependency on an external CAN backend crate.
//!
//! On non-Linux targets every method returns a configuration error, so the
//! binary still compiles but the live-socket subcommands explain clearly.

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

use crate::can::RawFrame;

/// `PF_CAN` / `AF_CAN` (Linux only; defined here so the constants resolve
/// uniformly regardless of the host's libc headers).
#[cfg(target_os = "linux")]
const PF_CAN: libc::c_int = 29;
#[cfg(target_os = "linux")]
const AF_CAN: libc::c_int = 29;
/// `CAN_RAW` socket protocol.
#[cfg(target_os = "linux")]
const CAN_RAW: libc::c_int = 1;

/// Maximum interface-name length (Linux `IFNAMSIZ` minus the NUL).
const MAX_IFNAME_LEN: usize = 15;

/// Open SocketCAN raw socket bound to `iface`.
///
/// Pass `"any"` to receive from every CAN interface (the socket is left
/// unbound to a specific index). Sending is only meaningful when bound to
/// a concrete interface.
pub fn open(iface: &str) -> io::Result<RawSocket> {
    if iface.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty interface name",
        ));
    }
    if iface.len() > MAX_IFNAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("interface name '{iface}' exceeds {MAX_IFNAME_LEN} bytes"),
        ));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = iface;
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "SocketCAN is only available on Linux",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        let fd = unsafe { libc::socket(PF_CAN, libc::SOCK_RAW, CAN_RAW) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };

        let ifindex: libc::c_int = if iface.eq_ignore_ascii_case("any") {
            0
        } else {
            resolve_ifindex(&fd, iface)?
        };

        // Bind to the address family + interface index (0 = all interfaces).
        let mut addr: libc::sockaddr_can = unsafe { std::mem::zeroed() };
        addr.can_family = AF_CAN as u16;
        addr.can_ifindex = ifindex;
        let rc = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                &addr as *const _ as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_can>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(RawSocket {
            fd,
            iface: iface.to_string(),
            ifindex,
        })
    }
}

#[cfg(target_os = "linux")]
fn resolve_ifindex(fd: &OwnedFd, iface: &str) -> io::Result<libc::c_int> {
    let c_name = CString::new(iface)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "interface name contains NUL"))?;
    let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
    let bytes = c_name.as_bytes();
    let n = bytes.len().min(ifr.ifr_name.len().saturating_sub(1));
    ifr.ifr_name[..n].copy_from_slice(unsafe {
        std::slice::from_raw_parts(bytes.as_ptr() as *const libc::c_char, n)
    });
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), libc::SIOCGIFINDEX as _, &mut ifr) };
    if rc < 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "interface '{iface}' not found: {}",
                io::Error::last_os_error()
            ),
        ));
    }
    // SAFETY: `ifru_ifindex` is a plain c_int written by the kernel.
    Ok(unsafe { ifr.ifr_ifru.ifru_ifindex })
}

/// Owned SocketCAN raw socket.
pub struct RawSocket {
    fd: OwnedFd,
    iface: String,
    #[allow(dead_code)]
    ifindex: libc::c_int,
}

impl RawSocket {
    /// Send one classical CAN frame.
    pub fn send(&self, frame: &RawFrame) -> io::Result<()> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = frame;
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "SocketCAN is only available on Linux",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            if self.iface.eq_ignore_ascii_case("any") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "cannot send on the 'any' interface — name a concrete device",
                ));
            }
            let n = unsafe {
                libc::write(
                    self.fd.as_raw_fd(),
                    frame as *const RawFrame as *const libc::c_void,
                    std::mem::size_of::<RawFrame>(),
                )
            };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }

    /// Receive one frame, waiting up to `timeout`. Returns `Ok(None)` on
    /// timeout so callers can poll for cancellation between frames.
    pub fn recv(&self, timeout: Duration) -> io::Result<Option<(RawFrame, String)>> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = timeout;
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "SocketCAN is only available on Linux",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            let mut pfd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ms = timeout.as_millis().min(i32::MAX as u128) as libc::c_int;
            let pr = unsafe { libc::poll(&mut pfd, 1, ms) };
            if pr < 0 {
                return Err(io::Error::last_os_error());
            }
            if pr == 0 {
                return Ok(None);
            }

            let mut frame = RawFrame::default();
            let mut addr: libc::sockaddr_can = unsafe { std::mem::zeroed() };
            let mut len: libc::socklen_t =
                std::mem::size_of::<libc::sockaddr_can>() as libc::socklen_t;
            let n = unsafe {
                libc::recvfrom(
                    self.fd.as_raw_fd(),
                    &mut frame as *mut RawFrame as *mut libc::c_void,
                    std::mem::size_of::<RawFrame>(),
                    0,
                    &mut addr as *mut _ as *mut libc::sockaddr,
                    &mut len,
                )
            };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            // For a bound socket the source interface is our own; for the
            // "any" socket resolve the name from the incoming ifindex.
            let iface = if addr.can_ifindex != 0 {
                ifindex_to_name(addr.can_ifindex).unwrap_or_else(|| self.iface.clone())
            } else {
                self.iface.clone()
            };
            Ok(Some((frame, iface)))
        }
    }

    /// Non-blocking receive: returns the next frame if one is immediately
    /// available, else `Ok(None)`. Used by the live VT pump so it never
    /// stalls the render loop.
    pub fn try_recv(&self) -> io::Result<Option<(RawFrame, String)>> {
        self.recv(Duration::ZERO)
    }
}

#[cfg(target_os = "linux")]
fn ifindex_to_name(ifindex: libc::c_int) -> Option<String> {
    let mut buf = [0u8; libc::IFNAMSIZ];
    let ptr = unsafe {
        libc::if_indextoname(
            ifindex as libc::c_uint,
            buf.as_mut_ptr() as *mut libc::c_char,
        )
    };
    if ptr.is_null() {
        return None;
    }
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Some(String::from_utf8_lossy(&buf[..len]).into_owned())
}
