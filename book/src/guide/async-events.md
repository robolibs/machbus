# 11. Async event streams

By [chapter 2](hello-world-explained.md) you knew the machbus heartbeat: poll the
driver, pump the bus, then **react to each event** it hands back. Every chapter
since has matched on those events by polling — `driver.poll_at(now)?` or
`driver.poll()?` in your own loop. That works, but in an async program it is
awkward: you end up threading the event loop through your own scheduling instead
of just `await`-ing the next thing that happens.

This chapter shows how to bridge the session's event loop into async code you can
`await`. You still drive `poll` and pump the bus yourself — the session does not
sprout a background thread — but a local task can sleep on an async channel until
the next event lands instead of busy-polling. This is an advanced, opt-in
pattern; it builds directly on the event model from chapter 2, so make sure that
one feels solid first.

## Why bridge to async

The polling API is fine for a tight, synchronous loop. An async application is
different: it already has an executor, and it wants to express "wait here until
something happens" as an `.await`. The pattern below gives you exactly that. The
events are the *same* events the sync API drains; the async channel is a thin
view over them. Nothing about the protocol changes — only how your code waits.

The one rule that shapes everything below: **the session is single-threaded and
`!Send` by design.** Its internal state lives in `Rc<RefCell<_>>`, so the session
itself cannot move to another thread. You run it on a *local* executor. There is
no tokio dependency baked in and no hidden runtime; CAN progress happens only when
you poll and pump.

## Step 1 — turn on the `async` feature

Async helpers live behind the `async` Cargo feature. Enable it on your dependency
line:

```toml
[dependencies]
machbus = { version = "0.1", features = ["async"] }
```

This pulls in `futures-core` (for the `Stream` trait). It does **not** add a
runtime — see [Feature flags](../reference/feature-flags.md) for exactly what each
flag pulls in.

## Step 2 — build the session and a local executor

The setup is the familiar one from chapter 1: a one-node topology, an endpoint,
and a session split into `(ctrl, driver)`:

```rust
let (ctrl, mut driver) = Session::builder(name, addr)
    .plug(Plugin::...)
    .spawn(transport)?;
ctrl.start()?;
```

Because we will not move the session across threads, the executor you pick is a
*local* one — for example `futures::executor::LocalPool`, which runs tasks on the
current thread, or a tokio `LocalSet` with `spawn_local`. A multi-threaded
executor cannot hold a `!Send` future, so do not reach for a plain `tokio::spawn`
or a worker pool.

## Step 3 — feed events into an async channel

The bridge is one idea: in your synchronous poll loop, push each event the driver
returns into an async channel, and let a local task `await` the receiving end.
The driver stays on the main thread; the channel carries owned `Event` values, so
the consumer side can live in any local task.

```
            ┌─────────────────────┐
 poll()  ─► │  Session (!Send)    │  produces events
 pump    ─► │   └─ each Event ◄────┼── push into an async channel (Sender)
            └─────────────────────┘
                     ▲
            await ───┘  local task awaits the channel's Receiver
```

When the channel is empty, awaiting the receiver parks the task and stores its
waker; the next pushed event wakes it. You drive the producer side; the consumer
side reads like ordinary async code.

## Step 4 — spawn a local task that awaits events

Spawn a task onto the local pool that loops over the receiver. This is the
ergonomic payoff: `while let Some(event) = rx.next().await` reads like ordinary
async code, and each iteration parks until an event actually arrives.

```rust
// On the local pool:
spawn_local(async move {
    while let Some(event) = rx.next().await {
        // react to `event` here, fully async
    }
});
```

`spawn_local` is the local-executor counterpart of a thread-spawning `spawn`; it
keeps the task on this thread, which is exactly what a `!Send` consumer requires.

## Step 5 — keep driving poll and pump yourself

Here is the part people miss: spawning the task did **not** start any CAN traffic.
The session still only makes progress when you poll it and pump the bus. So the
main thread runs the same poll-and-pump loop from chapter 1, pushing each event
into the channel and giving the executor a chance to run:

```rust
ctrl.start()?;
loop {
    while let Some(event) = driver.poll_at(now)? {
        tx.unbounded_send(event).ok();
    }
    pump_bus();
    pool.run_until_stalled(); // let the listening task drain the channel
    if ctrl.is_claimed() { break; }
    now = now.add_millis(50);
}
```

`ctrl.start()` queues the claim; each `poll_at` advances and lets the session send
and settle; pumping moves frames; `run_until_stalled()` gives the listening task a
chance to pull events off the channel and react. On a real host clock you would
call `driver.poll()` instead of `driver.poll_at(now)`.

## What just happened

```
main thread                         local task
-----------                         ----------
ctrl.start()
loop:
  driver.poll_at(now) ──pushes──►   channel
  pump / run_until_stalled ──wake─► rx.next().await returns event
                                     reacts to it, fully async
  (claimed?) break
```

The session produced an event the moment the claim landed; pushing it into the
channel woke the parked task; the task reacted to it. No polling on the consumer
side, and no extra thread anywhere.

## Things that trip people up

- **Forgetting to still poll and pump.** The async channel is not a runtime. If
  you spawn the listener but never call `driver.poll()` and pump the bus, no
  frames move, no events are produced, and your task sleeps forever. The
  application still owns the heartbeat.
- **Expecting `Send`.** The session is `!Send` on purpose. Trying to move it onto
  a multi-threaded worker (a plain `tokio::spawn`, a thread pool) will not compile
  — and you should not work around it. Keep the driver and its consumer on one
  thread.
- **Picking the wrong executor.** Use a *local* executor that polls on the current
  thread: `LocalPool`, or a tokio `LocalSet` with `spawn_local`. A multi-threaded
  executor cannot hold a `!Send` future.
- **Bounded channels back-pressure.** If you use a bounded channel and the
  consumer falls behind, the producer side fills up. Drain regularly, or size the
  channel for your worst-case burst.

## What this proves / does not prove

Proves: machbus can bridge its event loop into a runtime-agnostic async channel
that integrates with a local async executor, while you keep full control of the
poll-and-pump heartbeat.

Does not prove: any multi-threaded safety. The session stays single-threaded and
`!Send` even with `async` enabled — this surface is a consumption convenience, not
a concurrency model. The usual caveats hold: nothing here is certified, and a real
deployment still needs official standards, hardware, and interoperability
evidence.

## See also

- [Feature flags](../reference/feature-flags.md) — what `async` pulls in and why
  the session stays single-threaded.
- [Receiving and routing messages](../standards/iso11783-network-layer.md) —
  the event model the channel is a view over.

## Next

→ [12. Capstone: a complete implement ECU](capstone.md) — put the whole track
together into one working node.
