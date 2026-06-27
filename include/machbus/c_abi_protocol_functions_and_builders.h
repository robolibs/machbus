/**
 * Encode an ECU Identification from five (J1939) field strings plus an optional
 * hardware id (`hardware_id` may be `NULL` for the five-field J1939 form) into
 * `out` (cap bytes). Returns the full encoded length; if it exceeds `cap`
 * nothing is copied. Returns 0 on validation/null failure.
 */
uintptr_t machbus_ecu_identification_encode(const char *part_number,
                                            const char *serial_number,
                                            const char *location,
                                            const char *ecu_type,
                                            const char *manufacturer,
                                            const char *hardware_id,
                                            uint8_t *out,
                                            uintptr_t cap);

bool machbus_j1939_acknowledgment_decode(const uint8_t *data,
                                         uintptr_t len,
                                         MachbusAcknowledgment *out);

bool machbus_j1939_acknowledgment_encode(const MachbusAcknowledgment *input, uint8_t *out);

/**
 * Decode an 8-byte Language Command payload into `out`.
 */
bool machbus_j1939_language_data_decode(const uint8_t *data,
                                        uintptr_t len,
                                        MachbusLanguageData *out);

/**
 * Encode a LanguageData into the caller's 8-byte buffer `out`.
 */
bool machbus_j1939_language_data_encode(const MachbusLanguageData *input, uint8_t *out);

bool machbus_j1939_maintain_power_decode(const uint8_t *data,
                                         uintptr_t len,
                                         MachbusMaintainPowerData *out);

bool machbus_j1939_maintain_power_encode(const MachbusMaintainPowerData *input, uint8_t *out);

bool machbus_j1939_speed_and_distance_decode(const uint8_t *data,
                                             uintptr_t len,
                                             MachbusSpeedAndDistance *out);

bool machbus_j1939_speed_and_distance_encode(const MachbusSpeedAndDistance *input, uint8_t *out);

bool machbus_j1939_etc1_decode(const uint8_t *data, uintptr_t len, MachbusEtc1 *out);

bool machbus_j1939_etc1_encode(const MachbusEtc1 *input, uint8_t *out);

bool machbus_j1939_transmission_oil_temp_decode(const uint8_t *data,
                                                uintptr_t len,
                                                MachbusTransmissionOilTemp *out);

bool machbus_j1939_transmission_oil_temp_encode(const MachbusTransmissionOilTemp *input,
                                                uint8_t *out);

bool machbus_j1939_cruise_control_decode(const uint8_t *data,
                                         uintptr_t len,
                                         MachbusCruiseControl *out);

bool machbus_j1939_cruise_control_encode(const MachbusCruiseControl *input, uint8_t *out);

/**
 * Decode an 8-byte Shortcut Button payload into `out`.
 */
bool machbus_j1939_shortcut_button_decode(const uint8_t *data,
                                          uintptr_t len,
                                          MachbusShortcutButtonMessage *out);

/**
 * Encode a Shortcut Button message (state byte + transition count) into the
 * caller's 8-byte buffer `out`.
 */
bool machbus_j1939_shortcut_button_encode(uint8_t state, uint8_t transition_count, uint8_t *out);

/**
 * Decode an 8-byte Time/Date payload into `out`.
 */
bool machbus_j1939_time_date_decode(const uint8_t *data, uintptr_t len, MachbusTimeDate *out);

/**
 * Encode a TimeDate into the caller's 8-byte buffer `out`.
 */
bool machbus_j1939_time_date_encode(const MachbusTimeDate *input, uint8_t *out);

/**
 * Decode a Request2 payload. Returns an owned handle (free with
 * [`machbus_request2_msg_free`]) or `NULL` on failure.
 */
MachbusRequest2Msg *machbus_request2_msg_decode(const uint8_t *data, uintptr_t len);

/**
 * Requested PGN of a Request2 handle, or 0 if null.
 */
uint32_t machbus_request2_msg_requested_pgn(const MachbusRequest2Msg *h);

/**
 * Whether a Request2 handle asks the responder to reply via the Transfer PGN.
 */
bool machbus_request2_msg_use_transfer(const MachbusRequest2Msg *h);

/**
 * Copy the extended-id bytes (0..=3) into `out` (cap bytes). Returns the full
 * length; if it exceeds `cap` nothing is copied.
 */
uintptr_t machbus_request2_msg_extended_id_into(const MachbusRequest2Msg *h,
                                                uint8_t *out,
                                                uintptr_t cap);

/**
 * Free a Request2 handle. Accepts `NULL`.
 */
void machbus_request2_msg_free(MachbusRequest2Msg *h);

/**
 * Encode a Request2 message (requested PGN, up to 3 extended-id bytes, and the
 * use-transfer flag) into the caller's 8-byte buffer `out`. Returns false on
 * invalid PGN / overlong extended id / null pointers.
 */
bool machbus_request2_msg_encode(uint32_t requested_pgn,
                                 const uint8_t *extended_id,
                                 uintptr_t extended_id_len,
                                 bool use_transfer,
                                 uint8_t *out);

/**
 * Decode a Transfer payload. Returns an owned handle (free with
 * [`machbus_transfer_msg_free`]) or `NULL` on failure.
 */
MachbusTransferMsg *machbus_transfer_msg_decode(const uint8_t *data, uintptr_t len);

/**
 * Original PGN carried by a Transfer handle, or 0 if null.
 */
uint32_t machbus_transfer_msg_original_pgn(const MachbusTransferMsg *h);

/**
 * Copy the Transfer data bytes into `out` (cap bytes). Returns the full data
 * length; if it exceeds `cap` nothing is copied.
 */
uintptr_t machbus_transfer_msg_data_into(const MachbusTransferMsg *h, uint8_t *out, uintptr_t cap);

/**
 * Free a Transfer handle. Accepts `NULL`.
 */
void machbus_transfer_msg_free(MachbusTransferMsg *h);

/**
 * Encode a Transfer message (original PGN + response data) into `out` (cap
 * bytes). Returns the full encoded length (`3 + data_len`); if it exceeds `cap`
 * nothing is copied. Returns 0 on invalid PGN / null.
 */
uintptr_t machbus_transfer_msg_encode(uint32_t original_pgn,
                                      const uint8_t *data,
                                      uintptr_t data_len,
                                      uint8_t *out,
                                      uintptr_t cap);

/**
 * Build a PGN 129025 (position rapid update) payload from a WGS84 fix.
 * Writes 8 bytes into `out` (must hold at least 8). Returns `false` on null.
 */
bool machbus_nmea_build_position(double latitude_deg, double longitude_deg, uint8_t *out);

/**
 * Build a PGN 129026 (COG / SOG rapid update) payload.
 * `cog_rad` is course-over-ground in radians, `sog_mps` is speed in m/s.
 * Writes 8 bytes into `out` (must hold at least 8). Returns `false` on null.
 */
bool machbus_nmea_build_cog_sog(double cog_rad, double sog_mps, uint8_t *out);

/**
 * Build a PGN 127250 (vessel heading) payload. All angles are in radians.
 * Writes 8 bytes into `out` (must hold at least 8). Returns `false` on null.
 */
bool machbus_nmea_build_heading(double heading_rad,
                                double deviation_rad,
                                double variation_rad,
                                uint8_t *out);

bool machbus_j1939_heartbeat_request_decode(const uint8_t *data,
                                            uintptr_t len,
                                            MachbusHeartbeatRequest *out);

bool machbus_j1939_heartbeat_request_encode(const MachbusHeartbeatRequest *input, uint8_t *out);

/**
 * Last cached AUX-O status for `(source, function_number)`. Returns true and
 * fills `out` if present. Requires the auxiliary subsystem.
 */
bool machbus_session_auxiliary_last_aux_o(const MachbusSession *h,
                                          uint8_t source,
                                          uint8_t function_number,
                                          MachbusAuxFunction *out);

/**
 * Last cached AUX-N status for `(source, function_number)`. Requires aux.
 */
bool machbus_session_auxiliary_last_aux_n(const MachbusSession *h,
                                          uint8_t source,
                                          uint8_t function_number,
                                          MachbusAuxFunction *out);

/**
 * Queue a local AUX-O status broadcast (flushed on the next tick). Requires aux.
 */
bool machbus_session_auxiliary_broadcast_aux_o(MachbusSession *h, MachbusAuxFunction function);

/**
 * Queue a local AUX-N type-2 status broadcast (flushed on tick). Requires aux.
 */
bool machbus_session_auxiliary_broadcast_aux_n(MachbusSession *h, MachbusAuxFunction function);

/**
 * Request peer ECU identification (DM-style request). Requires DM memory.
 */
bool machbus_session_dm_memory_request_ecu_identification(MachbusSession *h, uint8_t destination);

/**
 * Request peer software identification. Requires DM memory.
 */
bool machbus_session_dm_memory_request_software_identification(MachbusSession *h,
                                                               uint8_t destination);

/**
 * Send a DM14 memory-access request to `destination`. Requires DM memory.
 */
bool machbus_session_dm_memory_send_dm14(MachbusSession *h,
                                         uint8_t destination,
                                         const MachbusDm14Request *request);

/**
 * Last received DM14 request `(source, request)`. Requires DM memory.
 */
bool machbus_session_dm_memory_last_dm14(const MachbusSession *h,
                                         uint8_t *out_source,
                                         MachbusDm14Request *out);

/**
 * Last received DM15 response `(source, response)`. Requires DM memory.
 */
bool machbus_session_dm_memory_last_dm15(const MachbusSession *h,
                                         uint8_t *out_source,
                                         MachbusDm15Response *out);

/**
 * Connect the file-client to a file-server address. Requires FS client.
 */
bool machbus_session_fs_client_connect_to(MachbusSession *h, uint8_t server);

/**
 * Disconnect the file-client. Requires FS client.
 */
bool machbus_session_fs_client_disconnect(MachbusSession *h);

/**
 * Whether the file-client is connected.
 */
bool machbus_session_fs_client_is_connected(const MachbusSession *h);

/**
 * Open a file by `path` with `flags`. On success writes the request TAN into
 * `out_tan` and returns true; the response arrives later as an FS event keyed
 * by that TAN. Requires FS client.
 */
bool machbus_session_fs_client_open(MachbusSession *h,
                                    const char *path,
                                    uint8_t flags,
                                    uint8_t *out_tan);

/**
 * Close an open file handle. Writes the request TAN into `out_tan`. Requires
 * FS client.
 */
bool machbus_session_fs_client_close(MachbusSession *h, uint8_t file_handle, uint8_t *out_tan);

/**
 * Read `count` bytes from an open file handle. Writes the request TAN into
 * `out_tan`; the data arrives later as an FS read-response event. Requires FS
 * client.
 */
bool machbus_session_fs_client_read(MachbusSession *h,
                                    uint8_t file_handle,
                                    uint16_t count,
                                    uint8_t *out_tan);

/**
 * Write `len` bytes to an open file handle. Writes the request TAN into
 * `out_tan`. Requires FS client.
 */
bool machbus_session_fs_client_write(MachbusSession *h,
                                     uint8_t file_handle,
                                     const uint8_t *data,
                                     uintptr_t len,
                                     uint8_t *out_tan);

/**
 * Seek an open file handle to `position`. Writes the request TAN into
 * `out_tan`. Requires FS client.
 */
bool machbus_session_fs_client_seek(MachbusSession *h,
                                    uint8_t file_handle,
                                    uint32_t position,
                                    uint8_t *out_tan);

/**
 * Request the server's current directory. Writes the request TAN into
 * `out_tan`. Requires FS client.
 */
bool machbus_session_fs_client_current_directory(MachbusSession *h, uint8_t *out_tan);

/**
 * Change the server's current directory. Writes the request TAN into
 * `out_tan`. Requires FS client.
 */
bool machbus_session_fs_client_change_directory(MachbusSession *h,
                                                const char *path,
                                                uint8_t *out_tan);

/**
 * Delete a file by `path`. Writes the request TAN into `out_tan`. Requires FS
 * client.
 */
bool machbus_session_fs_client_delete_file(MachbusSession *h, const char *path, uint8_t *out_tan);

/**
 * Add an in-memory file the server will expose. Requires FS server.
 */
bool machbus_session_fs_server_add_file(MachbusSession *h,
                                        const char *path,
                                        const uint8_t *data,
                                        uintptr_t len,
                                        uint8_t attrs);

/**
 * Add a directory entry the server will expose. Requires FS server.
 */
bool machbus_session_fs_server_add_directory(MachbusSession *h, const char *path);

/**
 * Set the server's reported volume name. Requires FS server.
 */
bool machbus_session_fs_server_set_volume_name(MachbusSession *h, const char *name);

/**
 * Track a peer address for heartbeat monitoring. Requires heartbeat.
 */
bool machbus_session_heartbeat_track(MachbusSession *h, uint8_t address);

/**
 * Stop tracking a peer address. Requires heartbeat.
 */
bool machbus_session_heartbeat_untrack(MachbusSession *h, uint8_t address);

/**
 * Last received heartbeat sequence for `address`. Returns true and fills
 * `out_sequence` if available. Requires heartbeat.
 */
bool machbus_session_heartbeat_last_sequence(const MachbusSession *h,
                                             uint8_t address,
                                             uint8_t *out_sequence);

/**
 * Count of missed heartbeats for `address`, or 0 if not tracked/plugged.
 */
uint32_t machbus_session_heartbeat_missed_count(const MachbusSession *h, uint8_t address);

/**
 * Signal an error state in the next heartbeat. Requires heartbeat.
 */
bool machbus_session_heartbeat_signal_error(MachbusSession *h);

/**
 * Signal a shutdown state in the next heartbeat. Requires heartbeat.
 */
bool machbus_session_heartbeat_signal_shutdown(MachbusSession *h);

/**
 * Local language/units data the command plugin reports. Requires language cmd.
 */
bool machbus_session_language_command_local(const MachbusSession *h, MachbusLanguageData *out);

/**
 * Broadcast the current local language/units command. Requires language cmd.
 */
bool machbus_session_language_command_broadcast(MachbusSession *h);

/**
 * MaintainPower role: 0 = CF/client, 1 = TECU server. Returns 0xFF if the
 * subsystem is not plugged.
 */
uint8_t machbus_session_maintain_power_role(const MachbusSession *h);

/**
 * MaintainPower state byte (0=Running,1=ShutdownPending,2=Maintaining,
 * 3=PowerOff). Returns 0xFF if not plugged.
 */
uint8_t machbus_session_maintain_power_state(const MachbusSession *h);

/**
 * Signal key-off (begins the maintain-power countdown). Requires maintain power.
 */
bool machbus_session_maintain_power_key_off(MachbusSession *h);

/**
 * Signal key-on. Requires maintain power.
 */
bool machbus_session_maintain_power_key_on(MachbusSession *h);

/**
 * Request (or release) maintained power. Requires maintain power.
 */
bool machbus_session_maintain_power_request_power(MachbusSession *h, bool need_power);

/**
 * Last received maintain-power data `(source, data)`. Requires maintain power.
 */
bool machbus_session_maintain_power_last(const MachbusSession *h,
                                         uint8_t *out_source,
                                         MachbusMaintainPowerData *out);

/**
 * Command a NAME change: stage `new_name_raw` for the control function whose
 * current identity is `current_identity`. Requires name management.
 */
bool machbus_session_name_management_set_pending(MachbusSession *h,
                                                 uint32_t current_identity,
                                                 uint64_t new_name_raw);

/**
 * Adopt the staged pending NAME. On success writes the adopted raw NAME into
 * `out_name_raw`. Requires name management.
 */
bool machbus_session_name_management_adopt_pending(MachbusSession *h, uint64_t *out_name_raw);

/**
 * Broadcast an EEC1 (engine speed/torque) message. Requires powertrain.
 */
bool machbus_session_powertrain_broadcast_eec1(MachbusSession *h, const MachbusEec1 *data);

/**
 * Broadcast an ETC1 (electronic transmission) message. Requires powertrain.
 */
bool machbus_session_powertrain_broadcast_etc1(MachbusSession *h, const MachbusEtc1 *data);

/**
 * Broadcast a vehicle identification (VIN) message (UTF-8, NUL-terminated).
 * Requires powertrain.
 */
bool machbus_session_powertrain_broadcast_vehicle_identification(MachbusSession *h,
                                                                 const char *vin);

/**
 * Latest decoded EEC1 from the powertrain snapshot. Returns true and fills
 * `out` if present. Requires powertrain.
 */
bool machbus_session_powertrain_snapshot_eec1(const MachbusSession *h, MachbusEec1 *out);

/**
 * Register a canned response for a requested PGN. Requires request2.
 */
bool machbus_session_request2_register_response(MachbusSession *h,
                                                uint32_t pgn,
                                                const uint8_t *data,
                                                uintptr_t len);

/**
 * Remove a previously-registered response for `pgn`. Returns true if one was
 * removed. Requires request2.
 */
bool machbus_session_request2_remove_response(MachbusSession *h, uint32_t pgn);

/**
 * Number of registered request2 responses, or 0 if not plugged.
 */
uintptr_t machbus_session_request2_response_count(const MachbusSession *h);

/**
 * Sequence-control client state byte (0=Idle,1=Ready,2=Active,3=Paused,
 * 4=Complete,5=Error). Returns 0xFF if not plugged.
 */
uint8_t machbus_session_sc_client_state(const MachbusSession *h);

/**
 * Whether the sequence-control client is busy.
 */
bool machbus_session_sc_client_is_busy(const MachbusSession *h);

/**
 * Set the client busy flag. Requires SC client.
 */
bool machbus_session_sc_client_set_busy(MachbusSession *h, bool busy);

/**
 * Report a sequence step complete by id. Requires SC client.
 */
bool machbus_session_sc_client_report_step_complete(MachbusSession *h, uint16_t step_id);

/**
 * Sequence-control master state byte (see `machbus_session_sc_client_state`).
 * Returns 0xFF if not plugged.
 */
uint8_t machbus_session_sc_master_state(const MachbusSession *h);

/**
 * Add a sequence step (`step_id`, `description` UTF-8/NUL-terminated,
 * `duration_ms`). Requires SC master.
 */
bool machbus_session_sc_master_add_step(MachbusSession *h,
                                        uint16_t step_id,
                                        const char *description,
                                        uint32_t duration_ms);

/**
 * Start the configured sequence. Requires SC master.
 */
bool machbus_session_sc_master_start(MachbusSession *h);

/**
 * Pause the running sequence. Requires SC master.
 */
bool machbus_session_sc_master_pause(MachbusSession *h);

/**
 * Resume a paused sequence. Requires SC master.
 */
bool machbus_session_sc_master_resume(MachbusSession *h);

/**
 * Abort the running sequence. Requires SC master.
 */
bool machbus_session_sc_master_abort(MachbusSession *h);

/**
 * Mark a sequence step completed by id. Requires SC master.
 */
bool machbus_session_sc_master_step_completed(MachbusSession *h, uint16_t step_id);

/**
 * Broadcast a shortcut-button state (0=StopImplementOperations,
 * 1=PermitAllImplementsToOperate, 2=Error, 3=NotAvailable). Requires shortcut.
 */
bool machbus_session_shortcut_button_broadcast(MachbusSession *h, uint8_t state);

/**
 * Broadcast a shortcut-button state with an explicit transition count.
 * Requires shortcut button.
 */
bool machbus_session_shortcut_button_broadcast_with_transition_count(MachbusSession *h,
                                                                     uint8_t state,
                                                                     uint8_t count);

/**
 * Request TIM authority for the option set encoded in the 3 `options` bytes.
 * Requires TIM.
 */
bool machbus_session_tim_request_authority(MachbusSession *h, const uint8_t *options);

/**
 * Grant TIM authority (server side). Requires TIM.
 */
bool machbus_session_tim_grant_authority(MachbusSession *h);

/**
 * Deny TIM authority. Requires TIM.
 */
bool machbus_session_tim_deny_authority(MachbusSession *h);

/**
 * Revoke TIM authority. Requires TIM.
 */
bool machbus_session_tim_revoke_authority(MachbusSession *h);

/**
 * Set the TIM safety interlocks. Requires TIM.
 */
bool machbus_session_tim_set_interlocks(MachbusSession *h, MachbusTimInterlocks interlocks);

/**
 * Command a hitch (front/rear) to a target position (0..=MAX) at `rate`.
 * Requires TIM.
 */
bool machbus_session_tim_command_hitch_position(MachbusSession *h,
                                                MachbusHitch hitch,
                                                uint16_t target_position,
                                                uint8_t rate);

/**
 * Engage a PTO (front/rear), `cw_direction` = clockwise. Requires TIM.
 */
bool machbus_session_tim_command_pto_engage(MachbusSession *h, MachbusPto pto, bool cw_direction);

/**
 * Disengage a PTO (front/rear). Requires TIM.
 */
bool machbus_session_tim_command_pto_disengage(MachbusSession *h, MachbusPto pto);

/**
 * Start the TC server. Requires TC server.
 */
bool machbus_session_tc_server_start(MachbusSession *h);

/**
 * Stop the TC server. Requires TC server.
 */
bool machbus_session_tc_server_stop(MachbusSession *h);

/**
 * TC server state byte (0=Disconnected,1=WaitForClients,2=Active). Returns
 * 0xFF if not plugged.
 */
uint8_t machbus_session_tc_server_state(MachbusSession *h);

/**
 * Number of connected TC clients, or 0 if not plugged.
 */
uintptr_t machbus_session_tc_server_client_count(MachbusSession *h);

/**
 * Start the VT server. Requires VT server.
 */
bool machbus_session_vt_server_start(MachbusSession *h);

/**
 * Stop the VT server. Requires VT server.
 */
bool machbus_session_vt_server_stop(MachbusSession *h);

/**
 * VT server state code (see `vt_server_state_code`). Returns 0 (Disconnected)
 * if not plugged.
 */
uint32_t machbus_session_vt_server_state(const MachbusSession *h);

/**
 * VT server active working-set address, or NULL_ADDRESS if not plugged.
 */
uint8_t machbus_session_vt_server_active_working_set(MachbusSession *h);

/**
 * Set the VT server active working-set address. Requires VT server.
 */
bool machbus_session_vt_server_set_active_working_set(MachbusSession *h, uint8_t address);

/**
 * Create a standalone NMEA decoder. If `listen_all` is true every supported
 * PGN is decoded; otherwise the default navigation subset is used. Returns
 * `NULL` only on allocation failure. Free with [`machbus_nmea_free`].
 */
MachbusNmea *machbus_nmea_new(bool listen_all);

/**
 * Free a standalone NMEA decoder. Accepts `NULL`.
 */
void machbus_nmea_free(MachbusNmea *h);

/**
 * Feed one NMEA 2000 message (`pgn`, `len` data bytes, `source` address) into
 * the decoder. Decoded results are queued for the `machbus_nmea_poll_*`
 * functions. Returns false on a null handle/data or an unusable source
 * address (0xFE/0xFF are rejected by the envelope gate).
 */
bool machbus_nmea_feed(MachbusNmea *h,
                       uint32_t pgn,
                       const uint8_t *data,
                       uintptr_t len,
                       uint8_t source);

/**
 * Drain the next decoded position into `out`. Returns true if one was
 * available. (FIFO across all events, but only position items are matched.)
 */
bool machbus_nmea_poll_position(MachbusNmea *h, MachbusGnssPosition *out);

/**
 * Drain the next decoded course-over-ground (radians) into `out`.
 */
bool machbus_nmea_poll_cog(MachbusNmea *h, double *out);

/**
 * Drain the next decoded speed-over-ground (m/s) into `out`.
 */
bool machbus_nmea_poll_sog(MachbusNmea *h, double *out);

/**
 * Drain the next decoded heading (radians) into `out`.
 */
bool machbus_nmea_poll_heading(MachbusNmea *h, double *out);

/**
 * Drain the next decoded magnetic variation (radians) into `out`.
 */
bool machbus_nmea_poll_magnetic_variation(MachbusNmea *h, double *out);

/**
 * Latest cached position without consuming the queue. Returns true and fills
 * `out` if a position has been decoded.
 */
bool machbus_nmea_latest_position(const MachbusNmea *h, MachbusGnssPosition *out);

/**
 * Create an empty VT object pool. Free with [`machbus_vt_pool_free`].
 */
MachbusVtPool *machbus_vt_pool_new(void);

/**
 * Load a prebuilt ISO 11783-6 `.iop` object-pool blob into a new VT pool.
 *
 * The bytes are first checked with [`crate::net::parse_iop_data`] /
 * [`crate::net::validate`], then converted into a structured [`ObjectPool`]
 * via [`ObjectPool::deserialize`] (which splits each parent object's inline
 * child / macro tail into the pool's structured fields so the pool re-
 * serializes byte-for-byte). Returns `NULL` on malformed input; inspect
 * [`machbus_session_last_error`]. Free with [`machbus_vt_pool_free`].
 */
MachbusVtPool *machbus_vt_pool_from_iop(const uint8_t *data, uintptr_t len);

/**
 * Free a VT pool handle. Accepts `NULL`. Do not call after the handle has
 * been consumed by [`machbus_session_new_with_content`].
 */
void machbus_vt_pool_free(MachbusVtPool *h);

/**
 * Number of objects currently in the pool, or 0 for a null handle.
 */
uintptr_t machbus_vt_pool_object_count(const MachbusVtPool *h);

/**
 * Serialize the pool to ISO 11783-6 object-pool bytes (length-query
 * convention: returns the full length; copies into `out` only when
 * `cap >= length`; pass `out = NULL` to query the size). Returns 0 on error.
 */
uintptr_t machbus_vt_pool_serialize(const MachbusVtPool *h, uint8_t *out, uintptr_t cap);

/**
 * Add a Working Set object (Type 0): background colour, `selectable`
 * (0/1), and the initial Data/Alarm Mask object id (`0xFFFF` = none).
 */
bool machbus_vt_pool_add_working_set(MachbusVtPool *h,
                                     uint16_t id,
                                     uint8_t background_color,
                                     uint8_t selectable,
                                     uint16_t active_mask);

/**
 * Add a Data Mask object (Type 1): background colour and soft-key mask id
 * (`0xFFFF` = none).
 */
bool machbus_vt_pool_add_data_mask(MachbusVtPool *h,
                                   uint16_t id,
                                   uint8_t background_color,
                                   uint16_t soft_key_mask);

/**
 * Add a Container object (Type 3): width, height, and `hidden` (0/1).
 */
bool machbus_vt_pool_add_container(MachbusVtPool *h,
                                   uint16_t id,
                                   uint16_t width,
                                   uint16_t height,
                                   bool hidden);

/**
 * Add a Soft Key Mask object (Type 4): background colour.
 */
bool machbus_vt_pool_add_soft_key_mask(MachbusVtPool *h, uint16_t id, uint8_t background_color);

/**
 * Add a Key object (Type 5): background colour and key code.
 */
bool machbus_vt_pool_add_key(MachbusVtPool *h,
                             uint16_t id,
                             uint8_t background_color,
                             uint8_t key_code);

/**
 * Add a Button object (Type 6): width, height, background/border colour,
 * key code, and options.
 */
bool machbus_vt_pool_add_button(MachbusVtPool *h,
                                uint16_t id,
                                uint16_t width,
                                uint16_t height,
                                uint8_t background_color,
                                uint8_t border_color,
                                uint8_t key_code,
                                uint8_t options);

/**
 * Add an Input Number object (Type 9): geometry, font attributes id, Options 1,
 * variable reference id (`0xFFFF` = use raw `value`), value, min/max/offset,
 * scale, decimals, format, justification, and Options 2.
 */
bool machbus_vt_pool_add_input_number(MachbusVtPool *h,
                                      uint16_t id,
                                      uint16_t width,
                                      uint16_t height,
                                      uint8_t background_color,
                                      uint16_t font_attributes,
                                      uint8_t options,
                                      uint16_t variable_reference,
                                      uint32_t value,
                                      int32_t min_value,
                                      int32_t max_value,
                                      int32_t offset,
                                      float scale,
                                      uint8_t number_of_decimals,
                                      uint8_t format,
                                      uint8_t justification,
                                      uint8_t options2);

/**
 * Add an Input List object (Type 10): geometry, variable reference id,
 * inline selected value, options, and the list of item object ids
 * (`items` / `item_count`; may be null/0 for an empty list).
 */
bool machbus_vt_pool_add_input_list(MachbusVtPool *h,
                                    uint16_t id,
                                    uint16_t width,
                                    uint16_t height,
                                    uint16_t variable_reference,
                                    uint8_t value,
                                    uint8_t options,
                                    const uint16_t *items,
                                    uintptr_t item_count);

/**
 * Add an Output Number object (Type 12): geometry, font attributes id,
 * options, variable reference id, value, offset, scale, decimals, format,
 * justification.
 */
bool machbus_vt_pool_add_output_number(MachbusVtPool *h,
                                       uint16_t id,
                                       uint16_t width,
                                       uint16_t height,
                                       uint8_t background_color,
                                       uint16_t font_attributes,
                                       uint8_t options,
                                       uint16_t variable_reference,
                                       uint32_t value,
                                       int32_t offset,
                                       float scale,
                                       uint8_t number_of_decimals,
                                       uint8_t format,
                                       uint8_t justification);

/**
 * Add an Output String object (Type 11): geometry, font attributes id,
 * options, variable reference id, justification, and the literal string
 * `value` (NUL-terminated; ignored when a variable reference is set).
 */
bool machbus_vt_pool_add_output_string(MachbusVtPool *h,
                                       uint16_t id,
                                       uint16_t width,
                                       uint16_t height,
                                       uint8_t background_color,
                                       uint16_t font_attributes,
                                       uint8_t options,
                                       uint16_t variable_reference,
                                       uint8_t justification,
                                       const char *value);

/**
 * Add an Output Rectangle object (Type 14): width, height, line attributes
 * id, line-suppression bitmask (bits 0..=3), and fill attributes id.
 */
bool machbus_vt_pool_add_output_rectangle(MachbusVtPool *h,
                                          uint16_t id,
                                          uint16_t width,
                                          uint16_t height,
                                          uint16_t line_attributes,
                                          uint8_t line_suppression,
                                          uint16_t fill_attributes);

/**
 * Add a Font Attributes object (Type 23): colour, size, type, style.
 */
bool machbus_vt_pool_add_font_attributes(MachbusVtPool *h,
                                         uint16_t id,
                                         uint8_t font_color,
                                         uint8_t font_size,
                                         uint8_t font_type,
                                         uint8_t font_style);

/**
 * Add a Fill Attributes object (Type 25): fill type (0..=3), fill colour,
 * and the fill-pattern Picture Graphic id (`0xFFFF` = none).
 */
bool machbus_vt_pool_add_fill_attributes(MachbusVtPool *h,
                                         uint16_t id,
                                         uint8_t fill_type,
                                         uint8_t fill_color,
                                         uint16_t fill_pattern);

/**
 * Add a Picture Graphic object (Type 20): display width, actual bitmap
 * width/height, colour format (0=1-bit,1=4-bit,2=8-bit), options
 * (bit 2 = RLE), transparency colour index, and the raw bitmap bytes
 * (`data` / `data_len`; may be null/0).
 */
bool machbus_vt_pool_add_picture_graphic(MachbusVtPool *h,
                                         uint16_t id,
                                         uint16_t width,
                                         uint16_t actual_width,
                                         uint16_t actual_height,
                                         uint8_t format,
                                         uint8_t options,
                                         uint8_t transparency,
                                         const uint8_t *data,
                                         uintptr_t data_len);

/**
 * Create an empty DDOP. Free with [`machbus_ddop_free`].
 */
MachbusDdop *machbus_ddop_new(void);

/**
 * Free a DDOP handle. Accepts `NULL`. Do not call after the handle has been
 * consumed by [`machbus_session_new_with_content`].
 */
void machbus_ddop_free(MachbusDdop *h);

/**
 * Add a Device object. `id` 0 = auto-assign; `designator` is required
 * (non-empty ASCII). `software_version` / `serial_number` may be NULL.
 * Returns the assigned ObjectID, or -1 on error.
 */
int32_t machbus_ddop_add_device(MachbusDdop *h,
                                uint16_t id,
                                const char *designator,
                                const char *software_version,
                                const char *serial_number);

/**
 * Add a Device Element. `id` 0 = auto-assign. `element_type` is the ISO
 * `DeviceElementType` code (1=Device,2=Function,3=Bin,4=Section,5=Unit,
 * 6=Connector,7=NavigationReference); out-of-range falls back to Device.
 * `parent_id` 0 = no parent. `designator` may be NULL. Returns the assigned
 * ObjectID, or -1 on error.
 */
int32_t machbus_ddop_add_element(MachbusDdop *h,
                                 uint16_t id,
                                 uint8_t element_type,
                                 uint16_t element_number,
                                 uint16_t parent_id,
                                 const char *designator);

/**
 * Add a Device Process Data object. `id` 0 = auto-assign. `trigger_methods`
 * is the ISO trigger bitmask. `presentation_id` `0xFFFF` = no presentation.
 * `designator` may be NULL. Returns the assigned ObjectID, or -1 on error.
 */
int32_t machbus_ddop_add_process_data(MachbusDdop *h,
                                      uint16_t id,
                                      uint16_t ddi,
                                      uint8_t trigger_methods,
                                      uint16_t presentation_id,
                                      const char *designator);

/**
 * Add a Device Property object. `id` 0 = auto-assign. `value` is the fixed
 * property value. `presentation_id` `0xFFFF` = no presentation. `designator`
 * may be NULL. Returns the assigned ObjectID, or -1 on error.
 */
int32_t machbus_ddop_add_property(MachbusDdop *h,
                                  uint16_t id,
                                  uint16_t ddi,
                                  int32_t value,
                                  uint16_t presentation_id,
                                  const char *designator);

/**
 * Add a Device Value Presentation object. `id` 0 = auto-assign. `offset` and
 * `scale` apply as `displayed = (value + offset) * scale`. `unit_designator`
 * may be NULL. Returns the assigned ObjectID, or -1 on error.
 */
int32_t machbus_ddop_add_value_presentation(MachbusDdop *h,
                                            uint16_t id,
                                            int32_t offset,
                                            float scale,
                                            uint8_t decimal_digits,
                                            const char *unit_designator);

/**
 * Serialize the DDOP to ISO 11783-10 object-pool bytes (length-query
 * convention: returns the full length; copies into `out` only when
 * `cap >= length`; pass `out = NULL` to query the size). Returns 0 on error.
 */
uintptr_t machbus_ddop_serialize(const MachbusDdop *h, uint8_t *out, uintptr_t cap);

/**
 * Create a session from `cfg` (or defaults if `cfg` is NULL) that plugs a
 * VT client carrying `vt_pool` (with a working set whose active mask is
 * `working_set_id`) and/or a TC client carrying `ddop`.
 *
 * Ownership: this call **consumes** the `vt_pool` and `ddop` handles — on both
 * success and failure they are freed internally, so the caller must not use or
 * free them afterwards (set them to NULL). Pass NULL for either to skip that
 * subsystem; the corresponding `enable_vt_client` / `enable_tc_client` config
 * flags are honoured too (a NULL handle with the flag set plugs an empty
 * pool/DDOP, matching [`machbus_session_new`]).
 *
 * Returns `NULL` on failure; inspect [`machbus_session_last_error`]. Free the
 * returned session with [`machbus_session_free`].
 */
MachbusSession *machbus_session_new_with_content(const MachbusConfig *cfg,
                                                 MachbusVtPool *vt_pool,
                                                 uint16_t working_set_id,
                                                 MachbusDdop *ddop);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

