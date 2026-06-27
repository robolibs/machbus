# Where to start

Welcome to the machbus guided walkthrough. This is a **hands-on, build-along
track**: you start a real Rust program, add a few lines, run it, watch ISOBUS
traffic happen, and then keep extending the same program until it does
something genuinely useful on a tractor bus. Every chapter ends with a program
you can actually run.

If the rest of the book is the reference manual, this track is the lab course.

> **New here?** Read [The session facade](session-facade.md) first — it shows the
> whole shape of the API end to end. Most chapters below teach the protocols one
> at a time using that same session facade, building one program up step by step;
> chapters 7 and 8 drop down to the lower-level VT and Task-Controller client
> codec APIs to show how a subsystem works underneath the facade.

## Who this is for

- You know a little Rust (enough to build a project with `cargo`) and want to
  learn ISOBUS by doing.
- Or you know ISOBUS and want to see how it maps onto the machbus API.

You do **not** need a CAN adapter or a tractor. The first ten chapters run
entirely in software on a simulated bus. Real hardware shows up in
[chapter 10](real-hardware.md), and it is optional.

## How the track is structured

The chapters build on each other. Each one keeps the program from the previous
chapter and adds to it:

| # | Chapter | You will build | Level |
| --- | --- | --- | --- |
| 1 | [The ISOBUS Hello World](hello-world.md) | A node that claims an address on a bus | Beginner |
| 2 | [The Hello World, line by line](hello-world-explained.md) | The same program, fully understood | Beginner |
| 3 | [Sending and receiving messages](sending-receiving.md) | Broadcast and receive a PGN | Beginner |
| 4 | [Requests and acknowledgements](requests-and-acks.md) | Ask another node for data | Beginner |
| 5 | [Moving big data with transport](transport.md) | Send a payload too large for one frame | Intermediate |
| 6 | [Talking diagnostics](diagnostics.md) | Publish and read fault codes | Intermediate |
| 7 | [Your first Virtual Terminal client](virtual-terminal.md) | Put a screen on the terminal | Intermediate |
| 8 | [Your first Task Controller client](task-controller.md) | Report a working value to a TC | Intermediate |
| 9 | [Tractor and implement personas](tractor-and-implement.md) | A tractor and an implement talking | Advanced |
| 10 | [Onto real hardware with SocketCAN](real-hardware.md) | The same code on a `vcan` interface | Advanced |
| 11 | [Async event streams](async-events.md) | An async, await-driven event loop | Advanced |
| 12 | [Capstone: a complete implement ECU](capstone.md) | Everything, combined | Advanced |

## How to follow along

Every chapter is anchored to a real example that ships in the repository, so the
code you read is code that compiles and runs. Wherever you see a code block
pulled from `examples/`, you can run that exact example:

```sh
cargo run --example session_minimal
# or, equivalently, through the Makefile:
make run EXAMPLE=session_minimal
```

The recommended way to work through the track is to keep your own scratch
example file open beside the book and paste each step into it as you go. If you
get stuck, the finished example for that chapter is named at the top of the
page — diff your version against it.

## What you need installed

- A recent stable Rust toolchain (see [Install Rust](../getting-started/install-rust.md)).
- The repository checked out, so `cargo run --example ...` works.
- Nothing else for chapters 1–9 and 11. Chapter 10 uses Linux SocketCAN and a
  `vcan` virtual interface.

## Before you dive in

You will get more out of the track if you have skimmed two short concept pages
first. They are quick reads and the walkthrough refers back to them:

- [ISOBUS in plain words](../isobus-basics/index.md) — the five-minute mental
  model.
- [NAME and address claim](../standards/iso11783-network-management.md) — why a
  node needs an identity before it can talk.

Everything else is explained as it comes up.

## A word on what this proves

The walkthrough teaches the machbus API and ISOBUS behavior in software. It is a
learning resource, not a certification. `machbus` is not certified; shipping a
real machine still needs the official standards, real hardware, and
interoperability testing with the actual terminals, task controllers, and ECUs
you intend to work with.

Ready? Start with [The ISOBUS Hello World](hello-world.md).
