/*
 * Compile-time ABI layout checks for the generated public C header.
 *
 * The Rust side has matching `repr(C)` layout expectations. Keeping both sides
 * explicit catches accidental field reorders, enum representation changes, and
 * cbindgen drift before downstream C users see an ABI break.
 */

#include <stddef.h>
#include <stdint.h>

#include "../../include/machbus.h"

#if UINTPTR_MAX == UINT64_MAX

/* ─── Enums: cbindgen emits each as a plain C enum (4 bytes). ────────── */
_Static_assert(sizeof(MachbusClaimState) == 4, "MachbusClaimState size changed");
_Static_assert(sizeof(MachbusEventKind) == 4, "MachbusEventKind size changed");
_Static_assert(sizeof(MachbusHitch) == 4, "MachbusHitch size changed");
_Static_assert(sizeof(MachbusPto) == 4, "MachbusPto size changed");
_Static_assert(sizeof(MachbusHitchCommand) == 4, "MachbusHitchCommand size changed");
_Static_assert(sizeof(MachbusPtoCommand) == 4, "MachbusPtoCommand size changed");
_Static_assert(sizeof(MachbusValveCommand) == 4, "MachbusValveCommand size changed");

/* ─── MachbusConfig ─────────────────────────────────────────────────── */
_Static_assert(sizeof(MachbusConfig) == 48, "MachbusConfig size changed");
_Static_assert(_Alignof(MachbusConfig) == 8, "MachbusConfig alignment changed");
_Static_assert(offsetof(MachbusConfig, name_raw) == 0, "MachbusConfig.name_raw offset changed");
_Static_assert(offsetof(MachbusConfig, preferred_address) == 8, "MachbusConfig.preferred_address offset changed");
_Static_assert(offsetof(MachbusConfig, enable_diagnostics) == 9, "MachbusConfig.enable_diagnostics offset changed");
_Static_assert(offsetof(MachbusConfig, diagnostics_interval_ms) == 12, "MachbusConfig.diagnostics_interval_ms offset changed");
_Static_assert(offsetof(MachbusConfig, enable_gnss) == 16, "MachbusConfig.enable_gnss offset changed");
_Static_assert(offsetof(MachbusConfig, enable_implement) == 17, "MachbusConfig.enable_implement offset changed");
_Static_assert(offsetof(MachbusConfig, enable_vt_client) == 18, "MachbusConfig.enable_vt_client offset changed");
_Static_assert(offsetof(MachbusConfig, enable_tc_client) == 19, "MachbusConfig.enable_tc_client offset changed");

/* ─── MachbusCanBusValidation ───────────────────────────────────────── */
_Static_assert(sizeof(MachbusCanBusValidation) == 5, "MachbusCanBusValidation size changed");
_Static_assert(_Alignof(MachbusCanBusValidation) == 1, "MachbusCanBusValidation alignment changed");
_Static_assert(offsetof(MachbusCanBusValidation, bitrate_ok) == 0, "MachbusCanBusValidation.bitrate_ok offset changed");
_Static_assert(offsetof(MachbusCanBusValidation, sample_point_ok) == 1, "MachbusCanBusValidation.sample_point_ok offset changed");
_Static_assert(offsetof(MachbusCanBusValidation, bit_timing_ok) == 2, "MachbusCanBusValidation.bit_timing_ok offset changed");
_Static_assert(offsetof(MachbusCanBusValidation, physical_mode_ok) == 3, "MachbusCanBusValidation.physical_mode_ok offset changed");
_Static_assert(offsetof(MachbusCanBusValidation, overall_ok) == 4, "MachbusCanBusValidation.overall_ok offset changed");

/* ─── MachbusEvent ──────────────────────────────────────────────────── */
_Static_assert(sizeof(MachbusEvent) == 40, "MachbusEvent size changed");
_Static_assert(_Alignof(MachbusEvent) == 8, "MachbusEvent alignment changed");
_Static_assert(offsetof(MachbusEvent, kind) == 0, "MachbusEvent.kind offset changed");
_Static_assert(offsetof(MachbusEvent, source) == 4, "MachbusEvent.source offset changed");
_Static_assert(offsetof(MachbusEvent, spn_or_pgn) == 8, "MachbusEvent.spn_or_pgn offset changed");
_Static_assert(offsetof(MachbusEvent, fmi_or_sub) == 12, "MachbusEvent.fmi_or_sub offset changed");
_Static_assert(offsetof(MachbusEvent, d0) == 16, "MachbusEvent.d0 offset changed");
_Static_assert(offsetof(MachbusEvent, d1) == 24, "MachbusEvent.d1 offset changed");
_Static_assert(offsetof(MachbusEvent, u0) == 32, "MachbusEvent.u0 offset changed");

/* ─── MachbusGnssPosition ───────────────────────────────────────────── */
_Static_assert(sizeof(MachbusGnssPosition) == 40, "MachbusGnssPosition size changed");
_Static_assert(_Alignof(MachbusGnssPosition) == 8, "MachbusGnssPosition alignment changed");
_Static_assert(offsetof(MachbusGnssPosition, latitude) == 0, "MachbusGnssPosition.latitude offset changed");
_Static_assert(offsetof(MachbusGnssPosition, longitude) == 8, "MachbusGnssPosition.longitude offset changed");
_Static_assert(offsetof(MachbusGnssPosition, altitude_m) == 16, "MachbusGnssPosition.altitude_m offset changed");
_Static_assert(offsetof(MachbusGnssPosition, speed_mps) == 24, "MachbusGnssPosition.speed_mps offset changed");
_Static_assert(offsetof(MachbusGnssPosition, heading_rad) == 32, "MachbusGnssPosition.heading_rad offset changed");

#endif
