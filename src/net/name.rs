//! ISO 11783-5 NAME — the 64-bit device identity used in address claim
//! arbitration.
//!
//! Mirrors the C++ `machbus::net::Name`. Bit layout (LSB first):
//!
//! ```text
//!   [0..20]   Identity Number       (21 bits)
//!   [21..31]  Manufacturer Code     (11 bits)
//!   [32..34]  ECU Instance          ( 3 bits)
//!   [35..39]  Function Instance     ( 5 bits)
//!   [40..47]  Function Code         ( 8 bits)
//!   [48]      Reserved              ( 1 bit )
//!   [49..55]  Device Class          ( 7 bits)
//!   [56..59]  Device Class Instance ( 4 bits)
//!   [60..62]  Industry Group        ( 3 bits)
//!   [63]      Self-Configurable     ( 1 bit )
//! ```
//!
//! Address claim arbitration treats the NAME as an unsigned 64-bit
//! integer: **lower NAME wins**. The derived [`Ord`] / [`PartialOrd`]
//! implementations on the `raw` field provide that comparison directly.

use core::cmp::Ordering;

/// 64-bit ISO 11783-5 device identity.
///
/// Use the `with_*` consuming setters to build a NAME, or
/// [`Name::from_bytes`] / [`Name::raw`] when interoperating with wire
/// formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Name {
    pub raw: u64,
}

impl Name {
    /// Wrap a raw 64-bit value.
    #[inline]
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self { raw: value }
    }

    // ─── Field accessors ─────────────────────────────────────────────
    #[inline]
    #[must_use]
    pub const fn identity_number(self) -> u32 {
        (self.raw & 0x1F_FFFF) as u32
    }

    #[inline]
    #[must_use]
    pub const fn manufacturer_code(self) -> u16 {
        ((self.raw >> 21) & 0x7FF) as u16
    }

    #[inline]
    #[must_use]
    pub const fn ecu_instance(self) -> u8 {
        ((self.raw >> 32) & 0x07) as u8
    }

    #[inline]
    #[must_use]
    pub const fn function_instance(self) -> u8 {
        ((self.raw >> 35) & 0x1F) as u8
    }

    #[inline]
    #[must_use]
    pub const fn function_code(self) -> u8 {
        ((self.raw >> 40) & 0xFF) as u8
    }

    #[inline]
    #[must_use]
    pub const fn device_class(self) -> u8 {
        ((self.raw >> 49) & 0x7F) as u8
    }

    #[inline]
    #[must_use]
    pub const fn device_class_instance(self) -> u8 {
        ((self.raw >> 56) & 0x0F) as u8
    }

    #[inline]
    #[must_use]
    pub const fn industry_group(self) -> u8 {
        ((self.raw >> 60) & 0x07) as u8
    }

    #[inline]
    #[must_use]
    pub const fn self_configurable(self) -> bool {
        ((self.raw >> 63) & 0x01) != 0
    }

    // ─── Builder-style setters (consuming) ───────────────────────────
    #[inline]
    #[must_use]
    pub const fn with_identity_number(mut self, val: u32) -> Self {
        self.raw = (self.raw & !0x1F_FFFFu64) | ((val as u64) & 0x1F_FFFF);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_manufacturer_code(mut self, val: u16) -> Self {
        self.raw = (self.raw & !(0x7FFu64 << 21)) | (((val as u64) & 0x7FF) << 21);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_ecu_instance(mut self, val: u8) -> Self {
        self.raw = (self.raw & !(0x07u64 << 32)) | (((val as u64) & 0x07) << 32);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_function_instance(mut self, val: u8) -> Self {
        self.raw = (self.raw & !(0x1Fu64 << 35)) | (((val as u64) & 0x1F) << 35);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_function_code(mut self, val: u8) -> Self {
        self.raw = (self.raw & !(0xFFu64 << 40)) | (((val as u64) & 0xFF) << 40);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_device_class(mut self, val: u8) -> Self {
        self.raw = (self.raw & !(0x7Fu64 << 49)) | (((val as u64) & 0x7F) << 49);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_device_class_instance(mut self, val: u8) -> Self {
        self.raw = (self.raw & !(0x0Fu64 << 56)) | (((val as u64) & 0x0F) << 56);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_industry_group(mut self, val: u8) -> Self {
        self.raw = (self.raw & !(0x07u64 << 60)) | (((val as u64) & 0x07) << 60);
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_self_configurable(mut self, val: bool) -> Self {
        self.raw = (self.raw & !(0x01u64 << 63)) | ((val as u64) << 63);
        self
    }

    // ─── Serialization (little-endian, 8 bytes) ──────────────────────
    /// Encode as 8 little-endian bytes (J1939-21 / ISO 11783-5
    /// Address Claimed payload format).
    #[inline]
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 8] {
        self.raw.to_le_bytes()
    }

    /// Decode from exactly 8 little-endian bytes. Returns `None` if
    /// the slice is not the canonical ISO 11783-5 NAME width.
    #[inline]
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(data);
        Some(Self::from_raw(u64::from_le_bytes(buf)))
    }
}

// Address claim arbitration: lower NAME wins. Derived ordering on the
// raw u64 implements that correctly.
impl PartialOrd for Name {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Name {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.raw.cmp(&other.raw)
    }
}

impl From<u64> for Name {
    #[inline]
    fn from(value: u64) -> Self {
        Self::from_raw(value)
    }
}

impl From<Name> for u64 {
    #[inline]
    fn from(name: Name) -> u64 {
        name.raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let n = Name::default();
        assert_eq!(n.raw, 0);
        assert_eq!(n.identity_number(), 0);
        assert_eq!(n.manufacturer_code(), 0);
        assert!(!n.self_configurable());
    }

    #[test]
    fn each_field_round_trips() {
        let n = Name::default()
            .with_identity_number(0x1F_ABCD) // 21-bit max-ish
            .with_manufacturer_code(0x456) // 11-bit
            .with_ecu_instance(0x05) // 3-bit
            .with_function_instance(0x0A) // 5-bit
            .with_function_code(0x77)
            .with_device_class(0x55) // 7-bit
            .with_device_class_instance(0x09) // 4-bit
            .with_industry_group(0x03) // 3-bit
            .with_self_configurable(true);

        assert_eq!(n.identity_number(), 0x1F_ABCD);
        assert_eq!(n.manufacturer_code(), 0x456);
        assert_eq!(n.ecu_instance(), 0x05);
        assert_eq!(n.function_instance(), 0x0A);
        assert_eq!(n.function_code(), 0x77);
        assert_eq!(n.device_class(), 0x55);
        assert_eq!(n.device_class_instance(), 0x09);
        assert_eq!(n.industry_group(), 0x03);
        assert!(n.self_configurable());
    }

    #[test]
    fn oversize_field_value_is_masked() {
        // Manufacturer is 11 bits; 0xFFFF should mask to 0x7FF.
        let n = Name::default().with_manufacturer_code(0xFFFF);
        assert_eq!(n.manufacturer_code(), 0x7FF);
        // Higher fields must remain untouched.
        assert_eq!(n.industry_group(), 0);
    }

    #[test]
    fn writes_do_not_clobber_other_fields() {
        let n = Name::default()
            .with_identity_number(0x12_3456)
            .with_industry_group(0x05);
        // Setting manufacturer must not clear identity or industry group.
        let n2 = n.with_manufacturer_code(0x123);
        assert_eq!(n2.identity_number(), 0x12_3456);
        assert_eq!(n2.industry_group(), 0x05);
        assert_eq!(n2.manufacturer_code(), 0x123);
    }

    #[test]
    fn arbitration_lower_wins() {
        let a = Name::from_raw(0x0000_0000_0000_0001);
        let b = Name::from_raw(0x0000_0000_0000_0002);
        // Lower NAME wins → a < b.
        assert!(a < b);
        assert!(b > a);
        assert_ne!(a, b);
    }

    #[test]
    fn to_from_bytes_round_trip() {
        let n = Name::default()
            .with_identity_number(0xABCDE)
            .with_manufacturer_code(0x123)
            .with_function_code(0x80);
        let bytes = n.to_bytes();
        let restored = Name::from_bytes(&bytes).expect("8 bytes is enough");
        assert_eq!(restored, n);
    }

    #[test]
    fn from_bytes_short_slice_returns_none() {
        assert!(Name::from_bytes(&[0u8; 7]).is_none());
        assert!(Name::from_bytes(&[]).is_none());
    }

    #[test]
    fn from_bytes_overlong_slice_returns_none() {
        let n = Name::from_raw(0x0123_4567_89AB_CDEF);
        let mut buf = [0u8; 16];
        buf[..8].copy_from_slice(&n.to_bytes());
        assert!(Name::from_bytes(&buf).is_none());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_from_bytes_accepts_only_exact_width(
            data in proptest::collection::vec(any::<u8>(), 0..=16)
        ) {
            let decoded = Name::from_bytes(&data);

            if data.len() == 8 {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data);
                prop_assert_eq!(decoded, Some(Name::from_raw(u64::from_le_bytes(bytes))));
            } else {
                prop_assert!(decoded.is_none());
            }
        }

        #[test]
        fn proptest_each_field_round_trips(
            id in 0u32..=0x1F_FFFF,
            mfg in 0u16..=0x7FF,
            ecu in 0u8..=0x07,
            fninst in 0u8..=0x1F,
            fncode: u8,
            devclass in 0u8..=0x7F,
            devclassinst in 0u8..=0x0F,
            ind in 0u8..=0x07,
            sc: bool,
        ) {
            let n = Name::default()
                .with_identity_number(id)
                .with_manufacturer_code(mfg)
                .with_ecu_instance(ecu)
                .with_function_instance(fninst)
                .with_function_code(fncode)
                .with_device_class(devclass)
                .with_device_class_instance(devclassinst)
                .with_industry_group(ind)
                .with_self_configurable(sc);
            prop_assert_eq!(n.identity_number(), id);
            prop_assert_eq!(n.manufacturer_code(), mfg);
            prop_assert_eq!(n.ecu_instance(), ecu);
            prop_assert_eq!(n.function_instance(), fninst);
            prop_assert_eq!(n.function_code(), fncode);
            prop_assert_eq!(n.device_class(), devclass);
            prop_assert_eq!(n.device_class_instance(), devclassinst);
            prop_assert_eq!(n.industry_group(), ind);
            prop_assert_eq!(n.self_configurable(), sc);
        }

        #[test]
        fn proptest_setters_mask_oversized_inputs(
            id in any::<u32>(),
            mfg in any::<u16>(),
            ecu in any::<u8>(),
            fninst in any::<u8>(),
            devclass in any::<u8>(),
            devclassinst in any::<u8>(),
            ind in any::<u8>(),
        ) {
            let n = Name::default()
                .with_identity_number(id)
                .with_manufacturer_code(mfg)
                .with_ecu_instance(ecu)
                .with_function_instance(fninst)
                .with_device_class(devclass)
                .with_device_class_instance(devclassinst)
                .with_industry_group(ind);

            prop_assert_eq!(n.identity_number(), id & 0x1F_FFFF);
            prop_assert_eq!(n.manufacturer_code(), mfg & 0x07FF);
            prop_assert_eq!(n.ecu_instance(), ecu & 0x07);
            prop_assert_eq!(n.function_instance(), fninst & 0x1F);
            prop_assert_eq!(n.device_class(), devclass & 0x7F);
            prop_assert_eq!(n.device_class_instance(), devclassinst & 0x0F);
            prop_assert_eq!(n.industry_group(), ind & 0x07);
        }

        #[test]
        fn proptest_bytes_round_trip(raw: u64) {
            let n = Name::from_raw(raw);
            let restored = Name::from_bytes(&n.to_bytes()).unwrap();
            prop_assert_eq!(restored.raw, raw);
        }
    }
}
