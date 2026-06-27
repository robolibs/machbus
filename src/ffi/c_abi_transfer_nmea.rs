/// Copy the Transfer data bytes into `out` (cap bytes). Returns the full data
/// length; if it exceeds `cap` nothing is copied.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_transfer_msg_data_into(
    h: *const MachbusTransferMsg,
    out: *mut u8,
    cap: usize,
) -> usize {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return 0;
    };
    let data = &h.0.data;
    if !out.is_null() && data.len() <= cap {
        unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), out, data.len()) };
    }
    data.len()
}

/// Free a Transfer handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_transfer_msg_free(h: *mut MachbusTransferMsg) {
    if h.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(h)) };
}

/// Encode a Transfer message (original PGN + response data) into `out` (cap
/// bytes). Returns the full encoded length (`3 + data_len`); if it exceeds `cap`
/// nothing is copied. Returns 0 on invalid PGN / null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_transfer_msg_encode(
    original_pgn: u32,
    data: *const u8,
    data_len: usize,
    out: *mut u8,
    cap: usize,
) -> usize {
    let payload = match read_bytes(data, data_len) {
        Ok(b) => b.to_vec(),
        Err(e) => {
            set_last_error(e);
            return 0;
        }
    };
    let msg = crate::j1939::TransferMsg {
        original_pgn,
        data: payload,
    };
    match msg.encode() {
        Ok(bytes) => {
            if !out.is_null() && bytes.len() <= cap {
                unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
            }
            clear_last_error();
            bytes.len()
        }
        Err(e) => {
            set_last_error(e.to_string());
            0
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// NMEA 2000 — payload builders
// ══════════════════════════════════════════════════════════════════════
//
// The `*Data` structs in `machbus::nmea` carry no standalone `decode`/`encode`
// methods (decoding happens inside `NMEAInterface::handle_message`, which mutates
// interface state rather than returning a value). The only standalone,
// value-returning encoders are the `NMEAInterface::build_*` free functions, each
// of which produces a fixed 8-byte payload. The three named in scope are mirrored
// below; each writes exactly 8 bytes into `out` and returns `true`.

/// Build a PGN 129025 (position rapid update) payload from a WGS84 fix.
/// Writes 8 bytes into `out` (must hold at least 8). Returns `false` on null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_build_position(
    latitude_deg: f64,
    longitude_deg: f64,
    out: *mut u8,
) -> bool {
    if out.is_null() {
        set_last_error("null pointer");
        return false;
    }
    let pos = crate::nmea::GNSSPosition {
        wgs: crate::geo::Wgs::new(latitude_deg, longitude_deg, 0.0),
        ..crate::nmea::GNSSPosition::default()
    };
    let bytes = crate::nmea::NMEAInterface::build_position(&pos);
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    clear_last_error();
    true
}

/// Build a PGN 129026 (COG / SOG rapid update) payload.
/// `cog_rad` is course-over-ground in radians, `sog_mps` is speed in m/s.
/// Writes 8 bytes into `out` (must hold at least 8). Returns `false` on null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_build_cog_sog(cog_rad: f64, sog_mps: f64, out: *mut u8) -> bool {
    if out.is_null() {
        set_last_error("null pointer");
        return false;
    }
    let bytes = crate::nmea::NMEAInterface::build_cog_sog(cog_rad, sog_mps);
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    clear_last_error();
    true
}

/// Build a PGN 127250 (vessel heading) payload. All angles are in radians.
/// Writes 8 bytes into `out` (must hold at least 8). Returns `false` on null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_build_heading(
    heading_rad: f64,
    deviation_rad: f64,
    variation_rad: f64,
    out: *mut u8,
) -> bool {
    if out.is_null() {
        set_last_error("null pointer");
        return false;
    }
    let bytes =
        crate::nmea::NMEAInterface::build_heading(heading_rad, deviation_rad, variation_rad);
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    clear_last_error();
    true
}

// ══════════════════════════════════════════════════════════════════════
// j1939::heartbeat — ISO 11783-7 heartbeat request
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::HeartbeatRequest`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusHeartbeatRequest {
    /// 18-bit requested PGN.
    pub requested_pgn: u32,
    /// Requested broadcast interval in milliseconds.
    pub interval_ms: u16,
}

impl From<crate::j1939::HeartbeatRequest> for MachbusHeartbeatRequest {
    fn from(h: crate::j1939::HeartbeatRequest) -> Self {
        Self {
            requested_pgn: h.requested_pgn,
            interval_ms: h.interval_ms,
        }
    }
}

impl From<MachbusHeartbeatRequest> for crate::j1939::HeartbeatRequest {
    fn from(h: MachbusHeartbeatRequest) -> Self {
        Self {
            requested_pgn: h.requested_pgn,
            interval_ms: h.interval_ms,
        }
    }
}

pod_codec_try_encode!(
    MachbusHeartbeatRequest,
    crate::j1939::HeartbeatRequest,
    8,
    decode = machbus_j1939_heartbeat_request_decode,
    encode = machbus_j1939_heartbeat_request_encode,
    err = "HeartbeatRequest decode failed"
);

// ═══════════════════════════════════════════════════════════════════════
// Session plugin surface — remaining plugins + servers.
//
// Mirrors the public command/getter API of every plugged subsystem 1:1,
// following the same conventions as the diagnostics/gnss/implement/vt/tc
// functions above: `plugin_mut!`/`get`, `set_last_error`/`clear_last_error`,
// `bool_result`, `Option<T>` -> value + `*_present` out-param.
// ═══════════════════════════════════════════════════════════════════════

/// Borrow the handle mutably or set last-error and `return false`.
macro_rules! try_handle_mut {
    ($h:expr) => {
        match handle_mut($h) {
            Ok(h) => h,
            Err(e) => {
                set_last_error(e);
                return false;
            }
        }
    };
}

// ─── Auxiliary ────────────────────────────────────────────────────────

/// `#[repr(C)]` mirror of [`machbus::isobus::AuxOFunction`] /
/// [`AuxNFunction`]. `kind`/`state` are the raw `AuxFunctionType` /
/// `AuxFunctionState` wire bytes (0=Type0/Off, 1=Type1/On, 2=Type2/Variable).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachbusAuxFunction {
    pub function_number: u8,
    pub kind: u8,
    pub state: u8,
    pub setpoint: u16,
}

impl From<crate::isobus::AuxOFunction> for MachbusAuxFunction {
    fn from(f: crate::isobus::AuxOFunction) -> Self {
        Self {
            function_number: f.function_number,
            kind: f.r#type.as_u8(),
            state: f.state.as_u8(),
            setpoint: f.setpoint,
        }
    }
}

impl From<crate::isobus::AuxNFunction> for MachbusAuxFunction {
    fn from(f: crate::isobus::AuxNFunction) -> Self {
        Self {
            function_number: f.function_number,
            kind: f.r#type.as_u8(),
            state: f.state.as_u8(),
            setpoint: f.setpoint,
        }
    }
}

impl From<MachbusAuxFunction> for crate::isobus::AuxOFunction {
    fn from(f: MachbusAuxFunction) -> Self {
        Self {
            function_number: f.function_number,
            r#type: crate::isobus::AuxFunctionType::from_u8(f.kind),
            state: crate::isobus::AuxFunctionState::from_u8(f.state),
            setpoint: f.setpoint,
        }
    }
}

impl From<MachbusAuxFunction> for crate::isobus::AuxNFunction {
    fn from(f: MachbusAuxFunction) -> Self {
        Self {
            function_number: f.function_number,
            r#type: crate::isobus::AuxFunctionType::from_u8(f.kind),
            state: crate::isobus::AuxFunctionState::from_u8(f.state),
            setpoint: f.setpoint,
        }
    }
}

/// Last cached AUX-O status for `(source, function_number)`. Returns true and
/// fills `out` if present. Requires the auxiliary subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_auxiliary_last_aux_o(
    h: *const MachbusSession,
    source: u8,
    function_number: u8,
    out: *mut MachbusAuxFunction,
) -> bool {
    let Some(aux) = handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Auxiliary>())
    else {
        return false;
    };
    match aux.last_aux_o(source as Address, function_number) {
        Some(f) if !out.is_null() => {
            unsafe { *out = f.into() };
            true
        }
        Some(_) => true,
        None => false,
    }
}

/// Last cached AUX-N status for `(source, function_number)`. Requires aux.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_auxiliary_last_aux_n(
    h: *const MachbusSession,
    source: u8,
    function_number: u8,
    out: *mut MachbusAuxFunction,
) -> bool {
    let Some(aux) = handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Auxiliary>())
    else {
        return false;
    };
    match aux.last_aux_n(source as Address, function_number) {
        Some(f) if !out.is_null() => {
            unsafe { *out = f.into() };
            true
        }
        Some(_) => true,
        None => false,
    }
}

/// Queue a local AUX-O status broadcast (flushed on the next tick). Requires aux.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_auxiliary_broadcast_aux_o(
    h: *mut MachbusSession,
    function: MachbusAuxFunction,
) -> bool {
    let h = try_handle_mut!(h);
    let aux = plugin_mut!(h, Auxiliary);
    aux.broadcast_aux_o(function.into());
    clear_last_error();
    true
}

/// Queue a local AUX-N type-2 status broadcast (flushed on tick). Requires aux.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_auxiliary_broadcast_aux_n(
    h: *mut MachbusSession,
    function: MachbusAuxFunction,
) -> bool {
    let h = try_handle_mut!(h);
    let aux = plugin_mut!(h, Auxiliary);
    aux.broadcast_aux_n(function.into());
    clear_last_error();
    true
}

// ─── DmMemory ─────────────────────────────────────────────────────────
//
// `MachbusDm14Request` / `MachbusDm15Response` repr(C) mirrors (+ conversions)
// are already defined in the j1939::dm_memory codec section above; reused here.

/// Request peer ECU identification (DM-style request). Requires DM memory.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_dm_memory_request_ecu_identification(
    h: *mut MachbusSession,
    destination: u8,
) -> bool {
    let h = try_handle_mut!(h);
    let dm = plugin_mut!(h, DmMemory);
    bool_result(dm.request_ecu_identification(destination as Address))
}

/// Request peer software identification. Requires DM memory.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_dm_memory_request_software_identification(
    h: *mut MachbusSession,
    destination: u8,
) -> bool {
    let h = try_handle_mut!(h);
    let dm = plugin_mut!(h, DmMemory);
    bool_result(dm.request_software_identification(destination as Address))
}

/// Send a DM14 memory-access request to `destination`. Requires DM memory.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_dm_memory_send_dm14(
    h: *mut MachbusSession,
    destination: u8,
    request: *const MachbusDm14Request,
) -> bool {
    let h = try_handle_mut!(h);
    if request.is_null() {
        set_last_error("null DM14 request pointer");
        return false;
    }
    let req: crate::j1939::Dm14Request = (unsafe { *request }).into();
    let dm = plugin_mut!(h, DmMemory);
    bool_result(dm.send_dm14(destination as Address, &req))
}

/// Last received DM14 request `(source, request)`. Requires DM memory.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_dm_memory_last_dm14(
    h: *const MachbusSession,
    out_source: *mut u8,
    out: *mut MachbusDm14Request,
) -> bool {
    let Some(dm) = handle_ref(h).ok().and_then(|h| h.session.get::<DmMemory>()) else {
        return false;
    };
    match dm.last_dm14() {
        Some((src, req)) => {
            if !out_source.is_null() {
                unsafe { *out_source = src };
            }
            if !out.is_null() {
                unsafe { *out = req.into() };
            }
            true
        }
        None => false,
    }
}

/// Last received DM15 response `(source, response)`. Requires DM memory.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_dm_memory_last_dm15(
    h: *const MachbusSession,
    out_source: *mut u8,
    out: *mut MachbusDm15Response,
) -> bool {
    let Some(dm) = handle_ref(h).ok().and_then(|h| h.session.get::<DmMemory>()) else {
        return false;
    };
    match dm.last_dm15() {
        Some((src, resp)) => {
            if !out_source.is_null() {
                unsafe { *out_source = src };
            }
            if !out.is_null() {
                unsafe { *out = resp.into() };
            }
            true
        }
        None => false,
    }
}

// ─── FsClient ─────────────────────────────────────────────────────────

/// Connect the file-client to a file-server address. Requires FS client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_connect_to(h: *mut MachbusSession, server: u8) -> bool {
    let h = try_handle_mut!(h);
    let fs = plugin_mut!(h, FsClient);
    bool_result(fs.connect_to(server as Address))
}

/// Disconnect the file-client. Requires FS client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_disconnect(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let fs = plugin_mut!(h, FsClient);
    fs.disconnect();
    clear_last_error();
    true
}

/// Whether the file-client is connected.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_is_connected(h: *const MachbusSession) -> bool {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<FsClient>())
        .map(FsClient::is_connected)
        .unwrap_or(false)
}

/// Open a file by `path` with `flags`. On success writes the request TAN into
/// `out_tan` and returns true; the response arrives later as an FS event keyed
/// by that TAN. Requires FS client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_open(
    h: *mut MachbusSession,
    path: *const c_char,
    flags: u8,
    out_tan: *mut u8,
) -> bool {
    let h = try_handle_mut!(h);
    let Some(path) = read_c_str(path) else {
        set_last_error("invalid file path");
        return false;
    };
    let fs = plugin_mut!(h, FsClient);
    match fs.open(&path, flags) {
        Ok(tan) => {
            if !out_tan.is_null() {
                unsafe { *out_tan = tan };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Close an open file handle. Writes the request TAN into `out_tan`. Requires
/// FS client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_close(
    h: *mut MachbusSession,
    file_handle: u8,
    out_tan: *mut u8,
) -> bool {
    let h = try_handle_mut!(h);
    let fs = plugin_mut!(h, FsClient);
    match fs.close(file_handle) {
        Ok(tan) => {
            if !out_tan.is_null() {
                unsafe { *out_tan = tan };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Read `count` bytes from an open file handle. Writes the request TAN into
/// `out_tan`; the data arrives later as an FS read-response event. Requires FS
/// client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_read(
    h: *mut MachbusSession,
    file_handle: u8,
    count: u16,
    out_tan: *mut u8,
) -> bool {
    let h = try_handle_mut!(h);
    let fs = plugin_mut!(h, FsClient);
    match fs.read(file_handle, count) {
        Ok(tan) => {
            if !out_tan.is_null() {
                unsafe { *out_tan = tan };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Write `len` bytes to an open file handle. Writes the request TAN into
/// `out_tan`. Requires FS client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_write(
    h: *mut MachbusSession,
    file_handle: u8,
    data: *const u8,
    len: usize,
    out_tan: *mut u8,
) -> bool {
    let h = try_handle_mut!(h);
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let fs = plugin_mut!(h, FsClient);
    match fs.write(file_handle, bytes) {
        Ok(tan) => {
            if !out_tan.is_null() {
                unsafe { *out_tan = tan };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Seek an open file handle to `position`. Writes the request TAN into
/// `out_tan`. Requires FS client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_seek(
    h: *mut MachbusSession,
    file_handle: u8,
    position: u32,
    out_tan: *mut u8,
) -> bool {
    let h = try_handle_mut!(h);
    let fs = plugin_mut!(h, FsClient);
    match fs.seek(file_handle, position) {
        Ok(tan) => {
            if !out_tan.is_null() {
                unsafe { *out_tan = tan };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Request the server's current directory. Writes the request TAN into
/// `out_tan`. Requires FS client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_current_directory(
    h: *mut MachbusSession,
    out_tan: *mut u8,
) -> bool {
    let h = try_handle_mut!(h);
    let fs = plugin_mut!(h, FsClient);
    match fs.current_directory() {
        Ok(tan) => {
            if !out_tan.is_null() {
                unsafe { *out_tan = tan };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Change the server's current directory. Writes the request TAN into
/// `out_tan`. Requires FS client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_change_directory(
    h: *mut MachbusSession,
    path: *const c_char,
    out_tan: *mut u8,
) -> bool {
    let h = try_handle_mut!(h);
    let Some(path) = read_c_str(path) else {
        set_last_error("invalid directory path");
        return false;
    };
    let fs = plugin_mut!(h, FsClient);
    match fs.change_directory(&path) {
        Ok(tan) => {
            if !out_tan.is_null() {
                unsafe { *out_tan = tan };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Delete a file by `path`. Writes the request TAN into `out_tan`. Requires FS
/// client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_client_delete_file(
    h: *mut MachbusSession,
    path: *const c_char,
    out_tan: *mut u8,
) -> bool {
    let h = try_handle_mut!(h);
    let Some(path) = read_c_str(path) else {
        set_last_error("invalid file path");
        return false;
    };
    let fs = plugin_mut!(h, FsClient);
    match fs.delete_file(&path) {
        Ok(tan) => {
            if !out_tan.is_null() {
                unsafe { *out_tan = tan };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

// ─── FsServer ─────────────────────────────────────────────────────────

/// Add an in-memory file the server will expose. Requires FS server.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_server_add_file(
    h: *mut MachbusSession,
    path: *const c_char,
    data: *const u8,
    len: usize,
    attrs: u8,
) -> bool {
    let h = try_handle_mut!(h);
    let Some(path) = read_c_str(path) else {
        set_last_error("invalid file path");
        return false;
    };
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let fs = plugin_mut!(h, FsServer);
    bool_result(fs.add_file(path, bytes.to_vec(), attrs))
}

/// Add a directory entry the server will expose. Requires FS server.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_server_add_directory(
    h: *mut MachbusSession,
    path: *const c_char,
) -> bool {
    let h = try_handle_mut!(h);
    let Some(path) = read_c_str(path) else {
        set_last_error("invalid directory path");
        return false;
    };
    let fs = plugin_mut!(h, FsServer);
    bool_result(fs.add_directory(path))
}

/// Set the server's reported volume name. Requires FS server.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_fs_server_set_volume_name(
    h: *mut MachbusSession,
    name: *const c_char,
) -> bool {
    let h = try_handle_mut!(h);
    let Some(name) = read_c_str(name) else {
        set_last_error("invalid volume name");
        return false;
    };
    let fs = plugin_mut!(h, FsServer);
    bool_result(fs.set_volume_name(name))
}

// ─── Heartbeat ────────────────────────────────────────────────────────

/// Track a peer address for heartbeat monitoring. Requires heartbeat.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_heartbeat_track(h: *mut MachbusSession, address: u8) -> bool {
    let h = try_handle_mut!(h);
    let hb = plugin_mut!(h, Heartbeat);
    hb.track(address as Address);
    clear_last_error();
    true
}

/// Stop tracking a peer address. Requires heartbeat.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_heartbeat_untrack(h: *mut MachbusSession, address: u8) -> bool {
    let h = try_handle_mut!(h);
    let hb = plugin_mut!(h, Heartbeat);
    hb.untrack(address as Address);
    clear_last_error();
    true
}

/// Last received heartbeat sequence for `address`. Returns true and fills
/// `out_sequence` if available. Requires heartbeat.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_heartbeat_last_sequence(
    h: *const MachbusSession,
    address: u8,
    out_sequence: *mut u8,
) -> bool {
    let Some(hb) = handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Heartbeat>())
    else {
        return false;
    };
    match hb.last_sequence(address as Address) {
        Some(seq) => {
            if !out_sequence.is_null() {
                unsafe { *out_sequence = seq };
            }
            true
        }
        None => false,
    }
}

/// Count of missed heartbeats for `address`, or 0 if not tracked/plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_heartbeat_missed_count(
    h: *const MachbusSession,
    address: u8,
) -> u32 {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Heartbeat>())
        .map(|hb| hb.missed_count(address as Address))
        .unwrap_or(0)
}

/// Signal an error state in the next heartbeat. Requires heartbeat.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_heartbeat_signal_error(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let hb = plugin_mut!(h, Heartbeat);
    hb.signal_error();
    clear_last_error();
    true
}

/// Signal a shutdown state in the next heartbeat. Requires heartbeat.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_heartbeat_signal_shutdown(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let hb = plugin_mut!(h, Heartbeat);
    hb.signal_shutdown();
    clear_last_error();
    true
}

// ─── LanguageCommand ──────────────────────────────────────────────────
//
// `MachbusLanguageData` repr(C) mirror (+ conversion) is already defined in the
// j1939::language codec section above; reused here.

/// Local language/units data the command plugin reports. Requires language cmd.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_language_command_local(
    h: *const MachbusSession,
    out: *mut MachbusLanguageData,
) -> bool {
    let Some(lc) = handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<LanguageCommand>())
    else {
        return false;
    };
    if out.is_null() {
        return false;
    }
    unsafe { *out = lc.local().into() };
    true
}

/// Broadcast the current local language/units command. Requires language cmd.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_language_command_broadcast(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let lc = plugin_mut!(h, LanguageCommand);
    lc.broadcast();
    clear_last_error();
    true
}

// ─── MaintainPower ────────────────────────────────────────────────────
//
// `MachbusMaintainPowerData` repr(C) mirror (+ conversion) is already defined in
// the j1939::maintain_power codec section above; reused here.

/// MaintainPower role: 0 = CF/client, 1 = TECU server. Returns 0xFF if the
/// subsystem is not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_maintain_power_role(h: *const MachbusSession) -> u8 {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<MaintainPower>())
        .map(|mp| match mp.role() {
            PowerRole::Cf => 0,
            PowerRole::Tecu => 1,
        })
        .unwrap_or(0xFF)
}

/// MaintainPower state byte (0=Running,1=ShutdownPending,2=Maintaining,
/// 3=PowerOff). Returns 0xFF if not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_maintain_power_state(h: *const MachbusSession) -> u8 {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<MaintainPower>())
        .map(|mp| mp.state() as u8)
        .unwrap_or(0xFF)
}

/// Signal key-off (begins the maintain-power countdown). Requires maintain power.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_maintain_power_key_off(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let mp = plugin_mut!(h, MaintainPower);
    mp.key_off();
    clear_last_error();
    true
}

/// Signal key-on. Requires maintain power.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_maintain_power_key_on(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let mp = plugin_mut!(h, MaintainPower);
    mp.key_on();
    clear_last_error();
    true
}

/// Request (or release) maintained power. Requires maintain power.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_maintain_power_request_power(
    h: *mut MachbusSession,
    need_power: bool,
) -> bool {
    let h = try_handle_mut!(h);
    let mp = plugin_mut!(h, MaintainPower);
    mp.request_power(need_power);
    clear_last_error();
    true
}

/// Last received maintain-power data `(source, data)`. Requires maintain power.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_maintain_power_last(
    h: *const MachbusSession,
    out_source: *mut u8,
    out: *mut MachbusMaintainPowerData,
) -> bool {
    let Some(mp) = handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<MaintainPower>())
    else {
        return false;
    };
    match mp.last() {
        Some((src, data)) => {
            if !out_source.is_null() {
                unsafe { *out_source = src };
            }
            if !out.is_null() {
                unsafe { *out = data.into() };
            }
            true
        }
        None => false,
    }
}

// ─── NameManagement ───────────────────────────────────────────────────

/// Command a NAME change: stage `new_name_raw` for the control function whose
/// current identity is `current_identity`. Requires name management.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_name_management_set_pending(
    h: *mut MachbusSession,
    current_identity: u32,
    new_name_raw: u64,
) -> bool {
    let h = try_handle_mut!(h);
    let nm = plugin_mut!(h, NameManagement);
    bool_result(
        nm.manager_mut()
            .set_pending(current_identity, Name::from_raw(new_name_raw)),
    )
}

/// Adopt the staged pending NAME. On success writes the adopted raw NAME into
/// `out_name_raw`. Requires name management.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_name_management_adopt_pending(
    h: *mut MachbusSession,
    out_name_raw: *mut u64,
) -> bool {
    let h = try_handle_mut!(h);
    let nm = plugin_mut!(h, NameManagement);
    match nm.manager_mut().adopt_pending() {
        Ok(name) => {
            if !out_name_raw.is_null() {
                unsafe { *out_name_raw = name.raw };
            }
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

// ─── Powertrain ───────────────────────────────────────────────────────

/// Broadcast an EEC1 (engine speed/torque) message. Requires powertrain.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_powertrain_broadcast_eec1(
    h: *mut MachbusSession,
    data: *const MachbusEec1,
) -> bool {
    let h = try_handle_mut!(h);
    if data.is_null() {
        set_last_error("null EEC1 pointer");
        return false;
    }
    let eec1: J1939Eec1 = (unsafe { *data }).into();
    let pt = plugin_mut!(h, Powertrain);
    pt.broadcast_eec1(&eec1);
    clear_last_error();
    true
}

// `MachbusEtc1` repr(C) mirror (+ `From<MachbusEtc1> for Etc1`) is already
// defined in the j1939::transmission codec section above; reused here.

/// Broadcast an ETC1 (electronic transmission) message. Requires powertrain.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_powertrain_broadcast_etc1(
    h: *mut MachbusSession,
    data: *const MachbusEtc1,
) -> bool {
    let h = try_handle_mut!(h);
    if data.is_null() {
        set_last_error("null ETC1 pointer");
        return false;
    }
    let etc1: J1939Etc1 = (unsafe { *data }).into();
    let pt = plugin_mut!(h, Powertrain);
    pt.broadcast_etc1(&etc1);
    clear_last_error();
    true
}

/// Broadcast a vehicle identification (VIN) message (UTF-8, NUL-terminated).
/// Requires powertrain.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_powertrain_broadcast_vehicle_identification(
    h: *mut MachbusSession,
    vin: *const c_char,
) -> bool {
    let h = try_handle_mut!(h);
    let Some(vin) = read_c_str(vin) else {
        set_last_error("invalid VIN string");
        return false;
    };
    let pt = plugin_mut!(h, Powertrain);
    pt.broadcast_vehicle_identification(&crate::j1939::VehicleIdentification { vin });
    clear_last_error();
    true
}

/// Latest decoded EEC1 from the powertrain snapshot. Returns true and fills
/// `out` if present. Requires powertrain.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_powertrain_snapshot_eec1(
    h: *const MachbusSession,
    out: *mut MachbusEec1,
) -> bool {
    let Some(pt) = handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Powertrain>())
    else {
        return false;
    };
    match pt.snapshot().eec1 {
        Some(eec1) if !out.is_null() => {
            unsafe { *out = eec1.into() };
            true
        }
        Some(_) => true,
        None => false,
    }
}

// ─── Request2 ─────────────────────────────────────────────────────────

/// Register a canned response for a requested PGN. Requires request2.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_request2_register_response(
    h: *mut MachbusSession,
    pgn: u32,
    data: *const u8,
    len: usize,
) -> bool {
    let h = try_handle_mut!(h);
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let r2 = plugin_mut!(h, Request2);
    bool_result(r2.responder_mut().register_response(pgn, bytes.to_vec()))
}

/// Remove a previously-registered response for `pgn`. Returns true if one was
/// removed. Requires request2.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_request2_remove_response(
    h: *mut MachbusSession,
    pgn: u32,
) -> bool {
    let h = try_handle_mut!(h);
    let r2 = plugin_mut!(h, Request2);
    let removed = r2.responder_mut().remove_response(pgn).is_some();
    clear_last_error();
    removed
}

/// Number of registered request2 responses, or 0 if not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_request2_response_count(h: *const MachbusSession) -> usize {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Request2>())
        .map(|r2| r2.responder().response_count())
        .unwrap_or(0)
}

// ─── ScClient ─────────────────────────────────────────────────────────

/// Sequence-control client state byte (0=Idle,1=Ready,2=Active,3=Paused,
/// 4=Complete,5=Error). Returns 0xFF if not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_client_state(h: *const MachbusSession) -> u8 {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<ScClient>())
        .map(|sc| sc.state() as u8)
        .unwrap_or(0xFF)
}

/// Whether the sequence-control client is busy.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_client_is_busy(h: *const MachbusSession) -> bool {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<ScClient>())
        .map(ScClient::is_busy)
        .unwrap_or(false)
}

/// Set the client busy flag. Requires SC client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_client_set_busy(h: *mut MachbusSession, busy: bool) -> bool {
    let h = try_handle_mut!(h);
    let sc = plugin_mut!(h, ScClient);
    sc.set_busy(busy);
    clear_last_error();
    true
}

/// Report a sequence step complete by id. Requires SC client.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_client_report_step_complete(
    h: *mut MachbusSession,
    step_id: u16,
) -> bool {
    let h = try_handle_mut!(h);
    let sc = plugin_mut!(h, ScClient);
    bool_result(sc.report_step_complete(step_id))
}

// ─── ScMaster ─────────────────────────────────────────────────────────

/// Sequence-control master state byte (see `machbus_session_sc_client_state`).
/// Returns 0xFF if not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_master_state(h: *const MachbusSession) -> u8 {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<ScMaster>())
        .map(|sc| sc.state() as u8)
        .unwrap_or(0xFF)
}

/// Add a sequence step (`step_id`, `description` UTF-8/NUL-terminated,
/// `duration_ms`). Requires SC master.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_master_add_step(
    h: *mut MachbusSession,
    step_id: u16,
    description: *const c_char,
    duration_ms: u32,
) -> bool {
    let h = try_handle_mut!(h);
    let description = read_c_str(description).unwrap_or_default();
    let sc = plugin_mut!(h, ScMaster);
    bool_result(sc.add_step(crate::isobus::SequenceStep {
        step_id,
        description,
        duration_ms,
        completed: false,
    }))
}

/// Start the configured sequence. Requires SC master.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_master_start(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let sc = plugin_mut!(h, ScMaster);
    bool_result(sc.start())
}

/// Pause the running sequence. Requires SC master.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_master_pause(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let sc = plugin_mut!(h, ScMaster);
    bool_result(sc.pause())
}

/// Resume a paused sequence. Requires SC master.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_master_resume(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let sc = plugin_mut!(h, ScMaster);
    bool_result(sc.resume())
}

/// Abort the running sequence. Requires SC master.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_master_abort(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let sc = plugin_mut!(h, ScMaster);
    bool_result(sc.abort())
}

/// Mark a sequence step completed by id. Requires SC master.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_sc_master_step_completed(
    h: *mut MachbusSession,
    step_id: u16,
) -> bool {
    let h = try_handle_mut!(h);
    let sc = plugin_mut!(h, ScMaster);
    bool_result(sc.step_completed(step_id))
}

// ─── ShortcutButton ───────────────────────────────────────────────────

/// Broadcast a shortcut-button state (0=StopImplementOperations,
/// 1=PermitAllImplementsToOperate, 2=Error, 3=NotAvailable). Requires shortcut.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_shortcut_button_broadcast(
    h: *mut MachbusSession,
    state: u8,
) -> bool {
    let h = try_handle_mut!(h);
    let sb = plugin_mut!(h, ShortcutButton);
    sb.broadcast(shortcut_state_from_u8(state));
    clear_last_error();
    true
}

/// Broadcast a shortcut-button state with an explicit transition count.
/// Requires shortcut button.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_shortcut_button_broadcast_with_transition_count(
    h: *mut MachbusSession,
    state: u8,
    count: u8,
) -> bool {
    let h = try_handle_mut!(h);
    let sb = plugin_mut!(h, ShortcutButton);
    sb.broadcast_with_transition_count(shortcut_state_from_u8(state), count);
    clear_last_error();
    true
}

fn shortcut_state_from_u8(v: u8) -> crate::j1939::shortcut_button::ShortcutButtonState {
    crate::j1939::shortcut_button::ShortcutButtonState::from_u8(v)
}

// ─── Tim (tractor-implement management) ───────────────────────────────

/// `#[repr(C)]` mirror of [`machbus::isobus::TimInterlocks`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusTimInterlocks {
    pub operator_present: bool,
    pub road_transport_mode: bool,
    pub external_stop: bool,
    pub implement_ready: bool,
}

impl From<MachbusTimInterlocks> for TimInterlocks {
    fn from(i: MachbusTimInterlocks) -> Self {
        Self {
            operator_present: i.operator_present,
            road_transport_mode: i.road_transport_mode,
            external_stop: i.external_stop,
            implement_ready: i.implement_ready,
        }
    }
}

/// Request TIM authority for the option set encoded in the 3 `options` bytes.
/// Requires TIM.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tim_request_authority(
    h: *mut MachbusSession,
    options: *const u8,
) -> bool {
    let h = try_handle_mut!(h);
    let bytes = match read_bytes(options, 3) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let set = TimOptionSet::from_bytes([bytes[0], bytes[1], bytes[2]]);
    let tim = plugin_mut!(h, Tim);
    bool_result(tim.request_authority(set))
}

/// Grant TIM authority (server side). Requires TIM.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tim_grant_authority(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let tim = plugin_mut!(h, Tim);
    bool_result(tim.grant_authority())
}

/// Deny TIM authority. Requires TIM.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tim_deny_authority(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let tim = plugin_mut!(h, Tim);
    tim.deny_authority();
    clear_last_error();
    true
}

/// Revoke TIM authority. Requires TIM.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tim_revoke_authority(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let tim = plugin_mut!(h, Tim);
    tim.revoke_authority();
    clear_last_error();
    true
}

/// Set the TIM safety interlocks. Requires TIM.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tim_set_interlocks(
    h: *mut MachbusSession,
    interlocks: MachbusTimInterlocks,
) -> bool {
    let h = try_handle_mut!(h);
    let tim = plugin_mut!(h, Tim);
    tim.set_interlocks(interlocks.into());
    clear_last_error();
    true
}

/// Command a hitch (front/rear) to a target position (0..=MAX) at `rate`.
/// Requires TIM.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tim_command_hitch_position(
    h: *mut MachbusSession,
    hitch: MachbusHitch,
    target_position: u16,
    rate: u8,
) -> bool {
    let h = try_handle_mut!(h);
    let tim = plugin_mut!(h, Tim);
    bool_result(tim.command_hitch_position(hitch.into(), target_position, rate))
}

/// Engage a PTO (front/rear), `cw_direction` = clockwise. Requires TIM.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tim_command_pto_engage(
    h: *mut MachbusSession,
    pto: MachbusPto,
    cw_direction: bool,
) -> bool {
    let h = try_handle_mut!(h);
    let tim = plugin_mut!(h, Tim);
    bool_result(tim.command_pto_engage(pto.into(), cw_direction))
}

/// Disengage a PTO (front/rear). Requires TIM.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tim_command_pto_disengage(
    h: *mut MachbusSession,
    pto: MachbusPto,
) -> bool {
    let h = try_handle_mut!(h);
    let tim = plugin_mut!(h, Tim);
    bool_result(tim.command_pto_disengage(pto.into()))
}

// ─── TcServer (task-controller server) ────────────────────────────────

/// Start the TC server. Requires TC server.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tc_server_start(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let tc = plugin_mut!(h, TcServer);
    bool_result(tc.server_mut().start())
}

/// Stop the TC server. Requires TC server.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tc_server_stop(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let tc = plugin_mut!(h, TcServer);
    bool_result(tc.server_mut().stop())
}

/// TC server state byte (0=Disconnected,1=WaitForClients,2=Active). Returns
/// 0xFF if not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tc_server_state(h: *mut MachbusSession) -> u8 {
    handle_mut(h)
        .ok()
        .and_then(|h| h.session.get_mut::<TcServer>())
        .map(|tc| tc.server_mut().state() as u8)
        .unwrap_or(0xFF)
}

/// Number of connected TC clients, or 0 if not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tc_server_client_count(h: *mut MachbusSession) -> usize {
    handle_mut(h)
        .ok()
        .and_then(|h| h.session.get_mut::<TcServer>())
        .map(|tc| tc.server_mut().clients().len())
        .unwrap_or(0)
}

// ─── VtServer (virtual terminal server) ───────────────────────────────

/// Start the VT server. Requires VT server.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_server_start(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let vt = plugin_mut!(h, VtServer);
    bool_result(vt.start())
}

/// Stop the VT server. Requires VT server.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_server_stop(h: *mut MachbusSession) -> bool {
    let h = try_handle_mut!(h);
    let vt = plugin_mut!(h, VtServer);
    bool_result(vt.stop())
}

/// VT server state code (see `vt_server_state_code`). Returns 0 (Disconnected)
/// if not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_server_state(h: *const MachbusSession) -> u32 {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<VtServer>())
        .map(|vt| vt_server_state_code(vt.state()))
        .unwrap_or(0)
}

/// VT server active working-set address, or NULL_ADDRESS if not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_server_active_working_set(h: *mut MachbusSession) -> u8 {
    handle_mut(h)
        .ok()
        .and_then(|h| h.session.get_mut::<VtServer>())
        .map(|vt| vt.server_mut().active_working_set())
        .unwrap_or(NULL_ADDRESS)
}

/// Set the VT server active working-set address. Requires VT server.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_server_set_active_working_set(
    h: *mut MachbusSession,
    address: u8,
) -> bool {
    let h = try_handle_mut!(h);
    let vt = plugin_mut!(h, VtServer);
    vt.server_mut().set_active_working_set(address as Address);
    clear_last_error();
    true
}

// ═══════════════════════════════════════════════════════════════════════
// Standalone NMEA decode handle — per-PGN NMEA 2000 navigation decode with
// no session. Feed (pgn, bytes, source) frames; poll decoded results out of
// an internal queue. Wraps `crate::nmea::NMEAInterface`.
// ═══════════════════════════════════════════════════════════════════════

use std::cell::RefCell as NmeaRefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// One decoded NMEA event drained from an [`MachbusNmea`] handle.
#[derive(Clone, Copy)]
enum NmeaItem {
    Position(MachbusGnssPosition),
    Cog(f64),
    Sog(f64),
    Heading(f64),
    MagneticVariation(f64),
}

type NmeaQueue = Rc<NmeaRefCell<VecDeque<NmeaItem>>>;

/// Opaque standalone NMEA decoder. Create with [`machbus_nmea_new`], feed frames
/// with [`machbus_nmea_feed`], drain results with the `machbus_nmea_poll_*`
/// functions, and release with [`machbus_nmea_free`]. Single-threaded: keep the
/// handle pinned to the thread that created it.
pub struct MachbusNmea {
    iface: NMEAInterface,
    queue: NmeaQueue,
}

impl From<GNSSPosition> for MachbusGnssPosition {
    fn from(p: GNSSPosition) -> Self {
        Self {
            latitude: p.wgs.latitude,
            longitude: p.wgs.longitude,
            altitude_m: p.altitude_m.unwrap_or(f64::NAN),
            speed_mps: p.speed_mps.unwrap_or(f64::NAN),
            heading_rad: p.heading_rad.unwrap_or(f64::NAN),
        }
    }
}

/// Create a standalone NMEA decoder. If `listen_all` is true every supported
/// PGN is decoded; otherwise the default navigation subset is used. Returns
/// `NULL` only on allocation failure. Free with [`machbus_nmea_free`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_new(listen_all: bool) -> *mut MachbusNmea {
    let config = if listen_all {
        NMEAConfig::default().with_all(true)
    } else {
        NMEAConfig::default()
    };
    let mut iface = NMEAInterface::new(config);
    let queue: NmeaQueue = Rc::new(NmeaRefCell::new(VecDeque::new()));

    let q = queue.clone();
    iface.on_position.subscribe(move |p: &GNSSPosition| {
        q.borrow_mut().push_back(NmeaItem::Position((*p).into()));
    });
    let q = queue.clone();
    iface.on_cog.subscribe(move |c: &f64| {
        q.borrow_mut().push_back(NmeaItem::Cog(*c));
    });
    let q = queue.clone();
    iface.on_sog.subscribe(move |s: &f64| {
        q.borrow_mut().push_back(NmeaItem::Sog(*s));
    });
    let q = queue.clone();
    iface.on_heading.subscribe(move |hd: &f64| {
        q.borrow_mut().push_back(NmeaItem::Heading(*hd));
    });
    let q = queue.clone();
    iface.on_magnetic_variation.subscribe(move |v: &f64| {
        q.borrow_mut().push_back(NmeaItem::MagneticVariation(*v));
    });

    clear_last_error();
    Box::into_raw(Box::new(MachbusNmea { iface, queue }))
}

/// Free a standalone NMEA decoder. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_free(h: *mut MachbusNmea) {
    if h.is_null() {
        return;
    }
    // SAFETY: pointer originated from Box::into_raw in machbus_nmea_new.
    unsafe { drop(Box::from_raw(h)) };
}

/// Feed one NMEA 2000 message (`pgn`, `len` data bytes, `source` address) into
/// the decoder. Decoded results are queued for the `machbus_nmea_poll_*`
/// functions. Returns false on a null handle/data or an unusable source
/// address (0xFE/0xFF are rejected by the envelope gate).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_feed(
    h: *mut MachbusNmea,
    pgn: u32,
    data: *const u8,
    len: usize,
    source: u8,
) -> bool {
    if h.is_null() {
        set_last_error("null NMEA handle");
        return false;
    }
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    // SAFETY: validated non-null.
    let nmea = unsafe { &mut *h };
    let msg = Message::new(pgn, bytes.to_vec(), source as Address);
    nmea.iface.handle_message(&msg);
    clear_last_error();
    true
}

fn nmea_pop<F>(h: *mut MachbusNmea, pred: F) -> Option<NmeaItem>
where
    F: FnMut(&NmeaItem) -> bool,
{
    if h.is_null() {
        return None;
    }
    // SAFETY: validated non-null.
    let nmea = unsafe { &mut *h };
    let mut q = nmea.queue.borrow_mut();
    let idx = q.iter().position(pred)?;
    q.remove(idx)
}

/// Drain the next decoded position into `out`. Returns true if one was
/// available. (FIFO across all events, but only position items are matched.)
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_poll_position(
    h: *mut MachbusNmea,
    out: *mut MachbusGnssPosition,
) -> bool {
    match nmea_pop(h, |it| matches!(it, NmeaItem::Position(_))) {
        Some(NmeaItem::Position(p)) => {
            if !out.is_null() {
                unsafe { *out = p };
            }
            true
        }
        _ => false,
    }
}

/// Drain the next decoded course-over-ground (radians) into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_poll_cog(h: *mut MachbusNmea, out: *mut f64) -> bool {
    match nmea_pop(h, |it| matches!(it, NmeaItem::Cog(_))) {
        Some(NmeaItem::Cog(v)) => {
            if !out.is_null() {
                unsafe { *out = v };
            }
            true
        }
        _ => false,
    }
}

/// Drain the next decoded speed-over-ground (m/s) into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_poll_sog(h: *mut MachbusNmea, out: *mut f64) -> bool {
    match nmea_pop(h, |it| matches!(it, NmeaItem::Sog(_))) {
        Some(NmeaItem::Sog(v)) => {
            if !out.is_null() {
                unsafe { *out = v };
            }
            true
        }
        _ => false,
    }
}

/// Drain the next decoded heading (radians) into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_poll_heading(h: *mut MachbusNmea, out: *mut f64) -> bool {
    match nmea_pop(h, |it| matches!(it, NmeaItem::Heading(_))) {
        Some(NmeaItem::Heading(v)) => {
            if !out.is_null() {
                unsafe { *out = v };
            }
            true
        }
        _ => false,
    }
}

/// Drain the next decoded magnetic variation (radians) into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_poll_magnetic_variation(h: *mut MachbusNmea, out: *mut f64) -> bool {
    match nmea_pop(h, |it| matches!(it, NmeaItem::MagneticVariation(_))) {
        Some(NmeaItem::MagneticVariation(v)) => {
            if !out.is_null() {
                unsafe { *out = v };
            }
            true
        }
        _ => false,
    }
}

/// Latest cached position without consuming the queue. Returns true and fills
/// `out` if a position has been decoded.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_nmea_latest_position(
    h: *const MachbusNmea,
    out: *mut MachbusGnssPosition,
) -> bool {
    if h.is_null() {
        return false;
    }
    // SAFETY: validated non-null.
    let nmea = unsafe { &*h };
    match nmea.iface.latest_position() {
        Some(p) if !out.is_null() => {
            unsafe { *out = p.into() };
            true
        }
        Some(_) => true,
        None => false,
    }
}

// ─── VT object pool / TC DDOP construction (milestone 3) ──────────────
//
// These opaque, `Box`-backed handles let a C caller assemble an ISO 11783-6
// VT [`ObjectPool`] (object-by-object or from a prebuilt `.iop` blob) and an
// ISO 11783-10 [`DDOP`] (object-by-object), serialize them, and hand them to a
// [`Session`] at construction via
// [`machbus_session_new_with_content`]. Free each handle exactly once with the
// matching `*_free`, or transfer ownership to the session (see that function).

/// Copy `bytes` into the caller buffer (length-query convention): returns the
/// full byte length required. If `out` is non-null and `cap >= bytes.len()` the
/// bytes are copied; otherwise nothing is written so the caller can size a
/// buffer with a first (null or short) call. A `0` return means error (see
/// [`machbus_session_last_error`]) — callers serializing a legitimately empty
/// payload should treat `0` as "nothing to write".
fn copy_bytes_out(bytes: &[u8], out: *mut u8, cap: usize) -> usize {
    if !out.is_null() && bytes.len() <= cap {
        // SAFETY: out is valid for `cap` bytes per the contract and we only
        // copy when it is large enough.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    }
    bytes.len()
}

// ── MachbusVtPool ──

/// Owned, opaque ISO 11783-6 VT object pool. Build with
/// [`machbus_vt_pool_new`] or [`machbus_vt_pool_from_iop`], populate with the
/// typed `machbus_vt_pool_add_*` adders, and release with
/// [`machbus_vt_pool_free`] (or transfer to a session).
pub struct MachbusVtPool(ObjectPool);

fn vt_pool_mut<'a>(p: *mut MachbusVtPool) -> Option<&'a mut ObjectPool> {
    if p.is_null() {
        set_last_error("null VT pool handle");
        return None;
    }
    // SAFETY: validated non-null; caller owns the box.
    Some(unsafe { &mut (*p).0 })
}

/// Create an empty VT object pool. Free with [`machbus_vt_pool_free`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_new() -> *mut MachbusVtPool {
    clear_last_error();
    Box::into_raw(Box::new(MachbusVtPool(ObjectPool::default())))
}

/// Load a prebuilt ISO 11783-6 `.iop` object-pool blob into a new VT pool.
///
/// The bytes are first checked with [`crate::net::parse_iop_data`] /
/// [`crate::net::validate`], then converted into a structured [`ObjectPool`]
/// via [`ObjectPool::deserialize`] (which splits each parent object's inline
/// child / macro tail into the pool's structured fields so the pool re-
/// serializes byte-for-byte). Returns `NULL` on malformed input; inspect
/// [`machbus_session_last_error`]. Free with [`machbus_vt_pool_free`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_from_iop(data: *const u8, len: usize) -> *mut MachbusVtPool {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    // Reject structurally malformed pools up front (empty / truncated /
    // unknown type / duplicate id). `validate` is the cheap bool gate;
    // `parse_iop_data` surfaces a precise reason on failure.
    if !crate::net::validate(bytes) {
        if let Err(e) = crate::net::parse_iop_data(bytes) {
            set_last_error(e.to_string());
        } else {
            set_last_error("invalid IOP object pool data");
        }
        return ptr::null_mut();
    }
    match ObjectPool::deserialize(bytes) {
        Ok(pool) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusVtPool(pool)))
        }
        Err(e) => {
            set_last_error(e.to_string());
            ptr::null_mut()
        }
    }
}

