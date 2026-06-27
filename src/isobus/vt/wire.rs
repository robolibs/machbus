//! Shared Virtual Terminal wire-format helpers.

/// Return true when a variable-length VT string payload is either exactly sized
/// or is a single CAN-frame payload padded with `0xFF` after the declared
/// string bytes.
#[inline]
pub(crate) fn vt_string_payload_is_canonical(data: &[u8], end: usize) -> bool {
    data.len() == end || (end <= 8 && data.len() == 8 && data[end..].iter().all(|&b| b == 0xFF))
}

/// Decode a string value carried by VT Change String Value commands.
///
/// The public Rust API accepts `&str` and encodes UTF-8 bytes. Inbound VT string
/// values must therefore decode as UTF-8 too; silently widening arbitrary bytes
/// to `char` corrupts multi-byte text (`é` becomes `Ã©`) and lets malformed
/// payloads mutate client/server state.
#[inline]
pub(crate) fn decode_vt_string_value(bytes: &[u8]) -> Option<&str> {
    core::str::from_utf8(bytes).ok()
}
