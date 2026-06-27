# machbus docs style guide (internal)

This file governs every page written under `PLAN_DOCS.md`. It is not part of the
published mdBook (it is not listed in `SUMMARY.md`); it is guidance for authors.

## Hard rules

1. **Docs only.** Changes land only under `book/`. Never edit Rust, tests,
   examples, `Makefile`, `Cargo.*`, or `PLAN.md`. To show code, reference an
   existing file under `examples/` — do not add or change one.
2. **Original words only.** Study the private standards under the author's
   private standards directory to get behavior right, then write everything in
   your own words. No copied sentences, tables, figures, annex text, or
   normative wording. No clause/table/figure/page citations. A short part-level
   reference like "ISO 11783-6 (Virtual Terminal)" is the only permitted
   citation form.
3. **Never leak the private path.** The literal private path string must never
   appear in any file under `book/`. Extract helper text only to `/tmp` and
   never commit it.
4. **Keep the non-claim posture.** `machbus` is not certified; no ISO/SAE/NMEA/
   AEF certification is shipped or implied. Real deployment still needs official
   standards, hardware, and interoperability evidence.

## Depth checklist (a page is "very extensive" when it has these, where they apply)

- Why it exists (the field/machine problem).
- Mental model + a small ASCII diagram.
- Anatomy of the message/object/field, tied to the machbus types.
- Full lifecycle / state machine: states, transitions, triggers, timeouts,
  aborts, error paths — as behavior, not transcribed tables.
- Worked walkthrough on the real machbus API, grounded in an `examples/` file.
- Events and application responsibilities.
- Edge cases and failure modes.
- Advanced notes (multi-node, performance, surface vs low-level API).
- "Validate locally" commands (`make` / `cargo`).
- "What it proves / what it does not prove".
- Cross-links to basics, related tutorials, and troubleshooting.

## House voice

- Direct, practical, second person ("you build...", "the stack sends...").
- Short paragraphs; tables for enumerations; ASCII diagrams for flows.
- Explain from the machbus API outward, not from the standard inward.
- One canonical explanation per concept; cross-link instead of repeating.
- Use the glossary terms; extend the glossary rather than coining synonyms.

## Code and includes

- Prefer real compiled examples via `{{#include ../../../examples/NAME.rs:anchor}}`.
- Prefer mdBook anchors over line ranges. When you must use a range, verify it
  against the current file and keep it small.
- Conceptual snippets that are not from an example must be clearly framed as
  illustrative shape, not a compiled call, to avoid implying a guaranteed API.

## Page skeleton (adapt per topic)

```
# Title

One-paragraph "what this is and who needs it".

## Why this exists
## Mental model
## Anatomy / the pieces
## Lifecycle (or workflow / state machine)
## Doing it with machbus
## Events and responsibilities
## Edge cases and failures
## Advanced
## Validate locally
## What this proves / does not prove
## See also
```
