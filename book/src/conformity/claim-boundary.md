# Claim boundary

machbus is a protocol implementation and development library. It is **not** an
officially certified ISOBUS product. Put another way: machbus is not an officially certified ISOBUS product.

## Safe wording

It is fine to say:

- machbus implements codecs and session behavior for many ISOBUS/J1939/NMEA
  workflows.
- machbus has local tests, fixtures, examples, and binding smoke tests.
- machbus is designed for agricultural CAN applications.

Do not say:

- machbus is ISO certified.
- machbus is AEF certified.
- machbus is guaranteed to work with every VT, TC, implement, or tractor.
- local unit tests are the same as formal conformance.

## What local tests prove

Local tests prove that the checked code paths behave as expected for the
fixtures, generated inputs, examples, and session scenarios in this repository.

They can catch:

- byte layout regressions
- malformed payload handling mistakes
- panics in public decode paths
- state-machine transition mistakes
- C/Python binding drift
- examples that no longer build

## What local tests cannot prove

Local tests cannot prove:

- that all official standard requirements have been reviewed
- that every vendor device accepts the behavior
- that timing on real CAN hardware is always suitable
- that a machine is safe to deploy
- that AEF conformance has been achieved

## Why standards text is not copied here

ISO 11783, SAE J1939, and NMEA 2000 are not fully public in the way normal open
source documentation is public. This book teaches concepts and documents this
library, but it does not reproduce normative standard text.

For product certification, use the official standards and the applicable AEF or
vendor validation process.
