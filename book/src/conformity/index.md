# Conformity and evidence

This book starts with conformity because agricultural networks are safety- and
interoperability-sensitive. It is not enough for code to compile. Users need to
know what was tested, what evidence exists, and where the boundary is.

In this book:

- **Conformity** means the implementation follows known protocol behavior and
  has local evidence.
- **Certification** means official or third-party validation. machbus does not
  claim that.

The rest of the book is practical. You will see tutorials and examples, but
those tutorials should always be read with this boundary in mind.

## Evidence categories

machbus uses several kinds of evidence:

| Evidence | Purpose |
| --- | --- |
| Unit tests | Validate small codecs and state transitions. |
| Golden fixtures | Pin exact bytes for known messages. |
| Property tests | Feed broad input ranges and hostile bytes. |
| Session/role tests | Prove roles work together over a bus abstraction. |
| Binding tests | Check Rust, C, and Python surfaces. |
| Trace replay | Check captured or fixture CAN logs. |
| Hardware evidence | Required before real deployment claims. |

Start with [Claim boundary](claim-boundary.md) before using the protocol
tutorials.
