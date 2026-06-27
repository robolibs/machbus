# ISO 11783-1 — general and device classes

Part 1 is the table of contents for the whole family: it lays out the layered
architecture, defines the roles a participant can play, and — most importantly for
day-to-day code — establishes the **NAME**, the 64-bit identity every control
function carries. Everything else in this section is a specialization of the picture
Part 1 paints.

## Why this exists

ISOBUS is a *system* standard, not a single protocol. Part 1 exists so that the
dozen later parts agree on the same vocabulary: what a "control function" is, what a
"device class" means, how identity is structured, and how the layers stack. Without
that shared frame, the VT part and the TC part would describe incompatible worlds.

## The layered picture

```
   application services      VT · TC · FS · SC · diagnostics · implement · GNSS
        ───────────────────────────────────────────────────────────────
   identity & messaging      NAME · address claim · requests · transport
        ───────────────────────────────────────────────────────────────
   naming                    PGN · source · destination · priority   (J1939)
        ───────────────────────────────────────────────────────────────
   wire                      250 kbit/s CAN                          (Part 2)
```

Part 1 owns the top-to-bottom shape; the other parts fill in each band.

## Roles and device classes

A participant is a **control function (CF)** — one job, one identity. A physical box
(an **ECU**) may host several CFs. Part 1's device-class vocabulary lets a CF say, in
its NAME, roughly *what kind of machine part it is* (a tractor ECU, a planter, a
fertilizer system, a positioning device, …). Other nodes use that to decide whether
they want to talk to it.

```
   ECU "ACME-Box"
     ├─ CF: Tractor ECU      (device class = tractor, function = TECU)
     └─ CF: Navigation       (device class = positioning)
```

machbus models roles in `net` (the NAME and its fields) and surfaces the
role/ownership mapping in [Role boundaries](../reference/role-boundaries.md).

## The NAME, field by field

The NAME answers "what is this?", not "where is it?". It is 64 bits, and every field
is exposed on `net::Name`:

```
   NAME (64 bits) — what a control function IS:

   ┌──────┬──────────┬──────────────┬──────────┬──────────┬──────────┬──────────────┐
   │ self │ industry │ device class │ function │ function │ ECU      │ manufacturer │
   │ cfg  │ group    │ (+ instance) │ code     │ instance │ instance │ + identity # │
   └──────┴──────────┴──────────────┴──────────┴──────────┴──────────┴──────────────┘

   self-cfg = "if I lose my address, I may pick another"
   manufacturer + identity number make every NAME globally unique
```

Two properties make the NAME the keystone of the whole network:

1. **Globally unique** — manufacturer code plus identity number guarantee no two CFs
   share a NAME.
2. **Totally ordered** — the 64-bit value is directly comparable, so any two CFs can
   deterministically decide who outranks whom.

Address claiming ([Part 5](iso11783-network-management.md)) needs exactly those two
properties and nothing more. The **self-configurable** bit decides a CF's fate in a
conflict: a self-configurable CF that loses its preferred address may take another;
one that is not must go silent.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Constructing a NAME | `net::Name` (`with_function_code`, `with_identity_number`, `with_self_configurable`, …) | [NAME management](../tutorials/name-management.md) |
| Roles a node plays | the session-facade plugins you `plug` | [The session facade](../guide/session-facade.md) |
| Where roles map to code | — | [Role boundaries](../reference/role-boundaries.md) |

## See also

- [ISO 11783-5 — address claiming](iso11783-network-management.md) — where the NAME
  earns its keep.
- [The standards, end to end](index.md) — the whole-system story.
