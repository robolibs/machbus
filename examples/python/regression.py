"""machbus Python binding regression checks.

A self-contained, dependency-free assertion suite over the `machbus.Session`
surface. Each `test_*` function raises `AssertionError` (or another exception)
on failure; `main` runs them all and exits non-zero if any fail, so the Makefile
gate (`make -C examples/python_binding regression`) is a real check.

Run with:
    make -C examples/python_binding regression
"""

from __future__ import annotations

import importlib.metadata
import math
from collections.abc import Callable

import machbus


def expect_raises(
    exc_type: type[BaseException],
    contains: str,
    fn: Callable[..., object],
    *args: object,
    **kwargs: object,
) -> None:
    try:
        fn(*args, **kwargs)
    except exc_type as exc:
        message = str(exc)
        assert contains in message, f"expected {contains!r} in {message!r}"
        return
    raise AssertionError(
        f"expected {exc_type.__name__} containing {contains!r}, nothing raised"
    )


def claim(session: machbus.Session, timeout_ms: int = 2_000) -> int:
    session.start()
    address = session.run_until_claimed(timeout_ms)
    assert session.claim_state() == "claimed"
    assert session.is_claimed()
    assert address == session.address()
    session.drain_events()
    return address


def test_version_and_exports() -> None:
    # The package version is published via distribution metadata.
    assert importlib.metadata.version("machbus")
    # The single high-level class is `Session`; the old classes are gone.
    assert hasattr(machbus, "Session")
    for removed in ("Stack", "Machbus", "Bus", "BusStack", "Tractor", "AlarmPanel"):
        assert not hasattr(machbus, removed), f"{removed} should no longer exist"
    # Module-level functions.
    for fn in ("name", "validate_can_bus_config", "enforce_iso_can_config"):
        assert hasattr(machbus, fn), f"missing module function {fn}"


def test_iso_can_config_validation() -> None:
    valid = machbus.validate_can_bus_config()
    assert valid["overall_ok"]
    assert valid["bitrate_ok"]
    assert valid["sample_point_ok"]
    assert valid["bit_timing_ok"]
    assert valid["physical_mode_ok"]
    assert valid["error_message"] == ""
    assert machbus.enforce_iso_can_config() is None

    wrong_bitrate = machbus.validate_can_bus_config(bitrate=500_000)
    assert not wrong_bitrate["overall_ok"]
    assert not wrong_bitrate["bitrate_ok"]
    expect_raises(RuntimeError, "250000", machbus.enforce_iso_can_config, bitrate=500_000)

    local_only = machbus.validate_can_bus_config(silent_mode=True)
    assert not local_only["overall_ok"]
    assert not local_only["physical_mode_ok"]


def test_name_helper() -> None:
    raw = machbus.name(0x100, 0x80, True)
    assert isinstance(raw, int)
    # Self-configurable bit and a differing identity number produce a
    # different NAME than the non-self-configurable variant.
    assert raw != machbus.name(0x100, 0x80, False)
    assert raw != machbus.name(0x101, 0x80, True)


def test_claim_and_introspection() -> None:
    session = machbus.Session(
        name=machbus.name(0x100, 0x80, True),
        preferred_address=0x80,
        enable_diagnostics=True,
    )
    assert session.claim_state() == "none"
    assert not session.is_claimed()
    assert claim(session) == session.address()
    assert session.has_diagnostics()
    assert not session.has_gnss()
    assert not session.has_implement()
    assert not session.has_vt_client()
    assert not session.has_tc_client()
    # now_ms advances with tick.
    before = session.now_ms()
    session.tick(100)
    assert session.now_ms() == before + 100


def test_run_until_claimed_timeout() -> None:
    # A session that never starts claiming should time out.
    session = machbus.Session(name=machbus.name(0x111, 0x80, True))
    expect_raises(RuntimeError, "timed out", session.run_until_claimed, 0)


def test_presets() -> None:
    for preset in ("tractor", "implement", "diagnostic_node"):
        session = machbus.Session(
            name=machbus.name(0x120, 0x80, True),
            preferred_address=0x81,
            preset=preset,
        )
        assert claim(session) == 0x81

    expect_raises(
        ValueError,
        "unknown preset",
        machbus.Session,
        preset="not_a_preset",
    )


def test_raw_send_and_poll_transmit() -> None:
    session = machbus.Session(
        name=machbus.name(0x130, 0x80, True),
        preferred_address=0x82,
    )
    assert claim(session) == 0x82
    # Drain anything pending from the claim handshake.
    session.poll_transmit_all()

    session.send_raw(0xEF00, [0x01, 0x02, 0x03], dst=0x21, priority=6)
    session.tick(0)
    frames = session.poll_transmit_all()
    assert frames, "expected at least one transmitted frame"
    for port, can_id, data in frames:
        assert isinstance(port, int)
        assert isinstance(can_id, int)
        # Payloads come back as bytes, padded to the 8-byte CAN frame width.
        assert isinstance(data, (bytes, bytearray))
        assert len(data) == 8
    # Our payload bytes lead the frame data.
    assert bytes(frames[0][2][:3]) == b"\x01\x02\x03"

    # poll_transmit yields one frame at a time (or None when drained).
    session.send_raw(0xEF00, [0xAA], dst=0x21)
    session.tick(0)
    one = session.poll_transmit()
    assert one is not None
    # Eventually drains to None.
    while session.poll_transmit() is not None:
        pass
    assert session.poll_transmit() is None

    expect_raises(ValueError, "invalid CAN priority", session.send_raw, 0xEF00, [0x00], 0x21, 9)


def test_diagnostics_lifecycle() -> None:
    session = machbus.Session(
        name=machbus.name(0x140, 0x80, True),
        preferred_address=0x83,
        enable_diagnostics=True,
        diagnostics_interval_ms=500,
    )
    assert claim(session) == 0x83

    assert session.diag_active_count() == 0
    session.diag_raise(100, 1)
    session.diag_raise(523_312, 0)
    assert session.diag_active_count() == 2
    spns = {dtc["spn"] for dtc in session.diag_active()}
    assert spns == {100, 523_312}
    for dtc in session.diag_active():
        assert "spn" in dtc and "fmi" in dtc and "occurrence_count" in dtc

    session.diag_clear()
    assert session.diag_active_count() == 0
    assert session.diag_active() == []


def test_diagnostics_guard_when_disabled() -> None:
    session = machbus.Session(name=machbus.name(0x150, 0x80, True))
    assert session.diag_active_count() == 0
    assert session.diag_active() == []
    expect_raises(RuntimeError, "diagnostics subsystem not enabled", session.diag_raise, 1, 1)
    expect_raises(RuntimeError, "diagnostics subsystem not enabled", session.diag_clear)


def test_gnss() -> None:
    session = machbus.Session(
        name=machbus.name(0x160, 0x80, True),
        preferred_address=0x84,
        enable_gnss=True,
    )
    assert claim(session) == 0x84
    assert session.has_gnss()
    # No position has been *received* yet (broadcast publishes, it does not
    # populate the inbound cache).
    assert session.gnss_latest_position() is None

    # Broadcasting our own position + COG/SOG must not raise.
    session.gnss_broadcast_position(
        latitude=52.52,
        longitude=13.405,
        altitude_m=34.0,
        speed_mps=3.5,
        heading_rad=math.pi / 4,
    )
    session.gnss_broadcast_cog_sog(0.75, 3.5)
    session.tick(0)
    # Whatever the broadcast queued should be valid CAN frames.
    for _port, _id, data in session.poll_transmit_all():
        assert len(data) == 8

    # Disabled session rejects gnss calls.
    plain = machbus.Session(name=machbus.name(0x161, 0x80, True))
    assert plain.gnss_latest_position() is None
    expect_raises(
        RuntimeError, "gnss subsystem not enabled", plain.gnss_broadcast_position, 0.0, 0.0
    )


def test_implement() -> None:
    session = machbus.Session(
        name=machbus.name(0x170, 0x80, True),
        preferred_address=0x85,
        enable_implement=True,
    )
    assert claim(session) == 0x85
    assert session.has_implement()

    session.imp_command_hitch("rear", "raise")
    session.imp_command_pto("rear", "engage")
    session.imp_command_pto_speed("rear", 4320, 10)
    session.imp_command_aux_valve(2, "extend", 0x4000)

    expect_raises(ValueError, "unknown hitch", session.imp_command_hitch, "middle", "raise")
    expect_raises(ValueError, "unknown pto", session.imp_command_pto, "side", "engage")
    expect_raises(
        RuntimeError, "out of range", session.imp_command_aux_valve, 16, "extend", 0x4000
    )

    plain = machbus.Session(name=machbus.name(0x171, 0x80, True))
    expect_raises(
        RuntimeError, "implement subsystem not enabled", plain.imp_command_hitch, "rear", "raise"
    )


def test_vt_and_tc_clients() -> None:
    session = machbus.Session(
        name=machbus.name(0x180, 0x80, True),
        preferred_address=0x86,
        enable_vt_client=True,
        enable_tc_client=True,
    )
    assert claim(session) == 0x86
    assert session.has_vt_client()
    assert session.has_tc_client()
    assert session.vt_is_connected() is False
    assert session.tc_is_connected() is False
    assert session.vt_state() == "disconnected"
    assert session.tc_state() == "disconnected"
    assert isinstance(session.tc_address(), int)

    # A VT connection request is accepted without raising.
    session.vt_connect_to(0x26)

    plain = machbus.Session(name=machbus.name(0x181, 0x80, True))
    assert plain.vt_is_connected() is False
    assert plain.tc_is_connected() is False
    expect_raises(RuntimeError, "vt client subsystem not enabled", plain.vt_connect_to, 0x26)
    expect_raises(RuntimeError, "tc client subsystem not enabled", plain.tc_connect)


def test_event_dict_schema() -> None:
    session = machbus.Session(
        name=machbus.name(0x190, 0x80, True),
        preferred_address=0x87,
        enable_diagnostics=True,
    )
    session.start()
    session.run_until_claimed(2_000)
    # poll_event returns dicts with a "kind" key, drained one at a time.
    seen = 0
    while (ev := session.poll_event()) is not None:
        assert isinstance(ev, dict)
        assert "kind" in ev
        seen += 1
    # After draining, both interfaces agree the queue is empty.
    assert session.poll_event() is None
    assert session.drain_events() == []
    assert seen >= 0  # claim may or may not have emitted, both are fine


def discovered_tests() -> list[tuple[str, Callable[[], None]]]:
    return sorted(
        (name, fn)
        for name, fn in globals().items()
        if name.startswith("test_") and callable(fn)
    )


def main() -> int:
    tests = discovered_tests()
    failures = 0
    for name, fn in tests:
        try:
            fn()
        except Exception as exc:  # noqa: BLE001 - report every failure
            failures += 1
            print(f"FAIL {name}: {type(exc).__name__}: {exc}")
        else:
            print(f"ok   {name}")
    if failures:
        print(f"\npython regression: {failures} of {len(tests)} tests FAILED")
        return 1
    print(f"\npython regression smoke: ok ({len(tests)} tests)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
