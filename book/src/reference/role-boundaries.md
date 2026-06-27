# Role boundaries

This page is the ISO 11783-1 orientation map for this crate. It names the
application roles the library models, where those roles live in the code, and
what evidence exists before a role should be trusted outside the local test
environment.

It is intentionally a boundary document, not a replacement for the standard.
The official documents define the requirements. This page only explains how the
repo is organized in original wording.

## Core terms used in this crate

| Term | Meaning in machbus | Main code/docs |
|---|---|---|
| Control Function | One named participant on the bus. It owns a NAME and must claim a source address before sending application traffic. | `src/net/name.rs`, `src/net/address_claimer.rs`, [NAME and address claim](../standards/iso11783-network-management.md) |
| ECU | The software/hardware node hosting one or more Control Functions. In machbus this is a `Session` composed from plugins. | `src/session/` ([The session facade](../guide/session-facade.md), [First node](../getting-started/first-node.md)) |
| NAME | The 64-bit identity used for arbitration and partner tracking. | `src/net/name.rs`, [Glossary](glossary.md) |
| Source address | The claimed 8-bit address used as the sender field in CAN identifiers. | `src/net/identifier.rs`, `src/net/address_claimer.rs` |
| Working Set | A functional group used by Virtual Terminal clients and object pools. | `src/isobus/vt/`, [Working sets and object pools](../standards/virtual-terminal.md) |
| Virtual Terminal | The display/input role and the implement client role around object pools and runtime commands. | `src/isobus/vt/`, [Virtual Terminal concepts](../standards/virtual-terminal.md) |
| Task Controller | The role that manages DDOP upload, process data, peer control, and TC-GEO helpers. | `src/isobus/tc/`, [Task Controller concepts](../standards/task-controller.md) |
| Tractor ECU | Tractor facilities, maintain-power, hitch/PTO, speed, lighting, and related messages. | `src/isobus/implement/`, `src/session/presets.rs` |
| Implement ECU | Implement-side control/status surfaces, including sections, guidance helpers, File Server, VT client, TC client, and diagnostics. | `src/isobus/`, `src/session/presets.rs` |
| File Server | The ISO file-access client/server role. | `src/isobus/fs/`, [File Server and large data](../standards/iso11783-file-server.md) |
| Sequence Control | Master/client workflow for ordered implement actions. | `src/isobus/sc/`, [Sequence Control and TIM](../standards/iso11783-sequence-control.md) |
| TIM | Automation authority and interlock helpers. | `src/isobus/tim.rs`, [TIM and automation](../tutorials/tim.md) |
| NIU | Network interconnect/routing helper. | `src/net/niu.rs`, [Network routing](../tutorials/network-routing.md) |

## Boundary rules

These rules keep examples, tests, and docs aligned:

1. A node must claim an address before it sends normal application traffic.
2. Address arbitration belongs to the network-management layer, not to VT, TC,
   FS, SC, or diagnostics code.
3. Protocol roles stay separate from machine-safety decisions. TIM and shortcut
   button helpers expose protocol state; applications still own the real safety
   policy.
4. A binding is a facade decision, not a new protocol definition. Rust, C, and
   Python should all point back to the same role behavior.
5. A local test or virtual-bus example is evidence for this repository only. It
   is not a vendor interoperability result and not product approval.

## Where to check the current status

- [Claim boundary](../conformity/claim-boundary.md) says what the project does
  and does not claim.
- [What is tested](../conformity/what-is-tested.md) summarizes the local test
  surface.
- [Protocol matrix](protocol-matrix.md) lists implemented protocol surfaces and
  remaining external-evidence needs.
- [Standard gap roadmap](standard-gap-roadmap.md) tracks gaps, completed
  hardening slices, and binding decisions.
- [Hardware evidence](hardware-evidence.md) explains what is required before a
  physical-bus or peer-device result counts as repository evidence.

## How this affects new work

When adding a feature, first decide which role owns it. Then update the gap
matrix and add tests in the standard-derived suite before making a completion
claim. If a feature spans roles, such as VT object pools over TP or TC DDOP
upload over TP, test both the codec and the cross-role workflow.

If a role is intentionally not exposed through C or Python yet, record that in
the binding matrix. That makes the absence explicit instead of accidental.
