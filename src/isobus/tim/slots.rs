//! AEF 023 TIM function value SLOTs (Annex D).
//!
//! Every TIM function value is a 2-byte SLOT with its own resolution and
//! offset, plus a shared band of special values at the top of the range. The
//! crate previously modelled these as plain unscaled integers — PTO speed as
//! `1 rpm/bit` with no offset where the spec says `0.125 1/min/bit, −4016`,
//! aux valve flow with "no scaling at all", and no vehicle-speed SLOT, so
//! reverse speed was unrepresentable. None of the `0xFB..` special values
//! existed anywhere.

/// Released control, requesting operator action before the value increases
/// (D.2.1). Also sent by a client immediately after initialisation to mean
/// "not ready to control".
pub const SPECIAL_RELEASED_AWAIT_OPERATOR: u16 = 0xFBFD;
/// Released control, accepting a value increase without operator awareness.
/// Not permitted for every function — see the per-function definition.
pub const SPECIAL_RELEASED_ACCEPT_INCREASE: u16 = 0xFBFE;
/// Ready to control (D.2.1). A server receiving this while automation is
/// already running must treat it as invalid.
pub const SPECIAL_READY_TO_CONTROL: u16 = 0xFBFF;

/// Highest raw that carries a scaled measurement.
const MAX_SCALED_RAW: u16 = 0xFAFF;
/// Midpoint raw, which every TIM SLOT uses for its neutral value.
const NEUTRAL_RAW: u16 = 0x7D80;

/// A TIM function value: either a scaled quantity or one of the special
/// commands the wire reserves at the top of the range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimValue {
    /// A real setpoint in the function's engineering unit.
    Value(f64),
    /// Function fully open / engaged in the negative direction, with the
    /// magnitude chosen by the server (raw `0x0000`).
    ServerSetNegative,
    /// Function fully open / engaged in the positive direction, with the
    /// magnitude chosen by the server (raw `0xFB00`).
    ServerSetPositive,
    /// Hydraulic float (raw `0xFB01`, auxiliary valve only).
    Float,
    /// Released control, awaiting operator action before any increase.
    ReleasedAwaitOperator,
    /// Released control, accepting an increase without operator awareness.
    ReleasedAcceptIncrease,
    /// Ready to control — the request that precedes engagement.
    ReadyToControl,
    /// Hitch motion: stop where it is (raw `0xFB01`, D.5.2.1).
    Stop,
    /// Hitch motion: lower until a Stop request (raw `0xFB02`).
    LowerUntilStop,
    /// Hitch motion: raise until a Stop request (raw `0xFB03`).
    RaiseUntilStop,
}

/// Scale and offset for one TIM function's value SLOT.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimSlot {
    /// Engineering units per bit.
    resolution: f64,
    /// Value at raw zero, in engineering units.
    offset: f64,
    /// Whether `0xFB01` means hydraulic float for this function.
    supports_float: bool,
    /// Which special-value table this function uses. A single `supports_float`
    /// flag cannot describe the hitch, whose whole range differs (D.5.2.1).
    shape: SlotShape,
}

/// The special-value layout a TIM function uses above its scaled band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotShape {
    /// `0x0000` server-set-negative, `0x0001..=0xFAFF` scaled, `0xFB00`
    /// server-set-positive, optional float at `0xFB01`.
    Bidirectional,
    /// D.5.2.1: `0x0000..=0x2710` is 0-100 % of travel, `0x2711..=0xFAFF` is
    /// reserved, and `0xFB00..=0xFB03` are Float / Stop / Lower-until-Stop /
    /// Raise-until-Stop. Decoding this through the bidirectional table dropped
    /// the **Stop** request entirely and turned a reserved raw into a position
    /// command of up to 642 % of travel.
    HitchPosition,
}

impl TimSlot {
    /// Auxiliary valve flow — D.3.2.1: `0.004 % per bit`, `−128.512 %` offset,
    /// with `0xFB01` as float.
    pub const AUX_VALVE_FLOW: Self = Self {
        resolution: 0.004,
        offset: -128.512,
        supports_float: true,
        shape: SlotShape::Bidirectional,
    };

    /// PTO shaft speed — D.4.2.1: `0.125 1/min per bit`, `−4016 1/min` offset.
    /// Negative is counter-clockwise.
    pub const PTO_SPEED: Self = Self {
        resolution: 0.125,
        offset: -4016.0,
        supports_float: false,
        shape: SlotShape::Bidirectional,
    };

    /// Vehicle speed — D.6.2.1: `0.001 m/s per bit`, `−32.128 m/s` offset. The
    /// offset is what makes reverse expressible; a plain unsigned slot cannot
    /// represent it at all.
    pub const VEHICLE_SPEED: Self = Self {
        resolution: 0.001,
        offset: -32.128,
        supports_float: false,
        shape: SlotShape::Bidirectional,
    };

    /// Hitch position — D.5.2.1: `0.01 %` per bit from zero.
    pub const HITCH_POSITION: Self = Self {
        resolution: 0.01,
        offset: 0.0,
        supports_float: true,
        shape: SlotShape::HitchPosition,
    };

    /// Highest raw carrying a hitch position: 0x2710 = 100.00 % of travel.
    const HITCH_MAX_RAW: u16 = 0x2710;

    /// The engineering value this SLOT's neutral raw (`0x7D80`) represents —
    /// zero flow, PTO off, or standstill.
    #[must_use]
    pub fn neutral(self) -> f64 {
        f64::from(NEUTRAL_RAW) * self.resolution + self.offset
    }

    /// Lowest and highest scaled values this SLOT can carry.
    #[must_use]
    pub fn range(self) -> (f64, f64) {
        (
            self.offset + self.resolution,
            f64::from(MAX_SCALED_RAW) * self.resolution + self.offset,
        )
    }

    /// Decode a raw 2-byte SLOT.
    ///
    /// Returns `None` only for the genuinely reserved bands, so a receiver can
    /// tell "a value I do not understand" from "a command I must act on".
    #[must_use]
    pub fn decode(self, raw: u16) -> Option<TimValue> {
        if self.shape == SlotShape::HitchPosition {
            return match raw {
                0..=Self::HITCH_MAX_RAW => Some(TimValue::Value(f64::from(raw) * self.resolution)),
                0xFB00 => Some(TimValue::Float),
                0xFB01 => Some(TimValue::Stop),
                0xFB02 => Some(TimValue::LowerUntilStop),
                0xFB03 => Some(TimValue::RaiseUntilStop),
                SPECIAL_RELEASED_AWAIT_OPERATOR => Some(TimValue::ReleasedAwaitOperator),
                SPECIAL_RELEASED_ACCEPT_INCREASE => Some(TimValue::ReleasedAcceptIncrease),
                SPECIAL_READY_TO_CONTROL => Some(TimValue::ReadyToControl),
                // 0x2711..=0xFAFF and 0xFB04..=0xFBFC are reserved.
                _ => None,
            };
        }
        match raw {
            0x0000 => Some(TimValue::ServerSetNegative),
            0x0001..=MAX_SCALED_RAW => Some(TimValue::Value(
                f64::from(raw) * self.resolution + self.offset,
            )),
            0xFB00 => Some(TimValue::ServerSetPositive),
            0xFB01 if self.supports_float => Some(TimValue::Float),
            SPECIAL_RELEASED_AWAIT_OPERATOR => Some(TimValue::ReleasedAwaitOperator),
            SPECIAL_RELEASED_ACCEPT_INCREASE => Some(TimValue::ReleasedAcceptIncrease),
            SPECIAL_READY_TO_CONTROL => Some(TimValue::ReadyToControl),
            // 0xFB01 without float support, 0xFB02..=0xFBFC, 0xFC00..=0xFFFF.
            _ => None,
        }
    }

    /// Encode a [`TimValue`] to its raw 2-byte form, clamping a scaled value
    /// into the SLOT's range rather than wrapping into the special band.
    #[must_use]
    pub fn encode(self, value: TimValue) -> u16 {
        if self.shape == SlotShape::HitchPosition {
            return match value {
                TimValue::Value(v) if v.is_finite() => {
                    let raw = v / self.resolution;
                    if raw <= 0.0 {
                        0
                    } else if raw >= f64::from(Self::HITCH_MAX_RAW) {
                        Self::HITCH_MAX_RAW
                    } else {
                        (raw + 0.5) as u16
                    }
                }
                TimValue::Value(_) => SPECIAL_RELEASED_AWAIT_OPERATOR,
                TimValue::Float => 0xFB00,
                TimValue::Stop => 0xFB01,
                TimValue::LowerUntilStop => 0xFB02,
                TimValue::RaiseUntilStop => 0xFB03,
                TimValue::ReleasedAwaitOperator | TimValue::ServerSetNegative => {
                    SPECIAL_RELEASED_AWAIT_OPERATOR
                }
                TimValue::ReleasedAcceptIncrease | TimValue::ServerSetPositive => {
                    SPECIAL_RELEASED_ACCEPT_INCREASE
                }
                TimValue::ReadyToControl => SPECIAL_READY_TO_CONTROL,
            };
        }
        match value {
            TimValue::Value(v) => {
                if !v.is_finite() {
                    return SPECIAL_RELEASED_AWAIT_OPERATOR;
                }
                let raw = (v - self.offset) / self.resolution;
                if raw <= 1.0 {
                    1
                } else if raw >= f64::from(MAX_SCALED_RAW) {
                    MAX_SCALED_RAW
                } else {
                    // Round rather than truncate so a value round-trips.
                    (raw + 0.5) as u16
                }
            }
            TimValue::ServerSetNegative => 0x0000,
            TimValue::ServerSetPositive => 0xFB00,
            TimValue::Float => 0xFB01,
            TimValue::ReleasedAwaitOperator => SPECIAL_RELEASED_AWAIT_OPERATOR,
            TimValue::ReleasedAcceptIncrease => SPECIAL_RELEASED_ACCEPT_INCREASE,
            TimValue::ReadyToControl => SPECIAL_READY_TO_CONTROL,
            // Hitch-motion commands have no meaning on a bidirectional slot.
            TimValue::Stop | TimValue::LowerUntilStop | TimValue::RaiseUntilStop => {
                SPECIAL_RELEASED_AWAIT_OPERATOR
            }
        }
    }

    /// The raw for this SLOT's neutral position (block / off / standstill).
    #[must_use]
    pub const fn neutral_raw(self) -> u16 {
        NEUTRAL_RAW
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_raw_is_zero_in_every_offset_slot() {
        // 0x7D80 is the neutral point of each SLOT: 0 % flow (block), PTO off,
        // and standstill. A plain unscaled integer cannot express this.
        assert!(TimSlot::AUX_VALVE_FLOW.neutral().abs() < 1e-9);
        assert!(TimSlot::PTO_SPEED.neutral().abs() < 1e-9);
        assert!(TimSlot::VEHICLE_SPEED.neutral().abs() < 1e-9);
    }

    #[test]
    fn vehicle_speed_can_express_reverse() {
        let slot = TimSlot::VEHICLE_SPEED;
        let (min, max) = slot.range();
        assert!(
            min < 0.0,
            "the -32.128 m/s offset is what makes reverse representable"
        );
        assert!((min + 32.127).abs() < 1e-6, "min is -32.127 m/s");
        assert!((max - 32.127).abs() < 1e-6, "max is +32.127 m/s");

        // A reverse setpoint survives a round trip.
        let raw = slot.encode(TimValue::Value(-1.5));
        match slot.decode(raw) {
            Some(TimValue::Value(v)) => assert!((v + 1.5).abs() < 1e-3),
            other => panic!("expected a scaled reverse speed, got {other:?}"),
        }
    }

    #[test]
    fn pto_speed_uses_the_annex_d4_scale() {
        let slot = TimSlot::PTO_SPEED;
        let (min, max) = slot.range();
        assert!((min + 4015.875).abs() < 1e-6);
        assert!((max - 4015.875).abs() < 1e-6);

        // 540 rpm clockwise, the classic PTO speed.
        let raw = slot.encode(TimValue::Value(540.0));
        match slot.decode(raw) {
            Some(TimValue::Value(v)) => assert!((v - 540.0).abs() < 0.125),
            other => panic!("expected 540 rpm, got {other:?}"),
        }
        // Counter-clockwise is a negative value, not a separate field.
        match slot.decode(slot.encode(TimValue::Value(-540.0))) {
            Some(TimValue::Value(v)) => assert!((v + 540.0).abs() < 0.125),
            other => panic!("expected -540 rpm, got {other:?}"),
        }
    }

    #[test]
    fn aux_valve_flow_uses_the_annex_d3_scale_and_supports_float() {
        let slot = TimSlot::AUX_VALVE_FLOW;
        let (min, max) = slot.range();
        assert!((min + 128.508).abs() < 1e-6);
        assert!((max - 128.508).abs() < 1e-6);

        assert_eq!(slot.decode(0x0000), Some(TimValue::ServerSetNegative));
        assert_eq!(slot.decode(0xFB00), Some(TimValue::ServerSetPositive));
        assert_eq!(slot.decode(0xFB01), Some(TimValue::Float));

        // Float is valve-only: the PTO slot must not silently accept it.
        assert_eq!(TimSlot::PTO_SPEED.decode(0xFB01), None);
    }

    #[test]
    fn the_common_special_values_decode_on_every_slot() {
        for slot in [
            TimSlot::AUX_VALVE_FLOW,
            TimSlot::PTO_SPEED,
            TimSlot::VEHICLE_SPEED,
            TimSlot::HITCH_POSITION,
        ] {
            assert_eq!(
                slot.decode(SPECIAL_READY_TO_CONTROL),
                Some(TimValue::ReadyToControl)
            );
            assert_eq!(
                slot.decode(SPECIAL_RELEASED_AWAIT_OPERATOR),
                Some(TimValue::ReleasedAwaitOperator)
            );
            assert_eq!(
                slot.decode(SPECIAL_RELEASED_ACCEPT_INCREASE),
                Some(TimValue::ReleasedAcceptIncrease)
            );
            // Reserved bands stay reserved.
            assert_eq!(slot.decode(0xFBF0), None);
            assert_eq!(slot.decode(0xFC00), None);
            assert_eq!(slot.decode(0xFFFF), None);
        }
    }

    /// C29 — the hitch SLOT is not a bidirectional one. D.5.2.1 puts 0-100 %
    /// of travel in 0x0000..=0x2710, reserves 0x2711..=0xFAFF, and defines
    /// 0xFB00..=0xFB03 as Float / Stop / Lower-until-Stop / Raise-until-Stop.
    /// Read through the generic table, a client's **Stop** request decoded to
    /// nothing at all — a failure to stop a moving three-point hitch — and a
    /// reserved raw became a position command of up to 642 % of travel.
    #[test]
    fn hitch_position_follows_its_own_annex_d5_table() {
        let slot = TimSlot::HITCH_POSITION;

        // The scaled band is 0-100 %, starting at raw zero.
        assert_eq!(slot.decode(0x0000), Some(TimValue::Value(0.0)));
        match slot.decode(TimSlot::HITCH_MAX_RAW) {
            Some(TimValue::Value(v)) => assert!((v - 100.0).abs() < 1e-9),
            other => panic!("0x2710 is full raise, got {other:?}"),
        }

        // The motion commands, which the generic table could not express.
        assert_eq!(slot.decode(0xFB00), Some(TimValue::Float));
        assert_eq!(
            slot.decode(0xFB01),
            Some(TimValue::Stop),
            "a hitch Stop request must never be silently dropped"
        );
        assert_eq!(slot.decode(0xFB02), Some(TimValue::LowerUntilStop));
        assert_eq!(slot.decode(0xFB03), Some(TimValue::RaiseUntilStop));

        // Reserved bands stay reserved rather than decoding as travel.
        assert_eq!(slot.decode(0x2711), None);
        assert_eq!(slot.decode(0xFAFF), None);
        assert_eq!(slot.decode(0xFB04), None);
        assert_eq!(slot.decode(0xFBFC), None);

        // Round trips, including the commands.
        for value in [
            TimValue::Stop,
            TimValue::Float,
            TimValue::LowerUntilStop,
            TimValue::RaiseUntilStop,
            TimValue::ReadyToControl,
        ] {
            assert_eq!(slot.decode(slot.encode(value)), Some(value), "{value:?}");
        }

        // A position outside 0-100 % clamps inside the scaled band rather than
        // wrapping into the motion commands.
        assert_eq!(slot.encode(TimValue::Value(250.0)), TimSlot::HITCH_MAX_RAW);
        assert_eq!(slot.encode(TimValue::Value(-50.0)), 0);
        assert!(matches!(
            slot.decode(slot.encode(TimValue::Value(1.0e9))),
            Some(TimValue::Value(_))
        ));
    }

    #[test]
    fn out_of_range_clamps_instead_of_wrapping_into_the_special_band() {
        let slot = TimSlot::VEHICLE_SPEED;
        // A wildly out-of-range setpoint must not become "ready to control".
        let raw = slot.encode(TimValue::Value(1.0e6));
        assert_eq!(raw, MAX_SCALED_RAW);
        assert!(matches!(slot.decode(raw), Some(TimValue::Value(_))));

        let low = slot.encode(TimValue::Value(-1.0e6));
        assert_eq!(low, 1, "clamps above the server-set-negative sentinel");

        // NaN is a released request, never a number.
        assert_eq!(
            slot.encode(TimValue::Value(f64::NAN)),
            SPECIAL_RELEASED_AWAIT_OPERATOR
        );
    }
}
