# Python

The Python binding is built on the [session facade](../guide/session-facade.md)
and exposed as a single `machbus.Session` class. It wraps the sans-IO `Session`
core, so the Python side drives the node explicitly: stamp inputs against a
millisecond clock, advance timers with `tick`, feed received CAN frames, and
drain the outbound frames and application events.

The Python bindings (pyo3, abi3 for Python 3.9+) are compiled into every hosted
(`std`/default) build. Build an importable wheel with `make bind-py`
(`maturin build --features pyo3/extension-module`).

## Constructing a session

```python
import machbus

s = machbus.Session(
    name=machbus.name(0x100, 0x80, True),
    preferred_address=0x80,
    enable_diagnostics=True,
)
```

Constructor signature:

```python
Session(
    name=0,
    preferred_address=0x80,
    preset=None,                  # "tractor" | "implement" | "diagnostic_node"
    enable_diagnostics=False,
    diagnostics_interval_ms=1000,
    enable_gnss=False,
    enable_implement=False,
    enable_vt_client=False,
    enable_tc_client=False,
)
```

When `preset` is set it is plugged first, then the `enable_*` flags add any extra
subsystems on top. Each subsystem type may only be plugged once.

## Driving the node

| Method | Purpose |
|---|---|
| `start()` | Begin address claiming. |
| `tick(dt_ms)` | Advance the clock by `dt_ms` and tick the session. |
| `now_ms()` | Current monotonic time cursor, in milliseconds. |
| `run_until_claimed(timeout_ms)` | Tick in 50 ms steps until claimed; returns the address. |
| `feed(port, can_id, data)` | Feed one received CAN frame (raw 29-bit id + payload bytes) on `port`. |

With no bus contention the claim completes purely by ticking:

```python
s.start()
addr = s.run_until_claimed(2000)
print(addr, s.claim_state())
```

## Outputs

| Method | Purpose |
|---|---|
| `poll_transmit()` | Next `(port, can_id, data)` to transmit, or `None`. |
| `poll_transmit_all()` | Drain every queued outbound frame as `(port, can_id, data)` tuples. |
| `poll_event()` | Next application event as a dict, or `None` when drained. |
| `drain_events()` | Drain all queued events as dicts. |

Each event dict carries a `kind` and a `sub`, plus kind-specific fields. Branch
on those:

```python
for (port, can_id, data) in s.poll_transmit_all():
    bus_write(port, can_id, data)

while ev := s.poll_event():
    if ev["kind"] == "diag" and ev["sub"] == "raised":
        print(ev["spn"], ev["fmi"])
```

## Address claim

`address()`, `claim_state()` (a string such as `"claimed"`), and `is_claimed()`.

## Raw send

```python
s.send_raw(pgn, data, dst=0xFF, priority=6)
```

`dst` is a destination address (`0xFF` for broadcast), `priority` is 0..=7.

## Diagnostics — ISO 11783-12 (Diagnostics)

`diag_raise(spn, fmi)`, `diag_clear()`, `diag_active_count()`, and
`diag_active()` (a list of dicts). Require the diagnostics subsystem.

## GNSS — NMEA 2000

`gnss_broadcast_position(latitude, longitude, altitude_m=None, speed_mps=None,
heading_rad=None)`, `gnss_broadcast_cog_sog(cog_rad, sog_mps)`, and
`gnss_latest_position()` (a dict or `None`). Require the GNSS subsystem.

## Implement — ISO 11783-7 / ISO 11783-9 (Tractor-implement)

Require the implement subsystem. Hitch and PTO take `"front"`/`"rear"`; commands
are strings such as `"raise"`, `"lower"`, `"engage"`, `"disengage"`.

| Method | Purpose |
|---|---|
| `imp_command_hitch(hitch, command)` | Front/rear hitch command. |
| `imp_command_pto(pto, command)` | Front/rear PTO command. |
| `imp_command_pto_speed(pto, rpm, ramp_rate)` | PTO target speed with a ramp rate. |
| `imp_command_aux_valve(valve_index, command, flow_rate)` | Auxiliary valve command. |

## VT client — ISO 11783-6 (Virtual Terminal)

Require the VT client subsystem.

| Method | Purpose |
|---|---|
| `vt_connect_to(server)` | Connect to a VT server address. |
| `vt_is_connected()` | Whether the client is connected. |
| `vt_state()` | Connection state as a string. |
| `vt_show(object_id)` / `vt_hide(object_id)` | Show/hide an object. |
| `vt_set_value(object_id, value)` | Set a numeric value. |
| `vt_set_string(object_id, value)` | Set a string value. |
| `vt_change_active_mask(ws, mask)` | Change the active mask of a working set. |

## TC client — ISO 11783-10 (Task Controller)

Require the TC client subsystem: `tc_connect()`, `tc_disconnect()`,
`tc_is_connected()`, `tc_state()` (a string), and `tc_address()`.

## Module functions

| Function | Purpose |
|---|---|
| `machbus.name(identity_number, function_code, self_configurable=True)` | Build a J1939/ISOBUS NAME, returns the raw 64-bit value. |
| `machbus.validate_can_bus_config(...)` | Returns a dict of per-field and overall checks. |
| `machbus.enforce_iso_can_config(...)` | Raises if the config is not ISO-conformant. |
