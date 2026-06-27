# Seeder reduced input evidence

Requirement: vt_external_pool_inputs
Fixture path: tests/fixtures/isobus/vt_external_pool_inputs_seeder_reduced.hex
Fixture hash: 1e38772302c42b68e374aa244b5fa03a28625479055927d578d9dfc5125571e2
Source: ISO11783-CAN-Stack seeder example input object IDs from `examples/seeder_example/object_pool.hpp`, repository https://github.com/ad3154/ISO11783-CAN-Stack at commit `eb6b741b92b13efec7e95adc36212d163ff3a7fb`.
License / redistribution basis: MIT License from the ISO11783-CAN-Stack repository root.
Acquired: 2026-06-25

Promotion command:

```sh
make run EXAMPLE='iop_inspect -- --strict --physical-soft-keys 10 --navigation-soft-keys 2 --write-report-json tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced.json --write-rgb888 tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced.rgb --write-rgb565-be tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced-be.rgb565 --write-rgb565-le tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced-le.rgb565 --expect-unsupported-records 0 --expect-placeholder-pixels 0 --expect-rgb888-fnv64 0x267C5E064882C6A5 --expect-rgb565-be-fnv64 0x9C0C90ED59863131 --expect-rgb565-le-fnv64 0x7D995AB737127369 tests/fixtures/isobus/vt_external_pool_inputs_seeder_reduced.hex vt_external_pool_inputs_seeder_reduced'
```

Inspector result:

- `rgb888_fnv64`: `0x267C5E064882C6A5`
- `rgb565_be_fnv64`: `0x9C0C90ED59863131`
- `rgb565_le_fnv64`: `0x7D995AB737127369`
- inspector JSON schema: `machbus-iop-inspect-report-v1`
- pool-buffer hash: `0x5923FB9AC0EAB555`
- layout profile: canvas `480x240`; soft-key area `x=480 y=0 width=64 height=240`; physical soft keys `10`; navigation soft keys `2`; soft-key page `0`
- unsupported records: `0`
- placeholder pixels: `0`
- input evidence: visible `InputBoolean` and enabled `InputList` nodes render with backing `NumberVariable` values and selectable `OutputString` items.
- selected/open input behavior: this fixture is static render evidence; operator select/open/commit behavior is covered by the runtime tests, not by this static pool promotion.
- raw framebuffer artifacts:
  - RGB888: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced.rgb`
  - RGB565 big-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced-be.rgb565`
  - RGB565 little-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced-le.rgb565`
- raw artifact hashes:
  - JSON report SHA-256: `857eec789359565b27874f0c860129c6d2541fa9234523fd5ea98b5862aca7d8`
  - RGB888 SHA-256: `1ce01a290bb5d79f279d24914c9537cc41c2bb0eef7036371d1ef54cef02d8fa`
  - RGB565 big-endian SHA-256: `2a589ae1f2fa2a6328223ff195a29c9244bec633dca49139f6f231e1d79c0eb2`
  - RGB565 little-endian SHA-256: `2a589ae1f2fa2a6328223ff195a29c9244bec633dca49139f6f231e1d79c0eb2`
- inspector JSON artifacts: `artifacts.report_json`, `artifacts.rgb888`,
  `artifacts.rgb565_be`, and `artifacts.rgb565_le` are all present in
  `tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced.json`.
- report_json: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_inputs_seeder_reduced.json`

Caveats and non-claims:

- This is a reviewable reduction from open-source seeder object-pool IDs, not the full external `BasePool.iop`.
- It proves input object parsing/layout/rendering evidence for one Input Boolean and one Input List; it does not prove all input object variants.
- The strict/hash gates are renderer regression evidence, not AEF conformance certification.
