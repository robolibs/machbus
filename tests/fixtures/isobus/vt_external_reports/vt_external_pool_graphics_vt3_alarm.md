# VT3 external graphics alarm-mask evidence

Requirement: vt_external_pool_graphics
Fixture path: tests/fixtures/isobus/VT3TestPool.iop
Fixture hash: c632fc5d73bb761596e4db826eeb6e77aab8d5fdfa135edc60f8b8579f504017
Source: ISO11783-CAN-Stack `examples/virtual_terminal/version3_object_pool/VT3TestPool.iop`, repository https://github.com/ad3154/ISO11783-CAN-Stack at commit `eb6b741b92b13efec7e95adc36212d163ff3a7fb`.
License / redistribution basis: MIT License from the ISO11783-CAN-Stack repository root.
Acquired: 2026-06-25

Promotion command:

```sh
make run EXAMPLE='iop_inspect -- --strict --active-mask 0x07D0 --physical-soft-keys 10 --navigation-soft-keys 2 --write-report-json tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm.json --write-rgb888 tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm.rgb --write-rgb565-be tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm-be.rgb565 --write-rgb565-le tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm-le.rgb565 --expect-unsupported-records 0 --expect-placeholder-pixels 0 --expect-rgb888-fnv64 0x69DF7BC191405D2D --expect-rgb565-be-fnv64 0xE9FE098586943290 --expect-rgb565-le-fnv64 0xCD9AD39A7FED720A tests/fixtures/isobus/VT3TestPool.iop'
```

Inspector result:

- `rgb888_fnv64`: `0x69DF7BC191405D2D`
- `rgb565_be_fnv64`: `0xE9FE098586943290`
- `rgb565_le_fnv64`: `0xCD9AD39A7FED720A`
- inspector JSON schema: `machbus-iop-inspect-report-v1`
- pool-buffer hash: `0xBB86201C4656A85E`
- active mask: `0x07D0` (`example_AlarmMask`)
- layout profile: canvas `480x240`; soft-key area `x=480 y=0 width=64 height=240`; physical soft keys `10`; navigation soft keys `2`; soft-key page `0`
- unsupported records: `0`
- placeholder pixels: `0`
- pool objects: `34`
- rendered graphics evidence: one `PictureGraphic` node is visible on the alarm mask; the GTUI preview emits an `Image` command at `x=0 y=80 width=480 height=247` from a `296x247` indexed image payload.
- GTUI commands: `9`
- raw framebuffer artifacts:
  - RGB888: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm.rgb`
  - RGB565 big-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm-be.rgb565`
  - RGB565 little-endian: `tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm-le.rgb565`
- raw artifact hashes:
  - JSON report SHA-256: `7a18307370ff9cfadaa1624f8a04c8fc5df8418fcdfc44de6a59fa4b04056208`
  - RGB888 SHA-256: `b1525a498fd052a9df260171046e3f2221b1c0379783798bfb3cbdea1d8cbf4c`
  - RGB565 big-endian SHA-256: `cac9e9b43f9439e94045314736d8f9f99038ef04c383d08b2caff04092a3cb28`
  - RGB565 little-endian SHA-256: `06ba062b1d3ab270a334c09788364e9b05ada250e7914fb3d033d76b7a6d33e5`
- inspector JSON artifacts: `artifacts.report_json`, `artifacts.rgb888`,
  `artifacts.rgb565_be`, and `artifacts.rgb565_le` are all present in
  `tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm.json`.

Caveats and non-claims:

- This fixture promotes the external graphics row by selecting the pool's alarm
  mask with `--active-mask 0x07D0` so the `PictureGraphic` is actually visible.
- This fixture proves one indexed PictureGraphic path only. It does not prove
  PNG-backed Graphic Data, Scaled Graphic, Animation, or Graphics Context
  fidelity.
- This fixture does not satisfy the still-missing soft-key-paging, input,
  user-layout/window-mask, or command-trace evidence categories.
- The strict/hash gates are renderer regression evidence, not AEF conformance
  certification or target-display visual equivalence.
