#include "machbus.h"

/*
 * Compile-only coverage for the generated C ABI surface.
 *
 * The warning-clean demos exercise representative runtime workflows, while
 * this translation unit keeps every exported `machbus_session_*` function name
 * visible to the C compiler. It catches stale headers, accidental symbol
 * renames, missing prototypes, and functions that were exported but never
 * referenced from the C smoke build.
 *
 * Every public `#[repr(C)]` type from the header is also instantiated below so
 * that a struct/enum rename or removal breaks this file at compile time.
 */

#define ABI_FN(name) (void (*)(void))name

static void (*const MACHBUS_C_ABI_SYMBOLS[])(void) = {
    /* lifecycle / config */
    ABI_FN(machbus_session_last_error),
    ABI_FN(machbus_session_abi_version),
    ABI_FN(machbus_session_default_config),
    ABI_FN(machbus_session_new),
    ABI_FN(machbus_session_free),

    /* CAN validation */
    ABI_FN(machbus_session_validate_can_bus_config),
    ABI_FN(machbus_session_enforce_iso_can_config),

    /* drive / IO */
    ABI_FN(machbus_session_start_address_claim),
    ABI_FN(machbus_session_tick),
    ABI_FN(machbus_session_feed),
    ABI_FN(machbus_session_poll_transmit),
    ABI_FN(machbus_session_send_raw),
    ABI_FN(machbus_session_poll_event),

    /* introspection */
    ABI_FN(machbus_session_address),
    ABI_FN(machbus_session_claim_state),
    ABI_FN(machbus_session_is_claimed),

    /* diagnostics */
    ABI_FN(machbus_session_diag_raise),
    ABI_FN(machbus_session_diag_clear),
    ABI_FN(machbus_session_diag_active_count),

    /* gnss */
    ABI_FN(machbus_session_gnss_broadcast_position),
    ABI_FN(machbus_session_gnss_broadcast_cog_sog),

    /* implement */
    ABI_FN(machbus_session_implement_command_hitch),
    ABI_FN(machbus_session_implement_command_hitch_position),
    ABI_FN(machbus_session_implement_command_pto),
    ABI_FN(machbus_session_implement_command_pto_speed),
    ABI_FN(machbus_session_implement_command_aux_valve),

    /* VT client */
    ABI_FN(machbus_session_vt_connect),
    ABI_FN(machbus_session_vt_disconnect),
    ABI_FN(machbus_session_vt_state),
    ABI_FN(machbus_session_vt_is_connected),
    ABI_FN(machbus_session_vt_show),
    ABI_FN(machbus_session_vt_hide),
    ABI_FN(machbus_session_vt_set_value),
    ABI_FN(machbus_session_vt_set_string),

    /* TC client */
    ABI_FN(machbus_session_tc_connect),
    ABI_FN(machbus_session_tc_disconnect),
    ABI_FN(machbus_session_tc_state),
    ABI_FN(machbus_session_tc_is_connected),
};

/*
 * Touch every public #[repr(C)] type so the surface check fails if any are
 * renamed or dropped. The opaque MachbusSession is referenced via a pointer.
 */
static MachbusSession *const MACHBUS_C_ABI_OPAQUE = NULL;

static const MachbusCanBusValidation MACHBUS_C_ABI_VALIDATION;
static const MachbusConfig           MACHBUS_C_ABI_CONFIG;
static const MachbusEvent            MACHBUS_C_ABI_EVENT;
static const MachbusGnssPosition     MACHBUS_C_ABI_GNSS;

static const MachbusClaimState   MACHBUS_C_ABI_CLAIM_STATE   = MACHBUS_CLAIM_STATE_NONE;
static const MachbusEventKind    MACHBUS_C_ABI_EVENT_KIND    = MACHBUS_EVENT_KIND_NONE;
static const MachbusHitch        MACHBUS_C_ABI_HITCH         = MACHBUS_HITCH_FRONT;
static const MachbusHitchCommand MACHBUS_C_ABI_HITCH_CMD     = MACHBUS_HITCH_COMMAND_NO_ACTION;
static const MachbusPto          MACHBUS_C_ABI_PTO           = MACHBUS_PTO_FRONT;
static const MachbusPtoCommand   MACHBUS_C_ABI_PTO_CMD       = MACHBUS_PTO_COMMAND_NO_ACTION;
static const MachbusValveCommand MACHBUS_C_ABI_VALVE_CMD     = MACHBUS_VALVE_COMMAND_NO_ACTION;

const void *machbus_c_abi_compile_surface_anchor(void);
const void *machbus_c_abi_compile_surface_anchor(void) {
    (void)MACHBUS_C_ABI_OPAQUE;
    (void)MACHBUS_C_ABI_VALIDATION;
    (void)MACHBUS_C_ABI_CONFIG;
    (void)MACHBUS_C_ABI_EVENT;
    (void)MACHBUS_C_ABI_GNSS;
    (void)MACHBUS_C_ABI_CLAIM_STATE;
    (void)MACHBUS_C_ABI_EVENT_KIND;
    (void)MACHBUS_C_ABI_HITCH;
    (void)MACHBUS_C_ABI_HITCH_CMD;
    (void)MACHBUS_C_ABI_PTO;
    (void)MACHBUS_C_ABI_PTO_CMD;
    (void)MACHBUS_C_ABI_VALVE_CMD;
    return MACHBUS_C_ABI_SYMBOLS;
}
