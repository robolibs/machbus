# Powertrain

The powertrain messages are how the engine and the transmission tell the rest
of the bus what they are doing right now: how fast the engine is turning, how
hard it is working, how hot it is running, how many hours it has logged, which
gear the transmission is in, and whether a shift is under way. Nothing on this
page *commands* the powertrain in normal operation — these are status reports
that a tractor ECU, a display, a logger, or an implement reads so it can
coordinate its own behavior. This tutorial explains what each message group
carries, how raw CAN bytes become engineering units, and how to produce and
consume them with `machbus` at both the codec level (`j1939::engine` /
`j1939::transmission`) and through the session facade's `Powertrain` plugin.

These messages come from the shared J1939 application layer; ISO 11783-8
(Power Train Messages) adopts that set for agricultural and forestry machines,
so the same frames you would see on a truck appear on an ISOBUS.

## Why this exists

A working machine is a team of ECUs that must agree on what the engine and
driveline are doing. A task controller deciding whether application can start,
a virtual terminal drawing a tachometer, an implement that must back off when
the engine bogs down, a fleet logger recording duty hours — all of them need a
trustworthy, periodically broadcast picture of powertrain state. Putting that
picture on the bus in a fixed, scaled binary format means every node decodes
the same numbers the same way, regardless of who built the engine.

The design choice that makes this robust is that almost every field has a
defined **not-available** encoding. A sensor that is missing, warming up, or
faulted does not vanish or send a misleading zero — it sends the reserved
"I don't know" pattern, and every well-behaved receiver treats that as *no
reading* rather than a real value.

## Mental model

```
   engine ECU                    transmission ECU
       │                                │
   broadcasts EEC1, ET1,            broadcasts ETC1,
   hours, fuel, ...                 oil temp, ...
       │                                │
       └──────────────┬─────────────────┘
                      ▼
                 the CAN bus
                      │
        ┌─────────────┼───────────────┐
        ▼             ▼               ▼
   tractor ECU     display          logger
   (coordinates)   (tachometer)     (duty hours)
```

Each producer broadcasts its frames to the whole bus on a fixed cadence; each
consumer keeps the latest decoded value per source address and acts on it. A
frame is a snapshot, not a stream of deltas: if a newer frame never arrives,
the last one simply ages, and it is the *receiver's* job to notice staleness.

## Anatomy: the message groups

`machbus` ports each powertrain frame as a small `Copy` struct with `encode`
returning the 8-byte wire form and `decode` returning `Option<Self>` — `None`
when the bytes are malformed or carry a not-available value the struct cannot
represent. The groups below are the ones you will reach for most.

### Engine speed, torque, and load

| Type | Carries |
| --- | --- |
| `Eec1` | Engine speed (rpm), actual/demanded/driver-demand torque as a percent, starter mode, and the source address that owns the speed signal. |
| `Eec2` | Accelerator pedal position, engine load percent, low-idle and kickdown switches, road-speed limit. |
| `Eec3` | Nominal friction percent, desired operating speed, operating-speed asymmetry. |
| `Tsc1` | A *request* to control speed or torque, with an override mode (`OverrideControlMode`). This is a command, not a status — treat it with care. |

`Eec1` is the workhorse. Its `engine_speed_rpm` field is the value a tachometer
reads; the three percent fields express torque relative to a reference and are
stored with an offset so they can go negative (engine braking shows as a
negative percent).

### Engine temperature and fluids

| Type | Carries |
| --- | --- |
| `EngineTemp1` | Coolant, fuel, oil, turbo-oil, and intercooler temperatures in °C. |
| `EngineTemp2` | A second set of oil/turbo/intercooler temperatures at finer resolution. |
| `EngineFluidLp` | Oil and coolant pressure (kPa), oil and coolant level (percent), fuel-delivery and crankcase pressure. |
| `DashDisplay` | Fuel level and washer-fluid level (percent), fuel- and oil-filter differential pressure, cargo/ambient temperature. |
| `AmbientConditions` | Barometric pressure, ambient/intake/road-surface temperature. |
| `Vep1` | Battery, charging-system, and key-switch voltage; alternator current. |

### Engine hours and totals

| Type | Carries |
| --- | --- |
| `EngineHours` | Total engine hours and total engine revolutions — lifetime counters. |
| `FuelEconomy` | Instantaneous and average fuel rate (L/h) and throttle position. |
| `FuelConsumption` | Trip and total fuel used (litres). |
| `Aftertreatment1` / `Aftertreatment2` | DEF tank level, NOx readings, DPF pressure/soot/regeneration status. |

These are large, slowly changing counters. They are 32-bit on the wire so they
do not roll over for the life of the machine.

### Transmission state

| Type | Carries |
| --- | --- |
| `Etc1` | Current gear and selected gear, output-shaft speed (rpm), shift-in-progress flag, torque-converter lockup flag. |
| `TransmissionOilTemp` | Transmission oil temperature (°C). |
| `CruiseControl` | Wheel-based vehicle speed, cruise/brake/clutch/park-brake switch states, cruise set speed. |

In `Etc1` the gears are signed: reverse gears are negative, neutral sits near
zero, and forward gears are positive. The struct exposes them as `i8`
(`current_gear`, `selected_gear`) so you read a gear number directly rather
than a biased byte. `selected_gear` is what the operator/controller asked for;
`current_gear` is what is actually engaged — they differ during a shift, which
is exactly when `shift_in_progress` is set.

### Identity

`ComponentIdentification` (make / model / serial / unit) and
`VehicleIdentification` (a VIN string) are `*`-delimited text rather than scaled
numbers. They are diagnostic identity, not a security credential — never treat a
VIN read off the bus as proof of who you are talking to.

## Sentinels, scale, and offset

Raw CAN carries unsigned bytes; the engineering value comes from three rules
applied per field: a **scale** (units per bit), an optional **offset**, and a
reserved **not-available** code at the top of the range.

- **Scale.** A 2-byte field at 0.125 rpm/bit means a raw count of `12000`
  decodes to `1500.0` rpm. Temperatures often use 0.03125 °C/bit for fine
  resolution, pressures 4 kPa/bit, hours 0.05 h/bit.
- **Offset.** Signed quantities are stored unsigned with a bias. Torque percent
  uses an offset of −125, so a raw `0` is −125 % and raw `250` is +125 %. The
  oil temperatures use an offset of −273, which is why a fresh-default value
  reads as a deeply negative °C rather than zero.
- **Not-available.** An all-ones field (`0xFF`, or `0xFFFF` for two bytes) means
  *no reading*. `machbus` encodes the not-available pattern into unused bytes
  and, on decode, returns `None` for frames whose mandatory fields are
  not-available rather than handing you a fake number.

Two consequences worth internalizing:

1. **`encode` clamps away from the sentinel.** If you ask for a value at or
   beyond the top of a field's range, the encoder writes the largest *real*
   code, not the not-available code, so your own frames are never mistaken for
   "no reading". A non-finite input (`NaN`/`inf`) encodes as the bottom of the
   range.
2. **`decode` is strict.** Wrong length, reserved bits set where they must be
   zero, or an unrepresentable not-available value all yield `None`. The
   `Default` for several of these structs deliberately *is* the not-available
   state (for example `Etc1::default()` reports gears at −125 and both flags as
   `0x03`, the "not available" code).

## Doing it with machbus

There are two layers, and they suit different jobs.

### The codecs (for tests and direct decode)

Each struct round-trips on its own with no network state. The powertrain demo
builds an `Eec1`, encodes it, decodes it back, and prints the engineering
values — the canonical pattern for a callback that received a raw frame:

```rust
{{#include ../../../examples/engine_powertrain_demo.rs:11:24}}
```

`EngineHours` works the same way, carrying the lifetime counters:

```rust
{{#include ../../../examples/engine_powertrain_demo.rs:54:63}}
```

To wire these into a low-level loop, register a callback for the relevant PGN
with `IsoNet::register_pgn_callback` and call the matching `decode` inside it.
That is the pump-style path used throughout the crate; the codecs hold no
`IsoNet` reference themselves.

### The Powertrain plugin (recommended for applications)

The `Powertrain` plugin routes every supported powertrain PGN for you. Reach it
through `ctrl.with_mut::<Powertrain, _>(|p| ...)`, which gives you two ways to
read state:

- A **snapshot** of the latest decoded value per message. `snapshot()` returns a
  `PowertrainSnapshot` whose fields (`eec1`, `etc1`, `engine_hours`, …) are
  `Option`, populated only once a valid frame has arrived. There are shortcuts
  like `latest_eec1()` and `latest_etc1()` / `latest_cruise_control()`.
- An **event drain.** Newly decoded frames arrive as `PowertrainEvent` on the
  event stream, one entry per frame, each tagged with the `source` address and
  the decoded `data`. Use the snapshot when you only care about "the current
  value" and events when you must react to *every* update or distinguish two
  sources.

The shape of a read loop is:

```rust
// illustrative shape, not a verbatim compiled call
driver.poll()?;
for event in ctrl.drain::<PowertrainEvent>() {
    if let PowertrainEvent::Eec1 { source, data } = event {
        println!("0x{source:02X} engine {:.0} rpm", data.engine_speed_rpm);
    }
}
let rpm = ctrl.with_mut::<Powertrain, _>(|p| p.snapshot().eec1.as_ref().map(|e| e.engine_speed_rpm));
```

To produce frames, the plugin exposes broadcast helpers:
`broadcast_eec1`, `broadcast_etc1`, and `broadcast_vehicle_identification`.
Each encodes the struct and sends it to the broadcast address at the default
priority, from the session's own control function. You only do this when your
node *is* the engine or transmission ECU (or a test stand emulating one).

## Events and responsibilities

| Event family | Meaning | Typical action |
| --- | --- | --- |
| `Eec1` / `Eec2` / `Eec3` | New engine speed/torque/load snapshot. | Update displays; gate work on load/rpm. |
| `EngineTemp1` / `EngineTemp2` / `EngineFluidLp` | New thermal/fluid reading. | Warn or derate on over-temperature; flag low fluid. |
| `EngineHours` / `FuelEconomy` / `FuelConsumption` | New counters. | Log duty and consumption. |
| `Etc1` / `TransmissionOilTemp` | New transmission state. | Coordinate with gear/shift; watch oil temp. |
| `CruiseControl` | Vehicle speed and driveline switches. | Combine with TECU speed; honor park-brake. |

Your responsibilities as a consumer:

- **Key on the source address.** Two engines (or an engine plus a test stand)
  can both broadcast `Eec1`. Keep state per `source`, not globally.
- **Honor not-available.** A `None` from `decode`, or an `Option` field still
  empty in the snapshot, means *no reading*. Do not substitute zero.
- **Do not broadcast what you are not.** Only the real producer should call the
  broadcast helpers for a given message.

## Edge cases and failures

- **Not-available decodes to `None`.** A frame whose required field is
  all-ones is rejected, not silently zeroed. Plan for `decode` returning
  `None` on perfectly valid "sensor absent" frames.
- **Reserved bits.** `Eec1` rejects a frame whose starter-mode nibble has the
  upper bits set; `Etc1` rejects a frame whose mode byte uses reserved bits or
  whose padding is not all-ones. Garbled frames do not corrupt your cache.
- **Out-of-range on encode.** Asking for a gear of 127 or an rpm of 99 999
  does not overflow — the encoder clamps to the largest representable real
  code, staying clear of the not-available pattern.
- **Wrong length.** Anything other than the exact 8-byte payload (too short or
  too long) decodes to `None`. The codecs never read past the buffer.
- **Stale data.** Nothing in the snapshot times out for you. If a producer
  goes quiet, the last value lingers. Track arrival time yourself and treat an
  un-refreshed value as stale past the expected update interval.
- **Identity is not trust.** A decoded VIN or component string is a label a
  peer chose to send; it is not an authenticated identity.

## Advanced

- **Update rates.** Engine speed/torque (`Eec1`) and transmission state
  (`Etc1`) refresh quickly — on the order of tens of milliseconds — because
  control loops depend on them. Temperatures, hours, and fuel totals change
  slowly and broadcast far less often. Size your staleness windows per message,
  not with a single global timeout.
- **Deriving values.** Some quantities you want are not on the bus directly.
  Output-shaft speed from `Etc1` combined with a known final-drive ratio
  approximates ground speed; `EngineHours` sampled over wall-clock time yields
  a duty ratio. Derive deliberately and label derived numbers as estimates.
- **Combining with TECU speed.** The tractor ECU publishes its own
  ground/wheel-based speed (see the tractor-ECU tutorial). Cross-check it
  against `CruiseControl` wheel speed: agreement builds confidence, a
  persistent gap points at a slipping wheel or a miscalibrated sensor.
- **Codec vs the session facade.** Reach for the bare codecs in unit tests and
  tight embedded loops where you own every callback. Reach for the `Powertrain`
  plugin in applications: it registers the PGNs, decodes, caches per message,
  and fans out events so you write only the reaction.

## Validate locally

```sh
make run EXAMPLE=engine_powertrain_demo
make test
```

The example builds `Eec1`, `EngineTemp1`, `FuelEconomy`, and `EngineHours`,
round-trips each through `encode`/`decode`, and prints the engineering values
so you can confirm the scaling. The test suite includes round-trip,
not-available-sentinel, reserved-bit, out-of-range, wrong-length, and
property-based fuzz tests for the engine and transmission codecs, plus
session-level tests that a malformed payload does not overwrite the last good
cached value.

## What this proves / does not prove

Proves: the engine and transmission codecs map raw bytes to engineering units
with the correct scale and offset, reject malformed and not-available frames,
clamp on encode, and that the `Powertrain` plugin caches and fans out the latest
value per message without being corrupted by bad input.

Does not prove: real-hardware timing or broadcast cadence, interoperability
with a specific engine or transmission ECU, sensor accuracy, or any
conformance/certification claim. A real deployment still needs official
standards, real hardware, and interoperability evidence.

## See also

- [Tractor ECU](tractor-ecu.md) — ground speed, PTO, hitch, and the messages a
  tractor ECU publishes alongside the powertrain.
- [Implement ECU](implement-ecu.md) — the implement side that consumes
  powertrain and tractor state to coordinate work.
- [Address claim](address-claim.md) — every producer and consumer here assumes
  a claimed source address first.
