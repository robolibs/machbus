use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use crate::geo::Wgs;

use crate::j1939::Fmi;
use crate::j1939::diagnostic::Dtc;
use crate::net::{ClaimState, Frame, Identifier, Name, Pgn, Priority};
use crate::nmea::{GNSSPosition, NMEAConfig};
use crate::session::plugins::{
    Auxiliary, ControlFunctionalities, Diagnostics, DmMemory, FsClient, FsServer, Gnss,
    GroupFunction, Guidance, Heartbeat, Implement, LanguageCommand, MaintainPower, NameManagement,
    Powertrain, Request2, ScClient, ScMaster, ShortcutButton, TcClient, TcServer, Tim, VtClient,
    VtServer,
};
use crate::session::{
    ClaimEvent, DiagEvent, Event, GnssEvent, GuidanceEvent, Hitch, ImplementEvent, Pto, Session,
    TcEvent, VtEvent, presets,
};
use crate::time::Instant;

fn err_runtime<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn parse_priority(p: u8) -> PyResult<Priority> {
    Priority::try_from_u8(p)
        .ok_or_else(|| PyValueError::new_err(format!("invalid CAN priority {p}")))
}

fn parse_hitch(s: &str) -> PyResult<Hitch> {
    match s {
        "front" => Ok(Hitch::Front),
        "rear" => Ok(Hitch::Rear),
        other => Err(PyValueError::new_err(format!(
            "unknown hitch `{other}` (expected 'front' or 'rear')"
        ))),
    }
}

fn parse_pto(s: &str) -> PyResult<Pto> {
    match s {
        "front" => Ok(Pto::Front),
        "rear" => Ok(Pto::Rear),
        other => Err(PyValueError::new_err(format!(
            "unknown pto `{other}` (expected 'front' or 'rear')"
        ))),
    }
}

fn parse_hitch_command(
    s: &str,
) -> PyResult<crate::isobus::implement::tractor_commands::HitchCommand> {
    use crate::isobus::implement::tractor_commands::HitchCommand;
    match s {
        "no_action" => Ok(HitchCommand::NoAction),
        "lower" => Ok(HitchCommand::Lower),
        "raise" => Ok(HitchCommand::Raise),
        "position" => Ok(HitchCommand::Position),
        other => Err(PyValueError::new_err(format!(
            "unknown hitch command `{other}`"
        ))),
    }
}

fn parse_pto_command(s: &str) -> PyResult<crate::isobus::implement::tractor_commands::PtoCommand> {
    use crate::isobus::implement::tractor_commands::PtoCommand;
    match s {
        "no_action" => Ok(PtoCommand::NoAction),
        "engage" => Ok(PtoCommand::Engage),
        "disengage" => Ok(PtoCommand::Disengage),
        "set_speed" => Ok(PtoCommand::SetSpeed),
        other => Err(PyValueError::new_err(format!(
            "unknown pto command `{other}`"
        ))),
    }
}

fn parse_valve_command(
    s: &str,
) -> PyResult<crate::isobus::implement::tractor_commands::ValveCommand> {
    use crate::isobus::implement::tractor_commands::ValveCommand;
    match s {
        "no_action" => Ok(ValveCommand::NoAction),
        "extend" => Ok(ValveCommand::Extend),
        "retract" => Ok(ValveCommand::Retract),
        "float" => Ok(ValveCommand::Float),
        "block" => Ok(ValveCommand::Block),
        other => Err(PyValueError::new_err(format!(
            "unknown valve command `{other}`"
        ))),
    }
}

fn claim_state_str(s: ClaimState) -> &'static str {
    match s {
        ClaimState::None => "none",
        ClaimState::WaitForClaim => "wait_for_claim",
        ClaimState::SendRequest => "send_request",
        ClaimState::WaitForContest => "wait_for_contest",
        ClaimState::SendClaim => "send_claim",
        ClaimState::Claimed => "claimed",
        ClaimState::Failed => "failed",
    }
}

fn event_to_dict<'py>(py: Python<'py>, ev: &Event) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    match ev {
        Event::AddressClaim(c) => {
            d.set_item("kind", "address_claim")?;
            match c {
                ClaimEvent::Claimed { address } => {
                    d.set_item("sub", "claimed")?;
                    d.set_item("address", *address)?;
                }
                ClaimEvent::Lost { previous_address } => {
                    d.set_item("sub", "lost")?;
                    d.set_item("previous_address", *previous_address)?;
                }
                ClaimEvent::Disconnected => {
                    d.set_item("sub", "disconnected")?;
                }
            }
        }
        Event::Diag(de) => {
            d.set_item("kind", "diag")?;
            match de {
                DiagEvent::Raised(dtc) => {
                    d.set_item("sub", "raised")?;
                    d.set_item("spn", dtc.spn)?;
                    d.set_item("fmi", dtc.fmi.as_u8())?;
                }
                DiagEvent::Cleared(dtc) => {
                    d.set_item("sub", "cleared")?;
                    d.set_item("spn", dtc.spn)?;
                    d.set_item("fmi", dtc.fmi.as_u8())?;
                }
                DiagEvent::Dm1Received { source, active, .. } => {
                    d.set_item("sub", "dm1_received")?;
                    d.set_item("source", *source)?;
                    let dtcs: Vec<(u32, u8)> =
                        active.iter().map(|d| (d.spn, d.fmi.as_u8())).collect();
                    d.set_item("dtcs", dtcs)?;
                }
                other => {
                    d.set_item("sub", "other")?;
                    d.set_item("debug", format!("{other:?}"))?;
                }
            }
        }
        Event::Gnss(ge) => {
            d.set_item("kind", "gnss")?;
            match ge {
                GnssEvent::Position(p) => {
                    d.set_item("sub", "position")?;
                    d.set_item("latitude", p.wgs.latitude)?;
                    d.set_item("longitude", p.wgs.longitude)?;
                }
                GnssEvent::Cog(c) => {
                    d.set_item("sub", "cog")?;
                    d.set_item("rad", *c)?;
                }
                GnssEvent::Sog(s) => {
                    d.set_item("sub", "sog")?;
                    d.set_item("mps", *s)?;
                }
                GnssEvent::Heading(h) => {
                    d.set_item("sub", "heading")?;
                    d.set_item("rad", *h)?;
                }
                other => {
                    d.set_item("sub", "other")?;
                    d.set_item("debug", format!("{other:?}"))?;
                }
            }
        }
        Event::Imp(ie) => {
            d.set_item("kind", "imp")?;
            match ie {
                ImplementEvent::HitchCommand { hitch, msg } => {
                    d.set_item("sub", "hitch_command")?;
                    d.set_item(
                        "hitch",
                        match hitch {
                            Hitch::Front => "front",
                            Hitch::Rear => "rear",
                        },
                    )?;
                    d.set_item("command", format!("{:?}", msg.command).to_lowercase())?;
                    d.set_item("target_position", msg.target_position)?;
                    d.set_item("rate", msg.rate)?;
                }
                ImplementEvent::PtoCommand { pto, msg } => {
                    d.set_item("sub", "pto_command")?;
                    d.set_item(
                        "pto",
                        match pto {
                            Pto::Front => "front",
                            Pto::Rear => "rear",
                        },
                    )?;
                    d.set_item("command", format!("{:?}", msg.command).to_lowercase())?;
                    d.set_item("target_speed_rpm", msg.target_speed_rpm)?;
                    d.set_item("ramp_rate", msg.ramp_rate)?;
                }
                ImplementEvent::AuxValveCommand(m) => {
                    d.set_item("sub", "aux_valve_command")?;
                    d.set_item("valve_index", m.valve_index)?;
                    d.set_item("command", format!("{:?}", m.command).to_lowercase())?;
                    d.set_item("flow_rate", m.flow_rate)?;
                }
                other => {
                    d.set_item("sub", "other")?;
                    d.set_item("debug", format!("{other:?}"))?;
                }
            }
        }
        Event::Vt(ve) => {
            d.set_item("kind", "vt")?;
            match ve {
                VtEvent::StateChanged(state) => {
                    d.set_item("sub", "state_changed")?;
                    d.set_item("state", format!("{state:?}").to_lowercase())?;
                }
                VtEvent::SoftKey { id, code } => {
                    d.set_item("sub", "soft_key")?;
                    d.set_item("id", id.0)?;
                    d.set_item("code", code.as_u8())?;
                }
                VtEvent::Button { id, code } => {
                    d.set_item("sub", "button")?;
                    d.set_item("id", id.0)?;
                    d.set_item("code", code.as_u8())?;
                }
                VtEvent::NumericValueChanged { id, value } => {
                    d.set_item("sub", "numeric_value_changed")?;
                    d.set_item("id", id.0)?;
                    d.set_item("value", *value)?;
                }
                VtEvent::StringValueChanged { id, value } => {
                    d.set_item("sub", "string_value_changed")?;
                    d.set_item("id", id.0)?;
                    d.set_item("value", value)?;
                }
                VtEvent::PoolError(code) => {
                    d.set_item("sub", "pool_error")?;
                    d.set_item("code", *code)?;
                }
                other => {
                    d.set_item("sub", "other")?;
                    d.set_item("debug", format!("{other:?}"))?;
                }
            }
        }
        Event::Tc(te) => {
            d.set_item("kind", "tc")?;
            match te {
                TcEvent::StateChanged(state) => {
                    d.set_item("sub", "state_changed")?;
                    d.set_item("state", format!("{state:?}").to_lowercase())?;
                }
            }
        }
        Event::Guidance(ge) => match ge {
            GuidanceEvent::MachineInfo {
                source,
                estimated_curvature,
                steering_ready,
                limit_status,
            } => {
                d.set_item("kind", "guidance")?;
                d.set_item("sub", "machine_info")?;
                d.set_item("source", *source)?;
                d.set_item("estimated_curvature", *estimated_curvature)?;
                d.set_item("steering_ready", *steering_ready)?;
                d.set_item("limit_status", *limit_status)?;
            }
        },
        other => {
            d.set_item("kind", "other")?;
            d.set_item("debug", format!("{other:?}"))?;
        }
    }
    Ok(d)
}

/// A single machbus node built on the sans-IO `Session` core.
#[pyclass(name = "Session", unsendable)]
pub struct PySession {
    session: Box<Session>,
    now: Instant,
}

impl PySession {
    fn diag(&mut self) -> PyResult<&mut Diagnostics> {
        self.session
            .get_mut::<Diagnostics>()
            .ok_or_else(|| err_runtime("diagnostics subsystem not enabled"))
    }

    fn gnss(&mut self) -> PyResult<&mut Gnss> {
        self.session
            .get_mut::<Gnss>()
            .ok_or_else(|| err_runtime("gnss subsystem not enabled"))
    }

    fn imp(&mut self) -> PyResult<&mut Implement> {
        self.session
            .get_mut::<Implement>()
            .ok_or_else(|| err_runtime("implement subsystem not enabled"))
    }

    #[allow(dead_code)]
    fn guidance(&mut self) -> PyResult<&mut Guidance> {
        self.session
            .get_mut::<Guidance>()
            .ok_or_else(|| err_runtime("guidance subsystem not enabled"))
    }

    fn vt(&mut self) -> PyResult<&mut VtClient> {
        self.session
            .get_mut::<VtClient>()
            .ok_or_else(|| err_runtime("vt client subsystem not enabled"))
    }

    fn tc(&mut self) -> PyResult<&mut TcClient> {
        self.session
            .get_mut::<TcClient>()
            .ok_or_else(|| err_runtime("tc client subsystem not enabled"))
    }

    fn auxiliary(&mut self) -> PyResult<&mut Auxiliary> {
        self.session
            .get_mut::<Auxiliary>()
            .ok_or_else(|| err_runtime("auxiliary subsystem not enabled"))
    }
    fn dm_memory(&mut self) -> PyResult<&mut DmMemory> {
        self.session
            .get_mut::<DmMemory>()
            .ok_or_else(|| err_runtime("dm_memory subsystem not enabled"))
    }
    fn functionalities(&mut self) -> PyResult<&mut ControlFunctionalities> {
        self.session
            .get_mut::<ControlFunctionalities>()
            .ok_or_else(|| err_runtime("functionalities subsystem not enabled"))
    }
    fn group_function(&mut self) -> PyResult<&mut GroupFunction> {
        self.session
            .get_mut::<GroupFunction>()
            .ok_or_else(|| err_runtime("group_function subsystem not enabled"))
    }
    fn heartbeat(&mut self) -> PyResult<&mut Heartbeat> {
        self.session
            .get_mut::<Heartbeat>()
            .ok_or_else(|| err_runtime("heartbeat subsystem not enabled"))
    }
    fn language(&mut self) -> PyResult<&mut LanguageCommand> {
        self.session
            .get_mut::<LanguageCommand>()
            .ok_or_else(|| err_runtime("language_command subsystem not enabled"))
    }
    fn maintain_power(&mut self) -> PyResult<&mut MaintainPower> {
        self.session
            .get_mut::<MaintainPower>()
            .ok_or_else(|| err_runtime("maintain_power subsystem not enabled"))
    }
    fn name_management(&mut self) -> PyResult<&mut NameManagement> {
        self.session
            .get_mut::<NameManagement>()
            .ok_or_else(|| err_runtime("name_management subsystem not enabled"))
    }
    fn powertrain(&mut self) -> PyResult<&mut Powertrain> {
        self.session
            .get_mut::<Powertrain>()
            .ok_or_else(|| err_runtime("powertrain subsystem not enabled"))
    }
    fn request2(&mut self) -> PyResult<&mut Request2> {
        self.session
            .get_mut::<Request2>()
            .ok_or_else(|| err_runtime("request2 subsystem not enabled"))
    }
    fn sc_client(&mut self) -> PyResult<&mut ScClient> {
        self.session
            .get_mut::<ScClient>()
            .ok_or_else(|| err_runtime("sc_client subsystem not enabled"))
    }
    fn sc_master(&mut self) -> PyResult<&mut ScMaster> {
        self.session
            .get_mut::<ScMaster>()
            .ok_or_else(|| err_runtime("sc_master subsystem not enabled"))
    }
    fn shortcut_button(&mut self) -> PyResult<&mut ShortcutButton> {
        self.session
            .get_mut::<ShortcutButton>()
            .ok_or_else(|| err_runtime("shortcut_button subsystem not enabled"))
    }
    fn tim(&mut self) -> PyResult<&mut Tim> {
        self.session
            .get_mut::<Tim>()
            .ok_or_else(|| err_runtime("tim subsystem not enabled"))
    }
    fn vt_server(&mut self) -> PyResult<&mut VtServer> {
        self.session
            .get_mut::<VtServer>()
            .ok_or_else(|| err_runtime("vt_server subsystem not enabled"))
    }
    fn tc_server(&mut self) -> PyResult<&mut TcServer> {
        self.session
            .get_mut::<TcServer>()
            .ok_or_else(|| err_runtime("tc_server subsystem not enabled"))
    }
    fn fs_server(&mut self) -> PyResult<&mut FsServer> {
        self.session
            .get_mut::<FsServer>()
            .ok_or_else(|| err_runtime("fs_server subsystem not enabled"))
    }
    fn fs_client(&mut self) -> PyResult<&mut FsClient> {
        self.session
            .get_mut::<FsClient>()
            .ok_or_else(|| err_runtime("fs_client subsystem not enabled"))
    }
}

#[pymethods]
impl PySession {
    /// Build a new session.
    ///
    /// `preset` may be one of `"tractor"`, `"implement"`, or `"diagnostic_node"`;
    /// when set it is plugged first, then the `enable_*` flags add any extra
    /// subsystems on top. Each subsystem type may only be plugged once.
    #[new]
    #[pyo3(signature = (
        name = 0,
        preferred_address = 0x80,
        preset = None,
        enable_diagnostics = false,
        diagnostics_interval_ms = 1000,
        enable_gnss = false,
        enable_guidance = false,
        enable_implement = false,
        enable_vt_client = false,
        enable_tc_client = false,
        enable_auxiliary = false,
        enable_dm_memory = false,
        enable_fs_client = false,
        enable_fs_server = false,
        enable_functionalities = false,
        enable_group_function = false,
        enable_heartbeat = false,
        heartbeat_interval_ms = 1000,
        enable_language_command = false,
        enable_maintain_power = false,
        maintain_power_role = "cf",
        enable_name_management = false,
        enable_powertrain = false,
        enable_request2 = false,
        enable_sc_client = false,
        enable_sc_master = false,
        enable_shortcut_button = false,
        enable_tc_server = false,
        enable_vt_server = false,
        enable_tim = false,
        vt_pool = None,
        working_set = None,
        ddop = None,
    ))]
    #[allow(clippy::too_many_arguments, unused_variables)]
    fn new(
        name: u64,
        preferred_address: u8,
        preset: Option<&str>,
        enable_diagnostics: bool,
        diagnostics_interval_ms: u32,
        enable_gnss: bool,
        enable_guidance: bool,
        enable_implement: bool,
        enable_vt_client: bool,
        enable_tc_client: bool,
        enable_auxiliary: bool,
        enable_dm_memory: bool,
        enable_fs_client: bool,
        enable_fs_server: bool,
        enable_functionalities: bool,
        enable_group_function: bool,
        enable_heartbeat: bool,
        heartbeat_interval_ms: u32,
        enable_language_command: bool,
        enable_maintain_power: bool,
        maintain_power_role: &str,
        enable_name_management: bool,
        enable_powertrain: bool,
        enable_request2: bool,
        enable_sc_client: bool,
        enable_sc_master: bool,
        enable_shortcut_button: bool,
        enable_tc_server: bool,
        enable_vt_server: bool,
        enable_tim: bool,
        vt_pool: Option<PyRef<VtPool>>,
        working_set: Option<u16>,
        ddop: Option<PyRef<Ddop>>,
    ) -> PyResult<Self> {
        let mut b = Session::builder(Name::from_raw(name), preferred_address);

        if let Some(preset) = preset {
            let group = match preset {
                "tractor" => presets::tractor(),
                "diagnostic_node" => presets::diagnostic_node(),
                "implement" => {
                    use crate::isobus::tc::{DDOP, TCClientConfig};
                    use crate::isobus::vt::{
                        DataMaskBody, ObjectPool, ObjectType, VTObject, WorkingSet,
                        create_data_mask,
                    };
                    let _ = TCClientConfig::default();
                    let pool = ObjectPool::default()
                        .with_object(
                            VTObject::default()
                                .with_id(1u16)
                                .with_type(ObjectType::WorkingSet)
                                .with_children(vec![2u16]),
                        )
                        .with_object(create_data_mask(2u16, &DataMaskBody::default()));
                    presets::implement(pool, WorkingSet::default(), DDOP::default())
                }
                other => {
                    return Err(PyValueError::new_err(format!(
                        "unknown preset `{other}` (expected 'tractor', 'implement', or 'diagnostic_node')"
                    )));
                }
            };
            b = b.plug_group(group);
        }

        if enable_diagnostics {
            b = b.plug(Diagnostics::every(diagnostics_interval_ms));
        }
        if enable_gnss {
            b = b.plug(Gnss::new(NMEAConfig::default().with_gnss_navigation(true)));
        }
        if enable_guidance {
            b = b.plug(Guidance::new());
        }
        if enable_implement {
            b = b.plug(Implement::new());
        }
        if enable_vt_client {
            use crate::isobus::vt::{
                DataMaskBody, ObjectPool, ObjectType, VTClientConfig, VTObject, WorkingSet,
                create_data_mask,
            };
            // Use a supplied VtPool if given; otherwise fall back to a
            // minimal default pool (Working Set + empty Data Mask).
            let pool = match &vt_pool {
                Some(p) => p.inner.clone(),
                None => ObjectPool::default()
                    .with_object(
                        VTObject::default()
                            .with_id(1u16)
                            .with_type(ObjectType::WorkingSet)
                            .with_children(vec![2u16]),
                    )
                    .with_object(create_data_mask(2u16, &DataMaskBody::default())),
            };
            let mut ws = WorkingSet::default();
            if let Some(active_mask) = working_set {
                ws.set_active_mask(active_mask);
            }
            b = b.plug(VtClient::new(VTClientConfig::default(), pool, ws));
        }
        if enable_tc_client {
            use crate::isobus::tc::{DDOP, TCClientConfig};
            let pool = match &ddop {
                Some(d) => d.inner.clone(),
                None => DDOP::default(),
            };
            b = b.plug(TcClient::new(TCClientConfig::default(), pool));
        }
        if enable_auxiliary {
            b = b.plug(Auxiliary::new());
        }
        if enable_dm_memory {
            b = b.plug(DmMemory::new(None));
        }
        if enable_fs_client {
            b = b.plug(FsClient::new(crate::isobus::fs::FileClientConfig::default()));
        }
        if enable_fs_server {
            b = b.plug(FsServer::new(crate::isobus::fs::FileServerConfig::default()));
        }
        if enable_functionalities {
            b = b.plug(ControlFunctionalities::new(
                crate::isobus::functionalities::Functionalities::default(),
            ));
        }
        if enable_group_function {
            b = b.plug(GroupFunction::new(
                crate::isobus::group_function::GroupFunctionResponder::default(),
            ));
        }
        if enable_heartbeat {
            b = b.plug(Heartbeat::every(heartbeat_interval_ms));
        }
        if enable_language_command {
            b = b.plug(LanguageCommand::new(crate::j1939::LanguageData::default()));
        }
        if enable_maintain_power {
            let role = match maintain_power_role {
                "tecu" => crate::j1939::PowerRole::Tecu,
                "cf" => crate::j1939::PowerRole::Cf,
                other => {
                    return Err(PyValueError::new_err(format!(
                        "unknown maintain_power_role `{other}` (expected 'tecu' or 'cf')"
                    )));
                }
            };
            b = b.plug(MaintainPower::new(role));
        }
        if enable_name_management {
            b = b.plug(NameManagement::new());
        }
        if enable_powertrain {
            b = b.plug(Powertrain::new());
        }
        if enable_request2 {
            b = b.plug(Request2::new(crate::j1939::Request2Responder::default()));
        }
        if enable_sc_client {
            b = b.plug(ScClient::new(crate::isobus::sc::SCClientConfig::default()));
        }
        if enable_sc_master {
            b = b.plug(ScMaster::new(crate::isobus::sc::SCMasterConfig::default()));
        }
        if enable_shortcut_button {
            b = b.plug(ShortcutButton::new());
        }
        if enable_tc_server {
            b = b.plug(
                TcServer::new(crate::isobus::tc::TCServerConfig::default()).map_err(err_runtime)?,
            );
        }
        if enable_vt_server {
            b = b.plug(
                VtServer::new(crate::isobus::vt::VTServerConfig::default()).map_err(err_runtime)?,
            );
        }
        if enable_tim {
            b = b.plug(Tim::new(crate::isobus::tim::TimAuthority::new(
                crate::isobus::tim::TimOptionSet::empty(),
            )));
        }

        let session = b.build().map_err(err_runtime)?;
        Ok(Self {
            session: Box::new(session),
            now: Instant::ZERO,
        })
    }

    // ─── Lifecycle / driving ─────────────────────────────────────

    /// Begin address claiming.
    fn start(&mut self) -> PyResult<()> {
        self.session.start().map_err(err_runtime)
    }

    /// Advance the time cursor by `dt_ms` milliseconds and tick the session.
    fn tick(&mut self, dt_ms: u64) {
        self.now = self.now.add_millis(dt_ms);
        self.session.tick(self.now);
    }

    /// Current monotonic time cursor, in milliseconds.
    fn now_ms(&self) -> u64 {
        self.now.as_millis()
    }

    /// Advance time in 50 ms steps until claimed or `timeout_ms` elapses.
    ///
    /// With no bus contention this completes the claim purely by ticking;
    /// returns the claimed address.
    fn run_until_claimed(&mut self, timeout_ms: u64) -> PyResult<u8> {
        let deadline = self.now.add_millis(timeout_ms);
        loop {
            if self.session.is_claimed() {
                return Ok(self.session.address());
            }
            if self.now >= deadline {
                return Err(err_runtime("run_until_claimed: timed out"));
            }
            self.now = self.now.add_millis(50);
            self.session.tick(self.now);
        }
    }

    /// Feed one received CAN frame (raw 29-bit id + payload) on `port`.
    fn feed(&mut self, port: u8, can_id: u32, data: Vec<u8>) -> PyResult<()> {
        if data.len() > 8 {
            return Err(PyValueError::new_err("CAN payload exceeds 8 bytes"));
        }
        let mut bytes = [0xFFu8; 8];
        bytes[..data.len()].copy_from_slice(&data);
        let frame = Frame::new(Identifier::from_raw(can_id), bytes, data.len() as u8);
        self.session.feed(port, &frame, self.now);
        Ok(())
    }

    // ─── Outputs ─────────────────────────────────────────────────

    /// Next `(port, can_id, data)` the core wants to transmit, or `None`.
    fn poll_transmit(&mut self) -> Option<(u8, u32, Vec<u8>)> {
        self.session
            .poll_transmit()
            .map(|(port, frame)| (port, frame.id.raw, frame.payload().to_vec()))
    }

    /// Drain every queued outbound frame as `(port, can_id, data)` tuples.
    fn poll_transmit_all(&mut self) -> Vec<(u8, u32, Vec<u8>)> {
        let mut out = Vec::new();
        while let Some((port, frame)) = self.session.poll_transmit() {
            out.push((port, frame.id.raw, frame.payload().to_vec()));
        }
        out
    }

    /// Next application event as a dict, or `None` when drained.
    fn poll_event<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(ev) = self.session.poll_event() else {
            return Ok(None);
        };
        Ok(Some(event_to_dict(py, &ev)?))
    }

    /// Drain all queued application events as dicts.
    fn drain_events<'py>(&mut self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyDict>>> {
        let mut out = Vec::new();
        while let Some(ev) = self.session.poll_event() {
            out.push(event_to_dict(py, &ev)?);
        }
        Ok(out)
    }

    // ─── Address claim ───────────────────────────────────────────

    fn address(&self) -> u8 {
        self.session.address()
    }

    fn claim_state(&self) -> &'static str {
        claim_state_str(self.session.claim_state())
    }

    fn is_claimed(&self) -> bool {
        self.session.is_claimed()
    }

    // ─── Raw send ────────────────────────────────────────────────

    /// Queue an application message from this session's control function.
    ///
    /// `dst` is a destination address (0xFF for broadcast), `priority` 0..=7.
    #[pyo3(signature = (pgn, data, dst=0xFF, priority=6))]
    fn send_raw(&mut self, pgn: u32, data: Vec<u8>, dst: u8, priority: u8) -> PyResult<()> {
        let prio = parse_priority(priority)?;
        self.session
            .send_raw(pgn as Pgn, &data, dst, prio)
            .map_err(err_runtime)
    }

    // ─── Diagnostics ─────────────────────────────────────────────

    fn diag_raise(&mut self, spn: u32, fmi: u8) -> PyResult<()> {
        self.diag()?.raise(Dtc {
            spn,
            fmi: Fmi::from_u8(fmi),
            occurrence_count: 1,
        });
        Ok(())
    }

    fn diag_clear(&mut self) -> PyResult<()> {
        self.diag()?.clear();
        Ok(())
    }

    fn diag_active_count(&mut self) -> usize {
        self.session
            .get::<Diagnostics>()
            .map_or(0, |d| d.active().len())
    }

    fn diag_active<'py>(&mut self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyDict>>> {
        let Some(diag) = self.session.get::<Diagnostics>() else {
            return Ok(Vec::new());
        };
        let dtcs: Vec<_> = diag
            .active()
            .iter()
            .map(|d| (d.spn, d.fmi.as_u8(), d.occurrence_count))
            .collect();
        let mut out = Vec::with_capacity(dtcs.len());
        for (spn, fmi, occ) in dtcs {
            let d = PyDict::new(py);
            d.set_item("spn", spn)?;
            d.set_item("fmi", fmi)?;
            d.set_item("occurrence_count", occ)?;
            out.push(d);
        }
        Ok(out)
    }

    // ─── GNSS ────────────────────────────────────────────────────

    #[pyo3(signature = (latitude, longitude, altitude_m=None, speed_mps=None, heading_rad=None))]
    fn gnss_broadcast_position(
        &mut self,
        latitude: f64,
        longitude: f64,
        altitude_m: Option<f64>,
        speed_mps: Option<f64>,
        heading_rad: Option<f64>,
    ) -> PyResult<()> {
        let pos = GNSSPosition {
            wgs: Wgs::new(latitude, longitude, altitude_m.unwrap_or(0.0)),
            altitude_m,
            speed_mps,
            heading_rad,
            ..GNSSPosition::default()
        };
        self.gnss()?.broadcast_position(&pos);
        Ok(())
    }

    fn gnss_broadcast_cog_sog(&mut self, cog_rad: f64, sog_mps: f64) -> PyResult<()> {
        self.gnss()?.broadcast_cog_sog(cog_rad, sog_mps);
        Ok(())
    }

    fn gnss_latest_position<'py>(
        &mut self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(gnss) = self.session.get::<Gnss>() else {
            return Ok(None);
        };
        let Some(p) = gnss.latest_position() else {
            return Ok(None);
        };
        let d = PyDict::new(py);
        d.set_item("latitude", p.wgs.latitude)?;
        d.set_item("longitude", p.wgs.longitude)?;
        d.set_item("altitude_m", p.altitude_m)?;
        d.set_item("speed_mps", p.speed_mps)?;
        d.set_item("heading_rad", p.heading_rad)?;
        Ok(Some(d))
    }

    // ─── Guidance (autosteer) ────────────────────────────────────

    /// Command path curvature in 1/km (0 = straight). Autosteer is
    /// curvature-based.
    fn guidance_command_curvature(&mut self, curvature_per_km: f64) -> PyResult<()> {
        self.guidance()?.command_curvature(curvature_per_km);
        Ok(())
    }

    /// Command with a robotics-style twist: linear velocity `linear_mps` (m/s,
    /// forward positive) and angular/yaw velocity `angular_rad_s` (rad/s, left
    /// positive). Sends both the steering curvature (κ = ω / v, PGN 0xAD00) and
    /// the target speed (PGN 0xFD43).
    fn guidance_command_velocity(&mut self, linear_mps: f64, angular_rad_s: f64) -> PyResult<()> {
        self.guidance()?.command_velocity(linear_mps, angular_rad_s);
        Ok(())
    }

    /// Command a turn radius in metres (curvature = 1000 / radius).
    fn guidance_command_radius(&mut self, radius_m: f64) -> PyResult<()> {
        self.guidance()?.command_radius(radius_m);
        Ok(())
    }

    /// Command straight-ahead steering (zero curvature).
    fn guidance_command_straight(&mut self) -> PyResult<()> {
        self.guidance()?.command_straight();
        Ok(())
    }

    /// Request the steering ECU to engage (Curvature Command Status = intended to
    /// steer on PGN 0xAD00); re-sends the last commanded curvature. The ECU only
    /// steers while it reports itself ready.
    fn guidance_engage(&mut self) -> PyResult<()> {
        self.guidance()?.engage();
        Ok(())
    }

    /// Stop requesting steering: clears the engage request and commands straight.
    fn guidance_disengage(&mut self) -> PyResult<()> {
        self.guidance()?.disengage();
        Ok(())
    }

    /// `True` if the controller is currently requesting steering (its own intent,
    /// not the steering ECU's readiness).
    fn guidance_is_engaged(&mut self) -> PyResult<bool> {
        Ok(self.guidance()?.is_engaged())
    }

    /// The steering system's last reported estimated curvature (1/km), or `None`.
    fn guidance_estimated_curvature(&mut self) -> PyResult<Option<f64>> {
        Ok(self.guidance()?.estimated_curvature())
    }

    /// `True` if the steering system reports it is ready to be steered.
    fn guidance_is_steering_ready(&mut self) -> PyResult<bool> {
        Ok(self.guidance()?.is_steering_ready())
    }

    // ─── Implement messages ──────────────────────────────────────

    fn imp_command_hitch(&mut self, hitch: &str, command: &str) -> PyResult<()> {
        let h = parse_hitch(hitch)?;
        let c = parse_hitch_command(command)?;
        self.imp()?.command_hitch(h, c);
        Ok(())
    }

    fn imp_command_pto(&mut self, pto: &str, command: &str) -> PyResult<()> {
        let p = parse_pto(pto)?;
        let c = parse_pto_command(command)?;
        self.imp()?.command_pto(p, c);
        Ok(())
    }

    fn imp_command_pto_speed(&mut self, pto: &str, rpm: u16, ramp_rate: u8) -> PyResult<()> {
        let p = parse_pto(pto)?;
        self.imp()?.command_pto_speed(p, rpm, ramp_rate);
        Ok(())
    }

    fn imp_command_aux_valve(
        &mut self,
        valve_index: u8,
        command: &str,
        flow_rate: u16,
    ) -> PyResult<()> {
        let c = parse_valve_command(command)?;
        self.imp()?
            .command_aux_valve(valve_index, c, flow_rate)
            .map_err(err_runtime)
    }

    // ─── VT client ───────────────────────────────────────────────

    fn vt_connect_to(&mut self, server: u8) -> PyResult<()> {
        self.vt()?.connect_to(server);
        Ok(())
    }

    fn vt_is_connected(&mut self) -> bool {
        self.session
            .get::<VtClient>()
            .is_some_and(VtClient::is_connected)
    }

    fn vt_state(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.vt()?.state()).to_lowercase())
    }

    fn vt_show(&mut self, object_id: u16) -> PyResult<()> {
        self.vt()?.show(object_id).map_err(err_runtime)
    }

    fn vt_hide(&mut self, object_id: u16) -> PyResult<()> {
        self.vt()?.hide(object_id).map_err(err_runtime)
    }

    fn vt_set_value(&mut self, object_id: u16, value: u32) -> PyResult<()> {
        self.vt()?.set_value(object_id, value).map_err(err_runtime)
    }

    fn vt_set_string(&mut self, object_id: u16, value: &str) -> PyResult<()> {
        self.vt()?.set_string(object_id, value).map_err(err_runtime)
    }

    fn vt_change_active_mask(&mut self, ws: u16, mask: u16) -> PyResult<()> {
        self.vt()?.change_active_mask(ws, mask).map_err(err_runtime)
    }

    // ─── TC client ───────────────────────────────────────────────

    fn tc_connect(&mut self) -> PyResult<()> {
        self.tc()?.connect().map_err(err_runtime)
    }

    fn tc_disconnect(&mut self) -> PyResult<()> {
        self.tc()?.disconnect().map_err(err_runtime)
    }

    fn tc_is_connected(&mut self) -> bool {
        self.session
            .get::<TcClient>()
            .is_some_and(TcClient::is_connected)
    }

    fn tc_state(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.tc()?.state()).to_lowercase())
    }

    fn tc_address(&mut self) -> PyResult<u8> {
        Ok(self.tc()?.tc_address())
    }

    // ─── Auxiliary ───────────────────────────────────────────────

    /// Broadcast an AUX-O function. `ty`/`state` are raw enum bytes,
    /// `setpoint` is the 16-bit value.
    fn auxiliary_broadcast_aux_o(
        &mut self,
        function_number: u8,
        ty: u8,
        state: u8,
        setpoint: u16,
    ) -> PyResult<()> {
        use crate::isobus::auxiliary::{AuxFunctionState, AuxFunctionType, AuxOFunction};
        self.auxiliary()?.broadcast_aux_o(AuxOFunction {
            function_number,
            r#type: AuxFunctionType::from_u8(ty),
            state: AuxFunctionState::from_u8(state),
            setpoint,
        });
        Ok(())
    }

    /// Broadcast an AUX-N function. `ty`/`state` are raw enum bytes,
    /// `setpoint` is the 16-bit value.
    fn auxiliary_broadcast_aux_n(
        &mut self,
        function_number: u8,
        ty: u8,
        state: u8,
        setpoint: u16,
    ) -> PyResult<()> {
        use crate::isobus::auxiliary::{AuxFunctionState, AuxFunctionType, AuxNFunction};
        self.auxiliary()?.broadcast_aux_n(AuxNFunction {
            function_number,
            r#type: AuxFunctionType::from_u8(ty),
            state: AuxFunctionState::from_u8(state),
            setpoint,
        });
        Ok(())
    }

    /// Last AUX-O function seen from `source` for `function_number`, as a dict.
    fn auxiliary_last_aux_o<'py>(
        &mut self,
        py: Python<'py>,
        source: u8,
        function_number: u8,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(aux) = self.session.get::<Auxiliary>() else {
            return Ok(None);
        };
        let Some(f) = aux.last_aux_o(source, function_number) else {
            return Ok(None);
        };
        let d = PyDict::new(py);
        d.set_item("function_number", f.function_number)?;
        d.set_item("type", f.r#type.as_u8())?;
        d.set_item("state", f.state.as_u8())?;
        d.set_item("setpoint", f.setpoint)?;
        Ok(Some(d))
    }

    /// Last AUX-N function seen from `source` for `function_number`, as a dict.
    fn auxiliary_last_aux_n<'py>(
        &mut self,
        py: Python<'py>,
        source: u8,
        function_number: u8,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(aux) = self.session.get::<Auxiliary>() else {
            return Ok(None);
        };
        let Some(f) = aux.last_aux_n(source, function_number) else {
            return Ok(None);
        };
        let d = PyDict::new(py);
        d.set_item("function_number", f.function_number)?;
        d.set_item("type", f.r#type.as_u8())?;
        d.set_item("state", f.state.as_u8())?;
        d.set_item("setpoint", f.setpoint)?;
        Ok(Some(d))
    }

    // ─── DM memory (DM14/15/16, ECU id) ──────────────────────────

    fn dm_memory_request_ecu_identification(&mut self, destination: u8) -> PyResult<()> {
        self.dm_memory()?
            .request_ecu_identification(destination)
            .map_err(err_runtime)
    }

    fn dm_memory_request_software_identification(&mut self, destination: u8) -> PyResult<()> {
        self.dm_memory()?
            .request_software_identification(destination)
            .map_err(err_runtime)
    }

    /// Last DM14 (memory access request) received, as `(source, address, length, command)`.
    fn dm_memory_last_dm14(&mut self) -> Option<(u8, u32, u16, u8)> {
        self.session.get::<DmMemory>().and_then(|m| {
            m.last_dm14()
                .map(|(src, req)| (src, req.address, req.length, req.command.as_u8()))
        })
    }

    // ─── Functionalities ─────────────────────────────────────────

    /// Debug dump of the local functionalities model.
    fn functionalities_model_debug(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.functionalities()?.model()))
    }

    // ─── Group function ──────────────────────────────────────────

    /// Number of registered group-function support entries.
    fn group_function_responder_debug(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.group_function()?.responder()))
    }

    // ─── Heartbeat ───────────────────────────────────────────────

    fn heartbeat_track(&mut self, address: u8) -> PyResult<()> {
        self.heartbeat()?.track(address);
        Ok(())
    }
    fn heartbeat_untrack(&mut self, address: u8) -> PyResult<()> {
        self.heartbeat()?.untrack(address);
        Ok(())
    }
    fn heartbeat_last_sequence(&mut self, address: u8) -> Option<u8> {
        self.session
            .get::<Heartbeat>()
            .and_then(|h| h.last_sequence(address))
    }
    fn heartbeat_missed_count(&mut self, address: u8) -> u32 {
        self.session
            .get::<Heartbeat>()
            .map_or(0, |h| h.missed_count(address))
    }
    fn heartbeat_signal_error(&mut self) -> PyResult<()> {
        self.heartbeat()?.signal_error();
        Ok(())
    }
    fn heartbeat_signal_shutdown(&mut self) -> PyResult<()> {
        self.heartbeat()?.signal_shutdown();
        Ok(())
    }

    // ─── Language command ────────────────────────────────────────

    fn language_broadcast(&mut self) -> PyResult<()> {
        self.language()?.broadcast();
        Ok(())
    }

    /// Local language code (two ASCII bytes, e.g. `"en"`).
    fn language_local_code(&mut self) -> PyResult<String> {
        let d = self.language()?.local();
        Ok(String::from_utf8_lossy(&d.language_code).into_owned())
    }

    /// Set the local language code from a 2-char string (e.g. `"de"`).
    fn language_set_local_code(&mut self, code: &str) -> PyResult<()> {
        let bytes = code.as_bytes();
        if bytes.len() != 2 {
            return Err(PyValueError::new_err("language code must be 2 ASCII chars"));
        }
        let mut data = self.language()?.local();
        data.language_code = [bytes[0], bytes[1]];
        self.language()?.set_local(data);
        Ok(())
    }

    /// Last received language command, as a debug string, or `None`.
    fn language_last_debug(&mut self) -> PyResult<Option<String>> {
        Ok(self.language()?.last().map(|e| format!("{e:?}")))
    }

    // ─── Maintain power ──────────────────────────────────────────

    fn maintain_power_role(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.maintain_power()?.role()).to_lowercase())
    }
    fn maintain_power_state(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.maintain_power()?.state()).to_lowercase())
    }
    fn maintain_power_key_off(&mut self) -> PyResult<()> {
        self.maintain_power()?.key_off();
        Ok(())
    }
    fn maintain_power_key_on(&mut self) -> PyResult<()> {
        self.maintain_power()?.key_on();
        Ok(())
    }
    fn maintain_power_request_power(&mut self, need_power: bool) -> PyResult<()> {
        self.maintain_power()?.request_power(need_power);
        Ok(())
    }

    // ─── Name management ─────────────────────────────────────────

    /// `True` if a NAME change is pending adoption.
    fn name_management_has_pending(&mut self) -> PyResult<bool> {
        Ok(self.name_management()?.manager().has_pending())
    }

    /// The pending NAME (raw 64-bit value), or `None`.
    fn name_management_pending_name(&mut self) -> PyResult<Option<u64>> {
        Ok(self
            .name_management()?
            .manager()
            .pending_name()
            .map(|n| n.raw))
    }

    /// Stage a NAME change: set the pending NAME for `current_identity`.
    fn name_management_set_pending(
        &mut self,
        current_identity: u32,
        new_name: u64,
    ) -> PyResult<()> {
        self.name_management()?
            .manager_mut()
            .set_pending(current_identity, Name::from_raw(new_name))
            .map_err(err_runtime)
    }

    // ─── Powertrain ──────────────────────────────────────────────

    fn powertrain_broadcast_eec1(&mut self, eec1: &PyEec1) -> PyResult<()> {
        let data = crate::j1939::Eec1 {
            engine_torque_percent: eec1.engine_torque_percent,
            driver_demand_percent: eec1.driver_demand_percent,
            actual_engine_percent: eec1.actual_engine_percent,
            engine_speed_rpm: eec1.engine_speed_rpm,
            starter_mode: eec1.starter_mode,
            source_address: eec1.source_address,
        };
        self.powertrain()?.broadcast_eec1(&data);
        Ok(())
    }

    fn powertrain_broadcast_etc1(&mut self, etc1: &PyEtc1) -> PyResult<()> {
        let data = crate::j1939::Etc1 {
            current_gear: etc1.current_gear,
            selected_gear: etc1.selected_gear,
            output_shaft_speed_rpm: etc1.output_shaft_speed_rpm,
            shift_in_progress: etc1.shift_in_progress,
            torque_converter_lockup: etc1.torque_converter_lockup,
        };
        self.powertrain()?.broadcast_etc1(&data);
        Ok(())
    }

    /// Debug dump of the latest powertrain snapshot.
    fn powertrain_snapshot_debug(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.powertrain()?.snapshot()))
    }

    // ─── Request2 (PGN request / commanded address) ──────────────

    /// Debug dump of the request2 responder registry.
    fn request2_responder_debug(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.request2()?.responder()))
    }

    // ─── Sequence control client ─────────────────────────────────

    fn sc_client_state(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.sc_client()?.state()).to_lowercase())
    }
    fn sc_client_is_busy(&mut self) -> bool {
        self.session
            .get::<ScClient>()
            .is_some_and(ScClient::is_busy)
    }
    fn sc_client_set_busy(&mut self, busy: bool) -> PyResult<()> {
        self.sc_client()?.set_busy(busy);
        Ok(())
    }
    fn sc_client_report_step_complete(&mut self, step_id: u16) -> PyResult<()> {
        self.sc_client()?
            .report_step_complete(step_id)
            .map_err(err_runtime)
    }

    // ─── Sequence control master ─────────────────────────────────

    fn sc_master_state(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.sc_master()?.state()).to_lowercase())
    }
    fn sc_master_start(&mut self) -> PyResult<()> {
        self.sc_master()?.start().map_err(err_runtime)
    }
    fn sc_master_pause(&mut self) -> PyResult<()> {
        self.sc_master()?.pause().map_err(err_runtime)
    }
    fn sc_master_resume(&mut self) -> PyResult<()> {
        self.sc_master()?.resume().map_err(err_runtime)
    }
    fn sc_master_abort(&mut self) -> PyResult<()> {
        self.sc_master()?.abort().map_err(err_runtime)
    }
    fn sc_master_step_completed(&mut self, step_id: u16) -> PyResult<()> {
        self.sc_master()?
            .step_completed(step_id)
            .map_err(err_runtime)
    }

    // ─── Shortcut button ─────────────────────────────────────────

    /// Broadcast the AEF stop-all-implements shortcut button state.
    ///
    /// `state` is the raw 2-bit value: 0=stop, 1=permit, 2=error, 3=n/a.
    fn shortcut_button_broadcast(&mut self, state: u8) -> PyResult<()> {
        use crate::j1939::shortcut_button::ShortcutButtonState;
        self.shortcut_button()?
            .broadcast(ShortcutButtonState::from_u8(state));
        Ok(())
    }

    fn shortcut_button_broadcast_with_transition_count(
        &mut self,
        state: u8,
        count: u8,
    ) -> PyResult<()> {
        use crate::j1939::shortcut_button::ShortcutButtonState;
        self.shortcut_button()?
            .broadcast_with_transition_count(ShortcutButtonState::from_u8(state), count);
        Ok(())
    }

    /// Last received shortcut button event, as a debug string, or `None`.
    fn shortcut_button_last_debug(&mut self) -> PyResult<Option<String>> {
        Ok(self.shortcut_button()?.last().map(|e| format!("{e:?}")))
    }

    // ─── TIM (Tractor-Implement Management) ──────────────────────

    /// Request TIM authority for the given 3-byte option set.
    fn tim_request_authority(&mut self, option_bytes: Vec<u8>) -> PyResult<()> {
        use crate::isobus::tim::{TIM_OPTION_BYTES, TimOptionSet};
        if option_bytes.len() != TIM_OPTION_BYTES {
            return Err(PyValueError::new_err(format!(
                "TIM option set requires exactly {TIM_OPTION_BYTES} bytes"
            )));
        }
        let mut bytes = [0u8; TIM_OPTION_BYTES];
        bytes.copy_from_slice(&option_bytes);
        self.tim()?
            .request_authority(TimOptionSet::from_bytes(bytes))
            .map_err(err_runtime)
    }
    fn tim_grant_authority(&mut self) -> PyResult<()> {
        self.tim()?.grant_authority().map_err(err_runtime)
    }
    fn tim_deny_authority(&mut self) -> PyResult<()> {
        self.tim()?.deny_authority();
        Ok(())
    }
    fn tim_revoke_authority(&mut self) -> PyResult<()> {
        self.tim()?.revoke_authority();
        Ok(())
    }
    fn tim_authority_state(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.tim()?.authority().state()).to_lowercase())
    }
    fn tim_set_interlocks(
        &mut self,
        operator_present: bool,
        road_transport_mode: bool,
        external_stop: bool,
        implement_ready: bool,
    ) -> PyResult<()> {
        use crate::isobus::tim::TimInterlocks;
        self.tim()?.set_interlocks(TimInterlocks {
            operator_present,
            road_transport_mode,
            external_stop,
            implement_ready,
        });
        Ok(())
    }
    fn tim_command_hitch_position(
        &mut self,
        hitch: &str,
        target_position: u16,
        rate: u8,
    ) -> PyResult<()> {
        let h = parse_hitch(hitch)?;
        self.tim()?
            .command_hitch_position(h, target_position, rate)
            .map_err(err_runtime)
    }
    fn tim_command_pto_engage(&mut self, pto: &str, cw_direction: bool) -> PyResult<()> {
        let p = parse_pto(pto)?;
        self.tim()?
            .command_pto_engage(p, cw_direction)
            .map_err(err_runtime)
    }
    fn tim_command_pto_disengage(&mut self, pto: &str) -> PyResult<()> {
        let p = parse_pto(pto)?;
        self.tim()?.command_pto_disengage(p).map_err(err_runtime)
    }
    fn tim_broadcast_pto_status(
        &mut self,
        pto: &str,
        engaged: bool,
        cw_direction: bool,
        speed: u16,
    ) -> PyResult<()> {
        use crate::isobus::tim::PtoState;
        let p = parse_pto(pto)?;
        self.tim()?.broadcast_pto_status(
            p,
            PtoState {
                engaged,
                cw_direction,
                speed,
            },
        );
        Ok(())
    }
    fn tim_broadcast_hitch_status(
        &mut self,
        hitch: &str,
        motion_enabled: bool,
        position: u16,
    ) -> PyResult<()> {
        use crate::isobus::tim::HitchState;
        let h = parse_hitch(hitch)?;
        self.tim()?
            .broadcast_hitch_status(
                h,
                HitchState {
                    motion_enabled,
                    position,
                },
            )
            .map_err(err_runtime)
    }
    /// Latest front PTO status `(engaged, cw_direction, speed)`, or `None`.
    fn tim_last_front_pto_status(&mut self) -> PyResult<Option<(bool, bool, u16)>> {
        Ok(self
            .tim()?
            .last_front_pto_status()
            .map(|s| (s.engaged, s.cw_direction, s.speed)))
    }
    /// Latest rear hitch status `(motion_enabled, position)`, or `None`.
    fn tim_last_rear_hitch_status(&mut self) -> PyResult<Option<(bool, u16)>> {
        Ok(self
            .tim()?
            .last_rear_hitch_status()
            .map(|s| (s.motion_enabled, s.position)))
    }
    /// Latest aux-valve command `(index, state, flow)`, or `None`.
    fn tim_last_aux_valve(&mut self) -> PyResult<Option<(u8, bool, u16)>> {
        Ok(self
            .tim()?
            .last_aux_valve()
            .map(|c| (c.index, c.state, c.flow)))
    }

    // ─── VT server (lifecycle/state) ─────────────────────────────

    fn vt_server_start(&mut self) -> PyResult<()> {
        self.vt_server()?.start().map_err(err_runtime)
    }
    fn vt_server_stop(&mut self) -> PyResult<()> {
        self.vt_server()?.stop().map_err(err_runtime)
    }
    fn vt_server_state(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.vt_server()?.state()).to_lowercase())
    }

    // ─── TC server (lifecycle/state) ─────────────────────────────

    /// Current TC server state.
    fn tc_server_state(&mut self) -> PyResult<String> {
        Ok(format!("{:?}", self.tc_server()?.server_mut().state()).to_lowercase())
    }

    // ─── FS server (file content setup) ──────────────────────────

    fn fs_server_set_volume_name(&mut self, name: String) -> PyResult<()> {
        self.fs_server()?.set_volume_name(name).map_err(err_runtime)
    }
    fn fs_server_add_directory(&mut self, path: String) -> PyResult<()> {
        self.fs_server()?.add_directory(path).map_err(err_runtime)
    }
    fn fs_server_add_file(&mut self, path: String, data: Vec<u8>, attrs: u8) -> PyResult<()> {
        self.fs_server()?
            .add_file(path, data, attrs)
            .map_err(err_runtime)
    }

    // ─── FS client (lifecycle + file ops) ────────────────────────

    fn fs_client_connect_to(&mut self, server: u8) -> PyResult<()> {
        self.fs_client()?.connect_to(server).map_err(err_runtime)
    }
    fn fs_client_disconnect(&mut self) -> PyResult<()> {
        self.fs_client()?.disconnect();
        Ok(())
    }
    fn fs_client_is_connected(&mut self) -> bool {
        self.session
            .get::<FsClient>()
            .is_some_and(FsClient::is_connected)
    }
    /// Open a file; returns the transaction number (TAN). Result arrives as an event.
    fn fs_client_open(&mut self, path: &str, flags: u8) -> PyResult<u8> {
        self.fs_client()?.open(path, flags).map_err(err_runtime)
    }
    fn fs_client_close(&mut self, handle: u8) -> PyResult<u8> {
        self.fs_client()?.close(handle).map_err(err_runtime)
    }
    fn fs_client_read(&mut self, handle: u8, count: u16) -> PyResult<u8> {
        self.fs_client()?.read(handle, count).map_err(err_runtime)
    }
    fn fs_client_write(&mut self, handle: u8, data: Vec<u8>) -> PyResult<u8> {
        self.fs_client()?.write(handle, &data).map_err(err_runtime)
    }
    fn fs_client_seek(&mut self, handle: u8, position: u32) -> PyResult<u8> {
        self.fs_client()?
            .seek(handle, position)
            .map_err(err_runtime)
    }
    fn fs_client_current_directory(&mut self) -> PyResult<u8> {
        self.fs_client()?.current_directory().map_err(err_runtime)
    }
    fn fs_client_change_directory(&mut self, path: &str) -> PyResult<u8> {
        self.fs_client()?
            .change_directory(path)
            .map_err(err_runtime)
    }
    fn fs_client_delete_file(&mut self, path: &str) -> PyResult<u8> {
        self.fs_client()?.delete_file(path).map_err(err_runtime)
    }

    // ─── Introspection ───────────────────────────────────────────

    fn has_diagnostics(&self) -> bool {
        self.session.get::<Diagnostics>().is_some()
    }
    fn has_gnss(&self) -> bool {
        self.session.get::<Gnss>().is_some()
    }
    fn has_guidance(&self) -> bool {
        self.session.get::<Guidance>().is_some()
    }
    fn has_implement(&self) -> bool {
        self.session.get::<Implement>().is_some()
    }
    fn has_vt_client(&self) -> bool {
        self.session.get::<VtClient>().is_some()
    }
    fn has_tc_client(&self) -> bool {
        self.session.get::<TcClient>().is_some()
    }
    fn has_auxiliary(&self) -> bool {
        self.session.get::<Auxiliary>().is_some()
    }
    fn has_dm_memory(&self) -> bool {
        self.session.get::<DmMemory>().is_some()
    }
    fn has_fs_client(&self) -> bool {
        self.session.get::<FsClient>().is_some()
    }
    fn has_fs_server(&self) -> bool {
        self.session.get::<FsServer>().is_some()
    }
    fn has_functionalities(&self) -> bool {
        self.session.get::<ControlFunctionalities>().is_some()
    }
    fn has_group_function(&self) -> bool {
        self.session.get::<GroupFunction>().is_some()
    }
    fn has_heartbeat(&self) -> bool {
        self.session.get::<Heartbeat>().is_some()
    }
    fn has_language_command(&self) -> bool {
        self.session.get::<LanguageCommand>().is_some()
    }
    fn has_maintain_power(&self) -> bool {
        self.session.get::<MaintainPower>().is_some()
    }
    fn has_name_management(&self) -> bool {
        self.session.get::<NameManagement>().is_some()
    }
    fn has_powertrain(&self) -> bool {
        self.session.get::<Powertrain>().is_some()
    }
    fn has_request2(&self) -> bool {
        self.session.get::<Request2>().is_some()
    }
    fn has_sc_client(&self) -> bool {
        self.session.get::<ScClient>().is_some()
    }
    fn has_sc_master(&self) -> bool {
        self.session.get::<ScMaster>().is_some()
    }
    fn has_shortcut_button(&self) -> bool {
        self.session.get::<ShortcutButton>().is_some()
    }
    fn has_tim(&self) -> bool {
        self.session.get::<Tim>().is_some()
    }
    fn has_vt_server(&self) -> bool {
        self.session.get::<VtServer>().is_some()
    }
    fn has_tc_server(&self) -> bool {
        self.session.get::<TcServer>().is_some()
    }

    fn __repr__(&self) -> String {
        format!(
            "Session(address=0x{:02X}, claim_state={})",
            self.session.address(),
            claim_state_str(self.session.claim_state())
        )
    }
}

/// Build a J1939/ISOBUS NAME (returns the raw 64-bit value).
#[pyfunction]
#[pyo3(name = "name", signature = (identity_number, function_code, self_configurable=true))]
fn py_name(identity_number: u32, function_code: u8, self_configurable: bool) -> u64 {
    Name::default()
        .with_identity_number(identity_number)
        .with_function_code(function_code)
        .with_self_configurable(self_configurable)
        .raw
}

#[pyfunction]
#[pyo3(
    name = "validate_can_bus_config",
    signature = (
        bitrate = crate::net::ISO_CAN_BITRATE,
        sample_point = crate::net::ISO_SAMPLE_POINT_NOMINAL,
        sjw = 1,
        prop_seg = 0,
        phase_seg1 = 0,
        phase_seg2 = 0,
        silent_mode = false,
        loopback = false,
    )
)]
#[allow(clippy::too_many_arguments)]
fn py_validate_can_bus_config<'py>(
    py: Python<'py>,
    bitrate: u32,
    sample_point: f64,
    sjw: u8,
    prop_seg: u8,
    phase_seg1: u8,
    phase_seg2: u8,
    silent_mode: bool,
    loopback: bool,
) -> PyResult<Bound<'py, PyDict>> {
    let validation = crate::net::validate_can_bus_config(&crate::net::CanBusConfig {
        bitrate,
        sample_point,
        sjw,
        prop_seg,
        phase_seg1,
        phase_seg2,
        silent_mode,
        loopback,
    });
    let dict = PyDict::new(py);
    dict.set_item("bitrate_ok", validation.bitrate_ok)?;
    dict.set_item("sample_point_ok", validation.sample_point_ok)?;
    dict.set_item("bit_timing_ok", validation.bit_timing_ok)?;
    dict.set_item("physical_mode_ok", validation.physical_mode_ok)?;
    dict.set_item("overall_ok", validation.overall_ok)?;
    dict.set_item("error_message", validation.error_message)?;
    Ok(dict)
}

#[pyfunction]
#[pyo3(
    name = "enforce_iso_can_config",
    signature = (
        bitrate = crate::net::ISO_CAN_BITRATE,
        sample_point = crate::net::ISO_SAMPLE_POINT_NOMINAL,
        sjw = 1,
        prop_seg = 0,
        phase_seg1 = 0,
        phase_seg2 = 0,
        silent_mode = false,
        loopback = false,
    )
)]
#[allow(clippy::too_many_arguments)]
fn py_enforce_iso_can_config(
    bitrate: u32,
    sample_point: f64,
    sjw: u8,
    prop_seg: u8,
    phase_seg1: u8,
    phase_seg2: u8,
    silent_mode: bool,
    loopback: bool,
) -> PyResult<()> {
    crate::net::enforce_iso_can_config(&crate::net::CanBusConfig {
        bitrate,
        sample_point,
        sjw,
        prop_seg,
        phase_seg1,
        phase_seg2,
        silent_mode,
        loopback,
    })
    .map_err(err_runtime)
}

// ── Standalone codec layer (net / j1939 / nmea) — no Session required ──
// Mirrors `machbus::net` / `machbus::j1939` / `machbus::nmea` 1:1.
// (Pattern slice: net::Identifier + j1939::Eec1.)

/// A decomposed 29-bit CAN identifier.
#[pyclass(name = "Identifier")]
pub struct PyIdentifier {
    raw: u32,
}

#[pymethods]
impl PyIdentifier {
    #[new]
    fn new(raw: u32) -> Self {
        Self { raw }
    }
    #[getter]
    fn raw(&self) -> u32 {
        self.raw
    }
    #[getter]
    fn priority(&self) -> u8 {
        u8::from(crate::net::Identifier::from_raw(self.raw).priority())
    }
    #[getter]
    fn pgn(&self) -> u32 {
        crate::net::Identifier::from_raw(self.raw).pgn()
    }
    #[getter]
    fn source(&self) -> u8 {
        crate::net::Identifier::from_raw(self.raw).source()
    }
    #[getter]
    fn destination(&self) -> u8 {
        crate::net::Identifier::from_raw(self.raw).destination()
    }
    #[getter]
    fn is_pdu2(&self) -> bool {
        crate::net::Identifier::from_raw(self.raw).is_pdu2()
    }
    #[getter]
    fn is_broadcast(&self) -> bool {
        crate::net::Identifier::from_raw(self.raw).is_broadcast()
    }
    fn __repr__(&self) -> String {
        let id = crate::net::Identifier::from_raw(self.raw);
        format!(
            "Identifier(0x{:08X}, prio={}, pgn=0x{:04X}, src=0x{:02X})",
            self.raw,
            u8::from(id.priority()),
            id.pgn(),
            id.source(),
        )
    }
}

/// EEC1 — engine speed / torque (PGN 61444).
#[pyclass(name = "Eec1", get_all, set_all)]
#[derive(Clone)]
pub struct PyEec1 {
    pub engine_torque_percent: f64,
    pub driver_demand_percent: f64,
    pub actual_engine_percent: f64,
    pub engine_speed_rpm: f64,
    pub starter_mode: u8,
    pub source_address: u8,
}

