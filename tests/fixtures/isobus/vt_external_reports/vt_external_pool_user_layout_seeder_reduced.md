# Seeder reduced user-layout evidence

Requirement: vt_external_pool_user_layout
Fixture path: tests/fixtures/isobus/vt_external_pool_user_layout_seeder_reduced.hex
Fixture hash: b2c2252536940b7a5099f692e786bd369b10b755825a758b8178774f3887fc42
Source: ISO11783-CAN-Stack seeder example Window Mask and Key Group object families from `examples/seeder_example/object_pool.hpp`, repository https://github.com/ad3154/ISO11783-CAN-Stack at commit `eb6b741b92b13efec7e95adc36212d163ff3a7fb`.
License / redistribution basis: MIT License from the ISO11783-CAN-Stack repository root.
Acquired: 2026-06-25

Promotion command:

```sh
make run EXAMPLE='iop_inspect -- --strict --physical-soft-keys 10 --navigation-soft-keys 2 --write-report-json tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced.json --write-rgb888 tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced.rgb --write-rgb565-be tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced-be.rgb565 --write-rgb565-le tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced-le.rgb565 --expect-unsupported-records 0 --expect-placeholder-pixels 0 --expect-rgb888-fnv64 0xE1AD2C71FAED1EAD --expect-rgb565-be-fnv64 0xD705E807CE742D75 --expect-rgb565-le-fnv64 0xD705E807CE742D75 tests/fixtures/isobus/vt_external_pool_user_layout_seeder_reduced.hex vt_external_pool_user_layout_seeder_reduced'
```

Inspector result:

- `rgb888_fnv64`: `0xE1AD2C71FAED1EAD`
- `rgb565_be_fnv64`: `0xD705E807CE742D75`
- `rgb565_le_fnv64`: `0xD705E807CE742D75`
- inspector JSON schema: `machbus-iop-inspect-report-v1`
- pool-buffer hash: `0x775C2167A74D3023`
- layout profile: canvas `480x240`; soft-key area `x=480 y=0 width=64 height=240`; physical soft keys `10`; navigation soft keys `2`; soft-key page `0`
- unsupported records: `0`
- placeholder pixels: `0`
- user-layout evidence: one free-form Window Mask with an Output String child and one Key Group with two Key children are placed on the active Data Mask.
- placement mode: free-form window cells plus key-group placement.
- raw framebuffer artifacts:
  - RGB888: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced.rgb`
  - RGB565 big-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced-be.rgb565`
  - RGB565 little-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced-le.rgb565`
- raw artifact hashes:
  - JSON report SHA-256: `f24c56a444b0730475fdfc238fc604741462b0d7fb7b69543ab5ca226faf537a`
  - RGB888 SHA-256: `b8115be99e49679f16479eaf1f6d42e308c1f19886450a004e292fec4dca0d32`
  - RGB565 big-endian SHA-256: `d37b3432527c82e75de95a98ba3d3e9beb8175044af629302651336ff08a34a9`
  - RGB565 little-endian SHA-256: `d37b3432527c82e75de95a98ba3d3e9beb8175044af629302651336ff08a34a9`
- inspector JSON artifacts: `artifacts.report_json`, `artifacts.rgb888`,
  `artifacts.rgb565_be`, and `artifacts.rgb565_le` are all present in
  `tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced.json`.
- report_json: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_user_layout_seeder_reduced.json`

Caveats and non-claims:

- This is a reviewable reduction from open-source seeder object-pool IDs, not the full external `BasePool.iop`.
- It proves free-form Window Mask and Key Group placement evidence; it does not prove every VT-formatted window type.
- The strict/hash gates are renderer regression evidence, not AEF conformance certification.
