use alloc::{format, string::String, vec::Vec};

use crate::net::error::{Error, Result};

#[inline]
fn decode_text_byte(byte: u8) -> Option<char> {
    match byte {
        0x20..=0x7E | 0xA0..=0xFF => char::from_u32(byte as u32),
        _ => None,
    }
}

#[must_use]
pub(crate) fn decode_iso11783_text_field(raw: &[u8]) -> Option<String> {
    let mut out = String::new();
    for &byte in raw {
        out.push(decode_text_byte(byte)?);
    }
    Some(out)
}

pub(crate) fn encode_iso11783_text_field(
    field_name: &'static str,
    value: &str,
    forbidden: &[char],
) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '*' || forbidden.contains(&ch) {
            return Err(Error::invalid_data(format!(
                "{field_name} contains a reserved delimiter character"
            )));
        }
        let code = ch as u32;
        match code {
            0x20..=0x7E | 0xA0..=0xFF => out.push(code as u8),
            _ => {
                return Err(Error::invalid_data(format!(
                    "{field_name} contains a non-printable diagnostic text character"
                )));
            }
        }
    }
    Ok(out)
}
