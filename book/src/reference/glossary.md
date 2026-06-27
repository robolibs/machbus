# Glossary

Plain-words definitions for the ISOBUS, J1939, NMEA, and `machbus` terms used
throughout this book. Definitions are written in the book's own words and aim to
match how each idea is used in the surrounding pages. Where a topic has a
dedicated chapter, a "(see ...)" pointer is given.

### A

**Address claim** — The handshake by which a device tells the bus it intends to
use a particular one-byte address, and defends that choice against any other
device that wants the same address. (see [NAME and address claim](../standards/iso11783-network-management.md))

**AEF** — Agricultural Industry Electronics Foundation, the group that runs the
interoperability test and certification program for ISOBUS products. `machbus`
ships no AEF certification.

**Alarm mask** — A Virtual Terminal screen layout that the operator's terminal
shows when a working set raises an alarm, typically grabbing attention and
optionally sounding the terminal. (see [Virtual Terminal concepts](../standards/virtual-terminal.md))

**Arbitration** — The mechanism on a CAN bus that decides which message goes
first when two devices transmit at once: the lower identifier wins, bit by bit,
without corrupting either frame. The same idea decides address-claim contests.

### B

**BAM** — Broadcast Announce Message: the transport mode that sends a large
message to everyone on the bus, one numbered chunk at a time, with no
handshake or flow control. (see [Transport Protocol](../standards/iso11783-datalink-transport.md))

**Broadcast address** — The destination value (255) meaning "every device on the
bus." A message addressed there is for all listeners rather than one partner.

### C

**CAN** — Controller Area Network: the low-level serial bus that carries every
ISOBUS and J1939 frame. It provides short frames, built-in priority, and
collision-free arbitration. (see [CAN and J1939](../standards/j1939.md))

**CMDT** — Connection-mode data transfer: the handshaked transport mode that
moves a large message to a single destination using request-to-send and
clear-to-send exchanges. (see [Transport Protocol](../standards/iso11783-datalink-transport.md))

**Commanded address** — A network management instruction that tells a specific
device (identified by its NAME) to move to a new address. The device obeys and
re-claims at the commanded value. (see [NAME and address claim](../standards/iso11783-network-management.md))

**Control function (CF)** — Any addressable participant on the bus: a logical
sender/receiver identified by a NAME and an address. One physical box can host
several control functions.

**CTS** — Clear To Send: the receiver's reply in a handshaked transport that
tells the sender how many chunks it may send next, and from which point. (see [Transport Protocol](../standards/iso11783-datalink-transport.md))

### D

**DDI** — Data Dictionary Identifier: a standardized code naming one quantity a
device can report or accept, such as an application rate or a section state.
DDIs make process data portable across vendors. (see [DDOP and process data](../standards/task-controller.md))

**DDOP** — Device Descriptor Object Pool: the structured self-description an
implement uploads to a Task Controller, listing its elements, the quantities it
exposes, and how to present them. (see [DDOP and process data](../standards/task-controller.md))

**Device element** — One node in a DDOP tree that represents a real or logical
part of the machine (the whole device, a boom, a single section) and carries
the process-data quantities tied to that part. (see [DDOP and process data](../standards/task-controller.md))

**DTC** — Diagnostic Trouble Code: a fault report combining a parameter number
and a failure-mode code, used to surface problems to operators and tools. (see [Diagnostics basics](../standards/iso11783-diagnostics.md))

### E

**ECU** — Electronic Control Unit: a physical computing box on the machine. An
ECU may host one or more control functions.

**EOM** — End-of-message acknowledgement: the receiver's confirmation, after a
handshaked transport, that it got every chunk and reassembled the whole message.

**ETP** — Extended Transport Protocol: the transport used for messages too large
for the ordinary transport protocol, using larger sequence numbers and
byte-offset bookkeeping. (see [Transport Protocol](../standards/iso11783-datalink-transport.md))

### F

**Fast Packet** — The NMEA 2000 multi-frame scheme that strings several CAN
frames together to carry a single larger marine/navigation message. (see [NMEA and GNSS basics](../standards/positioning.md))

**Fix quality** — The reported trustworthiness of a GNSS position, distinguishing
no fix, a basic fix, and corrected high-accuracy modes. Guidance and section
control care a great deal about this value. (see [NMEA and GNSS basics](../standards/positioning.md))

**FMI** — Failure Mode Identifier: the code that says *how* a parameter is
failing (too high, too low, open circuit, and so on) inside a fault report.

**FS** — File Server: a device that offers shared storage on the bus so other
control functions can read and write files. (see [File Server and large data](../standards/iso11783-file-server.md))

### G

**GNSS** — Global Navigation Satellite System: the umbrella term for satellite
positioning (GPS and its peers) that feeds position, speed, and heading into
guidance and mapping. (see [NMEA and GNSS basics](../standards/positioning.md))

### I

**Identifier (29-bit)** — The extended CAN frame header that ISOBUS uses. It
packs priority, the parameter group, the destination (when applicable), and the
source address into one value that also decides arbitration order. (see [PGNs, priority, source, destination](../standards/j1939.md))

**Industry group** — A field inside a NAME that says which family of machinery
the device belongs to (for example, agricultural equipment), helping classify
participants on a shared bus.

**Internal vs partner CF** — From your stack's viewpoint, an *internal* control
function is one your own code owns and operates; a *partner* control function is
another device you have chosen to talk to. The distinction drives filtering and
routing. (see [Control functions and partners](../standards/iso11783-general-device-classes.md))

**ISB (Shortcut Button)** — The ISOBUS Shortcut Button: a single operator control
whose press commands every listening implement to drop into a safe, stopped
state. (see [Shortcut Button and safe-state thinking](../standards/implement-and-services.md))

**ISOBUS** — The agricultural networking standard defined by the ISO 11783 family
of parts, built on top of J1939, that lets tractors, implements, and terminals
from different makers interoperate.

### M

**Manufacturer code** — A field in a NAME identifying which company made the
device, assigned so that NAMEs stay globally distinct.

**Mask (data / alarm / soft key)** — A Virtual Terminal screen region. The *data
mask* is the main working area an implement draws into, the *alarm mask*
interrupts with a warning, and the *soft key mask* holds the row of programmable
buttons. (see [Virtual Terminal concepts](../standards/virtual-terminal.md))

### N

**NAME** — The 64-bit identity of a control function. It encodes who made it,
what function it performs, its industry group, and whether it can move its own
address; its numeric value also sets priority in address-claim contests. (see [NAME and address claim](../standards/iso11783-network-management.md))

**NIU / router** — Network Interconnection Unit: a node that bridges two CAN
segments, forwarding the traffic that belongs across the boundary. In `machbus`
the routing layer plays this role. (see [Network routing](../tutorials/network-routing.md))

**NMEA** — The marine-electronics body behind the NMEA 0183 serial sentences and
the CAN-based NMEA 2000 messages, several of which carry the GNSS and navigation
data ISOBUS machines consume. (see [NMEA and GNSS basics](../standards/positioning.md))

**Null address** — The placeholder source address (254) a device uses while it
has no valid claimed address, for example after losing an address contest. A
device at the null address cannot do normal traffic.

### O

**Object pool** — The bundle of drawing and interaction objects a Virtual
Terminal client uploads so the terminal can render and run its user interface.
(see [Working sets and object pools](../standards/virtual-terminal.md))

**ObjectID** — The numeric handle that names one object inside an object pool, so
later messages can reference, change, or read that exact object. (see [VT object pools](../tutorials/vt-object-pools.md))

### P

**PDU1 / PDU2** — Two header formats for a parameter group. PDU1 carries a
destination address (a directed message to one device), while PDU2 has no
destination field and is always broadcast. (see [PGNs, priority, source, destination](../standards/j1939.md))

**PGN** — Parameter Group Number: the identifier of a kind of message, telling
you what the payload means regardless of who sent it. (see [PGNs, priority, source, destination](../standards/j1939.md))

**Priority** — The few header bits that bias arbitration: lower-numbered priority
wins the bus sooner, so urgent messages can preempt routine ones. (see [PGNs, priority, source, destination](../standards/j1939.md))

**Process data** — The live, named quantities an implement and a Task Controller
exchange during work: measured values flowing up and setpoints flowing down,
each tagged by a DDI. (see [DDOP and process data](../standards/task-controller.md))

### R

**RTS** — Request To Send: the opening message of a handshaked transport, naming
the message to come and its size so the receiver can agree to take it. (see [Transport Protocol](../standards/iso11783-datalink-transport.md))

### S

**Safe state** — The defined, predictable condition a device falls back to when
something goes wrong or an operator demands a stop: outputs off, motion halted,
no surprises. (see [Shortcut Button and safe-state thinking](../standards/implement-and-services.md))

**SC** — Sequence Control (and, by extension, section control work): the
mechanism for running ordered, scripted automation steps between cooperating
devices. (see [Sequence Control and TIM](../standards/iso11783-sequence-control.md))

**Section control** — Automatically switching individual implement sections on
and off based on position, coverage, and boundaries, so you avoid double-applying
or treating outside the field. (see [Task Controller concepts](../standards/task-controller.md))

**Self-configurable address** — A flag in a NAME marking a device that is allowed
to pick a different address on its own if its first choice is taken. Devices
without it must keep a fixed address. (see [NAME and address claim](../standards/iso11783-network-management.md))

**Sequence control** — See **SC**: coordinated, step-by-step command sequences
between an initiator and the devices it drives. (see [Sequence Control and TIM](../standards/iso11783-sequence-control.md))

**Setpoint** — A target value a controller asks an implement to achieve, such as
a commanded application rate, expressed as process data. (see [DDOP and process data](../standards/task-controller.md))

**Soft key** — A programmable on-screen button on a Virtual Terminal whose
meaning is defined by the current object pool and whose presses are reported back
to the working set. (see [Virtual Terminal concepts](../standards/virtual-terminal.md))

**Source address** — The one-byte field in every frame saying which control
function sent it. A device must own a claimed address before using it as a
source. (see [PGNs, priority, source, destination](../standards/j1939.md))

**SPN** — Suspect Parameter Number: the code naming *which* parameter a fault
report is about. Paired with an FMI it identifies a specific problem.

### T

**TAN** — Transaction number: a small rolling counter carried in some Virtual
Terminal exchanges so a request and its matching response can be paired
unambiguously. (see [VT updates](../tutorials/vt-updates.md))

**TC** — Task Controller: the device that drives documentation and control of
field work, receiving process data, applying prescriptions, and coordinating
section control with implements. (see [Task Controller concepts](../standards/task-controller.md))

**TECU** — Tractor ECU: the tractor-side control function that publishes machine
data (speed, distance, power-takeoff, hitch, and similar) for implements to use.
(see [Tractor ECU](../tutorials/tractor-ecu.md))

**TIM** — Tractor Implement Management: the framework that lets an approved
implement request limited control over tractor functions, under operator
oversight and safety conditions. (see [Sequence Control and TIM](../standards/iso11783-sequence-control.md))

**Transport protocol** — The set of rules for splitting a message larger than one
CAN frame into numbered chunks and reassembling them, whether broadcast (BAM) or
handshaked (CMDT). (see [Transport Protocol](../standards/iso11783-datalink-transport.md))

### V

**Value presentation** — The formatting metadata in a DDOP that says how to turn
a raw process-data number into something human-readable: scale, offset, decimal
places, and units. (see [DDOP and process data](../standards/task-controller.md))

**VT** — Virtual Terminal: the shared operator display in the cab that renders the
user interface uploaded by each implement and reports operator input back to it.
(see [Virtual Terminal concepts](../standards/virtual-terminal.md))

### W

**Working set** — The identity an implement presents to services such as the
Virtual Terminal: a root object that groups the device's interface and its member
control functions. (see [Working sets and object pools](../standards/virtual-terminal.md))

**Working-set master** — The control function that speaks for a working set,
performing the upload and interaction handshakes on behalf of any members behind
it. (see [Working sets and object pools](../standards/virtual-terminal.md))

---

*Reminder: these definitions explain how the book uses each term. `machbus`
carries no ISO, SAE, NMEA, or AEF certification; real deployment still requires
the official standards, hardware, and interoperability evidence.*
