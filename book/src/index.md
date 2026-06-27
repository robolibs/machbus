# machbus

machbus is a Rust library for building agricultural CAN and ISOBUS-style
applications. It includes protocol codecs, protocol state machines, helper
surfaces, examples, and C/Python bindings.

It is tested locally by a large suite of unit, fixture, integration, binding, trace,
and fuzz-smoke checks. In plain words: machbus is not certified. It ships with
no ISO, SAE, NMEA, or AEF certification. Certification and deployment approval
still require the official standards, real hardware, and interoperability
evidence.

## Fast path

If you are new to the project, read in this order:

First, read conformity boundary material so every API example is interpreted
inside the correct evidence and certification limits. Then learn ISOBUS basics,
build your first node, and pick tutorial material for the role you are building.

1. [Claim boundary](conformity/claim-boundary.md)
2. [The standards, end to end](standards/index.md) — the story of how ISOBUS works,
   or [ISOBUS in plain words](isobus-basics/index.md) for a quicker primer.
3. [Build and verify](getting-started/build-and-verify.md)
4. [The session facade](guide/session-facade.md) — the recommended API for new code.
5. Pick the tutorial for the role you are building.

## The application surface

machbus gives you one application surface over the protocol codecs:
**[the session facade](guide/session-facade.md)** (`machbus::session`). You compose
a node from plugins, drive a pure sans-IO core, and use a driver/handle split. It
is the recommended hosted API and the same feed/tick/drain shape is the embedded
API. In hosted/default builds you also get the high-level plugin stack, C/Python
bindings, virtual-bus adapters, and richer geo helpers. In embedded builds you
disable defaults and use the `no_std + alloc` surface with board-owned time, CAN
I/O, and storage. Every guided walkthrough and tutorial page teaches the hosted
shape first; MCU users should also read
[`no_std` on microcontrollers](getting-started/no-std-microcontrollers.md).

## What machbus gives you

- J1939 and ISOBUS-oriented CAN identifiers, address claiming, requests, and
  transport helpers.
- Higher-level services for Virtual Terminal, Task Controller, File Server,
  diagnostics, Sequence Control, tractor/implement facilities, GNSS, and NMEA.
- A session surface that lets multiple roles talk over a virtual bus or real
  SocketCAN endpoints.
- Rust APIs first, with C ABI and Python bindings for integration.

## What you still own

- Choosing device identities and safe addresses for your machine.
- Testing with your actual CAN hardware and wiring.
- Validating behavior with real Virtual Terminals, Task Controllers, service
  tools, and other ECUs.
- Official conformance/certification work.

## Where the old audit docs went

The previous audit-oriented documentation was moved to `book/src/reference/audit/`. Use it as a
migration reference when maintaining the book, but write new user-facing content
in this `book/` mdBook.
