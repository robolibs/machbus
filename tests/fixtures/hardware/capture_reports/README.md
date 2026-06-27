# Hardware capture reports

This directory is intentionally empty until a real `vcan` or physical-bus
capture is reduced into a repository fixture.

For each completed hardware evidence row in
`tests/fixtures/hardware/capture_requirements.txt`, add a short Markdown report
here. Use `tests/fixtures/hardware/capture_playbook.txt` as the run plan for
the matching requirement, then document the actual capture here with:

- machbus commit;
- interface and bitrate;
- exact capture command copied from the matching capture playbook row;
- peer tool/device and version;
- reduced trace id and path;
- exact behavior proven;
- caveats and non-claims.

Do not mark a requirement `complete` without both a reduced trace fixture and a
report. Completed reports must replace every placeholder in this template and
must use the exact `Capture command:` from the matching
`capture_playbook.txt` row.

Copy this shape for every completed row:

```md
# <short title>

Requirement: <capture_requirements id>
Trace id: <trace manifest id>
Trace path: tests/fixtures/traces/<name>.candump

Machbus commit: <git sha>
Interface: <vcan0/can0/etc>
Bitrate: <250000 or n/a for vcan>
Capture command: <exact command from capture_playbook.txt>
Peer/tool: <tool, device, firmware/software version>

Behavior proven:

- <specific address/PGN/state transition/response observed>

Caveats and non-claims:

- <what this capture does not prove>
```
