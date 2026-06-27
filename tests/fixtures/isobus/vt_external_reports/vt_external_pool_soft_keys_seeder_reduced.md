# Seeder reduced soft-key evidence

Requirement: vt_external_pool_soft_keys
Fixture path: tests/fixtures/isobus/vt_external_pool_soft_keys_seeder_reduced.hex
Fixture hash: 2ab1f3de29b6d7c2f51e02b5c71f5eed57132d7294feca2d7f07c7272720d4a7
Source: ISO11783-CAN-Stack seeder example object IDs and soft-key layout names from `examples/seeder_example/object_pool.hpp`, repository https://github.com/ad3154/ISO11783-CAN-Stack at commit `eb6b741b92b13efec7e95adc36212d163ff3a7fb`.
License / redistribution basis: MIT License from the ISO11783-CAN-Stack repository root.
Acquired: 2026-06-25

Promotion command:

```sh
make run EXAMPLE='iop_inspect -- --strict --physical-soft-keys 2 --navigation-soft-keys 0 --write-report-json tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced.json --write-rgb888 tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced.rgb --write-rgb565-be tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced-be.rgb565 --write-rgb565-le tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced-le.rgb565 --expect-unsupported-records 0 --expect-placeholder-pixels 0 --expect-rgb888-fnv64 0xC9794EC65F322925 --expect-rgb565-be-fnv64 0xB54639AA571C2725 --expect-rgb565-le-fnv64 0xB54639AA571C2725 tests/fixtures/isobus/vt_external_pool_soft_keys_seeder_reduced.hex vt_external_pool_soft_keys_seeder_reduced'
```

Inspector result:

- `rgb888_fnv64`: `0xC9794EC65F322925`
- `rgb565_be_fnv64`: `0xB54639AA571C2725`
- `rgb565_le_fnv64`: `0xB54639AA571C2725`
- inspector JSON schema: `machbus-iop-inspect-report-v1`
- pool-buffer hash: `0xF1BF252E5466E266`
- layout profile: canvas `480x240`; soft-key area `x=480 y=0 width=64 height=240`; physical soft keys `2`; navigation soft keys `0`; soft-key page `0`
- unsupported records: `0`
- placeholder pixels: `0`
- soft-key evidence: four Key objects are present, with two visible physical cells on page 0.
- raw framebuffer artifacts:
  - RGB888: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced.rgb`
  - RGB565 big-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced-be.rgb565`
  - RGB565 little-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced-le.rgb565`
- raw artifact hashes:
  - JSON report SHA-256: `d4e8f40fc94b6a140f8900ab3cac0958a22c267dfa19d5ce7fcf780821809fb4`
  - RGB888 SHA-256: `3e8106a6e51de0b94d80ad24ca02e8abff9a427cc040c3a5ea347141cf80d43f`
  - RGB565 big-endian SHA-256: `5b51073f25fed6bd6b2d434a6ab303efa9b355324742feed191c0c306bd89940`
  - RGB565 little-endian SHA-256: `5b51073f25fed6bd6b2d434a6ab303efa9b355324742feed191c0c306bd89940`
- inspector JSON artifacts: `artifacts.report_json`, `artifacts.rgb888`,
  `artifacts.rgb565_be`, and `artifacts.rgb565_le` are all present in
  `tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced.json`.
- report_json: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_soft_keys_seeder_reduced.json`

Caveats and non-claims:

- This is a reviewable reduction from open-source seeder object-pool IDs, not the full external `BasePool.iop`.
- It proves soft-key paging/profile handling for more keys than physical cells; it does not prove every seeder screen.
- The strict/hash gates are renderer regression evidence, not AEF conformance certification.
