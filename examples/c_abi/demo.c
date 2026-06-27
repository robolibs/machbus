/*
 * machbus C-ABI smoke demo.
 *
 * Builds a single sans-IO ISO 11783 node with the diagnostics subsystem
 * enabled, starts address claiming, drives the virtual clock until the node
 * is claimed (with no bus partner it claims uncontested), raises a couple of
 * DTCs, runs a bit so the DM1 cadence fires, then drains and prints the
 * unified event queue.
 *
 * Build:
 *   make -C examples/c_abi run
 *
 * Or by hand from the crate root:
 *   cargo build
 *   cc -Iinclude examples/c_abi/demo.c \
 *      -Ltarget/debug -lmachbus -Wl,-rpath,$(pwd)/target/debug \
 *      -lm -lpthread -ldl -o target/debug/c_api_demo
 *   ./target/debug/c_api_demo
 */

#include <stdio.h>
#include <stdlib.h>

#include "../../include/machbus.h"

static const char *event_kind_str(MachbusEventKind k) {
    switch (k) {
    case MACHBUS_EVENT_KIND_NONE:                       return "None";
    case MACHBUS_EVENT_KIND_ADDRESS_CLAIM_CLAIMED:      return "AddressClaim::Claimed";
    case MACHBUS_EVENT_KIND_ADDRESS_CLAIM_LOST:         return "AddressClaim::Lost";
    case MACHBUS_EVENT_KIND_ADDRESS_CLAIM_DISCONNECTED: return "AddressClaim::Disconnected";
    case MACHBUS_EVENT_KIND_BUS_ERROR:                  return "Bus::Error";
    case MACHBUS_EVENT_KIND_BUS_DROPPED_FRAME:          return "Bus::DroppedFrame";
    case MACHBUS_EVENT_KIND_DIAG_RAISED:                return "Diag::Raised";
    case MACHBUS_EVENT_KIND_DIAG_CLEARED:               return "Diag::Cleared";
    case MACHBUS_EVENT_KIND_DIAG_DM1_RECEIVED:          return "Diag::Dm1Received";
    default:                                            return "Other";
    }
}

int main(void) {
    if (machbus_session_abi_version() != 3) {
        fprintf(stderr, "unexpected machbus C ABI version: %u\n",
                machbus_session_abi_version());
        return 1;
    }
    printf("machbus C ABI v%u\n", machbus_session_abi_version());

    /* NAME is a raw 64-bit ISO 11783-5 value; any non-zero value works for a
     * standalone node. Use a recognisable arbitrary identity. */
    MachbusConfig cfg = machbus_session_default_config();
    cfg.name_raw                 = 0xA000820000FFFFFFULL;
    cfg.preferred_address        = 0x80;
    cfg.enable_diagnostics       = true;
    cfg.diagnostics_interval_ms  = 100; /* fast cadence for the demo */

    MachbusSession *h = machbus_session_new(&cfg);
    if (!h) {
        fprintf(stderr, "machbus_session_new failed: %s\n",
                machbus_session_last_error());
        return 1;
    }

    if (!machbus_session_start_address_claim(h)) {
        fprintf(stderr, "start_address_claim failed: %s\n",
                machbus_session_last_error());
        machbus_session_free(h);
        return 1;
    }

    /* Drive ticks until claimed (uncontested, so it converges quickly).
     * Drain the transmit queue each tick so it never backs up. */
    uint8_t port;
    uint32_t raw_id;
    uint8_t data[8];
    uintptr_t len;
    for (int i = 0; i < 100 && !machbus_session_is_claimed(h); ++i) {
        machbus_session_tick(h, 50);
        while (machbus_session_poll_transmit(h, &port, &raw_id, data, &len)) {
            /* discard outbound frames: no real bus in this demo */
        }
    }

    if (!machbus_session_is_claimed(h)) {
        fprintf(stderr, "node did not claim (state=%d): %s\n",
                (int)machbus_session_claim_state(h),
                machbus_session_last_error());
        machbus_session_free(h);
        return 1;
    }
    printf("address     = 0x%02X\n", machbus_session_address(h));
    printf("claim_state = %d (claimed)\n", (int)machbus_session_claim_state(h));

    /* Drain claim events so the rest of the demo is clean. */
    MachbusEvent ev;
    while (machbus_session_poll_event(h, &ev)) { /* ignore */ }

    /* Raise a couple of DTCs. */
    machbus_session_diag_raise(h, 100,    1, 1); /* SPN 100, FMI BelowNormal */
    machbus_session_diag_raise(h, 523312, 0, 1); /* SPN 523312, FMI AboveNormal */
    printf("\ndiag.active_count = %zu\n", machbus_session_diag_active_count(h));

    /* Pump for ~1 simulated second so the DM1 cadence fires. */
    for (int i = 0; i < 20; ++i) {
        machbus_session_tick(h, 50);
        while (machbus_session_poll_transmit(h, &port, &raw_id, data, &len)) {
            /* discard */
        }
    }

    printf("\nevents after pumping the bus:\n");
    int total = 0;
    while (machbus_session_poll_event(h, &ev)) {
        switch (ev.kind) {
        case MACHBUS_EVENT_KIND_DIAG_RAISED:
        case MACHBUS_EVENT_KIND_DIAG_CLEARED:
            printf("  -> %s SPN=%u FMI=%u\n",
                   event_kind_str(ev.kind), ev.spn_or_pgn, ev.fmi_or_sub);
            break;
        case MACHBUS_EVENT_KIND_DIAG_DM1_RECEIVED:
            printf("  -> %s from 0x%02X (u0=%u)\n",
                   event_kind_str(ev.kind), ev.source, ev.u0);
            break;
        default:
            printf("  -> %s\n", event_kind_str(ev.kind));
            break;
        }
        total += 1;
    }
    printf("\ntotal events drained: %d\n", total);

    machbus_session_diag_clear(h);
    printf("after clear, diag.active_count = %zu\n",
           machbus_session_diag_active_count(h));

    machbus_session_free(h);
    return 0;
}
