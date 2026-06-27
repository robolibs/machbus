impl From<MachbusDm21Readiness> for crate::j1939::Dm21Readiness {
    fn from(d: MachbusDm21Readiness) -> Self {
        Self {
            distance_with_mil_on_km: d.distance_with_mil_on_km,
            distance_since_codes_cleared_km: d.distance_since_codes_cleared_km,
            minutes_with_mil_on: d.minutes_with_mil_on,
            time_since_codes_cleared_min: d.time_since_codes_cleared_min,
            comprehensive_component: d.comprehensive_component,
            fuel_system: d.fuel_system,
            misfire: d.misfire,
        }
    }
}

pod_codec!(
    MachbusDm21Readiness,
    crate::j1939::Dm21Readiness,
    11,
    decode = machbus_j1939_dm21_readiness_decode,
    encode = machbus_j1939_dm21_readiness_encode,
    err = "Dm21Readiness decode failed"
);

// ── Dm22Message (8-byte fixed; control + optional nack reason) ──

/// `#[repr(C)]` mirror of [`machbus::j1939::Dm22Message`]. `control` is the
/// raw `Dm22Control` byte; `nack_reason` is the raw `Dm22NackReason` byte and is
/// only meaningful when `nack_reason_present` is true; `fmi` is the raw FMI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm22Message {
    pub control: u8,
    pub nack_reason: u8,
    pub nack_reason_present: bool,
    pub spn: u32,
    pub fmi: u8,
}

impl From<crate::j1939::Dm22Message> for MachbusDm22Message {
    fn from(d: crate::j1939::Dm22Message) -> Self {
        Self {
            control: d.control.as_u8(),
            nack_reason: d.nack_reason.map(|r| r.as_u8()).unwrap_or(0),
            nack_reason_present: d.nack_reason.is_some(),
            spn: d.spn,
            fmi: d.fmi.as_u8(),
        }
    }
}

impl From<MachbusDm22Message> for crate::j1939::Dm22Message {
    fn from(d: MachbusDm22Message) -> Self {
        use crate::j1939::diagnostic::{Dm22Control, Dm22NackReason};
        Self {
            control: Dm22Control::from_u8(d.control).unwrap_or(Dm22Control::ClearPreviouslyActive),
            nack_reason: if d.nack_reason_present {
                Dm22NackReason::from_u8(d.nack_reason)
            } else {
                None
            },
            spn: d.spn,
            fmi: Fmi::from_u8(d.fmi),
        }
    }
}

pod_codec!(
    MachbusDm22Message,
    crate::j1939::Dm22Message,
    8,
    decode = machbus_j1939_dm22_message_decode,
    encode = machbus_j1939_dm22_message_encode,
    err = "Dm22Message decode failed"
);

// ── DM9 request / DM10 VIN response ──

/// Decode a DM9 Vehicle Identification request (a PGN-request naming the VIN
/// PGN). Returns true on a valid DM9 request.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_dm9_request_decode(data: *const u8, len: usize) -> bool {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    if crate::j1939::Dm9VehicleIdentificationRequest::decode(bytes).is_some() {
        clear_last_error();
        true
    } else {
        set_last_error("Dm9 request decode failed");
        false
    }
}

/// Encode a DM9 Vehicle Identification request into the caller's 3-byte buffer.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_dm9_request_encode(out: *mut u8) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    match crate::j1939::Dm9VehicleIdentificationRequest.encode() {
        Ok(bytes) => {
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, 3) };
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Decode a DM10 Vehicle Identification (`*`-terminated VIN) into `out` (cap
/// bytes). Returns the VIN byte length (excluding NUL), or 0 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_dm10_vehicle_identification_decode(
    data: *const u8,
    len: usize,
    out: *mut c_char,
    cap: usize,
) -> usize {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return 0;
        }
    };
    match crate::j1939::Dm10VehicleIdentification::decode(bytes) {
        Some(v) => {
            clear_last_error();
            copy_str_out(&v.vin, out, cap)
        }
        None => {
            set_last_error("Dm10 decode failed");
            0
        }
    }
}

/// Encode a DM10 Vehicle Identification from a NUL-terminated VIN into `out`
/// (cap bytes). Returns the full encoded length; if it exceeds `cap` nothing is
/// copied. Returns 0 on validation/null failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_dm10_vehicle_identification_encode(
    vin: *const c_char,
    out: *mut u8,
    cap: usize,
) -> usize {
    let Some(vin) = read_c_str(vin) else {
        set_last_error("null/invalid VIN string");
        return 0;
    };
    match (crate::j1939::Dm10VehicleIdentification { vin }).encode() {
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

// ── ProductIdentification (3 `*`-delimited strings) — opaque handle ──

/// Owned, opaque decoded [`machbus::j1939::ProductIdentification`].
pub struct MachbusProductIdentification(crate::j1939::ProductIdentification);

/// Decode a ProductIdentification payload. Returns an owned handle (free with
/// [`machbus_j1939_product_identification_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_product_identification_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusProductIdentification {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::ProductIdentification::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusProductIdentification(v)))
        }
        None => {
            set_last_error("ProductIdentification decode failed");
            ptr::null_mut()
        }
    }
}

str_field_accessor!(
    machbus_j1939_product_identification_make_into,
    MachbusProductIdentification,
    make
);
str_field_accessor!(
    machbus_j1939_product_identification_model_into,
    MachbusProductIdentification,
    model
);
str_field_accessor!(
    machbus_j1939_product_identification_serial_number_into,
    MachbusProductIdentification,
    serial_number
);

/// Free a ProductIdentification handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_product_identification_free(h: *mut MachbusProductIdentification) {
    if h.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(h)) };
}

/// Encode a ProductIdentification from three NUL-terminated field strings into
/// `out` (cap bytes). Returns the full encoded length; if it exceeds `cap`
/// nothing is copied. Returns 0 on validation/null failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_product_identification_encode(
    make: *const c_char,
    model: *const c_char,
    serial_number: *const c_char,
    out: *mut u8,
    cap: usize,
) -> usize {
    let (Some(make), Some(model), Some(serial_number)) = (
        read_c_str(make),
        read_c_str(model),
        read_c_str(serial_number),
    ) else {
        set_last_error("null/invalid field string");
        return 0;
    };
    let v = crate::j1939::ProductIdentification {
        make,
        model,
        serial_number,
    };
    match v.encode() {
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

// ── SoftwareIdentification (Vec<String>) — opaque handle ──

/// Owned, opaque decoded [`machbus::j1939::SoftwareIdentification`].
pub struct MachbusSoftwareIdentification(crate::j1939::SoftwareIdentification);

/// Decode a SoftwareIdentification payload. Returns an owned handle (free with
/// [`machbus_j1939_software_identification_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_software_identification_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusSoftwareIdentification {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::SoftwareIdentification::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusSoftwareIdentification(v)))
        }
        None => {
            set_last_error("SoftwareIdentification decode failed");
            ptr::null_mut()
        }
    }
}

/// Number of version strings in a SoftwareIdentification handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_software_identification_count(
    h: *const MachbusSoftwareIdentification,
) -> usize {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.versions.len(),
        None => 0,
    }
}

/// Copy version `idx` as a NUL-terminated string into `out` (cap bytes).
/// Returns the byte length (excluding NUL), or 0 if `idx` is out of range.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_software_identification_get_into(
    h: *const MachbusSoftwareIdentification,
    idx: usize,
    out: *mut c_char,
    cap: usize,
) -> usize {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return 0;
    };
    match h.0.versions.get(idx) {
        Some(v) => copy_str_out(v, out, cap),
        None => {
            set_last_error("version index out of range");
            0
        }
    }
}

/// Free a SoftwareIdentification handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_software_identification_free(
    h: *mut MachbusSoftwareIdentification,
) {
    if h.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(h)) };
}

// ── MonitorPerformanceRatio (7-byte fixed) ──

/// `#[repr(C)]` mirror of [`machbus::j1939::MonitorPerformanceRatio`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusMonitorPerformanceRatio {
    pub spn: u32,
    pub numerator: u16,
    pub denominator: u16,
}

impl From<crate::j1939::MonitorPerformanceRatio> for MachbusMonitorPerformanceRatio {
    fn from(d: crate::j1939::MonitorPerformanceRatio) -> Self {
        Self {
            spn: d.spn,
            numerator: d.numerator,
            denominator: d.denominator,
        }
    }
}

impl From<MachbusMonitorPerformanceRatio> for crate::j1939::MonitorPerformanceRatio {
    fn from(d: MachbusMonitorPerformanceRatio) -> Self {
        Self {
            spn: d.spn,
            numerator: d.numerator,
            denominator: d.denominator,
        }
    }
}

pod_codec!(
    MachbusMonitorPerformanceRatio,
    crate::j1939::MonitorPerformanceRatio,
    7,
    decode = machbus_j1939_monitor_performance_ratio_decode,
    encode = machbus_j1939_monitor_performance_ratio_encode,
    err = "MonitorPerformanceRatio decode failed"
);

// ── Dm20Response (header + Vec<MonitorPerformanceRatio>) — opaque ──

/// Owned, opaque decoded [`machbus::j1939::Dm20Response`].
pub struct MachbusDm20Response(crate::j1939::Dm20Response);

/// Decode a DM20 response. Returns an owned handle (free with
/// [`machbus_dm20_response_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm20_response_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusDm20Response {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::Dm20Response::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusDm20Response(v)))
        }
        None => {
            set_last_error("Dm20Response decode failed");
            ptr::null_mut()
        }
    }
}

/// DM20 ignition-cycle counter, or 0 if the handle is null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm20_response_ignition_cycles(h: *const MachbusDm20Response) -> u8 {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.ignition_cycles,
        None => 0,
    }
}

/// DM20 OBD monitoring-conditions counter, or 0 if the handle is null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm20_response_obd_conditions(h: *const MachbusDm20Response) -> u8 {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.obd_monitoring_conditions_met,
        None => 0,
    }
}

/// Number of performance ratios in a DM20 response handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm20_response_count(h: *const MachbusDm20Response) -> usize {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.ratios.len(),
        None => 0,
    }
}

/// Copy the performance ratio at `idx` out of a DM20 response handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm20_response_get(
    h: *const MachbusDm20Response,
    idx: usize,
    out: *mut MachbusMonitorPerformanceRatio,
) -> bool {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return false;
    };
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    match h.0.ratios.get(idx) {
        Some(r) => {
            unsafe { *out = (*r).into() };
            clear_last_error();
            true
        }
        None => {
            set_last_error("ratio index out of range");
            false
        }
    }
}

/// Free a DM20 response handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm20_response_free(h: *mut MachbusDm20Response) {
    if h.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(h)) };
}

// ── SpnSnapshot (7-byte fixed) ──

/// `#[repr(C)]` mirror of [`machbus::j1939::SpnSnapshot`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusSpnSnapshot {
    pub spn: u32,
    pub value: u32,
}

impl From<crate::j1939::SpnSnapshot> for MachbusSpnSnapshot {
    fn from(d: crate::j1939::SpnSnapshot) -> Self {
        Self {
            spn: d.spn,
            value: d.value,
        }
    }
}

impl From<MachbusSpnSnapshot> for crate::j1939::SpnSnapshot {
    fn from(d: MachbusSpnSnapshot) -> Self {
        Self {
            spn: d.spn,
            value: d.value,
        }
    }
}

pod_codec!(
    MachbusSpnSnapshot,
    crate::j1939::SpnSnapshot,
    7,
    decode = machbus_j1939_spn_snapshot_decode,
    encode = machbus_j1939_spn_snapshot_encode,
    err = "SpnSnapshot decode failed"
);

// ── FreezeFrame (DTC + timestamp + Vec<SpnSnapshot>) — opaque ──

/// Owned, opaque decoded [`machbus::j1939::FreezeFrame`].
pub struct MachbusFreezeFrame(crate::j1939::FreezeFrame);

/// Decode a FreezeFrame payload. Returns an owned handle (free with
/// [`machbus_freeze_frame_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_freeze_frame_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusFreezeFrame {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::FreezeFrame::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusFreezeFrame(v)))
        }
        None => {
            set_last_error("FreezeFrame decode failed");
            ptr::null_mut()
        }
    }
}

/// Copy the FreezeFrame DTC out of a handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_freeze_frame_dtc(
    h: *const MachbusFreezeFrame,
    out: *mut MachbusDtc,
) -> bool {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return false;
    };
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    unsafe { *out = h.0.dtc.into() };
    clear_last_error();
    true
}

/// FreezeFrame internal timestamp (ms), or 0 if the handle is null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_freeze_frame_timestamp_ms(h: *const MachbusFreezeFrame) -> u32 {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.timestamp_ms,
        None => 0,
    }
}

/// Number of SPN snapshots in a FreezeFrame handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_freeze_frame_count(h: *const MachbusFreezeFrame) -> usize {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.snapshots.len(),
        None => 0,
    }
}

/// Copy the SPN snapshot at `idx` out of a FreezeFrame handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_freeze_frame_get(
    h: *const MachbusFreezeFrame,
    idx: usize,
    out: *mut MachbusSpnSnapshot,
) -> bool {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return false;
    };
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    match h.0.snapshots.get(idx) {
        Some(s) => {
            unsafe { *out = (*s).into() };
            clear_last_error();
            true
        }
        None => {
            set_last_error("snapshot index out of range");
            false
        }
    }
}

/// Free a FreezeFrame handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_freeze_frame_free(h: *mut MachbusFreezeFrame) {
    if h.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(h)) };
}

// ── Dm25Request (8-byte fixed) ──

/// `#[repr(C)]` mirror of [`machbus::j1939::Dm25Request`]. `fmi` is the raw FMI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm25Request {
    pub spn: u32,
    pub fmi: u8,
    pub frame_number: u8,
}

impl From<crate::j1939::Dm25Request> for MachbusDm25Request {
    fn from(d: crate::j1939::Dm25Request) -> Self {
        Self {
            spn: d.spn,
            fmi: d.fmi.as_u8(),
            frame_number: d.frame_number,
        }
    }
}

impl From<MachbusDm25Request> for crate::j1939::Dm25Request {
    fn from(d: MachbusDm25Request) -> Self {
        Self {
            spn: d.spn,
            fmi: Fmi::from_u8(d.fmi),
            frame_number: d.frame_number,
        }
    }
}

pod_codec!(
    MachbusDm25Request,
    crate::j1939::Dm25Request,
    8,
    decode = machbus_j1939_dm25_request_decode,
    encode = machbus_j1939_dm25_request_encode,
    err = "Dm25Request decode failed"
);

// ══════════════════════════════════════════════════════════════════════
// j1939::dm_memory
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::Dm14Request`]. `command` is the raw
/// `Dm14Command` byte; `pointer_type` the raw `Dm14PointerType` byte.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm14Request {
    pub command: u8,
    pub pointer_type: u8,
    pub address: u32,
    pub length: u16,
    pub key: u8,
}

impl From<crate::j1939::Dm14Request> for MachbusDm14Request {
    fn from(d: crate::j1939::Dm14Request) -> Self {
        Self {
            command: d.command.as_u8(),
            pointer_type: d.pointer_type.as_u8(),
            address: d.address,
            length: d.length,
            key: d.key,
        }
    }
}

impl From<MachbusDm14Request> for crate::j1939::Dm14Request {
    fn from(d: MachbusDm14Request) -> Self {
        use crate::j1939::dm_memory::{Dm14Command, Dm14PointerType};
        Self {
            command: Dm14Command::from_u8(d.command),
            pointer_type: Dm14PointerType::from_u8(d.pointer_type),
            address: d.address,
            length: d.length,
            key: d.key,
        }
    }
}

pod_codec_try_encode!(
    MachbusDm14Request,
    crate::j1939::Dm14Request,
    8,
    decode = machbus_j1939_dm14_request_decode,
    encode = machbus_j1939_dm14_request_encode,
    err = "Dm14Request decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::Dm15Response`]. `status` is the raw
/// `Dm15Status` byte.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm15Response {
    pub status: u8,
    pub length: u16,
    pub address: u32,
    pub edcp_extension: u8,
    pub seed: u8,
}

impl From<crate::j1939::Dm15Response> for MachbusDm15Response {
    fn from(d: crate::j1939::Dm15Response) -> Self {
        Self {
            status: d.status.as_u8(),
            length: d.length,
            address: d.address,
            edcp_extension: d.edcp_extension,
            seed: d.seed,
        }
    }
}

impl From<MachbusDm15Response> for crate::j1939::Dm15Response {
    fn from(d: MachbusDm15Response) -> Self {
        Self {
            status: crate::j1939::dm_memory::Dm15Status::from_u8(d.status),
            length: d.length,
            address: d.address,
            edcp_extension: d.edcp_extension,
            seed: d.seed,
        }
    }
}

pod_codec_try_encode!(
    MachbusDm15Response,
    crate::j1939::Dm15Response,
    8,
    decode = machbus_j1939_dm15_response_decode,
    encode = machbus_j1939_dm15_response_encode,
    err = "Dm15Response decode failed"
);

// ── Dm16Transfer (num_bytes + Vec<u8>) — opaque handle ──

/// Owned, opaque decoded [`machbus::j1939::Dm16Transfer`].
pub struct MachbusDm16Transfer(crate::j1939::Dm16Transfer);

/// Decode a DM16 binary transfer payload. Returns an owned handle (free with
/// [`machbus_dm16_transfer_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm16_transfer_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusDm16Transfer {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::Dm16Transfer::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusDm16Transfer(v)))
        }
        None => {
            set_last_error("Dm16Transfer decode failed");
            ptr::null_mut()
        }
    }
}

/// Declared `num_bytes` of a DM16 transfer handle, or 0 if null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm16_transfer_num_bytes(h: *const MachbusDm16Transfer) -> u8 {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.num_bytes,
        None => 0,
    }
}

/// Copy the DM16 data bytes into `out` (cap bytes). Returns the full data
/// length; if it exceeds `cap` nothing is copied.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm16_transfer_data_into(
    h: *const MachbusDm16Transfer,
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

/// Free a DM16 transfer handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm16_transfer_free(h: *mut MachbusDm16Transfer) {
    if h.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(h)) };
}

/// Encode a DM16 single-frame transfer from `data` (`len` ≤ 7 bytes) into the
/// caller's 8-byte buffer `out`. Returns false on length/null errors.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm16_transfer_encode(data: *const u8, len: usize, out: *mut u8) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let transfer = crate::j1939::Dm16Transfer {
        num_bytes: len as u8,
        data: bytes.to_vec(),
    };
    match transfer.encode() {
        Ok(buf) => {
            unsafe { core::ptr::copy_nonoverlapping(buf.as_ptr(), out, 8) };
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

// ── EcuIdentification (5/6 `*`-delimited strings) — opaque handle ──

/// Owned, opaque decoded [`machbus::j1939::EcuIdentification`].
pub struct MachbusEcuIdentification(crate::j1939::EcuIdentification);

/// Decode an ECU Identification payload. Returns an owned handle (free with
/// [`machbus_ecu_identification_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ecu_identification_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusEcuIdentification {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::EcuIdentification::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusEcuIdentification(v)))
        }
        None => {
            set_last_error("EcuIdentification decode failed");
            ptr::null_mut()
        }
    }
}

str_field_accessor!(
    machbus_ecu_identification_part_number_into,
    MachbusEcuIdentification,
    ecu_part_number
);
str_field_accessor!(
    machbus_ecu_identification_serial_number_into,
    MachbusEcuIdentification,
    ecu_serial_number
);
str_field_accessor!(
    machbus_ecu_identification_location_into,
    MachbusEcuIdentification,
    ecu_location
);
str_field_accessor!(
    machbus_ecu_identification_type_into,
    MachbusEcuIdentification,
    ecu_type
);
str_field_accessor!(
    machbus_ecu_identification_manufacturer_into,
    MachbusEcuIdentification,
    ecu_manufacturer
);

/// Whether the ECU Identification has the optional ISO 11783 hardware-id field.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ecu_identification_has_hardware_id(
    h: *const MachbusEcuIdentification,
) -> bool {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.ecu_hardware_id.is_some(),
        None => false,
    }
}

/// Copy the optional hardware-id field as a NUL-terminated string into `out`.
/// Returns the byte length (excluding NUL); 0 if absent.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ecu_identification_hardware_id_into(
    h: *const MachbusEcuIdentification,
    out: *mut c_char,
    cap: usize,
) -> usize {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return 0;
    };
    match &h.0.ecu_hardware_id {
        Some(s) => copy_str_out(s, out, cap),
        None => 0,
    }
}

/// Free an ECU Identification handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ecu_identification_free(h: *mut MachbusEcuIdentification) {
    if h.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(h)) };
}

/// Encode an ECU Identification from five (J1939) field strings plus an optional
/// hardware id (`hardware_id` may be `NULL` for the five-field J1939 form) into
/// `out` (cap bytes). Returns the full encoded length; if it exceeds `cap`
/// nothing is copied. Returns 0 on validation/null failure.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ecu_identification_encode(
    part_number: *const c_char,
    serial_number: *const c_char,
    location: *const c_char,
    ecu_type: *const c_char,
    manufacturer: *const c_char,
    hardware_id: *const c_char,
    out: *mut u8,
    cap: usize,
) -> usize {
    let (
        Some(part_number),
        Some(serial_number),
        Some(location),
        Some(ecu_type),
        Some(manufacturer),
    ) = (
        read_c_str(part_number),
        read_c_str(serial_number),
        read_c_str(location),
        read_c_str(ecu_type),
        read_c_str(manufacturer),
    )
    else {
        set_last_error("null/invalid field string");
        return 0;
    };
    let v = crate::j1939::EcuIdentification {
        ecu_part_number: part_number,
        ecu_serial_number: serial_number,
        ecu_location: location,
        ecu_type,
        ecu_manufacturer: manufacturer,
        ecu_hardware_id: read_c_str(hardware_id),
    };
    match v.encode() {
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
// j1939::acknowledgment
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::AckControl`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusAckControl {
    PositiveAck = 0,
    NegativeAck = 1,
    AccessDenied = 2,
    CannotRespond = 3,
}

impl From<crate::j1939::AckControl> for MachbusAckControl {
    fn from(c: crate::j1939::AckControl) -> Self {
        match c {
            crate::j1939::AckControl::PositiveAck => Self::PositiveAck,
            crate::j1939::AckControl::NegativeAck => Self::NegativeAck,
            crate::j1939::AckControl::AccessDenied => Self::AccessDenied,
            crate::j1939::AckControl::CannotRespond => Self::CannotRespond,
        }
    }
}

impl From<MachbusAckControl> for crate::j1939::AckControl {
    fn from(c: MachbusAckControl) -> Self {
        crate::j1939::AckControl::from_u8(c as u8)
    }
}

/// `#[repr(C)]` mirror of [`machbus::j1939::Acknowledgment`]. `control` is the
/// raw [`MachbusAckControl`] byte.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusAcknowledgment {
    pub control: u8,
    pub group_function: u8,
    pub acknowledged_pgn: u32,
    pub address: u8,
}

impl From<crate::j1939::Acknowledgment> for MachbusAcknowledgment {
    fn from(a: crate::j1939::Acknowledgment) -> Self {
        Self {
            control: a.control.as_u8(),
            group_function: a.group_function,
            acknowledged_pgn: a.acknowledged_pgn,
            address: a.address,
        }
    }
}

impl From<MachbusAcknowledgment> for crate::j1939::Acknowledgment {
    fn from(a: MachbusAcknowledgment) -> Self {
        Self {
            control: crate::j1939::AckControl::from_u8(a.control),
            group_function: a.group_function,
            acknowledged_pgn: a.acknowledged_pgn,
            address: a.address,
        }
    }
}

pod_codec_try_encode!(
    MachbusAcknowledgment,
    crate::j1939::Acknowledgment,
    8,
    decode = machbus_j1939_acknowledgment_decode,
    encode = machbus_j1939_acknowledgment_encode,
    err = "Acknowledgment decode failed"
);

// ══════════════════════════════════════════════════════════════════════
// j1939::language — LanguageData (decode takes a Message)
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::LanguageData`]. The unit-system
/// fields are the raw enum bytes (all 0=Metric, with Imperial/US variants where
/// defined). `language_code` / `country_code` are the two ASCII bytes each.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusLanguageData {
    pub language_code: [u8; 2],
    pub decimal: u8,
    pub time_format: u8,
    pub date_format: u8,
    pub distance: u8,
    pub area: u8,
    pub volume: u8,
    pub mass: u8,
    pub temperature: u8,
    pub pressure: u8,
    pub force: u8,
    pub country_code: [u8; 2],
    pub generic: u8,
}

impl From<crate::j1939::LanguageData> for MachbusLanguageData {
    fn from(l: crate::j1939::LanguageData) -> Self {
        Self {
            language_code: l.language_code,
            decimal: l.decimal.as_u8(),
            time_format: l.time_format.as_u8(),
            date_format: l.date_format.as_u8(),
            distance: l.distance.as_u8(),
            area: l.area.as_u8(),
            volume: l.volume.as_u8(),
            mass: l.mass.as_u8(),
            temperature: l.temperature.as_u8(),
            pressure: l.pressure.as_u8(),
            force: l.force.as_u8(),
            country_code: l.country_code,
            generic: l.generic.as_u8(),
        }
    }
}

impl From<MachbusLanguageData> for crate::j1939::LanguageData {
    fn from(l: MachbusLanguageData) -> Self {
        use crate::j1939::language::{
            AreaUnit, DateFormat, DecimalSymbol, DistanceUnit, ForceUnit, MassUnit, PressureUnit,
            TemperatureUnit, TimeFormat, UnitSystem, VolumeUnit,
        };
        Self {
            language_code: l.language_code,
            decimal: DecimalSymbol::try_from_u8(l.decimal).unwrap_or_default(),
            time_format: TimeFormat::try_from_u8(l.time_format).unwrap_or_default(),
            date_format: DateFormat::try_from_u8(l.date_format).unwrap_or_default(),
            distance: DistanceUnit::try_from_u8(l.distance).unwrap_or_default(),
            area: AreaUnit::try_from_u8(l.area).unwrap_or_default(),
            volume: VolumeUnit::try_from_u8(l.volume).unwrap_or_default(),
            mass: MassUnit::try_from_u8(l.mass).unwrap_or_default(),
            temperature: TemperatureUnit::try_from_u8(l.temperature).unwrap_or_default(),
            pressure: PressureUnit::try_from_u8(l.pressure).unwrap_or_default(),
            force: ForceUnit::try_from_u8(l.force).unwrap_or_default(),
            country_code: l.country_code,
            generic: UnitSystem::try_from_u8(l.generic).unwrap_or_default(),
        }
    }
}

/// Decode an 8-byte Language Command payload into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_language_data_decode(
    data: *const u8,
    len: usize,
    out: *mut MachbusLanguageData,
) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let msg = crate::net::message::Message::new(
        crate::net::pgn_defs::PGN_LANGUAGE_COMMAND,
        bytes.to_vec(),
        0,
    );
    match crate::j1939::LanguageData::decode(&msg) {
        Some(v) => {
            unsafe { *out = v.into() };
            clear_last_error();
            true
        }
        None => {
            set_last_error("LanguageData decode failed");
            false
        }
    }
}

/// Encode a LanguageData into the caller's 8-byte buffer `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_language_data_encode(
    input: *const MachbusLanguageData,
    out: *mut u8,
) -> bool {
    if input.is_null() || out.is_null() {
        set_last_error("null pointer");
        return false;
    }
    let v: crate::j1939::LanguageData = (unsafe { *input }).into();
    let bytes = v.encode();
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, 8) };
    clear_last_error();
    true
}

// ══════════════════════════════════════════════════════════════════════
// j1939::maintain_power — MaintainPowerData
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::MaintainPowerData`]. State fields
/// are raw `MaintainPowerState` bytes (0=Inactive, 1=Active, 2=Error, 3=N/A);
/// maintain fields are raw `MaintainPowerRequirement` bytes. `timestamp_us` is
/// repo-internal and not on the wire.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusMaintainPowerData {
    pub implement_in_work_state: u8,
    pub implement_park_state: u8,
    pub implement_ready_to_work_state: u8,
    pub implement_transport_state: u8,
    pub maintain_actuator_power: u8,
    pub maintain_ecu_power: u8,
    pub timestamp_us: u64,
}

impl From<crate::j1939::MaintainPowerData> for MachbusMaintainPowerData {
    fn from(m: crate::j1939::MaintainPowerData) -> Self {
        Self {
            implement_in_work_state: m.implement_in_work_state.as_u8(),
            implement_park_state: m.implement_park_state.as_u8(),
            implement_ready_to_work_state: m.implement_ready_to_work_state.as_u8(),
            implement_transport_state: m.implement_transport_state.as_u8(),
            maintain_actuator_power: m.maintain_actuator_power.as_u8(),
            maintain_ecu_power: m.maintain_ecu_power.as_u8(),
            timestamp_us: m.timestamp_us,
        }
    }
}

impl From<MachbusMaintainPowerData> for crate::j1939::MaintainPowerData {
    fn from(m: MachbusMaintainPowerData) -> Self {
        use crate::j1939::maintain_power::{MaintainPowerRequirement, MaintainPowerState};
        Self {
            implement_in_work_state: MaintainPowerState::from_u8(m.implement_in_work_state),
            implement_park_state: MaintainPowerState::from_u8(m.implement_park_state),
            implement_ready_to_work_state: MaintainPowerState::from_u8(
                m.implement_ready_to_work_state,
            ),
            implement_transport_state: MaintainPowerState::from_u8(m.implement_transport_state),
            maintain_actuator_power: MaintainPowerRequirement::from_u8(m.maintain_actuator_power),
            maintain_ecu_power: MaintainPowerRequirement::from_u8(m.maintain_ecu_power),
            timestamp_us: m.timestamp_us,
        }
    }
}

pod_codec!(
    MachbusMaintainPowerData,
    crate::j1939::MaintainPowerData,
    8,
    decode = machbus_j1939_maintain_power_decode,
    encode = machbus_j1939_maintain_power_encode,
    err = "MaintainPowerData decode failed"
);

// ══════════════════════════════════════════════════════════════════════
// j1939::speed_distance — SpeedAndDistance (Option<f64> fields)
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::SpeedAndDistance`]. `*_present`
/// flags carry the `Option` discriminant; the value is ignored when false.
/// `timestamp_us` is repo-internal.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusSpeedAndDistance {
    pub speed_mps: f64,
    pub speed_mps_present: bool,
    pub distance_m: f64,
    pub distance_m_present: bool,
    pub timestamp_us: u64,
}

impl From<crate::j1939::SpeedAndDistance> for MachbusSpeedAndDistance {
    fn from(s: crate::j1939::SpeedAndDistance) -> Self {
        Self {
            speed_mps: s.speed_mps.unwrap_or(0.0),
            speed_mps_present: s.speed_mps.is_some(),
            distance_m: s.distance_m.unwrap_or(0.0),
            distance_m_present: s.distance_m.is_some(),
            timestamp_us: s.timestamp_us,
        }
    }
}

impl From<MachbusSpeedAndDistance> for crate::j1939::SpeedAndDistance {
    fn from(s: MachbusSpeedAndDistance) -> Self {
        Self {
            speed_mps: s.speed_mps_present.then_some(s.speed_mps),
            distance_m: s.distance_m_present.then_some(s.distance_m),
            timestamp_us: s.timestamp_us,
        }
    }
}

pod_codec!(
    MachbusSpeedAndDistance,
    crate::j1939::SpeedAndDistance,
    8,
    decode = machbus_j1939_speed_and_distance_decode,
    encode = machbus_j1939_speed_and_distance_encode,
    err = "SpeedAndDistance decode failed"
);

// ══════════════════════════════════════════════════════════════════════
// j1939::transmission — Etc1 / TransmissionOilTemp / CruiseControl
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::Etc1`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEtc1 {
    pub current_gear: i8,
    pub selected_gear: i8,
    pub output_shaft_speed_rpm: f64,
    pub shift_in_progress: u8,
    pub torque_converter_lockup: u8,
}

impl From<crate::j1939::Etc1> for MachbusEtc1 {
    fn from(e: crate::j1939::Etc1) -> Self {
        Self {
            current_gear: e.current_gear,
            selected_gear: e.selected_gear,
            output_shaft_speed_rpm: e.output_shaft_speed_rpm,
            shift_in_progress: e.shift_in_progress,
            torque_converter_lockup: e.torque_converter_lockup,
        }
    }
}

impl From<MachbusEtc1> for crate::j1939::Etc1 {
    fn from(e: MachbusEtc1) -> Self {
        Self {
            current_gear: e.current_gear,
            selected_gear: e.selected_gear,
            output_shaft_speed_rpm: e.output_shaft_speed_rpm,
            shift_in_progress: e.shift_in_progress,
            torque_converter_lockup: e.torque_converter_lockup,
        }
    }
}

pod_codec!(
    MachbusEtc1,
    crate::j1939::Etc1,
    8,
    decode = machbus_j1939_etc1_decode,
    encode = machbus_j1939_etc1_encode,
    err = "ETC1 decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::TransmissionOilTemp`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusTransmissionOilTemp {
    pub oil_temp_c: f64,
}

impl From<crate::j1939::TransmissionOilTemp> for MachbusTransmissionOilTemp {
    fn from(t: crate::j1939::TransmissionOilTemp) -> Self {
        Self {
            oil_temp_c: t.oil_temp_c,
        }
    }
}

impl From<MachbusTransmissionOilTemp> for crate::j1939::TransmissionOilTemp {
    fn from(t: MachbusTransmissionOilTemp) -> Self {
        Self {
            oil_temp_c: t.oil_temp_c,
        }
    }
}

pod_codec!(
    MachbusTransmissionOilTemp,
    crate::j1939::TransmissionOilTemp,
    8,
    decode = machbus_j1939_transmission_oil_temp_decode,
    encode = machbus_j1939_transmission_oil_temp_encode,
    err = "TransmissionOilTemp decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::CruiseControl`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusCruiseControl {
    pub wheel_speed_kmh: f64,
    pub cc_active: u8,
    pub brake_switch: u8,
    pub clutch_switch: u8,
    pub park_brake: u8,
    pub cc_set_speed_kmh: f64,
}

impl From<crate::j1939::CruiseControl> for MachbusCruiseControl {
    fn from(c: crate::j1939::CruiseControl) -> Self {
        Self {
            wheel_speed_kmh: c.wheel_speed_kmh,
            cc_active: c.cc_active,
            brake_switch: c.brake_switch,
            clutch_switch: c.clutch_switch,
            park_brake: c.park_brake,
            cc_set_speed_kmh: c.cc_set_speed_kmh,
        }
    }
}

impl From<MachbusCruiseControl> for crate::j1939::CruiseControl {
    fn from(c: MachbusCruiseControl) -> Self {
        Self {
            wheel_speed_kmh: c.wheel_speed_kmh,
            cc_active: c.cc_active,
            brake_switch: c.brake_switch,
            clutch_switch: c.clutch_switch,
            park_brake: c.park_brake,
            cc_set_speed_kmh: c.cc_set_speed_kmh,
        }
    }
}

pod_codec!(
    MachbusCruiseControl,
    crate::j1939::CruiseControl,
    8,
    decode = machbus_j1939_cruise_control_decode,
    encode = machbus_j1939_cruise_control_encode,
    err = "CruiseControl decode failed"
);

// ══════════════════════════════════════════════════════════════════════
// j1939::shortcut_button — ShortcutButtonMessage (decode takes a Message)
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::shortcut_button::ShortcutButtonMessage`].
/// `state` is the raw `ShortcutButtonState` byte (0=Stop, 1=Permit, 2=Error,
/// 3=N/A).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusShortcutButtonMessage {
    pub state: u8,
    pub transition_count: u8,
}

/// Decode an 8-byte Shortcut Button payload into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_shortcut_button_decode(
    data: *const u8,
    len: usize,
    out: *mut MachbusShortcutButtonMessage,
) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let msg = crate::net::message::Message::new(
        crate::net::pgn_defs::PGN_SHORTCUT_BUTTON,
        bytes.to_vec(),
        0,
    );
    match crate::j1939::shortcut_button::decode_message(&msg) {
        Some(v) => {
            unsafe {
                *out = MachbusShortcutButtonMessage {
                    state: v.state.as_u8(),
                    transition_count: v.transition_count,
                };
            }
            clear_last_error();
            true
        }
        None => {
            set_last_error("ShortcutButton decode failed");
            false
        }
    }
}

/// Encode a Shortcut Button message (state byte + transition count) into the
/// caller's 8-byte buffer `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_shortcut_button_encode(
    state: u8,
    transition_count: u8,
    out: *mut u8,
) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    let st = crate::j1939::shortcut_button::ShortcutButtonState::from_u8(state);
    let bytes = crate::j1939::shortcut_button::encode_with_transition_count(st, transition_count);
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, 8) };
    clear_last_error();
    true
}

// ══════════════════════════════════════════════════════════════════════
// j1939::time_date — TimeDate (Option fields, decode takes a Message)
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::TimeDate`]. Each `*_present` flag
/// carries the `Option` discriminant; the paired value is ignored when false.
/// `timestamp_us` is repo-internal.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusTimeDate {
    pub seconds: u8,
    pub seconds_present: bool,
    pub minutes: u8,
    pub minutes_present: bool,
    pub hours: u8,
    pub hours_present: bool,
    pub day: u8,
    pub day_present: bool,
    pub month: u8,
    pub month_present: bool,
    pub year: u16,
    pub year_present: bool,
    pub utc_offset_min: i16,
    pub utc_offset_min_present: bool,
    pub utc_offset_hours: i8,
    pub utc_offset_hours_present: bool,
    pub timestamp_us: u64,
}

impl From<crate::j1939::TimeDate> for MachbusTimeDate {
    fn from(t: crate::j1939::TimeDate) -> Self {
        Self {
            seconds: t.seconds.unwrap_or(0),
            seconds_present: t.seconds.is_some(),
            minutes: t.minutes.unwrap_or(0),
            minutes_present: t.minutes.is_some(),
            hours: t.hours.unwrap_or(0),
            hours_present: t.hours.is_some(),
            day: t.day.unwrap_or(0),
            day_present: t.day.is_some(),
            month: t.month.unwrap_or(0),
            month_present: t.month.is_some(),
            year: t.year.unwrap_or(0),
            year_present: t.year.is_some(),
            utc_offset_min: t.utc_offset_min.unwrap_or(0),
            utc_offset_min_present: t.utc_offset_min.is_some(),
            utc_offset_hours: t.utc_offset_hours.unwrap_or(0),
            utc_offset_hours_present: t.utc_offset_hours.is_some(),
            timestamp_us: t.timestamp_us,
        }
    }
}

impl From<MachbusTimeDate> for crate::j1939::TimeDate {
    fn from(t: MachbusTimeDate) -> Self {
        Self {
            seconds: t.seconds_present.then_some(t.seconds),
            minutes: t.minutes_present.then_some(t.minutes),
            hours: t.hours_present.then_some(t.hours),
            day: t.day_present.then_some(t.day),
            month: t.month_present.then_some(t.month),
            year: t.year_present.then_some(t.year),
            utc_offset_min: t.utc_offset_min_present.then_some(t.utc_offset_min),
            utc_offset_hours: t.utc_offset_hours_present.then_some(t.utc_offset_hours),
            timestamp_us: t.timestamp_us,
        }
    }
}

/// Decode an 8-byte Time/Date payload into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_time_date_decode(
    data: *const u8,
    len: usize,
    out: *mut MachbusTimeDate,
) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let msg =
        crate::net::message::Message::new(crate::net::pgn_defs::PGN_TIME_DATE, bytes.to_vec(), 0);
    match crate::j1939::TimeDate::decode(&msg) {
        Some(v) => {
            unsafe { *out = v.into() };
            clear_last_error();
            true
        }
        None => {
            set_last_error("TimeDate decode failed");
            false
        }
    }
}

/// Encode a TimeDate into the caller's 8-byte buffer `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_time_date_encode(
    input: *const MachbusTimeDate,
    out: *mut u8,
) -> bool {
    if input.is_null() || out.is_null() {
        set_last_error("null pointer");
        return false;
    }
    let v: crate::j1939::TimeDate = (unsafe { *input }).into();
    let bytes = v.encode();
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, 8) };
    clear_last_error();
    true
}

// ══════════════════════════════════════════════════════════════════════
// j1939::request2 — Request2Msg (Vec extended_id) / TransferMsg (Vec data)
// ══════════════════════════════════════════════════════════════════════

/// Owned, opaque decoded [`machbus::j1939::Request2Msg`].
pub struct MachbusRequest2Msg(crate::j1939::Request2Msg);

/// Decode a Request2 payload. Returns an owned handle (free with
/// [`machbus_request2_msg_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_request2_msg_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusRequest2Msg {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::Request2Msg::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusRequest2Msg(v)))
        }
        None => {
            set_last_error("Request2Msg decode failed");
            ptr::null_mut()
        }
    }
}

/// Requested PGN of a Request2 handle, or 0 if null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_request2_msg_requested_pgn(h: *const MachbusRequest2Msg) -> u32 {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.requested_pgn,
        None => 0,
    }
}

/// Whether a Request2 handle asks the responder to reply via the Transfer PGN.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_request2_msg_use_transfer(h: *const MachbusRequest2Msg) -> bool {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.use_transfer,
        None => false,
    }
}

/// Copy the extended-id bytes (0..=3) into `out` (cap bytes). Returns the full
/// length; if it exceeds `cap` nothing is copied.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_request2_msg_extended_id_into(
    h: *const MachbusRequest2Msg,
    out: *mut u8,
    cap: usize,
) -> usize {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return 0;
    };
    let id = &h.0.extended_id;
    if !out.is_null() && id.len() <= cap {
        unsafe { core::ptr::copy_nonoverlapping(id.as_ptr(), out, id.len()) };
    }
    id.len()
}

/// Free a Request2 handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_request2_msg_free(h: *mut MachbusRequest2Msg) {
    if h.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(h)) };
}

/// Encode a Request2 message (requested PGN, up to 3 extended-id bytes, and the
/// use-transfer flag) into the caller's 8-byte buffer `out`. Returns false on
/// invalid PGN / overlong extended id / null pointers.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_request2_msg_encode(
    requested_pgn: u32,
    extended_id: *const u8,
    extended_id_len: usize,
    use_transfer: bool,
    out: *mut u8,
) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    let ext = match read_bytes(extended_id, extended_id_len) {
        Ok(b) => b.to_vec(),
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let msg = crate::j1939::Request2Msg {
        requested_pgn,
        extended_id: ext,
        use_transfer,
    };
    match msg.encode() {
        Ok(bytes) => {
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, 8) };
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Owned, opaque decoded [`machbus::j1939::TransferMsg`].
pub struct MachbusTransferMsg(crate::j1939::TransferMsg);

/// Decode a Transfer payload. Returns an owned handle (free with
/// [`machbus_transfer_msg_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_transfer_msg_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusTransferMsg {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::TransferMsg::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusTransferMsg(v)))
        }
        None => {
            set_last_error("TransferMsg decode failed");
            ptr::null_mut()
        }
    }
}

/// Original PGN carried by a Transfer handle, or 0 if null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_transfer_msg_original_pgn(h: *const MachbusTransferMsg) -> u32 {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.original_pgn,
        None => 0,
    }
}

