# Standards capability map

The narrative chapters tell the story; this page is the map. Every standard that
matters to an ISOBUS machine has a row here: what it does in one line, where machbus
implements it, and which deep-dive chapter (or concept primer) covers it. Use it to
jump straight to what you need, or to confirm that a given standard area exists in
machbus at all.

> Standards are named at the part level only. "Where in machbus" points at the
> source module and, where it exists, the session-facade plugin; "Read more" points
> at the chapter that explains the behavior.

## The foundation

| Standard | What it does, in one line | Where in machbus | Read more |
| --- | --- | --- | --- |
| ISO 11898 (CAN) | The two-wire bus, bit timing, and non-destructive arbitration that everything rides on. | validated by `net` CAN-config checks | [The networking foundation](foundations.md) |
| SAE J1939 | Turns CAN bits into *named* messages (PGNs) sent between addresses; the parent of ISOBUS. | `net` (identifiers, PGNs), `j1939` | [The networking foundation](foundations.md) |
| NMEA 2000 | CAN-based positioning/instrument standard; carries the GNSS fix on the bus. | `nmea`, `session::plugins::Gnss` | [Positioning](positioning.md) |
| NMEA 0183 | Older serial GNSS sentences (`GGA`, `RMC`, …) for simpler receivers and benches. | `nmea` | [Positioning](positioning.md) |

## ISO 11783 — the ISOBUS parts

| Part | What it does, in one line | Where in machbus | Read more |
| --- | --- | --- | --- |
| **-1** General & device classes | The overall architecture and the device-class/role vocabulary. | `net` (NAME, roles) | [Role boundaries](../reference/role-boundaries.md) |
| **-2** Physical layer | The 250 kbit/s bus profile: cabling, termination, bit timing, sample point. | `net` CAN-config validation | [The networking foundation](foundations.md) |
| **-3** Data link layer | Framing plus the multi-packet Transport Protocol (TP) and Extended TP (ETP). | `net` (TP/ETP engines) | [The networking foundation](foundations.md) |
| **-4** Network layer | Joining CAN segments: which PGNs forward across a router, and loop/clash guards. | `net::niu` (interconnect) | [The networking foundation](foundations.md) |
| **-5** Network management | NAME-based address claiming — plug-and-play between strangers. | `net` (address claimer) | [The networking foundation](foundations.md) |
| **-6** Virtual Terminal | Screen sharing: an implement ships its UI to the cab terminal and drives it. | `isobus::vt`, `VtClient` / `VtServer` | [The Virtual Terminal](virtual-terminal.md) |
| **-7** Implement messages | Hitch, PTO, aux valves, speed/distance, lighting — status and commands. | `isobus::implement`, `Implement` | [Implement & services](implement-and-services.md) |
| **-8** Power train messages | Engine/transmission/powertrain status used across the machine. | `j1939` (engine/powertrain), `Powertrain` | [Implement & services](implement-and-services.md) |
| **-9** Tractor ECU | The TECU and its classes: which facilities (speed, hitch, PTO, guidance) a tractor offers. | `isobus::implement` (facilities), TECU persona | [Implement & services](implement-and-services.md) |
| **-10** Task Controller | Documented work: upload a device description, then trade process data for a job. | `isobus::tc`, `TcClient` / `TcServer` | [The Task Controller](task-controller.md) |
| **-11** Data dictionary (DDI) | The shared vocabulary so "application rate" means the same number to everyone. | `isobus::tc::ddi_database` | [The Task Controller](task-controller.md) |
| **-12** Diagnostics services | Active/previous faults, clears, freeze frames, memory access, identity strings. | `j1939::diagnostic`, `Diagnostics` / `DmMemory` | [Implement & services](implement-and-services.md) |
| **-13** File Server | A shared filesystem on the bus: volumes, directories, open/read/write/close. | `isobus::fs`, `FsClient` / `FsServer` | [Implement & services](implement-and-services.md) |
| **-14** Sequence Control | Run saved sequences of steps — headland automation and the like. | `isobus::sc`, `ScMaster` / `ScClient` | [Implement & services](implement-and-services.md) |

## AEF and certified capabilities

| Capability | What it does, in one line | Where in machbus | Read more |
| --- | --- | --- | --- |
| AEF functionalities | Vendor-interoperability functionalities + the certification process (machbus ships the mechanism, **not** certification). | `isobus::functionalities`, `ControlFunctionalities` | [Conformity first](../conformity/index.md) |
| TIM (Tractor Implement Management) | Lets an implement command the tractor's hitch/PTO under authority + safety interlocks. | `isobus::tim`, `Tim` | [TIM (AEF)](tim.md) |

## The supporting cast (ISOBUS/J1939 services)

These are smaller but real parts of a working node, each an machbus plugin:

| Service | One line | Plugin |
| --- | --- | --- |
| Heartbeat | Periodic "I'm alive" for liveness detection. | `Heartbeat` |
| Maintain Power | Ask the tractor to keep power after key-off to finish safely. | `MaintainPower` |
| Shortcut Button / ISB | The cab "stop everything" safe-state signal. | `ShortcutButton` |
| Language Command | Broadcast locale and unit preferences. | `LanguageCommand` |
| Auxiliary (AUX-O / AUX-N) | Joystick/switch-bank inputs assigned to implement functions. | `Auxiliary` |
| Group Function / Request2 / NAME management | Request/response and dynamic-NAME plumbing that keeps the network self-describing. | `GroupFunction`, `Request2`, `NameManagement` |

## Suggested reading order

1. [The standards, end to end](index.md) — the landscape and the wake-up timeline.
2. [The networking foundation](foundations.md) — until this clicks, nothing above it
   makes sense.
3. The service you are building: [VT](virtual-terminal.md),
   [TC](task-controller.md), or [implement & the rest](implement-and-services.md).
4. [Positioning](positioning.md) if guidance or TC-GEO is in scope.
5. Then build: [The session facade](../guide/session-facade.md).

For where each part lives in code, see the [Crate map](../reference/crate-map.md);
for exactly which messages are implemented and tested, see
[Protocol coverage](../reference/protocol-coverage.md).
