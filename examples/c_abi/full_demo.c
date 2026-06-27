/*
 * machbus C-ABI full-surface demo.
 *
 * Drives a single sans-IO node with every optional subsystem enabled
 * (diagnostics, GNSS, implement, VT client, TC client) and exercises the
 * representative call of each family: CAN-config validation, address claim,
 * tick/poll_transmit/feed loop, send_raw, diagnostics, GNSS broadcasts,
 * implement commands, and VT/TC connect calls. There is no bus partner, so
 * the connect calls just move local client state; the demo prints rather than
 * asserts a peer.
 *
 * Build:
 *   make -C examples/c_abi run-full
 */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "../../include/machbus.h"

static const char *event_kind_str(MachbusEventKind k) {
    switch (k) {
    case MACHBUS_EVENT_KIND_ADDRESS_CLAIM_CLAIMED: return "AddressClaim::Claimed";
    case MACHBUS_EVENT_KIND_DIAG_RAISED:           return "Diag::Raised";
    case MACHBUS_EVENT_KIND_DIAG_CLEARED:          return "Diag::Cleared";
    case MACHBUS_EVENT_KIND_DIAG_DM1_RECEIVED:     return "Diag::Dm1Received";
    case MACHBUS_EVENT_KIND_GNSS_POSITION:         return "Gnss::Position";
    case MACHBUS_EVENT_KIND_GNSS_COG:              return "Gnss::Cog";
    case MACHBUS_EVENT_KIND_GNSS_SOG:              return "Gnss::Sog";
    case MACHBUS_EVENT_KIND_IMP_HITCH_COMMAND:     return "Imp::HitchCommand";
    case MACHBUS_EVENT_KIND_IMP_PTO_COMMAND:       return "Imp::PtoCommand";
    case MACHBUS_EVENT_KIND_IMP_AUX_VALVE_COMMAND: return "Imp::AuxValveCommand";
    case MACHBUS_EVENT_KIND_VT_STATE_CHANGED:      return "Vt::StateChanged";
    case MACHBUS_EVENT_KIND_TC_STATE_CHANGED:      return "Tc::StateChanged";
    case MACHBUS_EVENT_KIND_CUSTOM:                return "Custom";
    default:                                       return "Other";
    }
}

static size_t drain_transmit(MachbusSession *h) {
    uint8_t port;
    uint32_t raw_id;
    uint8_t data[8];
    uintptr_t len;
    size_t n = 0;
    while (machbus_session_poll_transmit(h, &port, &raw_id, data, &len)) {
        n += 1;
    }
    return n;
}

int main(void) {
    if (machbus_session_abi_version() != 3) {
        fprintf(stderr, "unexpected machbus C ABI version: %u\n",
                machbus_session_abi_version());
        return 1;
    }
    printf("machbus full-demo (C ABI v%u)\n\n", machbus_session_abi_version());

    /* ─── CAN bus configuration validation ──────────────────────────── */
    MachbusCanBusValidation v = machbus_session_validate_can_bus_config(
        250000,   /* bitrate */
        0.875,    /* sample point */
        1,        /* sjw */
        2,        /* prop_seg */
        7,        /* phase_seg1 */
        2,        /* phase_seg2 */
        false,    /* silent_mode */
        false);   /* loopback */
    printf("can-config: bitrate=%d sample=%d timing=%d phys=%d overall=%d\n",
           v.bitrate_ok, v.sample_point_ok, v.bit_timing_ok,
           v.physical_mode_ok, v.overall_ok);
    printf("enforce_iso_can_config = %d\n\n",
           (int)machbus_session_enforce_iso_can_config(
               250000, 0.875, 1, 2, 7, 2, false, false));

    /* ─── Node with every optional subsystem enabled ────────────────── */
    MachbusConfig cfg = machbus_session_default_config();
    cfg.name_raw                = 0xA000820000FFFFFFULL;
    cfg.preferred_address       = 0x80;
    cfg.enable_diagnostics      = true;
    cfg.diagnostics_interval_ms = 100;
    cfg.enable_gnss             = true;
    cfg.enable_implement        = true;
    cfg.enable_vt_client        = true;
    cfg.enable_tc_client        = true;

    MachbusSession *h = machbus_session_new(&cfg);
    if (!h) {
        fprintf(stderr, "machbus_session_new: %s\n", machbus_session_last_error());
        return 1;
    }

    /* Drive the claim (uncontested). */
    if (!machbus_session_start_address_claim(h)) {
        fprintf(stderr, "claim: %s\n", machbus_session_last_error());
        machbus_session_free(h);
        return 1;
    }
    for (int i = 0; i < 100 && !machbus_session_is_claimed(h); ++i) {
        machbus_session_tick(h, 50);
        drain_transmit(h);
    }
    if (!machbus_session_is_claimed(h)) {
        fprintf(stderr, "node did not claim: %s\n", machbus_session_last_error());
        machbus_session_free(h);
        return 1;
    }
    printf("address = 0x%02X (claim_state=%d)\n",
           machbus_session_address(h), (int)machbus_session_claim_state(h));

    /* Drain claim events. */
    MachbusEvent ev;
    while (machbus_session_poll_event(h, &ev)) {}

    /* ─── Diagnostics ───────────────────────────────────────────────── */
    machbus_session_diag_raise(h, 100,    1, 1);
    machbus_session_diag_raise(h, 110,    0, 1);
    machbus_session_diag_raise(h, 523312, 0, 2);
    printf("\ndiag.active_count = %zu\n", machbus_session_diag_active_count(h));

    /* ─── GNSS broadcasts ───────────────────────────────────────────── */
    MachbusGnssPosition pos = {
        .latitude    = 52.5200,
        .longitude   = 13.4050,
        .altitude_m  = 34.0,
        .speed_mps   = 3.5,
        .heading_rad = NAN,
    };
    if (!machbus_session_gnss_broadcast_position(h, &pos)) {
        fprintf(stderr, "gnss_broadcast_position: %s\n", machbus_session_last_error());
    }
    if (!machbus_session_gnss_broadcast_cog_sog(h, 0.75, 3.5)) {
        fprintf(stderr, "gnss_broadcast_cog_sog: %s\n", machbus_session_last_error());
    }

    /* ─── Implement commands ────────────────────────────────────────── */
    machbus_session_implement_command_hitch(h, MACHBUS_HITCH_REAR, MACHBUS_HITCH_COMMAND_RAISE);
    machbus_session_implement_command_hitch_position(h, MACHBUS_HITCH_FRONT, 500, 0x10);
    machbus_session_implement_command_pto(h, MACHBUS_PTO_REAR, MACHBUS_PTO_COMMAND_ENGAGE);
    machbus_session_implement_command_pto_speed(h, MACHBUS_PTO_REAR, 4320, 10);
    machbus_session_implement_command_aux_valve(h, 0, MACHBUS_VALVE_COMMAND_EXTEND, 250);

    /* ─── VT / TC client connect (no server present) ────────────────── */
    machbus_session_vt_connect(h, 0x26);     /* conventional VT address */
    machbus_session_vt_show(h, 1000);
    machbus_session_vt_set_value(h, 2000, 42);
    machbus_session_vt_set_string(h, 2001, "hello");
    printf("vt: connected=%d state=%u\n",
           (int)machbus_session_vt_is_connected(h),
           machbus_session_vt_state(h));

    machbus_session_tc_connect(h);
    printf("tc: connected=%d state=%u\n",
           (int)machbus_session_tc_is_connected(h),
           machbus_session_tc_state(h));

    /* ─── Raw escape hatch: queue a proprietary message ─────────────── */
    const uint8_t raw[] = {0xDE, 0xAD, 0xBE, 0xEF};
    if (!machbus_session_send_raw(h, 0xEF00, raw, sizeof(raw), 0xFF, 6)) {
        fprintf(stderr, "send_raw: %s\n", machbus_session_last_error());
    }

    /* ─── Feed a synthetic inbound frame (request PGN, here ignored) ── */
    const uint8_t req[] = {0x00, 0xEE, 0x00};
    machbus_session_feed(h, 0, 0x18EAFFFEu, req, sizeof(req));

    /* Pump ~1 s of bus time so cadences fire; count outbound frames. */
    size_t tx = 0;
    for (int i = 0; i < 20; ++i) {
        machbus_session_tick(h, 50);
        tx += drain_transmit(h);
    }
    printf("\noutbound frames produced while pumping: %zu\n", tx);

    /* ─── Drain and histogram the unified event queue ───────────────── */
    int total = 0;
    printf("\nevents:\n");
    while (machbus_session_poll_event(h, &ev)) {
        printf("  -> %-24s src=0x%02X spn/pgn=%u sub=%u u0=%u\n",
               event_kind_str(ev.kind), ev.source, ev.spn_or_pgn,
               ev.fmi_or_sub, ev.u0);
        total += 1;
    }
    printf("total events drained: %d\n", total);

    machbus_session_vt_disconnect(h);
    machbus_session_tc_disconnect(h);
    machbus_session_diag_clear(h);
    machbus_session_free(h);
    return 0;
}
