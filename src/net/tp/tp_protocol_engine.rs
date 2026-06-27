impl TransportProtocol {
    pub const MAX_DATA_LENGTH: u32 = TP_MAX_DATA_LENGTH;
    pub const BYTES_PER_FRAME: u32 = TP_BYTES_PER_FRAME;

    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: TpSessions::new(),
            timer_sessions: TpTimerSessions::new(),
            max_receive_bytes: TP_MAX_DATA_LENGTH,
            max_sessions: TP_DEFAULT_MAX_SESSIONS,
            max_retransmits: TP_DEFAULT_MAX_RETRANSMITS,
            advertised_packets_per_cts: TP_MAX_PACKETS_PER_CTS as u8,
            stats: TransportStats::default(),
            on_complete: Event::new(),
            on_abort: Event::new(),
            on_session_timeout: Event::new(),
        }
    }

    /// Build a TP engine with a receive-side allocation cap.
    ///
    /// The cap is clamped to the TP protocol maximum. Frames that advertise a
    /// larger payload are rejected before allocating a reassembly buffer.
    #[must_use]
    pub fn with_max_receive_bytes(max_receive_bytes: u32) -> Self {
        let mut this = Self::new();
        this.set_max_receive_bytes(max_receive_bytes);
        this
    }

    /// Set the receive-side reassembly allocation cap.
    pub fn set_max_receive_bytes(&mut self, max_receive_bytes: u32) {
        self.max_receive_bytes = max_receive_bytes.min(TP_MAX_DATA_LENGTH);
    }

    #[inline]
    #[must_use]
    pub const fn max_receive_bytes(&self) -> u32 {
        self.max_receive_bytes
    }

    /// Build a TP engine with a cap for simultaneous active sessions.
    #[must_use]
    pub fn with_max_sessions(max_sessions: usize) -> Self {
        let mut this = Self::new();
        this.set_max_sessions(max_sessions);
        this
    }

    /// Set the active-session cap. A value of `0` rejects new sessions.
    pub fn set_max_sessions(&mut self, max_sessions: usize) {
        #[cfg(feature = "embedded")]
        let max_sessions = max_sessions.min(TP_DEFAULT_MAX_SESSIONS);
        self.max_sessions = max_sessions;
    }

    #[inline]
    #[must_use]
    pub const fn max_sessions(&self) -> usize {
        self.max_sessions
    }

    /// Build a TP engine with a cap for duplicate/backward CTS-triggered
    /// retransmissions. A value of `0` aborts on the first retry request.
    #[must_use]
    pub fn with_max_retransmits(max_retransmits: u8) -> Self {
        let mut this = Self::new();
        this.set_max_retransmits(max_retransmits);
        this
    }

    /// Set the duplicate/backward CTS retransmit cap.
    pub fn set_max_retransmits(&mut self, max_retransmits: u8) {
        self.max_retransmits = max_retransmits;
    }

    #[inline]
    #[must_use]
    pub const fn max_retransmits(&self) -> u8 {
        self.max_retransmits
    }

    /// Build a TP engine with a custom TP.RTS byte-4 packet-window
    /// advertisement.
    ///
    /// Values outside `1..=TP_MAX_PACKETS_PER_CTS` are clamped into the valid
    /// wire range. This controls the RTS advertisement only; outbound data
    /// windowing still follows the receiver's CTS command, capped by the
    /// protocol maximum.
    #[must_use]
    pub fn with_advertised_packets_per_cts(advertised_packets_per_cts: u8) -> Self {
        let mut this = Self::new();
        this.set_advertised_packets_per_cts(advertised_packets_per_cts);
        this
    }

    /// Set the TP.RTS byte-4 packet-window advertisement.
    pub fn set_advertised_packets_per_cts(&mut self, advertised_packets_per_cts: u8) {
        self.advertised_packets_per_cts =
            advertised_packets_per_cts.clamp(1, TP_MAX_PACKETS_PER_CTS as u8);
    }

    #[inline]
    #[must_use]
    pub const fn advertised_packets_per_cts(&self) -> u8 {
        self.advertised_packets_per_cts
    }

    /// Snapshot the lossy-path counters accumulated by this protocol object.
    #[inline]
    #[must_use]
    pub const fn stats(&self) -> TransportStats {
        self.stats
    }

    /// Reset lossy-path counters without disturbing active sessions.
    #[inline]
    pub fn clear_stats(&mut self) {
        self.stats = TransportStats::default();
    }

    // ─── Public API ────────────────────────────────────────────────

    /// Start a transmit session. Returns the initial control frame
    /// (BAM for broadcast, RTS for connection-mode).
    pub fn send(
        &mut self,
        pgn: Pgn,
        data: &[u8],
        source: Address,
        dest: Address,
        port: u8,
        priority: Priority,
    ) -> Result<Vec<Frame>> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_data(format!(
                "TP target PGN 0x{pgn:X} exceeds the 18-bit J1939/ISOBUS PGN range"
            )));
        }
        if !valid_tp_source(source) {
            return Err(Error::invalid_address(source));
        }
        if !valid_tp_destination(dest) {
            return Err(Error::invalid_address(dest));
        }
        if data.len() as u32 > Self::MAX_DATA_LENGTH {
            tracing::error!(
                target: "machbus.transport.tp",
                size = data.len(),
                max = Self::MAX_DATA_LENGTH,
                "data exceeds TP max",
            );
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }
        if data.len() as u32 <= CAN_DATA_LENGTH {
            return Err(Error::invalid_state("use single frame for <= 8 bytes"));
        }

        // TP.DT frames do not carry the transferred PGN. Keep one active
        // transmit session per DT endpoint path so two different PGNs cannot
        // become ambiguous once data frames start flowing.
        if self.sessions.iter().any(|s| {
            s.source_address == source
                && s.destination_address == dest
                && s.direction == TransportDirection::Transmit
                && s.can_port == port
        }) {
            tracing::error!(
                target: "machbus.transport.tp",
                pgn = pgn,
                src = source,
                dst = dest,
                "session already active",
            );
            return Err(Error::with_message(
                super::error::ErrorCode::SessionExists,
                "session already active",
            ));
        }

        if self.sessions.len() >= self.max_sessions {
            tracing::error!(
                target: "machbus.transport.tp",
                max_sessions = self.max_sessions,
                "no TP session slots available",
            );
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::with_message(
                super::error::ErrorCode::NoResources,
                "no TP session slots available",
            ));
        }

        let mut session = TransportSession {
            direction: TransportDirection::Transmit,
            pgn,
            data: data.to_vec(),
            total_bytes: data.len() as u32,
            source_address: source,
            destination_address: dest,
            can_port: port,
            priority,
            advertised_packets_per_cts: self.advertised_packets_per_cts,
            ..Default::default()
        };

        let mut frames = Vec::new();
        if dest == BROADCAST_ADDRESS {
            session.state = SessionState::SendingData;
            frames.push(make_bam(&session));
            tracing::debug!(
                target: "machbus.transport.tp",
                pgn = pgn,
                bytes = data.len(),
                "BAM started",
            );
        } else {
            session.state = SessionState::WaitingForCTS;
            frames.push(make_rts(&session));
            tracing::debug!(
                target: "machbus.transport.tp",
                pgn = pgn,
                bytes = data.len(),
                "RTS sent",
            );
        }
        if self.push_session(session).is_err() {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::with_message(
                super::error::ErrorCode::NoResources,
                "no TP session slots available",
            ));
        }
        Ok(frames)
    }

    /// Build a complete TP/BAM broadcast transfer into fixed-capacity inline
    /// frame storage without allocating or storing a transport session.
    ///
    /// Available with `embedded-fixed`. The returned frame list starts with
    /// the BAM control frame followed by every TP.DT frame. The caller owns
    /// CAN-port selection and BAM inter-packet pacing before transmission.
    #[cfg(feature = "embedded")]
    pub fn send_bam_fixed<const N: usize>(
        &mut self,
        pgn: Pgn,
        data: &[u8],
        source: Address,
        priority: Priority,
    ) -> Result<FixedSlots<Frame, N>> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        if !valid_tp_source(source) {
            return Err(Error::invalid_address(source));
        }
        if data.len() as u32 > Self::MAX_DATA_LENGTH {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }
        if data.len() as u32 <= CAN_DATA_LENGTH {
            return Err(Error::invalid_state("use single frame for <= 8 bytes"));
        }

        let total_bytes = data.len() as u32;
        let total_packets = total_bytes.div_ceil(TP_BYTES_PER_FRAME) as usize;
        let needed = 1usize.saturating_add(total_packets);
        if needed > N {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }

        let mut frames = FixedSlots::new();
        frames
            .push(make_bam_fields(pgn, total_bytes, source))
            .map_err(|_| Error::buffer_overflow())?;
        generate_bam_data_frames_fixed(data, source, priority, &mut frames)?;
        Ok(frames)
    }

    /// Process an incoming TP frame (CM or DT). Returns the frames
    /// to transmit in response.
    pub fn process_frame(&mut self, frame: &Frame, port: u8) -> Vec<Frame> {
        if matches!(frame.pgn(), PGN_TP_CM | PGN_TP_DT) && frame.length < CAN_DATA_LENGTH as u8 {
            tracing::warn!(
                target: "machbus.transport.tp",
                pgn = frame.pgn(),
                length = frame.length,
                "dropping short TP frame",
            );
            self.note_dropped_frame();
            return Vec::new();
        }
        if matches!(frame.pgn(), PGN_TP_CM | PGN_TP_DT)
            && (!valid_tp_source(frame.source()) || !valid_tp_destination(frame.destination()))
        {
            tracing::warn!(
                target: "machbus.transport.tp",
                source = frame.source(),
                destination = frame.destination(),
                "dropping TP frame with invalid endpoint address",
            );
            self.note_dropped_frame();
            return Vec::new();
        }

        match frame.pgn() {
            PGN_TP_CM => self.handle_cm(frame, port),
            PGN_TP_DT => self.handle_dt(frame, port),
            _ => Vec::new(),
        }
    }

    /// Drive session timers. Returns frames produced by:
    /// (1) BAM continuations spaced by [`TP_BAM_INTER_PACKET_MS`],
    /// (2) timeout-induced aborts.
    pub fn update(&mut self, elapsed_ms: u32) -> Vec<Frame> {
        let mut emitted = Vec::new();
        let mut i = 0;
        while i < self.sessions.len() {
            self.sessions[i].timer_ms = self.sessions[i].timer_ms.saturating_add(elapsed_ms);

            // BAM continuation: emit one DT per interval.
            let is_bam_tx = self.sessions[i].is_broadcast()
                && self.sessions[i].state == SessionState::SendingData
                && self.sessions[i].direction == TransportDirection::Transmit;
            if is_bam_tx && self.sessions[i].timer_ms >= TP_BAM_INTER_PACKET_MS {
                self.sessions[i].timer_ms = 0;
                let dt = generate_data_frames(&mut self.sessions[i], 1);
                emitted.extend(dt);
                if self.sessions[i].bytes_transferred >= self.sessions[i].total_bytes {
                    self.sessions[i].state = SessionState::Complete;
                    let session = self.remove_session(i);
                    self.on_complete.emit(&session);
                    continue;
                }
            }

            // Timeout per state.
            let timed_out = match self.sessions[i].state {
                SessionState::WaitingForCTS => self.sessions[i].timer_ms >= TP_TIMEOUT_T3_MS,
                SessionState::WaitingForData => self.sessions[i].timer_ms >= TP_TIMEOUT_T1_MS,
                SessionState::WaitingForEndOfMsg => self.sessions[i].timer_ms >= TP_TIMEOUT_T3_MS,
                SessionState::ReceivingData => self.sessions[i].timer_ms >= TP_TIMEOUT_T1_MS,
                _ => false,
            };
            if timed_out {
                tracing::warn!(
                    target: "machbus.transport.tp",
                    pgn = self.sessions[i].pgn,
                    "session timeout",
                );
                self.sessions[i].state = SessionState::Aborted;
                let evt = TransportAbortEvent::from_session(
                    &self.sessions[i],
                    TransportAbortReason::Timeout,
                );
                self.on_abort.emit(&evt);
                if !self.sessions[i].is_broadcast() {
                    self.note_abort_sent();
                    emitted.push(make_session_abort(
                        &self.sessions[i],
                        TransportAbortReason::Timeout,
                    ));
                }
                self.note_timeout();
                self.note_dropped_session();
                self.remove_session(i);
                continue;
            }

            i += 1;
        }
        emitted
    }

    /// Generate the next batch of CMDT data frames after a CTS. Each
    /// transmit session in [`SessionState::SendingData`] is drained by
    /// `packets_to_send` frames or to completion, then transitions to
    /// either [`SessionState::WaitingForEndOfMsg`] or back to
    /// [`SessionState::WaitingForCTS`].
    pub fn get_pending_data_frames(&mut self) -> Vec<Frame> {
        let mut emitted = Vec::new();
        for s in self.sessions.iter_mut() {
            if s.state == SessionState::SendingData
                && s.direction == TransportDirection::Transmit
                && !s.is_broadcast()
            {
                let count = s.packets_to_send;
                emitted.extend(generate_data_frames(s, count));
                if s.bytes_transferred >= s.total_bytes {
                    s.state = SessionState::WaitingForEndOfMsg;
                    s.timer_ms = 0;
                } else {
                    s.state = SessionState::WaitingForCTS;
                    s.timer_ms = 0;
                }
            }
        }
        emitted
    }

    /// Fixed-capacity variant of [`Self::get_pending_data_frames`].
    ///
    /// Available with `embedded-fixed`. It rejects the call before mutating any
    /// session if the caller-provided frame storage cannot hold the currently
    /// pending CMDT data window.
    #[cfg(feature = "embedded")]
    pub fn get_pending_data_frames_fixed<const N: usize>(
        &mut self,
    ) -> Result<FixedSlots<Frame, N>> {
        let mut needed = 0usize;
        for s in self.sessions.iter() {
            if s.state == SessionState::SendingData
                && s.direction == TransportDirection::Transmit
                && !s.is_broadcast()
            {
                let remaining = s.total_bytes.saturating_sub(s.bytes_transferred);
                let remaining_frames = remaining.div_ceil(TP_BYTES_PER_FRAME) as usize;
                needed = needed.saturating_add((s.packets_to_send as usize).min(remaining_frames));
            }
        }
        if needed > N {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }

        let mut emitted = FixedSlots::new();
        for s in self.sessions.iter_mut() {
            if s.state == SessionState::SendingData
                && s.direction == TransportDirection::Transmit
                && !s.is_broadcast()
            {
                let count = s.packets_to_send;
                generate_data_frames_fixed(s, count, &mut emitted)?;
                if s.bytes_transferred >= s.total_bytes {
                    s.state = SessionState::WaitingForEndOfMsg;
                    s.timer_ms = 0;
                } else {
                    s.state = SessionState::WaitingForCTS;
                    s.timer_ms = 0;
                }
            }
        }
        Ok(emitted)
    }

    #[inline]
    pub fn active_sessions_iter(&self) -> impl Iterator<Item = &TransportSession> {
        self.sessions.iter()
    }

    #[cfg(feature = "default")]
    #[inline]
    #[must_use]
    pub fn active_sessions(&self) -> &[TransportSession] {
        &self.sessions
    }

    #[cfg(feature = "embedded")]
    fn push_session(
        &mut self,
        session: TransportSession,
    ) -> core::result::Result<(), TransportSession> {
        self.sessions.push(session)
    }

    #[cfg(feature = "default")]
    fn push_session(
        &mut self,
        session: TransportSession,
    ) -> core::result::Result<(), TransportSession> {
        self.sessions.push(session);
        Ok(())
    }

    #[cfg(feature = "embedded")]
    fn remove_session(&mut self, idx: usize) -> TransportSession {
        self.sessions
            .swap_remove(idx)
            .expect("session index came from active session table")
    }

    #[cfg(feature = "default")]
    fn remove_session(&mut self, idx: usize) -> TransportSession {
        self.sessions.swap_remove(idx)
    }

    fn session_position(
        &self,
        mut predicate: impl FnMut(&TransportSession) -> bool,
    ) -> Option<usize> {
        for (idx, session) in self.sessions.iter().enumerate() {
            if predicate(session) {
                return Some(idx);
            }
        }
        None
    }

    #[cfg(feature = "embedded")]
    fn push_timer_session(
        &mut self,
        session: TpTimerSession,
    ) -> core::result::Result<(), TpTimerSession> {
        self.timer_sessions.push(session)
    }

    #[cfg(feature = "default")]
    fn push_timer_session(
        &mut self,
        session: TpTimerSession,
    ) -> core::result::Result<(), TpTimerSession> {
        self.timer_sessions.push(session);
        Ok(())
    }

    #[inline]
    fn note_dropped_frame(&mut self) {
        self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
    }

    #[inline]
    fn note_dropped_session(&mut self) {
        self.stats.dropped_sessions = self.stats.dropped_sessions.saturating_add(1);
    }

    #[inline]
    fn note_abort_sent(&mut self) {
        self.stats.aborts_sent = self.stats.aborts_sent.saturating_add(1);
    }

    #[inline]
    fn note_abort_received(&mut self) {
        self.stats.aborts_received = self.stats.aborts_received.saturating_add(1);
    }

    #[inline]
    fn note_timeout(&mut self) {
        self.stats.timeouts = self.stats.timeouts.saturating_add(1);
    }

    #[inline]
    fn note_resource_rejection(&mut self) {
        self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
    }

    // ─── Auxiliary timer-session API ──────────────────────────────

    pub fn track_session(
        &mut self,
        src: Address,
        dst: Address,
        pgn: Pgn,
        state: TpSessionState,
        port: u8,
    ) {
        if src == NULL_ADDRESS
            || src == BROADCAST_ADDRESS
            || dst == NULL_ADDRESS
            || dst == BROADCAST_ADDRESS
            || !pgn_is_valid(pgn)
            || !state.is_active()
        {
            return;
        }
        if self
            .push_timer_session(TpTimerSession {
                source: src,
                destination: dst,
                pgn,
                timer_state: state,
                port,
                ..Default::default()
            })
            .is_err()
        {
            self.note_resource_rejection();
        }
    }

    pub fn set_receiver_paused(&mut self, src: Address, dst: Address, pgn: Pgn, port: u8) {
        if let Some(ts) = self
            .timer_sessions
            .iter_mut()
            .find(|t| t.source == src && t.destination == dst && t.pgn == pgn && t.port == port)
        {
            ts.receiver_paused = true;
            ts.cts_keepalive_timer_ms = 0;
        }
    }

    pub fn reset_session_timer(&mut self, src: Address, dst: Address, pgn: Pgn, port: u8) {
        if let Some(ts) = self
            .timer_sessions
            .iter_mut()
            .find(|t| t.source == src && t.destination == dst && t.pgn == pgn && t.port == port)
        {
            ts.last_activity_ms = 0;
        }
    }

    pub fn set_session_state(
        &mut self,
        src: Address,
        dst: Address,
        pgn: Pgn,
        state: TpSessionState,
        port: u8,
    ) {
        if let Some(ts) = self
            .timer_sessions
            .iter_mut()
            .find(|t| t.source == src && t.destination == dst && t.pgn == pgn && t.port == port)
        {
            ts.timer_state = state;
            ts.last_activity_ms = 0;
        }
    }

    /// Drive auxiliary timer sessions: timeouts → abort frames + event,
    /// receiver-paused → CTS keep-alive every `TP_T_HOLD_MS`.
    pub fn update_sessions(&mut self, elapsed_ms: u32) -> Vec<Frame> {
        let mut emitted = Vec::new();
        let mut timeouts = 0u64;
        let mut dropped_sessions = 0u64;
        let mut aborts_sent = 0u64;
        for ts in self.timer_sessions.iter_mut() {
            if !ts.is_active() {
                continue;
            }
            ts.last_activity_ms = ts.last_activity_ms.saturating_add(elapsed_ms);

            let threshold = match ts.timer_state {
                TpSessionState::WaitForCts => TP_TIMEOUT_T3_MS,
                TpSessionState::Sending => TP_TIMEOUT_T4_MS,
                TpSessionState::WaitForEndOfMsgAck => TP_TIMEOUT_T3_MS,
                _ => continue,
            };

            if ts.last_activity_ms >= threshold {
                tracing::warn!(
                    target: "machbus.transport.tp",
                    pgn = ts.pgn,
                    state = ?ts.timer_state,
                    "timer session timeout",
                );
                ts.timer_state = TpSessionState::TimedOut;
                ts.abort_reason = tp_abort::TIMEOUT;
                self.on_session_timeout.emit(ts);
                timeouts = timeouts.saturating_add(1);
                dropped_sessions = dropped_sessions.saturating_add(1);

                if ts.destination != BROADCAST_ADDRESS {
                    let id =
                        Identifier::encode(Priority::Lowest, PGN_TP_CM, ts.source, ts.destination);
                    let mut data = [0xFFu8; 8];
                    data[0] = tp_cm::ABORT;
                    data[1] = tp_abort::TIMEOUT;
                    data[5] = (ts.pgn & 0xFF) as u8;
                    data[6] = ((ts.pgn >> 8) & 0xFF) as u8;
                    data[7] = ((ts.pgn >> 16) & 0xFF) as u8;
                    emitted.push(Frame::new(id, data, 8));
                    aborts_sent = aborts_sent.saturating_add(1);
                }
                continue;
            }

            if ts.receiver_paused {
                ts.cts_keepalive_timer_ms = ts.cts_keepalive_timer_ms.saturating_add(elapsed_ms);
                if ts.cts_keepalive_timer_ms >= TP_T_HOLD_MS {
                    ts.cts_keepalive_timer_ms = 0;
                    emitted.push(make_cts(ts.destination, ts.source, 0, 0, ts.pgn));
                    tracing::trace!(
                        target: "machbus.transport.tp",
                        pgn = ts.pgn,
                        "CTS keepalive sent",
                    );
                }
            }
        }
        self.stats.timeouts = self.stats.timeouts.saturating_add(timeouts);
        self.stats.dropped_sessions = self.stats.dropped_sessions.saturating_add(dropped_sessions);
        self.stats.aborts_sent = self.stats.aborts_sent.saturating_add(aborts_sent);
        emitted
    }

    #[inline]
    pub fn timer_sessions_iter(&self) -> impl Iterator<Item = &TpTimerSession> {
        self.timer_sessions.iter()
    }

    #[cfg(feature = "default")]
    #[inline]
    #[must_use]
    pub fn timer_sessions(&self) -> &[TpTimerSession] {
        &self.timer_sessions
    }

    // ─── Frame ingest helpers ──────────────────────────────────────

    fn handle_cm(&mut self, frame: &Frame, port: u8) -> Vec<Frame> {
        let mut responses = Vec::new();
        let control_byte = frame.data[0];
        let src = frame.source();
        let dst = frame.destination();
        let cm_pgn = pgn_from_cm_bytes(&frame.data);
        if !tp_cm_reserved_bytes_are_canonical(control_byte, &frame.data) {
            tracing::warn!(
                target: "machbus.transport.tp",
                control_byte,
                "dropping TP CM frame with non-canonical reserved bytes"
            );
            self.note_dropped_frame();
            return responses;
        }
        if !pgn_is_valid(cm_pgn) {
            tracing::warn!(
                target: "machbus.transport.tp",
                pgn = cm_pgn,
                "dropping TP CM frame with invalid target PGN"
            );
            self.note_dropped_frame();
            return responses;
        }
        if control_byte != tp_cm::BAM && dst == BROADCAST_ADDRESS {
            tracing::warn!(
                target: "machbus.transport.tp",
                control_byte,
                "dropping destination-specific TP CM frame sent to broadcast"
            );
            self.note_dropped_frame();
            return responses;
        }

        match control_byte {
            tp_cm::RTS => {
                let msg_size = frame.data[1] as u32 | ((frame.data[2] as u32) << 8);
                let total_packets = frame.data[3];
                let max_per_cts = frame.data[4];

                if !valid_tp_payload_shape(msg_size, total_packets) || max_per_cts == 0 {
                    tracing::warn!(
                        target: "machbus.transport.tp",
                        bytes = msg_size,
                        total_packets,
                        max_per_cts,
                        "rejecting malformed RTS",
                    );
                    let tmp = TransportSession {
                        source_address: dst,
                        destination_address: src,
                        pgn: cm_pgn,
                        ..Default::default()
                    };
                    responses.push(make_abort(&tmp, TransportAbortReason::UnexpectedDataSize));
                    self.note_dropped_frame();
                    self.note_abort_sent();
                    return responses;
                }

                if self.receive_dt_path_is_active(src, dst, port) {
                    let tmp = TransportSession {
                        source_address: dst,
                        destination_address: src,
                        pgn: cm_pgn,
                        ..Default::default()
                    };
                    responses.push(make_abort(&tmp, TransportAbortReason::AlreadyInSession));
                    self.note_dropped_frame();
                    self.note_abort_sent();
                    return responses;
                }

                if self.sessions.len() >= self.max_sessions {
                    tracing::warn!(
                        target: "machbus.transport.tp",
                        max_sessions = self.max_sessions,
                        "rejecting RTS because TP session cap is full",
                    );
                    let tmp = TransportSession {
                        source_address: dst,
                        destination_address: src,
                        pgn: cm_pgn,
                        ..Default::default()
                    };
                    responses.push(make_abort(&tmp, TransportAbortReason::ResourcesUnavailable));
                    self.note_dropped_frame();
                    self.note_resource_rejection();
                    self.note_abort_sent();
                    return responses;
                }

                let max_pkts = max_per_cts.min(TP_MAX_PACKETS_PER_CTS as u8);
                let cts_count = total_packets.min(max_pkts);
                let data = match rx_buffer(msg_size, self.max_receive_bytes) {
                    Ok(data) => data,
                    Err(reason) => {
                        tracing::warn!(
                            target: "machbus.transport.tp",
                            bytes = msg_size,
                            max_receive_bytes = self.max_receive_bytes,
                            ?reason,
                            "rejecting RTS before allocation",
                        );
                        let tmp = TransportSession {
                            source_address: dst,
                            destination_address: src,
                            pgn: cm_pgn,
                            ..Default::default()
                        };
                        responses.push(make_abort(&tmp, reason));
                        self.note_dropped_frame();
                        if reason == TransportAbortReason::ResourcesUnavailable {
                            self.note_resource_rejection();
                        }
                        self.note_abort_sent();
                        return responses;
                    }
                };
                let session = TransportSession {
                    direction: TransportDirection::Receive,
                    state: SessionState::WaitingForData,
                    pgn: cm_pgn,
                    total_bytes: msg_size,
                    source_address: src,
                    destination_address: dst,
                    can_port: port,
                    priority: frame.priority(),
                    max_packets_per_cts: max_pkts,
                    data,
                    cts_window_start: 1,
                    cts_window_size: cts_count,
                    ..Default::default()
                };
                let cts = make_cts(dst, src, cts_count, 1, cm_pgn);
                if self.push_session(session).is_err() {
                    let tmp = TransportSession {
                        source_address: dst,
                        destination_address: src,
                        pgn: cm_pgn,
                        ..Default::default()
                    };
                    responses.push(make_abort(&tmp, TransportAbortReason::ResourcesUnavailable));
                    self.note_dropped_frame();
                    self.note_resource_rejection();
                    self.note_abort_sent();
                    return responses;
                }
                responses.push(cts);
                tracing::debug!(
                    target: "machbus.transport.tp",
                    pgn = cm_pgn,
                    bytes = msg_size,
                    "RTS received",
                );
            }

            tp_cm::CTS => {
                let num_packets = frame.data[1];
                let next_seq = frame.data[2];
                let max_retransmits = self.max_retransmits;

                let mut abort_index: Option<usize> = None;
                let mut abort_reason = TransportAbortReason::None;
                let mut matched = false;
                for (idx, s) in self.sessions.iter_mut().enumerate() {
                    if s.direction == TransportDirection::Transmit
                        && s.source_address == dst
                        && s.destination_address == src
                        && s.pgn == cm_pgn
                        && matches!(
                            s.state,
                            SessionState::WaitingForCTS
                                | SessionState::SendingData
                                | SessionState::WaitingForEndOfMsg
                        )
                        && s.can_port == port
                    {
                        matched = true;
                        if s.state == SessionState::SendingData {
                            let is_duplicate_current_window = num_packets != 0
                                && next_seq != 0
                                && next_seq as u32 <= s.total_packets()
                                && next_seq == s.last_sequence.saturating_add(1)
                                && s.packets_to_send
                                    == (num_packets as u32)
                                        .min(s.total_packets() - (next_seq as u32 - 1))
                                        .min(s.max_packets_per_cts as u32)
                                        as u8;
                            if is_duplicate_current_window {
                                // The sender has accepted this CTS window but
                                // the caller has not drained the pending DT
                                // frames yet. Treat an exact duplicate CTS as
                                // an idempotent retry so pump ordering cannot
                                // abort an otherwise valid transfer.
                                s.timer_ms = 0;
                                break;
                            }
                            tracing::warn!(
                                target: "machbus.transport.tp",
                                packets = num_packets,
                                next_seq = next_seq,
                                "CTS received while sender is already sending data",
                            );
                            s.state = SessionState::Aborted;
                            abort_index = Some(idx);
                            abort_reason = TransportAbortReason::ConnectionModeError;
                        } else if num_packets == 0 {
                            // CTS hold (receiver paused).
                            s.timer_ms = 0;
                        } else if next_seq == 0 || next_seq as u32 > s.total_packets() {
                            tracing::warn!(
                                target: "machbus.transport.tp",
                                next_seq = next_seq,
                                total_packets = s.total_packets(),
                                "CTS invalid next_seq",
                            );
                            s.state = SessionState::Aborted;
                            abort_index = Some(idx);
                            abort_reason = TransportAbortReason::BadSequence;
                        } else {
                            let requested_start = next_seq as u32 - 1;
                            if requested_start < s.bytes_transferred.div_ceil(TP_BYTES_PER_FRAME) {
                                s.retransmit_count = s.retransmit_count.saturating_add(1);
                                if s.retransmit_count > max_retransmits {
                                    tracing::warn!(
                                        target: "machbus.transport.tp",
                                        next_seq = next_seq,
                                        retransmit_count = s.retransmit_count,
                                        max_retransmits = max_retransmits,
                                        "CTS retransmit cap exceeded",
                                    );
                                    s.state = SessionState::Aborted;
                                    abort_index = Some(idx);
                                    abort_reason = TransportAbortReason::MaxRetransmitsExceeded;
                                    break;
                                }
                            } else {
                                s.retransmit_count = 0;
                            }
                            let remaining_packets = s.total_packets() - (next_seq as u32 - 1);
                            let clamped = (num_packets as u32)
                                .min(remaining_packets)
                                .min(s.max_packets_per_cts as u32)
                                as u8;
                            s.state = SessionState::SendingData;
                            s.packets_to_send = clamped;
                            s.bytes_transferred = (next_seq as u32 - 1) * 7;
                            s.last_sequence = next_seq - 1;
                            s.timer_ms = 0;
                        }
                        tracing::debug!(
                            target: "machbus.transport.tp",
                            packets = num_packets,
                            next_seq = next_seq,
                            "CTS received",
                        );
                        break;
                    }
                }
                if let Some(idx) = abort_index {
                    let evt = TransportAbortEvent::from_session(&self.sessions[idx], abort_reason);
                    self.on_abort.emit(&evt);
                    let abort_frame = make_session_abort(&self.sessions[idx], abort_reason);
                    responses.push(abort_frame);
                    self.note_dropped_frame();
                    self.note_dropped_session();
                    self.note_abort_sent();
                    self.remove_session(idx);
                } else if !matched {
                    self.note_dropped_frame();
                }
            }

            tp_cm::EOMA => {
                if let Some(idx) = self.session_position(|s| {
                    s.direction == TransportDirection::Transmit
                        && s.source_address == dst
                        && s.destination_address == src
                        && s.pgn == cm_pgn
                        && s.can_port == port
                }) {
                    let expected_total = self.sessions[idx].total_bytes;
                    let expected_packets = self.sessions[idx].total_packets() as u8;
                    let ack_total = frame.data[1] as u32 | ((frame.data[2] as u32) << 8);
                    let ack_packets = frame.data[3];
                    if self.sessions[idx].state != SessionState::WaitingForEndOfMsg
                        || ack_total != expected_total
                        || ack_packets != expected_packets
                    {
                        tracing::warn!(
                            target: "machbus.transport.tp",
                            state = ?self.sessions[idx].state,
                            ack_total,
                            expected_total,
                            ack_packets,
                            expected_packets,
                            "dropping TP EOMA that does not match a completed transmit session"
                        );
                        self.note_dropped_frame();
                        return responses;
                    }
                    let mut session = self.remove_session(idx);
                    session.state = SessionState::Complete;
                    self.on_complete.emit(&session);
                    tracing::debug!(target: "machbus.transport.tp", "EOMA — session complete");
                } else {
                    self.note_dropped_frame();
                }
            }

            tp_cm::BAM => {
                let msg_size = frame.data[1] as u32 | ((frame.data[2] as u32) << 8);
                let total_packets = frame.data[3];
                let over_receive_cap = msg_size > self.max_receive_bytes;
                if self.receive_dt_path_is_active(src, BROADCAST_ADDRESS, port) {
                    tracing::warn!(
                        target: "machbus.transport.tp",
                        source = src,
                        "dropping BAM because TP DT endpoint path is already active",
                    );
                    self.note_dropped_frame();
                    return responses;
                }
                if !frame.is_broadcast()
                    || !valid_tp_payload_shape(msg_size, total_packets)
                    || over_receive_cap
                {
                    tracing::warn!(
                        target: "machbus.transport.tp",
                        bytes = msg_size,
                        total_packets,
                        max_receive_bytes = self.max_receive_bytes,
                        "dropping malformed BAM",
                    );
                    self.note_dropped_frame();
                    if over_receive_cap {
                        self.note_resource_rejection();
                    }
                    return responses;
                }
                if self.sessions.len() >= self.max_sessions {
                    tracing::warn!(
                        target: "machbus.transport.tp",
                        max_sessions = self.max_sessions,
                        "dropping BAM because TP session cap is full",
                    );
                    self.note_dropped_frame();
                    self.note_resource_rejection();
                    return responses;
                }
                let data = match rx_buffer(msg_size, self.max_receive_bytes) {
                    Ok(data) => data,
                    Err(reason) => {
                        tracing::warn!(
                            target: "machbus.transport.tp",
                            bytes = msg_size,
                            max_receive_bytes = self.max_receive_bytes,
                            ?reason,
                            "dropping BAM before allocation",
                        );
                        self.note_dropped_frame();
                        if reason == TransportAbortReason::ResourcesUnavailable {
                            self.note_resource_rejection();
                        }
                        return responses;
                    }
                };
                let session = TransportSession {
                    direction: TransportDirection::Receive,
                    state: SessionState::ReceivingData,
                    pgn: cm_pgn,
                    total_bytes: msg_size,
                    source_address: src,
                    destination_address: BROADCAST_ADDRESS,
                    can_port: port,
                    priority: frame.priority(),
                    data,
                    ..Default::default()
                };
                if self.push_session(session).is_err() {
                    self.note_dropped_frame();
                    self.note_resource_rejection();
                    return responses;
                }
                tracing::debug!(
                    target: "machbus.transport.tp",
                    pgn = cm_pgn,
                    bytes = msg_size,
                    "BAM received",
                );
            }

            tp_cm::ABORT => {
                let Some(reason) = TransportAbortReason::try_from_u8(frame.data[1]) else {
                    self.note_dropped_frame();
                    return responses;
                };
                if let Some(idx) = self.session_position(|s| {
                    s.pgn == cm_pgn
                        && s.can_port == port
                        && ((s.source_address == dst && s.destination_address == src)
                            || (s.source_address == src && s.destination_address == dst))
                }) {
                    self.sessions[idx].state = SessionState::Aborted;
                    let evt = TransportAbortEvent::from_session(&self.sessions[idx], reason);
                    self.on_abort.emit(&evt);
                    self.remove_session(idx);
                    self.note_abort_received();
                    self.note_dropped_session();
                    tracing::warn!(
                        target: "machbus.transport.tp",
                        reason = ?reason,
                        "abort received",
                    );
                } else {
                    self.note_dropped_frame();
                }
            }

            _ => self.note_dropped_frame(),
        }
        responses
    }

    fn handle_dt(&mut self, frame: &Frame, port: u8) -> Vec<Frame> {
        let mut responses = Vec::new();
        let src = frame.source();
        let dst = frame.destination();
        let seq = frame.data[0];

        let Some(idx) = self.session_position(|s| {
            s.direction == TransportDirection::Receive
                && s.source_address == src
                && s.can_port == port
                && (s.state == SessionState::WaitingForData
                    || s.state == SessionState::ReceivingData)
                && (s.is_broadcast() || s.destination_address == dst)
        }) else {
            self.note_dropped_frame();
            return responses;
        };

        if seq == 0 {
            tracing::warn!(target: "machbus.transport.tp", "bad zero DT sequence");
            if !self.sessions[idx].is_broadcast() {
                responses.push(make_session_abort(
                    &self.sessions[idx],
                    TransportAbortReason::BadSequence,
                ));
                self.note_abort_sent();
            }
            self.sessions[idx].state = SessionState::Aborted;
            let evt = TransportAbortEvent::from_session(
                &self.sessions[idx],
                TransportAbortReason::BadSequence,
            );
            self.on_abort.emit(&evt);
            self.note_dropped_frame();
            self.note_dropped_session();
            self.remove_session(idx);
            return responses;
        }

        let expected = self.sessions[idx].last_sequence + 1;
        if seq != expected {
            // Distinguish duplicate vs out-of-order.
            if seq <= self.sessions[idx].last_sequence && seq != 0 {
                tracing::warn!(
                    target: "machbus.transport.tp",
                    seq = seq,
                    expected = expected,
                    "duplicate DT",
                );
                if !self.sessions[idx].is_broadcast() {
                    responses.push(make_session_abort(
                        &self.sessions[idx],
                        TransportAbortReason::DuplicateSequence,
                    ));
                    self.note_abort_sent();
                }
                self.sessions[idx].state = SessionState::Aborted;
                let evt = TransportAbortEvent::from_session(
                    &self.sessions[idx],
                    TransportAbortReason::DuplicateSequence,
                );
                self.on_abort.emit(&evt);
                self.note_dropped_frame();
                self.note_dropped_session();
                self.remove_session(idx);
                return responses;
            }
            if seq > expected {
                tracing::warn!(
                    target: "machbus.transport.tp",
                    seq = seq,
                    expected = expected,
                    "out-of-order DT",
                );
                if !self.sessions[idx].is_broadcast() {
                    responses.push(make_session_abort(
                        &self.sessions[idx],
                        TransportAbortReason::BadSequence,
                    ));
                    self.note_abort_sent();
                }
                self.sessions[idx].state = SessionState::Aborted;
                let evt = TransportAbortEvent::from_session(
                    &self.sessions[idx],
                    TransportAbortReason::BadSequence,
                );
                self.on_abort.emit(&evt);
                self.note_dropped_frame();
                self.note_dropped_session();
                self.remove_session(idx);
                return responses;
            }
        }

        // Ingest 7 data bytes.
        let s = &mut self.sessions[idx];
        let offset = (seq as u32 - 1) * 7;
        for i in 0..TP_BYTES_PER_FRAME {
            let abs = (offset + i) as usize;
            if abs < s.total_bytes as usize {
                s.data[abs] = frame.data[i as usize + 1];
            }
        }
        s.bytes_transferred = (offset + TP_BYTES_PER_FRAME).min(s.total_bytes);
        s.last_sequence = seq;
        s.timer_ms = 0;

        // Completion path (CMDT or BAM).
        if s.bytes_transferred >= s.total_bytes {
            s.state = SessionState::Complete;
            let send_eoma = !s.is_broadcast();
            let (eoma_src, eoma_dst, eoma_total, eoma_packets, eoma_pgn) = (
                s.destination_address,
                s.source_address,
                s.total_bytes,
                s.total_packets() as u8,
                s.pgn,
            );
            let session = self.remove_session(idx);
            if send_eoma {
                responses.push(make_eoma(
                    eoma_src,
                    eoma_dst,
                    eoma_total,
                    eoma_packets,
                    eoma_pgn,
                ));
            }
            self.on_complete.emit(&session);
            tracing::debug!(
                target: "machbus.transport.tp",
                pgn = session.pgn,
                "session complete",
            );
            return responses;
        }

        // CMDT only: emit next CTS once the granted window is exhausted.
        if !self.sessions[idx].is_broadcast() {
            let s = &mut self.sessions[idx];
            let in_window = seq - (s.cts_window_start - 1);
            if in_window >= s.cts_window_size {
                let remaining = s.total_packets() - seq as u32;
                let next_count = (remaining as u8).min(s.max_packets_per_cts);
                s.cts_window_start = seq + 1;
                s.cts_window_size = next_count;
                responses.push(make_cts(
                    s.destination_address,
                    s.source_address,
                    next_count,
                    seq + 1,
                    s.pgn,
                ));
            }
        }
        responses
    }

    fn receive_dt_path_is_active(&self, source: Address, destination: Address, port: u8) -> bool {
        self.sessions.iter().any(|s| {
            s.direction == TransportDirection::Receive
                && s.source_address == source
                && s.can_port == port
                && (s.destination_address == BROADCAST_ADDRESS
                    || destination == BROADCAST_ADDRESS
                    || s.destination_address == destination)
        })
    }
}

// ─── Frame builders ─────────────────────────────────────────────────────

#[inline]
fn valid_tp_payload_shape(total_bytes: u32, total_packets: u8) -> bool {
    total_bytes > CAN_DATA_LENGTH
        && total_bytes <= TP_MAX_DATA_LENGTH
        && total_packets != 0
        && total_packets as u32 == total_bytes.div_ceil(TP_BYTES_PER_FRAME)
}

fn tp_cm_reserved_bytes_are_canonical(control_byte: u8, data: &[u8; 8]) -> bool {
    match control_byte {
        tp_cm::CTS => data[3] == 0xFF && data[4] == 0xFF,
        tp_cm::EOMA | tp_cm::BAM => data[4] == 0xFF,
        tp_cm::ABORT => data[2..5].iter().all(|&byte| byte == 0xFF),
        _ => true,
    }
}

fn rx_buffer(
    total_bytes: u32,
    max_receive_bytes: u32,
) -> core::result::Result<Vec<u8>, TransportAbortReason> {
    if total_bytes > max_receive_bytes {
        return Err(TransportAbortReason::ResourcesUnavailable);
    }

    let mut data = Vec::new();
    data.try_reserve_exact(total_bytes as usize)
        .map_err(|_| TransportAbortReason::ResourcesUnavailable)?;
    data.resize(total_bytes as usize, 0xFF);
    Ok(data)
}

fn make_bam(s: &TransportSession) -> Frame {
    make_bam_fields(s.pgn, s.total_bytes, s.source_address)
}

fn make_bam_fields(pgn: Pgn, total_bytes: u32, source_address: Address) -> Frame {
    let id = Identifier::encode(
        Priority::Lowest,
        PGN_TP_CM,
        source_address,
        BROADCAST_ADDRESS,
    );
    let mut data = [0xFFu8; 8];
    data[0] = tp_cm::BAM;
    data[1] = (total_bytes & 0xFF) as u8;
    data[2] = ((total_bytes >> 8) & 0xFF) as u8;
    data[3] = total_bytes.div_ceil(TP_BYTES_PER_FRAME) as u8;
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_rts(s: &TransportSession) -> Frame {
    make_rts_fields(
        s.pgn,
        s.total_bytes,
        s.source_address,
        s.destination_address,
        s.advertised_packets_per_cts,
    )
}

fn make_rts_fields(
    pgn: Pgn,
    total_bytes: u32,
    source_address: Address,
    destination_address: Address,
    advertised_packets_per_cts: u8,
) -> Frame {
    let id = Identifier::encode(
        Priority::Lowest,
        PGN_TP_CM,
        source_address,
        destination_address,
    );
    let mut data = [0u8; 8];
    data[0] = tp_cm::RTS;
    data[1] = (total_bytes & 0xFF) as u8;
    data[2] = ((total_bytes >> 8) & 0xFF) as u8;
    data[3] = total_bytes.div_ceil(TP_BYTES_PER_FRAME) as u8;
    data[4] = advertised_packets_per_cts;
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_cts(src: Address, dst: Address, num_packets: u8, next_seq: u8, pgn: Pgn) -> Frame {
    let id = Identifier::encode(Priority::Lowest, PGN_TP_CM, src, dst);
    let mut data = [0xFFu8; 8];
    data[0] = tp_cm::CTS;
    data[1] = num_packets;
    data[2] = next_seq;
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_eoma(src: Address, dst: Address, total_bytes: u32, total_packets: u8, pgn: Pgn) -> Frame {
    let id = Identifier::encode(Priority::Lowest, PGN_TP_CM, src, dst);
    let mut data = [0xFFu8; 8];
    data[0] = tp_cm::EOMA;
    data[1] = (total_bytes & 0xFF) as u8;
    data[2] = ((total_bytes >> 8) & 0xFF) as u8;
    data[3] = total_packets;
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_abort(s: &TransportSession, reason: TransportAbortReason) -> Frame {
    make_abort_fields(s.source_address, s.destination_address, s.pgn, reason)
}

fn make_abort_fields(
    source_address: Address,
    destination_address: Address,
    pgn: Pgn,
    reason: TransportAbortReason,
) -> Frame {
    let id = Identifier::encode(
        Priority::Lowest,
        PGN_TP_CM,
        source_address,
        destination_address,
    );
    let mut data = [0xFFu8; 8];
    data[0] = tp_cm::ABORT;
    data[1] = reason.as_u8();
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_session_abort(s: &TransportSession, reason: TransportAbortReason) -> Frame {
    let (source_address, destination_address) = match s.direction {
        TransportDirection::Transmit => (s.source_address, s.destination_address),
        TransportDirection::Receive => (s.destination_address, s.source_address),
    };
    let wire_session = TransportSession {
        source_address,
        destination_address,
        pgn: s.pgn,
        ..Default::default()
    };
    make_abort(&wire_session, reason)
}

fn generate_data_frames(session: &mut TransportSession, count: u8) -> Vec<Frame> {
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if session.bytes_transferred >= session.total_bytes {
            break;
        }
        let id = Identifier::encode(
            Priority::Lowest,
            PGN_TP_DT,
            session.source_address,
            session.destination_address,
        );
        let mut data = [0xFFu8; 8];
        session.last_sequence = session.last_sequence.wrapping_add(1);
        data[0] = session.last_sequence;
        for j in 0..7u32 {
            let idx = session.bytes_transferred + j;
            if idx < session.total_bytes {
                data[(j + 1) as usize] = session.data[idx as usize];
            }
        }
        session.bytes_transferred =
            (session.bytes_transferred + TP_BYTES_PER_FRAME).min(session.total_bytes);
        out.push(Frame::new(id, data, 8));
    }
    out
}

#[cfg(feature = "embedded")]
fn generate_data_frames_fixed<const N: usize>(
    session: &mut TransportSession,
    count: u8,
    out: &mut FixedSlots<Frame, N>,
) -> Result<()> {
    for _ in 0..count {
        if session.bytes_transferred >= session.total_bytes {
            break;
        }
        let id = Identifier::encode(
            Priority::Lowest,
            PGN_TP_DT,
            session.source_address,
            session.destination_address,
        );
        let mut data = [0xFFu8; 8];
        session.last_sequence = session.last_sequence.wrapping_add(1);
        data[0] = session.last_sequence;
        for j in 0..TP_BYTES_PER_FRAME {
            let idx = session.bytes_transferred + j;
            if idx < session.total_bytes {
                data[(j + 1) as usize] = session.data[idx as usize];
            }
        }
        session.bytes_transferred =
            (session.bytes_transferred + TP_BYTES_PER_FRAME).min(session.total_bytes);
        out.push(Frame::new(id, data, 8))
            .map_err(|_| Error::buffer_overflow())?;
    }
    Ok(())
}

#[cfg(feature = "embedded")]
fn generate_bam_data_frames_fixed<const N: usize>(
    payload: &[u8],
    source_address: Address,
    priority: Priority,
    out: &mut FixedSlots<Frame, N>,
) -> Result<()> {
    let id = Identifier::encode(priority, PGN_TP_DT, source_address, BROADCAST_ADDRESS);
    let mut offset = 0usize;
    let mut sequence = 1u8;
    while offset < payload.len() {
        let mut data = [0xFFu8; 8];
        data[0] = sequence;
        for j in 0..TP_BYTES_PER_FRAME as usize {
            let idx = offset + j;
            if idx < payload.len() {
                data[j + 1] = payload[idx];
            }
        }
        out.push(Frame::new(id, data, 8))
            .map_err(|_| Error::buffer_overflow())?;
        offset = offset.saturating_add(TP_BYTES_PER_FRAME as usize);
        sequence = sequence.wrapping_add(1);
    }
    Ok(())
}

#[inline]
fn pgn_from_cm_bytes(data: &[u8; 8]) -> Pgn {
    (data[5] as u32) | ((data[6] as u32) << 8) | ((data[7] as u32) << 16)
}

#[inline]
const fn valid_tp_source(source: Address) -> bool {
    source != NULL_ADDRESS && source != BROADCAST_ADDRESS
}

#[inline]
const fn valid_tp_destination(destination: Address) -> bool {
    destination != NULL_ADDRESS
}

