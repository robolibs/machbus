# C ABI

The C ABI is built on the [session facade](../guide/session-facade.md). Every
symbol is prefixed `machbus_session_`, and a node is one opaque
`MachbusSession*` handle wrapping the sans-IO `Session` core. The header
`include/machbus.h` is generated from `src/ffi.rs` with cbindgen.

This is **ABI version 3**. Probe it at runtime with
`machbus_session_abi_version()` and fail fast if it does not match what you
compiled against.

## The model: sans-IO

The core does no IO. You bridge it explicitly:

1. feed received CAN frames with `machbus_session_feed`,
2. advance the virtual clock with `machbus_session_tick`,
3. drain outbound frames with `machbus_session_poll_transmit` and write them to
   your bus,
4. drain application events with `machbus_session_poll_event`.

This replaces the old internal virtual-bus topology: there is no bundled bus and
no implicit transmit. The caller owns IO.

## Conventions

- **Handles** are `Box`-backed and opaque. Create with `machbus_session_new`,
  release exactly once with `machbus_session_free`. `machbus_session_free(NULL)`
  is a no-op; double-free is outside the contract. Set your pointer to `NULL`
  after freeing.
- **Errors**: fallible calls return `bool` (or a sentinel). On failure the
  reason is in the thread-local `machbus_session_last_error()`, valid until the
  next ABI call on the same thread. A `false` from `poll_transmit`/`poll_event`
  means "queue drained", not an error, and does not set the error string.
- **POD types** are `#[repr(C)]` structs and enums (`MachbusConfig`,
  `MachbusEvent`, `MachbusGnssPosition`, `MachbusClaimState`,
  `MachbusEventKind`, the command enums). Borrowed byte/string views stay owned
  by the handle they came from.

## Surface

### Lifecycle and config

| Function | Purpose |
|---|---|
| `machbus_session_default_config()` | Returns an `MachbusConfig` with defaults to override. |
| `machbus_session_new(cfg)` | Build a node (`NULL` cfg = defaults). Returns `NULL` on failure. |
| `machbus_session_free(h)` | Release a node. Accepts `NULL`. |
| `machbus_session_abi_version()` | Current ABI version (`3`). |
| `machbus_session_last_error()` | Thread-local last error string, or `NULL`. |

`MachbusConfig` carries the raw 64-bit ISO 11783-5 NAME, the preferred address,
and `enable_*` flags that plug subsystems: `enable_diagnostics`
(`diagnostics_interval_ms`, 0 = 1000 ms default), `enable_gnss`,
`enable_implement`, `enable_vt_client`, `enable_tc_client`.

### CAN-config validation

| Function | Purpose |
|---|---|
| `machbus_session_validate_can_bus_config(...)` | Returns an `MachbusCanBusValidation` of per-field and overall checks. |
| `machbus_session_enforce_iso_can_config(...)` | Returns `false` and sets the error if the config is not ISO-conformant. |

### Drive and IO

| Function | Purpose |
|---|---|
| `machbus_session_start_address_claim(h)` | Begin address claiming. |
| `machbus_session_tick(h, dt_ms)` | Advance the clock by `dt_ms` and run timers/cadences. |
| `machbus_session_feed(h, port, raw_id, data, len)` | Feed one received frame (29-bit id, up to 8 bytes) on `port`. |
| `machbus_session_poll_transmit(h, out_port, out_raw_id, out_data, out_len)` | Drain one outbound frame. `out_data` needs room for 8 bytes. `false` = drained. Call until it returns `false`. |
| `machbus_session_send_raw(h, pgn, data, len, dst, priority)` | Queue an application message from the local control function (`priority` 0 = highest, 6 = default). |
| `machbus_session_poll_event(h, out)` | Drain one event into an `MachbusEvent`. `false` = drained. |

### Introspection

| Function | Purpose |
|---|---|
| `machbus_session_address(h)` | Current source address (`NULL_ADDRESS` if unclaimed). |
| `machbus_session_claim_state(h)` | `MachbusClaimState` enum. |
| `machbus_session_is_claimed(h)` | Whether the address is claimed. |

### Diagnostics — ISO 11783-12 (Diagnostics)

Require the diagnostics subsystem.

| Function | Purpose |
|---|---|
| `machbus_session_diag_raise(h, spn, fmi, occurrence_count)` | Raise a DTC (broadcast on the next DM1 cadence). |
| `machbus_session_diag_clear(h)` | Clear all active DTCs. |
| `machbus_session_diag_active_count(h)` | Count of active local DTCs (0 if not plugged). |

### GNSS — NMEA 2000

Require the GNSS subsystem.

| Function | Purpose |
|---|---|
| `machbus_session_gnss_broadcast_position(h, pos)` | Broadcast an `MachbusGnssPosition`. |
| `machbus_session_gnss_broadcast_cog_sog(h, cog_rad, sog_mps)` | Broadcast course/speed over ground. |

### Implement — ISO 11783-7 / ISO 11783-9 (Tractor-implement)

Require the implement subsystem.

| Function | Purpose |
|---|---|
| `machbus_session_implement_command_hitch(h, hitch, command)` | Front/rear hitch raise/lower/no-action. |
| `machbus_session_implement_command_hitch_position(h, hitch, target_position, rate)` | Hitch to a target position (0..=1000 per mille). |
| `machbus_session_implement_command_pto(h, pto, command)` | Front/rear PTO engage/disengage/no-action. |
| `machbus_session_implement_command_pto_speed(h, pto, rpm, ramp_rate)` | PTO target speed with a ramp rate. |
| `machbus_session_implement_command_aux_valve(h, valve_index, command, flow_rate)` | Auxiliary valve command. |

### VT client — ISO 11783-6 (Virtual Terminal)

Require the VT client subsystem.

| Function | Purpose |
|---|---|
| `machbus_session_vt_connect(h, server)` | Connect to a VT server address. |
| `machbus_session_vt_disconnect(h)` | Disconnect. |
| `machbus_session_vt_state(h)` | Connection state code (0 = disconnected). |
| `machbus_session_vt_is_connected(h)` | Whether the client is connected. |
| `machbus_session_vt_show(h, object_id)` / `machbus_session_vt_hide(h, object_id)` | Show/hide an object. |
| `machbus_session_vt_set_value(h, object_id, value)` | Set a numeric value. |
| `machbus_session_vt_set_string(h, object_id, value)` | Set a string value (UTF-8, NUL-terminated). |

### TC client — ISO 11783-10 (Task Controller)

Require the TC client subsystem.

| Function | Purpose |
|---|---|
| `machbus_session_tc_connect(h)` | Begin connection / DDOP upload. |
| `machbus_session_tc_disconnect(h)` | Disconnect. |
| `machbus_session_tc_state(h)` | Connection state code (0 = disconnected). |
| `machbus_session_tc_is_connected(h)` | Whether the client is connected. |

## Events

`machbus_session_poll_event` flattens one event into an `MachbusEvent`: a `kind`
discriminant (`MachbusEventKind`) plus generic payload fields (`source`,
`spn_or_pgn`, `fmi_or_sub`, `d0`, `d1`, `u0`) whose meaning depends on the kind.
Subsystem events that have no stable C payload yet collapse to
`MachbusEventKind::Other`; reach for the Rust API when you need their full
detail.

## Drive loop sketch

```c
MachbusConfig cfg = machbus_session_default_config();
cfg.name_raw = my_name_raw;
cfg.enable_diagnostics = true;

MachbusSession *s = machbus_session_new(&cfg);
if (!s) { fprintf(stderr, "%s\n", machbus_session_last_error()); return 1; }

machbus_session_start_address_claim(s);

for (;;) {
    /* feed frames you received from the bus */
    machbus_session_feed(s, port, rx_id, rx_data, rx_len);

    machbus_session_tick(s, 10 /* ms */);

    uint8_t out_port; uint32_t out_id; uint8_t out[8]; size_t out_len;
    while (machbus_session_poll_transmit(s, &out_port, &out_id, out, &out_len)) {
        bus_write(out_port, out_id, out, out_len);
    }

    MachbusEvent ev;
    while (machbus_session_poll_event(s, &ev)) {
        handle_event(&ev);
    }
}

machbus_session_free(s);
```

Regenerate the header with `make bind-c` and prove it is stable with
`make bind-c-check`.
