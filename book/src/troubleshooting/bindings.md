# Binding problems

## C

- Regenerate/check `include/machbus.h`.
- Verify ownership: destroy only through matching machbus functions.
- Run `make c-demo` and `make c-full-demo`.

## Python

- Use `make python-demo`.
- Avoid stale installed wheels.
- Check disabled-subsystem guards before expecting a handle.
- Drain events after ticking the session.
