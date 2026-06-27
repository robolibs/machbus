# Further reading

These public resources are useful while learning the concepts in this section.
They are references for orientation, not a substitute for official protocol
documents when building a product.

## AgIsoStack++

The public AgIsoStack++ docs have a clear beginner path that influenced the
shape of these chapters:

- Concepts overview: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus/blob/main/sphinx/source/Concepts.rst>
- Tutorial index: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus/blob/main/sphinx/source/Tutorials.rst>
- Hello world tutorial: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus/blob/main/sphinx/source/Tutorials/The%20ISOBUS%20Hello%20World.rst>
- Adding a destination: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus/blob/main/sphinx/source/Tutorials/Adding%20a%20Destination.rst>
- Receiving messages: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus/blob/main/sphinx/source/Tutorials/Receiving%20Messages.rst>
- Transport layer: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus/blob/main/sphinx/source/Tutorials/Transport%20Layer.rst>
- Virtual Terminal basics: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus/blob/main/sphinx/source/Tutorials/Virtual%20Terminal%20Basics.rst>
- Task Controller basics/client docs: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus/tree/main/sphinx/source/Tutorials>

## Public lookup databases

- ISOBUS.net landing page and lookup categories:
  <https://www.isobus.net/isobus/>
- Manufacturer codes:
  <https://www.isobus.net/isobus/manufacturerCode>
- Device class/function:
  <https://www.isobus.net/isobus/nameFunction>
- Source addresses:
  <https://www.isobus.net/isobus/sourceAddress>
- PGN/SPN lookup:
  <https://www.isobus.net/isobus/pGNAndSPN/?type=PGN>
- Process Data DDI lookup:
  <https://www.isobus.net/isobus/dDEntity>

## machbus reference pages

- Protocol coverage: `book/src/reference/protocol-coverage.md`
- Hardware evidence: `book/src/reference/hardware-evidence.md`
- Claim boundary: `book/src/reference/audit/conformance.md`
- Binding contracts: `book/src/reference/audit/bindings.md`

When a public resource and machbus behavior differ, trust the executable tests
for what this repository currently does, and use the resource to decide what
should be improved next.
