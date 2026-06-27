# Positioning: NMEA and GNSS

Guidance, section control, and TC-GEO all need one thing the rest of ISOBUS does not
provide: an accurate, fresh answer to "where am I, which way am I pointed, and how
fast am I going?" That answer comes from a GNSS receiver, and on a modern machine it
usually arrives over **NMEA 2000** — a CAN-based standard that is a cousin, not a
child, of ISOBUS. This chapter explains how the fix gets onto the bus and into a
prescription.

## Why this is a separate world

ISO 11783 is about *machine control*; positioning is a *measurement domain* with its
own committee, its own message catalog, and its own history (the older serial
**NMEA 0183** sentences and the CAN-based **NMEA 2000**). Rather than reinvent it,
ISOBUS machines simply carry NMEA 2000 traffic on the same wire and let agricultural
software consume it. machbus treats positioning as a first-class but bounded
subset: it decodes the PGNs guidance and TC-GEO actually need.

```
   GNSS receiver ──(NMEA 2000 PGNs)──► bus ──► guidance / TC-GEO / logging
        │                                            │
        position, COG/SOG, heading, attitude,        turns a fix + a map into
        dilution-of-precision, system time           per-section setpoints
```

## What a fix actually contains

A position is more than a latitude/longitude pair. The signals that matter for
agriculture, each its own NMEA 2000 PGN that machbus decodes:

| Signal | Why a machine cares |
| --- | --- |
| Rapid position (lat/lon) | the basic fix, updated frequently for guidance |
| Detailed GNSS position | full fix with quality/satellite info |
| COG / SOG (course & speed over ground) | direction and speed for guidance and logging |
| Heading / track control | where the vehicle points (not always the same as COG) |
| Attitude (yaw/pitch/roll) | terrain compensation for accurate ground position |
| Rate of turn | smoothing and prediction |
| DOPs (dilution of precision) | *how much to trust* the fix |
| System time / date, local-time offset | timestamping the as-applied log |

The recurring theme: a raw lat/lon is not enough. Accurate section control needs
heading and attitude (the GNSS antenna is not at ground level, and a tilted machine
projects its position sideways), and the DOP values tell the software when the fix
is too poor to act on.

## Fast Packet: the right-sized transport

A full GNSS position record does not fit in 8 bytes, but it is far smaller than an
object pool. NMEA 2000 uses **Fast Packet** — a lightweight multi-frame scheme — for
these modest records, rather than the heavier ISOBUS Transport Protocol. machbus's
network layer reassembles Fast Packet PGNs (the GNSS position-data message among
them) so the decoder sees a complete record.

```
   Fast Packet:  frame 0 (header + first bytes) · frame 1 · frame 2 …
                 → reassembled into one GNSS position record → decoded
```

## NMEA 0183, briefly

Some receivers still speak the older serial **NMEA 0183** — comma-separated ASCII
sentences (`GGA`, `RMC`, `VTG`, …) over a UART rather than CAN. machbus can parse
these too, which is useful for bench setups and simpler receivers. The two worlds
carry the same underlying information in very different envelopes; machbus normalizes
both into the same position types.

## From a fix to a setpoint

The payoff is the loop that closes guidance and variable-rate work:

```
   NMEA fix (lat/lon, heading, attitude, quality)
        │  normalize, check quality (DOP)
        ▼
   machine ground position (antenna offset + tilt corrected)
        │  +  DDOP geometry (section offsets, working width)
        │  +  prescription map
        ▼
   per-section coverage + setpoint rates          (TC-GEO)
        │
        ▼
   process data DOWN to the implement  ·  as-applied logged UP
```

This is why the positioning chapter sits next to the
[Task Controller](task-controller.md): the fix is an input to TC-GEO, and the DDOP
geometry is what turns "the machine is here" into "boom section 7 is over this
square metre."

## Doing it with machbus

On the session facade, plug `Gnss`. It decodes the NMEA 2000 GNSS/navigation PGNs
into cached state plus `GnssEvent`s, and can broadcast a few of them:

```
   Session::builder(name, addr)
       .plug(Gnss::new(NMEAConfig::default().with_all(true)))
       .spawn(transport)
              │
              ▼  driver.poll()
       inbound NMEA PGNs → Event::Gnss(GnssEvent::Position / Cog / Sog / …)
       cached fix available via get::<Gnss>().latest_position()
```

The hands-on paths are [NMEA 2000](../tutorials/nmea-2000.md) and
[Serial GNSS](../tutorials/serial-gnss.md).

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Decoding NMEA 2000 GNSS PGNs | `session::plugins::Gnss` | [NMEA 2000](../tutorials/nmea-2000.md) |
| Serial NMEA 0183 receivers | `nmea` parser | [Serial GNSS](../tutorials/serial-gnss.md) |
| Steering on top of the fix | guidance helpers | [Guidance](../tutorials/guidance.md) |
| Position → setpoint | TC-GEO + DDOP geometry | [TC-GEO prescription](../tutorials/tc-geo-prescription.md) |

## Scope and honesty

machbus implements the GNSS/navigation PGNs that agricultural guidance and TC-GEO
need, decoded and tested locally; it is deliberately a *subset* of the full NMEA 2000
catalog (it is not a marine chartplotter). It does **not** provide RTK correction,
a positioning engine, or certified positioning accuracy — that is the receiver's
job. What machbus guarantees is faithful decoding of the records the receiver puts on
the bus.

## Failure modes worth knowing

- **Trusting a bad fix** — acting on a position without checking DOP/quality leads to
  misapplied product; gate work on fix quality.
- **Ignoring attitude** — on slopes, an uncorrected antenna position is metres off at
  ground level.
- **COG ≠ heading** — at low speed, course-over-ground is noisy; heading is the
  reliable signal for orientation.
- **Stale fix** — if the receiver drops out, the last position must be treated as
  stale, not current.

## See also

- [The Task Controller and the data dictionary](task-controller.md) — where the fix
  becomes a setpoint.
- [The networking foundation](foundations.md) — Fast Packet and the transport family.
- [Guidance](../tutorials/guidance.md) — steering and curvature on top of the fix.
