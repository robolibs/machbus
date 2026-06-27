# Fuzz and validation

Fuzz/property smoke tests feed broad byte ranges into decoders and parsers.

They are good at finding:

- panics
- unchecked indexing
- length overflows
- non-canonical re-encoding

They are not a replacement for official conformance. Use fuzzing together with
golden fixtures, stack tests, trace replay, and hardware evidence.

## Local commands

```sh
make fuzz-smoke
make trace-replay-demo
```

## Adding a new failure as evidence

1. Reduce the failing input to the smallest useful fixture.
2. Add a unit, property, or replay test that fails without the fix.
3. Document the expected behavior in the relevant tutorial/reference chapter.
4. Run `make verify`.
