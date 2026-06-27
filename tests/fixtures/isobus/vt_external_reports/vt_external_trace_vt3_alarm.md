# VT3 external command-trace evidence

Requirement: vt_external_command_trace
Fixture path: tests/fixtures/isobus/vt_external_trace_vt3_alarm.hex
Fixture hash: 23c5d8e6e50502e51e5bf0bb7aabcb4a423897f563606bd541e4d5dfed6b99aa
Source: ISO11783-CAN-Stack `examples/virtual_terminal/version3_object_pool/main.cpp` callback path plus `examples/virtual_terminal/version3_object_pool/VT3TestPool.iop`, repository https://github.com/ad3154/ISO11783-CAN-Stack at commit `eb6b741b92b13efec7e95adc36212d163ff3a7fb`.
License / redistribution basis: MIT License from the ISO11783-CAN-Stack repository root.
Acquired: 2026-06-25

Starting pool:

- fixture path: `tests/fixtures/isobus/VT3TestPool.iop`
- fixture SHA-256: `c632fc5d73bb761596e4db826eeb6e77aab8d5fdfa135edc60f8b8579f504017`
- pool-buffer hash: `0xBB86201C4656A85E`
- starting active mask: `0x03E8` (`mainRunscreen_DataMask`)

Trace reduction:

- source callback: `handle_softkey_event()` in the external example calls
  `set_active_data_or_alarm_mask(example_WorkingSet, example_AlarmMask)` when
  `alarm_SoftKey` is released.
- external object IDs: `example_WorkingSet = 0x0000`,
  `example_AlarmMask = 0x07D0`, `alarm_SoftKey = 0x1388`.
- reduced trace row:
  `change_active_mask_to_alarm_mask=AD0000D007FFFFFF`.
- trace payload hash: `0x3AA1026CF16675B4`
- accepted-effect count: `1`
- accepted-effect: `ChangeActiveMask { mask: ObjectID(2000) }`

Promotion command:

```sh
make run EXAMPLE='vt_trace_inspect -- --strict --pool tests/fixtures/isobus/VT3TestPool.iop --physical-soft-keys 10 --navigation-soft-keys 2 --trace tests/fixtures/isobus/vt_external_trace_vt3_alarm.hex --write-report-json tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm.json --write-initial-rgb888 tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-initial.rgb --write-final-rgb888 tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-final.rgb --write-initial-rgb565-be tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-initial-be.rgb565 --write-initial-rgb565-le tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-initial-le.rgb565 --write-final-rgb565-be tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-final-be.rgb565 --write-final-rgb565-le tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-final-le.rgb565 --expect-accepted-effects 1 --expect-initial-placeholder-pixels 0 --expect-final-placeholder-pixels 0 --expect-rgb888-fnv64 0x69DF7BC191405D2D --expect-rgb565-be-fnv64 0xE9FE098586943290 --expect-rgb565-le-fnv64 0xCD9AD39A7FED720A'
```

Inspector result:

- inspector JSON schema: `machbus-vt-trace-inspect-report-v1`
- starting-pool static inspection schema: `machbus-iop-inspect-report-v1`
- `rgb888_fnv64`: initial `0x77683241FAAA1F7B`, final `0x69DF7BC191405D2D`
- `rgb565_be_fnv64`: initial `0xF13724DEE9229271`, final
  `0xE9FE098586943290`
- `rgb565_le_fnv64`: initial `0xF13724DEE9229271`, final
  `0xCD9AD39A7FED720A`
- layout profile: canvas `480x240`; soft-key area
  `x=480 y=0 width=64 height=240`; physical soft keys `10`; navigation soft
  keys `2`; soft-key page `0`
- unsupported records: `0` from the starting pool's `iop_inspect --strict`
  promotion
- placeholder pixels: initial `0`, final `0`
- command checks: `--expect-accepted-effects 1`,
  `--expect-initial-placeholder-pixels 0`,
  `--expect-final-placeholder-pixels 0`
- starting-pool static checks: `--expect-unsupported-records 0` and
  `--expect-placeholder-pixels 0`
- raw framebuffer artifacts:
  - initial RGB888:
    `tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-initial.rgb`
  - final RGB888:
    `tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-final.rgb`
  - initial RGB565 big-endian:
    `tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-initial-be.rgb565`
  - initial RGB565 little-endian:
    `tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-initial-le.rgb565`
  - final RGB565 big-endian:
    `tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-final-be.rgb565`
  - final RGB565 little-endian:
    `tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm-final-le.rgb565`
- raw artifact hashes:
  - JSON report SHA-256:
    `620e4cf196d9f5521a6fc48ae2d1d4772861640ab709901513e1b8f7df7dd2a2`
  - initial RGB888 SHA-256:
    `1361440d69379279514a3b1edfa0db16a19e0f404b1d8a35526349eb622e9022`
  - final RGB888 SHA-256:
    `b1525a498fd052a9df260171046e3f2221b1c0379783798bfb3cbdea1d8cbf4c`
  - initial RGB565 big-endian SHA-256:
    `aed3e7da26b7d56b9585dcbff75d684f64988d72b4178895ae846d1170e01955`
  - initial RGB565 little-endian SHA-256:
    `aed3e7da26b7d56b9585dcbff75d684f64988d72b4178895ae846d1170e01955`
  - final RGB565 big-endian SHA-256:
    `cac9e9b43f9439e94045314736d8f9f99038ef04c383d08b2caff04092a3cb28`
  - final RGB565 little-endian SHA-256:
    `06ba062b1d3ab270a334c09788364e9b05ada250e7914fb3d033d76b7a6d33e5`
- artifacts object: `artifacts.report_json`,
  `artifacts.initial_rgb888`, `artifacts.final_rgb888`,
  `artifacts.initial_rgb565_be`, `artifacts.initial_rgb565_le`,
  `artifacts.final_rgb565_be`, and `artifacts.final_rgb565_le` are all
  present in `tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm.json`.
- inspector JSON artifacts: `artifacts.report_json` and all initial/final
  RGB888/RGB565 artifact fields are present.
- report_json: `tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm.json`

Caveats and non-claims:

- This is an independently reduced one-command trace from open-source example
  code, not a raw CAN candump from a commercial VT or an AEF conformance trace.
- The trace proves server replay of a standard Change Active Mask command into
  the render runtime, including switching to an Alarm Mask and producing the
  same framebuffer hashes as the independently inspected alarm-mask view.
- This does not satisfy the separate soft-key-paging, input, or
  user-layout/window-mask external evidence rows.
- The strict/hash gates are renderer regression evidence, not AEF conformance
  certification or target-display visual equivalence.
