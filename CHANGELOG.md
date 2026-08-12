# Changelog

## [0.1.7] - 2026-08-12

### <!-- 0 -->⛰️  Features

- Type the guidance and speed facilities
- Add an autodrive tab to machbus live
- Drive machbus drive with autodrive
- Stop autonomy when the key switch goes off
- Expose autodrive to the python bindings
- Expose autodrive to the c abi
- Release assignments on client status timeout
- Enforce iso 11783-9 safe mode on guarded commands
- Add server status byte 4 states
- Gate function assignment on mutual authentication
- Add the guidance curvature slot, gate 0xFBFE per function
- Acknowledge filter database commands
- Nack addressed requests for unsupported pgns
- Add microsecond tick entry points
- Expose stop reason and clear_stop to c and python
- Expose the stop latch in the embedded profile
- Send working set maintenance, fix status busy codes
- Stop autonomy on stale or unusable fix
- Reject two authors of one command pgn
- Implement AEF 023 TIM protocol layer
- Add AutoDrive steering and speed layer
- Add autonomy safety supervisor and cadence
- Better messages
- Better messages

### <!-- 1 -->🐛 Bug Fixes

- Reserved sequence id costs binding not data
- Send each pgn at its default priority
- Expose speed prefix decode to python
- Set process data priority per command value
- Skip unknown functionalities on decode
- Ignore reserved bits on volume requests
- Zero reserved bits in facility request
- Gate driving on tractor facilities
- Survive cramped terminals in every tui
- Clip key hints so a small terminal cannot panic
- Make space a held dead-man on the keyboard
- Treat a lost gamepad as a released dead-man
- Scope crl, arm watchdog and drop legacy table
- Ignore reserved bits and keep signals distinct
- Widen option masks and count macro groupings
- Correct lamp bits, nack scope and padding
- Honour name bits, reserved bytes and addressing
- Accept client task and correct ddi metadata
- Decode entries, statuses and errors per annex c
- Ignore reserved bits and transmit them as ones
- Answer, decode and recover per annex f
- Report a refused clear_stop to callers
- Decode quality, altitude and station type
- Stop dropping stop, heartbeat and power frames
- Read get memory size at the annex d.2 offset
- Keep sequences alive across abort and completion
- Gate tim-auth and give g8 real teeth
- Classify ddis from the dictionary, not ranges
- One layout per pgn and class 3 front gate
- Scale enu by latitude and gate on integrity
- Stop transport sessions on a surrendered address
- Extend request timeout while the server is busy
- Normalize dot path segments per a.2.3.1
- Use the annex b.15 attribute bits
- Seek with position mode and range checks
- Two-byte path lengths and c.2.2.3 cwd response
- Decode properties per c.1.5 and accept any version
- Renumber commands and fix ccm per annex c
- Renumber error codes to annex b.9
- Leave abort once every client confirms
- Encode inactive status sentinels per annex f
- Watchdog client and master status reception
- Encode timelog dlv count and ordering bytes
- Enforce the annex a ddop object hierarchy
- Accept utf-8 ddop text within the 128-byte limit
- Correct status payloads and send client task
- Decode 16-bit macro refs and declare our own version
- Stage multi-session pool uploads and report why
- Answer every annex f command with a response
- Handle working set maintenance and watchdog it
- Report mask ids and version errors in the right bytes
- Split cmac keys by role and classify heartbeats
- Take the ecdh key from the device certificate
- Verify certificate chain signatures
- Keep a faulted curvature sensor distinguishable
- Arm iso 11783-9 safe mode from real faults
- Refuse clear_stop while a gnss hazard is live
- Send the periodic client status cadence
- Never put the reserved 0xfbfe raw on the wire
- Band the 0xad00 curvature instead of dropping it
- Treat undefined bits as don't care on receive
- Keep the pto pg when no pto is fitted
- Keep the hitch pg when a sensor is absent
- Decode 129029 dop fields as signed
- Accept vt6 tan bytes on activations and touches
- Share the isb guard with autodrive
- Drop the two stop triggers with no producer
- Keep dash, position, fuel and aftertreatment pgs
- Keep eec2, eec3 and tsc1 on absent parameters
- Keep vep1 and ambient pgs on absent sensors
- Keep fluid, hours and economy pgs on absent fields
- Report absent temperatures instead of dropping et1/et2
- Keep the eec1 frame when a parameter is absent
- Stop the tecu transmitting the implement maintain-power pg
- Throttle the address-violation response per source
- Delay the cannot-claim response by rtxd
- Scope sessions by pgn and dt path per 5.10.4.2
- Stop on an operator-limited guidance status
- Keep the machine info when curvature is absent
- Refuse distance_to on embedded rather than lie
- Accept reserved fmi codes instead of dropping the dm1
- Announce the transition to no active dtcs
- Block address claims at a router, fix mfdb response
- Keep authority alive while the peer is talking
- Give the hitch slot its annex d5 value table
- Reject non-contributory keys, derive per-direction keys
- Require addressed commands, refuse wildcard filter
- Translate destination independently, narrow claim contest
- Honour reserved-bit encoding and keep partial fixes
- Decode gnss dops as signed, ignore reserved bits
- Serialize ddop at the negotiated version
- Accept annex b pool transfer and activate responses
- Cap macro chain depth to stop stack overflow
- Read get memory status from the status byte
- Stop 129025 inventing a gnss fix quality
- Carry sub-ms residue in every plugin watchdog
- Command at group rate while engaged
- Honour held ISB, its loss, and live preconditions
- Refuse unencodable curvature and speed
- Make wire curvature right-positive per AEF D.7.2.1
- Enter fault state on sender error and shutdown
- Issue n cleanup
- Surface guidance refusals across bindings
- Correct part 6/12/14 conformance defects
- Correct part 2/3/5 conformance defects
- Align with ISO 11783-4 tables 2 and 4
- Align DDOP records with ISO 11783-10 annex A
- Decode real speed frames per ISO 11783-7/9
- Drop fabricated guidance PGNs, guard collisions
- Carry sub-ms time and surface send failures

### <!-- 2 -->🚜 Refactor

- Remove the guidance plugin, superseded by autodrive
- Move guidance examples onto autodrive

### <!-- 3 -->📚 Documentation

- Record the standards text audit
- Cite annex j for auxiliary, not part 11
- Correct the TIM and facilities answer
- Drop the deleted legacy file-transfer types
- Add a runnable autodrive driving example
- Document the drive tool safety model
- Retire autosteer as a separate concept
- Explain autodrive, tim scope and the guidance split
- Fix coverage ledgers citing a missing file

### <!-- 5 -->🎨 Styling

- Use is_multiple_of for entry length check

### <!-- 6 -->🧪 Testing

- Drive the keyboard input path end to end
- Drive the c3 clear-stop guard through session
- Make the g8 producer check able to fail
- Address control commands per part 4
- Correct fixtures that encoded the defects

## [0.1.6] - 2026-06-30

### <!-- 0 -->⛰️  Features

- Changes

## [0.1.5] - 2026-06-30

### <!-- 0 -->⛰️  Features

- Changes

## [0.1.4] - 2026-06-30

### <!-- 0 -->⛰️  Features

- Changes
- Autosteer fix
- Autosteer fix

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Remove CI workflow for project cleanup

## [0.1.3] - 2026-06-29

### <!-- 1 -->🐛 Bug Fixes

- Allow rustdoc broken/private intra-doc links

## [0.1.0] - 2026-06-28

### <!-- 0 -->⛰️  Features

- Terminal virtual terminal :P
- Terminal virtual terminal :P
- Created a simple TOOL
- OPENSOURCEed

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Reset version to 0.0.1 and clean changelog

# Changelog
