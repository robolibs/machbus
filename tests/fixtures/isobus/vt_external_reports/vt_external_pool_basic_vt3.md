# VT3 external object-pool smoke evidence

Requirement: vt_external_pool_basic
Fixture path: tests/fixtures/isobus/VT3TestPool.iop
Fixture hash: c632fc5d73bb761596e4db826eeb6e77aab8d5fdfa135edc60f8b8579f504017
Source: ISO11783-CAN-Stack `examples/virtual_terminal/version3_object_pool/VT3TestPool.iop`, repository https://github.com/ad3154/ISO11783-CAN-Stack at commit `eb6b741b92b13efec7e95adc36212d163ff3a7fb`.
License / redistribution basis: MIT License from the ISO11783-CAN-Stack repository root.
Acquired: 2026-06-25

Promotion command:

```sh
make run EXAMPLE='iop_inspect -- --strict --physical-soft-keys 10 --navigation-soft-keys 2 --write-report-json tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3.json --write-rgb888 tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3.rgb --write-rgb565-be tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3-be.rgb565 --write-rgb565-le tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3-le.rgb565 --expect-unsupported-records 0 --expect-placeholder-pixels 0 --expect-rgb888-fnv64 0x77683241FAAA1F7B --expect-rgb565-be-fnv64 0xF13724DEE9229271 --expect-rgb565-le-fnv64 0xF13724DEE9229271 tests/fixtures/isobus/VT3TestPool.iop'
```

Inspector result:

- `rgb888_fnv64`: `0x77683241FAAA1F7B`
- `rgb565_be_fnv64`: `0xF13724DEE9229271`
- `rgb565_le_fnv64`: `0xF13724DEE9229271`
- inspector JSON schema: `machbus-iop-inspect-report-v1`
- pool-buffer hash: `0xBB86201C4656A85E`
- layout profile: canvas `480x240`; soft-key area `x=480 y=0 width=64 height=240`; physical soft keys `10`; navigation soft keys `2`; soft-key page `0`
- unsupported records: `0`
- placeholder pixels: `0`
- pool objects: `34`
- active mask: `0x03E8`
- GTUI commands: `19`
- raw framebuffer artifacts:
  - RGB888: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3.rgb`
  - RGB565 big-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3-be.rgb565`
  - RGB565 little-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3-le.rgb565`
- raw artifact hashes:
  - JSON report SHA-256: `0ce933fdffd315580c6bffc4415d3765e6776633ed3471644aa74284578fa2a9`
  - RGB888 SHA-256: `1361440d69379279514a3b1edfa0db16a19e0f404b1d8a35526349eb622e9022`
  - RGB565 big-endian SHA-256: `aed3e7da26b7d56b9585dcbff75d684f64988d72b4178895ae846d1170e01955`
  - RGB565 little-endian SHA-256: `aed3e7da26b7d56b9585dcbff75d684f64988d72b4178895ae846d1170e01955`
- inspector JSON artifacts: `artifacts.report_json`, `artifacts.rgb888`,
  `artifacts.rgb565_be`, and `artifacts.rgb565_le` are all present in
  `tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3.json`.

Caveats and non-claims:

- This fixture promotes the first external basic object-pool evidence row only:
  it proves that this VT3 pool parses, lowers, and renders deterministically
  through the current hosted renderer.
- This default-mask basic evidence does not itself prove the Phase 10 graphics,
  soft-key-paging, input, user-layout/window-mask, or command-trace evidence
  categories; graphics evidence for the same pool is recorded separately by
  `vt_external_pool_graphics_vt3_alarm.md`.
- The promotion used `--strict` plus zero unsupported-record and placeholder
  gates, but it is not an AEF conformance result and is not a target-display
  visual comparison.
- The source path and MIT license were checked against the ISO11783-CAN-Stack
  checkout listed above. This is still not an AEF conformance result.
