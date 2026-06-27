//! ISO 11783-13 File Server connection manager.
//!
//! Mirrors the C++ `machbus::isobus::fs::ConnectionManager`. Tracks
//! per-client state, timeouts, status-cadence pacing, and rate-
//! limited burst sends.

use alloc::vec::Vec;

use crate::net::types::Address;

/// Inactivity timeout (6 s).
pub const FS_CLIENT_TIMEOUT_MS: u32 = 6000;
/// Idle status broadcast cadence.
pub const FS_STATUS_IDLE_INTERVAL_MS: u32 = 2000;
/// Busy status broadcast cadence.
pub const FS_STATUS_BUSY_INTERVAL_MS: u32 = 200;
/// Maximum on-change status bursts per second.
pub const FS_MAX_STATUS_BURST_PER_SEC: u32 = 5;

/// One client's connection record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientConnection {
    pub client_address: Address,
    pub last_activity_ms: u32,
    pub connected: bool,
    pub open_file_count: u8,
}

impl Default for ClientConnection {
    fn default() -> Self {
        Self {
            client_address: 0xFF,
            last_activity_ms: 0,
            connected: false,
            open_file_count: 0,
        }
    }
}

impl ClientConnection {
    #[must_use]
    pub fn is_timed_out(&self, current_time_ms: u32) -> bool {
        if !self.connected {
            return false;
        }
        current_time_ms.saturating_sub(self.last_activity_ms) >= FS_CLIENT_TIMEOUT_MS
    }

    pub fn touch(&mut self, current_time_ms: u32) {
        self.last_activity_ms = current_time_ms;
    }

    pub fn reset(&mut self) {
        self.connected = false;
        self.open_file_count = 0;
        self.last_activity_ms = 0;
    }
}

/// Per-server connection / timing manager.
#[derive(Debug, Clone)]
pub struct ConnectionManager {
    connections: Vec<ClientConnection>,
    max_clients: u8,
    elapsed_total_ms: u32,
    status_timer_ms: u32,
    burst_count: u32,
    burst_window_ms: u32,
    has_active_clients: bool,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new(4)
    }
}

impl ConnectionManager {
    #[must_use]
    pub const fn new(max_clients: u8) -> Self {
        Self {
            connections: Vec::new(),
            max_clients,
            elapsed_total_ms: 0,
            status_timer_ms: 0,
            burst_count: 0,
            burst_window_ms: 0,
            has_active_clients: false,
        }
    }

    /// Register or refresh a client connection. Returns `false` if at
    /// capacity.
    pub fn connect(&mut self, client: Address, current_time_ms: u32) -> bool {
        if let Some(conn) = self
            .connections
            .iter_mut()
            .find(|c| c.client_address == client)
        {
            conn.connected = true;
            conn.touch(current_time_ms);
            return true;
        }
        if self.connections.len() >= self.max_clients as usize {
            return false;
        }
        self.connections.push(ClientConnection {
            client_address: client,
            connected: true,
            last_activity_ms: current_time_ms,
            open_file_count: 0,
        });
        self.has_active_clients = true;
        true
    }

    pub fn disconnect(&mut self, client: Address) {
        if let Some(pos) = self
            .connections
            .iter()
            .position(|c| c.client_address == client)
        {
            self.connections.remove(pos);
            self.update_active_state();
        }
    }

    pub fn record_activity(&mut self, client: Address, current_time_ms: u32) {
        if let Some(conn) = self
            .connections
            .iter_mut()
            .find(|c| c.client_address == client)
        {
            conn.touch(current_time_ms);
        }
    }

    /// Advance timers and prune timed-out connections. Returns the
    /// addresses that were just dropped.
    pub fn update(&mut self, elapsed_ms: u32) -> Vec<Address> {
        self.elapsed_total_ms = self.elapsed_total_ms.saturating_add(elapsed_ms);
        self.burst_window_ms = self.burst_window_ms.saturating_add(elapsed_ms);
        if self.burst_window_ms >= 1000 {
            self.burst_window_ms -= 1000;
            self.burst_count = 0;
        }
        let mut timed_out = Vec::new();
        let now = self.elapsed_total_ms;
        let mut i = 0;
        while i < self.connections.len() {
            if self.connections[i].is_timed_out(now) {
                timed_out.push(self.connections[i].client_address);
                self.connections.remove(i);
            } else {
                i += 1;
            }
        }
        self.update_active_state();
        timed_out
    }

    #[must_use]
    pub const fn current_status_interval(&self) -> u32 {
        if self.has_active_clients {
            FS_STATUS_BUSY_INTERVAL_MS
        } else {
            FS_STATUS_IDLE_INTERVAL_MS
        }
    }

    /// Rate-limited status burst gate. Returns `true` if a burst is
    /// allowed and consumes a slot.
    pub fn can_send_status_burst(&mut self) -> bool {
        if self.burst_count < FS_MAX_STATUS_BURST_PER_SEC {
            self.burst_count += 1;
            return true;
        }
        false
    }

    /// Cadence-driven status check. Returns `true` and resets the
    /// timer when the cadence elapses.
    pub fn should_send_status(&mut self, elapsed_ms: u32) -> bool {
        self.status_timer_ms = self.status_timer_ms.saturating_add(elapsed_ms);
        let interval = self.current_status_interval();
        if self.status_timer_ms >= interval {
            self.status_timer_ms -= interval;
            true
        } else {
            false
        }
    }

    #[inline]
    #[must_use]
    pub fn connections(&self) -> &[ClientConnection] {
        &self.connections
    }

    #[inline]
    #[must_use]
    pub const fn has_active_clients(&self) -> bool {
        self.has_active_clients
    }

    #[inline]
    #[must_use]
    pub fn active_client_count(&self) -> u8 {
        self.connections.len() as u8
    }

    #[inline]
    #[must_use]
    pub const fn max_clients(&self) -> u8 {
        self.max_clients
    }

    #[inline]
    #[must_use]
    pub const fn elapsed_total(&self) -> u32 {
        self.elapsed_total_ms
    }

    #[must_use]
    pub fn get_connection(&self, client: Address) -> Option<ClientConnection> {
        self.connections
            .iter()
            .find(|c| c.client_address == client)
            .copied()
    }

    pub fn increment_open_files(&mut self, client: Address) {
        if let Some(conn) = self
            .connections
            .iter_mut()
            .find(|c| c.client_address == client)
        {
            conn.open_file_count = conn.open_file_count.saturating_add(1);
        }
    }

    pub fn decrement_open_files(&mut self, client: Address) {
        if let Some(conn) = self
            .connections
            .iter_mut()
            .find(|c| c.client_address == client && c.open_file_count > 0)
        {
            conn.open_file_count -= 1;
        }
    }

    fn update_active_state(&mut self) {
        self.has_active_clients = self.connections.iter().any(|c| c.connected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_caps_at_max_clients() {
        let mut m = ConnectionManager::new(2);
        assert!(m.connect(0x10, 0));
        assert!(m.connect(0x20, 0));
        assert!(!m.connect(0x30, 0));
    }

    #[test]
    fn connect_existing_refreshes_activity() {
        let mut m = ConnectionManager::new(4);
        m.connect(0x10, 100);
        m.connect(0x10, 500);
        let conn = m.get_connection(0x10).unwrap();
        assert_eq!(conn.last_activity_ms, 500);
    }

    #[test]
    fn timeout_drops_connection() {
        let mut m = ConnectionManager::new(4);
        m.connect(0x10, 0);
        // Advance well past the timeout.
        let dropped = m.update(FS_CLIENT_TIMEOUT_MS + 100);
        assert_eq!(dropped, vec![0x10]);
        assert_eq!(m.active_client_count(), 0);
        assert!(!m.has_active_clients());
    }

    #[test]
    fn record_activity_resets_timeout() {
        let mut m = ConnectionManager::new(4);
        m.connect(0x10, 0);
        let _ = m.update(FS_CLIENT_TIMEOUT_MS - 1);
        m.record_activity(0x10, m.elapsed_total());
        let dropped = m.update(2);
        assert!(dropped.is_empty());
    }

    #[test]
    fn status_interval_switches_on_active_clients() {
        let mut m = ConnectionManager::new(4);
        assert_eq!(m.current_status_interval(), FS_STATUS_IDLE_INTERVAL_MS);
        m.connect(0x10, 0);
        assert_eq!(m.current_status_interval(), FS_STATUS_BUSY_INTERVAL_MS);
    }

    #[test]
    fn burst_gate_rate_limits() {
        let mut m = ConnectionManager::new(4);
        for _ in 0..FS_MAX_STATUS_BURST_PER_SEC {
            assert!(m.can_send_status_burst());
        }
        assert!(!m.can_send_status_burst());
        // After 1s, the window resets.
        let _ = m.update(1000);
        assert!(m.can_send_status_burst());
    }

    #[test]
    fn should_send_status_obeys_cadence() {
        let mut m = ConnectionManager::new(4);
        // Idle interval = 2000.
        assert!(!m.should_send_status(1999));
        assert!(m.should_send_status(1));
    }

    #[test]
    fn open_file_counters() {
        let mut m = ConnectionManager::new(4);
        m.connect(0x10, 0);
        m.increment_open_files(0x10);
        m.increment_open_files(0x10);
        assert_eq!(m.get_connection(0x10).unwrap().open_file_count, 2);
        m.decrement_open_files(0x10);
        assert_eq!(m.get_connection(0x10).unwrap().open_file_count, 1);
    }
}
