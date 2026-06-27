# VT external evidence reports

This directory stores the provenance reports for promoted independent VT
evidence. All Phase 10 manifest rows currently have checked-in reports:
the VT3 `.iop` pool covers basic and graphics evidence, the VT3 callback
reduction covers command-trace evidence, and reduced open-license seeder
fixtures cover soft-key paging, input objects, and user-layout/window-mask
placement.

For each completed row in
`tests/fixtures/isobus/vt_external_evidence_requirements.txt`, add one Markdown
report here and keep the matching requirement row synchronized.

Reports must include:

- requirement id;
- fixture path and fixture hash;
- source tool/device/project name, version, license, and acquisition date;
- whether the fixture is raw `.iop`/`.bin` bytes or reviewable `.hex`;
- exact `iop_inspect` command used for promotion;
- active-mask override used for promotion (`--active-mask`) when the evidence
  targets a mask other than the Working Set default active mask;
- layout profile flags used for promotion (`--canvas`, `--soft-key-area`,
  `--physical-soft-keys`, `--navigation-soft-keys`, `--soft-key-page`);
- `--write-report-json` path and reported `rgb888_fnv64`;
- reported RGB565 big-endian and little-endian framebuffer hashes
  (`rgb565_be_fnv64` and `rgb565_le_fnv64`) when the framebuffer rendered;
- inspector JSON schema (`machbus-iop-inspect-report-v1` for pool inspection,
  or `machbus-vt-trace-inspect-report-v1` for command-trace inspection);
- inspector JSON `artifacts` object, including the report path and any raw
  RGB888/RGB565 paths requested during promotion;
- reported pool-buffer hash;
- for command-trace requirements, exact `vt_trace_inspect` command, trace
  fixture path, trace payload hash, accepted-effect count, and final
  `rgb888_fnv64`;
- whether `--strict` passed;
- unsupported-record and placeholder-pixel counts;
- expected-count gates used during promotion (`--expect-unsupported-records`,
  `--expect-placeholder-pixels`, `--expect-accepted-effects`,
  `--expect-initial-placeholder-pixels`, and
  `--expect-final-placeholder-pixels` as applicable);
- any raw RGB888/RGB565 artifacts kept for display comparison;
- caveats and non-claims.

Do not mark a requirement `complete` without a fixture, an inspector JSON
report, and a Markdown report. Independent evidence should prove that machbus
can consume externally authored pools; repo-owned synthetic pools remain useful
regressions but do not satisfy Phase 10.

Copy this shape for every completed row:

````md
# <short title>

Requirement: <vt_external_evidence_requirements id>
Fixture path: tests/fixtures/isobus/<name>.iop
Fixture hash: <sha256 or other explicit hash>
Source: <tool/device/project name and version>
License / redistribution basis: <license or explicit permission>
Acquired: <YYYY-MM-DD>

Promotion command:

```sh
cargo run --example iop_inspect -- --strict \
  --active-mask <object-id-if-not-default> \
  --physical-soft-keys <count> \
  --navigation-soft-keys <count> \
  --write-report-json <report>.json \
  --expect-unsupported-records <count> \
  --expect-placeholder-pixels <count> \
  --expect-rgb888-fnv64 <hash> \
  --expect-rgb565-be-fnv64 <hash> \
  --expect-rgb565-le-fnv64 <hash> \
  tests/fixtures/isobus/<name>.iop
```

Inspector result:

- `rgb888_fnv64`: `<hash>`
- `rgb565_be_fnv64`: `<hash or null>`
- `rgb565_le_fnv64`: `<hash or null>`
- inspector JSON schema: `machbus-iop-inspect-report-v1`
- pool-buffer hash: `<hash from report>`
- layout profile: `<canvas / soft-key area / physical keys / nav keys / page>`
- unsupported records: `<count>`
- placeholder pixels: `<count>`
- raw framebuffer artifacts: `<paths or none>`
- inspector JSON artifacts: `artifacts.report_json`, `artifacts.rgb888`,
  `artifacts.rgb565_be`, `artifacts.rgb565_le`

Caveats and non-claims:

- <what this fixture does not prove>
````

For `vt_external_command_trace`, use `vt_trace_inspect` and record the starting
pool fixture id as well as the exact trace rows:

```sh
cargo run --example vt_trace_inspect -- --strict \
  --pool tests/fixtures/isobus/<name>.iop \
  --canvas <width>x<height> \
  --soft-key-area <x>,<y>,<w>,<h> \
  --physical-soft-keys <count> \
  --navigation-soft-keys <count> \
  --trace tests/fixtures/isobus/<trace>.hex \
  --write-report-json <trace-report>.json \
  --write-initial-rgb888 <trace-initial.rgb> \
  --write-final-rgb888 <trace-final.rgb> \
  --write-initial-rgb565-be <trace-initial-be.rgb565> \
  --write-initial-rgb565-le <trace-initial-le.rgb565> \
  --write-final-rgb565-be <trace-final-be.rgb565> \
  --write-final-rgb565-le <trace-final-le.rgb565> \
  --expect-accepted-effects <count> \
  --expect-initial-placeholder-pixels <count> \
  --expect-final-placeholder-pixels <count> \
  --expect-rgb888-fnv64 <final-hash> \
  --expect-rgb565-be-fnv64 <final-hash> \
  --expect-rgb565-le-fnv64 <final-hash>
```

If the starting pool is stored in a reviewable named `.hex` fixture, add
`--pool-fixture <name>` and cite that name in the report.
Trace reports should also cite `artifacts.initial_rgb888`,
`artifacts.final_rgb888`, `artifacts.initial_rgb565_be`,
`artifacts.initial_rgb565_le`, `artifacts.final_rgb565_be`, and
`artifacts.final_rgb565_le` when those dumps are requested.
