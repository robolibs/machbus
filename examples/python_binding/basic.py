"""machbus Python binding smoke demo.

Builds a single node on the sans-IO `machbus.Session` facade, claims an
address, raises a diagnostic trouble code, advances simulated bus time, and
drains the resulting application events.

Because the core is sans-IO, the Python side drives it explicitly: there is no
background thread. We advance a millisecond cursor with `tick`, and (with no bus
contention) the address claim completes purely by letting time pass.

Run with:
    make -C examples/python_binding basic
"""

import importlib.metadata

import machbus


def main() -> None:
    print(f"machbus version: {importlib.metadata.version('machbus')}")

    session = machbus.Session(
        name=machbus.name(0x100, 0x80, True),
        preferred_address=0x80,
        enable_diagnostics=True,
    )

    session.start()
    address = session.run_until_claimed(2_000)
    print(f"address     = 0x{address:02X}")
    print(f"claim_state = {session.claim_state()}")
    print(f"is_claimed  = {session.is_claimed()}")

    # Drop the claim events so the rest of the run prints cleanly.
    session.drain_events()

    # Raise a couple of diagnostic trouble codes.
    session.diag_raise(100, 1)        # SPN 100, FMI 1 (below normal)
    session.diag_raise(523_312, 0)    # SPN 523312, FMI 0 (above normal)
    print(f"\nactive DTC count = {session.diag_active_count()}")
    for dtc in session.diag_active():
        print(f"  DTC {dtc}")

    # Advance ~2 simulated seconds so the periodic DM1 broadcast fires, then
    # forward whatever the core wants to transmit (here we just count frames).
    frames = 0
    for _ in range(40):
        session.tick(50)
        frames += len(session.poll_transmit_all())

    print(f"\nframes queued during 2 s of bus time: {frames}")

    print("\nevents after 2 s of bus time:")
    events = session.drain_events()
    for ev in events:
        kind = ev.get("kind")
        sub = ev.get("sub", "")
        rest = {k: v for k, v in ev.items() if k not in ("kind", "sub")}
        print(f"  {kind}::{sub} {rest}")
    print(f"\ntotal events drained: {len(events)}")
    print(f"now_ms              = {session.now_ms()}")
    print(f"final repr          = {session!r}")


if __name__ == "__main__":
    main()
