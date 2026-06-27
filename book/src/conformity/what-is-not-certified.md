# What is not certified

machbus does not include an official certificate for:

- ISO 11783
- SAE J1939
- NMEA 2000
- AEF ISOBUS conformance

The code can be useful before certification. It can help build prototypes,
tools, tests, simulators, gateways, and product code. But shipping on real
machines requires a separate validation process.

## Treat these as open deployment responsibilities

- Run with the actual CAN interface and bitrate.
- Test against the actual VT, TC, service tool, and peer ECUs.
- Capture and review traces.
- Validate machine safety behavior.
- Use official standards and AEF processes where required.

The repository may contain external-oracle rows in its evidence matrix. Those
rows mean “implemented locally but still needs external proof”, not “certified”.
