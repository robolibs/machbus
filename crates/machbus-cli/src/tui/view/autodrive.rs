//! The "AutoDrive" tab: the automatic-guidance conversation, decoded live.
//!
//! Three messages make up the exchange and none of them is readable alone:
//!
//! - **PGN 0xAD00** Guidance System Command — the commanded curvature *and* the
//!   2-bit intent-to-steer status. A curvature without intent steers nothing,
//!   which is the single most common "why isn't it moving" cause.
//! - **PGN 0xAC00** Machine Info — the steering ECU's estimated curvature,
//!   readiness, lockout and limit status.
//! - **PGN 0xFD43** Machine Selected Speed Command.
//!
//! Showing command beside feedback is what tells you whether the machine is
//! actually following, and the Δ between commanded and estimated curvature is
//! the number that says so.
//!
//! This tab is **read-only**: `machbus live` observes, it never transmits. To
//! drive a machine use `machbus drive`, which owns an `AutoDrive` plugin and a
//! dead-man/arm model.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use machbus::isobus::implement::CurvatureCommandStatus;
use machbus::isobus::implement::guidance::{
    GenericSaeBs02SlotValue, GuidanceLimitStatus, MechanicalLockout,
};

use crate::tui::App;
use crate::tui::view::theme::{ACCENT, DIM, EXT, GOLD, OK, TEXT, bold, dim, fg, panel};

/// Milliseconds before a cached message is called stale. ISO 11783-7 §5.2.7.2
/// puts the guidance group at 100 ms, and three missed broadcasts is the loss
/// threshold both plugins use.
const STALE_MS: u128 = 300;

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let a = &app.autodrive;
    let title = format!(
        "AutoDrive  ·  ISO 11783-7 guidance  ·  cmd {}  info {}",
        a.cmd_count, a.info_count,
    );
    let block = panel(&title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    if a.cmd.is_none() && a.info.is_none() && a.speed.is_none() {
        lines.push(Line::from(Span::styled(
            "No guidance traffic seen yet.",
            fg(TEXT),
        )));
        lines.push(Line::from(Span::styled(
            "Waiting for PGN 0xAD00 (command), 0xAC00 (machine info) or 0xFD43 (speed).",
            dim(),
        )));
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    // ── commanded curvature + intent ────────────────────────────────────
    lines.push(section("COMMAND  ·  PGN 0xAD00  guidance system command"));
    match &a.cmd {
        Some((src, cmd, at)) => {
            let age = at.elapsed().as_millis();
            lines.push(kv(
                "source",
                vec![
                    Span::styled(format!("0x{src:02X}"), fg(EXT)),
                    Span::raw("  "),
                    age_span(age),
                ],
            ));
            lines.push(kv(
                "curvature",
                vec![signal_span(cmd.commanded_curvature, "1/km")],
            ));
            // The intent flag is the engage signal: a curvature sent without it
            // is a report, not a request, and a conformant ECU moves nothing.
            let (label, colour) = match cmd.status {
                CurvatureCommandStatus::IntendedToSteer => ("INTEND TO STEER", OK),
                CurvatureCommandStatus::NotIntendedToSteer => ("not intended to steer", DIM),
                CurvatureCommandStatus::ErrorIndication => ("ERROR", ACCENT),
                CurvatureCommandStatus::NotAvailable => ("not available", DIM),
            };
            lines.push(kv("intent", vec![Span::styled(label, bold(colour))]));
        }
        None => lines.push(kv("—", vec![Span::styled("no command seen", dim())])),
    }

    lines.push(Line::from(""));

    // ── machine feedback ────────────────────────────────────────────────
    lines.push(section("FEEDBACK  ·  PGN 0xAC00  machine info"));
    match &a.info {
        Some((src, info, at)) => {
            let age = at.elapsed().as_millis();
            lines.push(kv(
                "source",
                vec![
                    Span::styled(format!("0x{src:02X}"), fg(EXT)),
                    Span::raw("  "),
                    age_span(age),
                ],
            ));
            lines.push(kv(
                "estimated",
                vec![signal_span(info.estimated_curvature, "1/km")],
            ));

            // The number that matters: commanded vs actually produced. Never
            // assume the machine reached what was asked for.
            if let (Some(c), Some(e)) = (
                a.cmd
                    .as_ref()
                    .and_then(|(_, c, _)| c.commanded_curvature.value()),
                info.estimated_curvature.value(),
            ) {
                let delta = c - e;
                let colour = if delta.abs() < 1.0 { OK } else { GOLD };
                lines.push(kv(
                    "tracking Δ",
                    vec![Span::styled(format!("{delta:+.2} 1/km"), bold(colour))],
                ));
            }

            let (rdy, rdy_col) = match info.steering_system_readiness_state {
                GenericSaeBs02SlotValue::EnabledOnActive => ("enabled / on", OK),
                GenericSaeBs02SlotValue::DisabledOffPassive => ("disabled / off", DIM),
                GenericSaeBs02SlotValue::ErrorIndication => ("error", ACCENT),
                GenericSaeBs02SlotValue::NotAvailableTakeNoAction => ("not available", DIM),
            };
            lines.push(kv("readiness", vec![Span::styled(rdy, fg(rdy_col))]));

            let (lock, lock_col) = match info.lockout {
                MechanicalLockout::Active => ("ACTIVE — steering locked out", ACCENT),
                MechanicalLockout::NotActive => ("clear", OK),
                MechanicalLockout::Error => ("error", ACCENT),
                MechanicalLockout::NotAvailable => ("not available", DIM),
            };
            lines.push(kv("lockout", vec![Span::styled(lock, fg(lock_col))]));

            // Limit status is the anti-windup signal: at a limit the ECU is
            // saturated and an outer loop that does not know will wind up.
            let (lim, lim_col) = match info.guidance_limit_status {
                GuidanceLimitStatus::NotLimited => ("not limited", OK),
                GuidanceLimitStatus::OperatorLimitedControlled => {
                    ("OPERATOR IN CONTROL", ACCENT)
                }
                GuidanceLimitStatus::LimitedHigh => ("limited high (saturated)", GOLD),
                GuidanceLimitStatus::LimitedLow => ("limited low (saturated)", GOLD),
                GuidanceLimitStatus::NonRecoverableFault => ("NON-RECOVERABLE FAULT", ACCENT),
                _ => ("reserved / not available", DIM),
            };
            lines.push(kv("limit", vec![Span::styled(lim, bold(lim_col))]));

            let (eng, eng_col) = match info.remote_engage_switch_status {
                GenericSaeBs02SlotValue::EnabledOnActive => ("armed", OK),
                GenericSaeBs02SlotValue::DisabledOffPassive => {
                    ("NOT ARMED — operator switch off", GOLD)
                }
                GenericSaeBs02SlotValue::ErrorIndication => ("error", ACCENT),
                GenericSaeBs02SlotValue::NotAvailableTakeNoAction => ("not available", DIM),
            };
            lines.push(kv("engage sw", vec![Span::styled(eng, fg(eng_col))]));
        }
        None => lines.push(kv(
            "—",
            vec![Span::styled(
                "no machine info — nothing is answering, so no command can be followed",
                fg(GOLD),
            )],
        )),
    }

    lines.push(Line::from(""));

    // ── speed ───────────────────────────────────────────────────────────
    lines.push(section("SPEED  ·  PGN 0xFD43  machine selected speed command"));
    match &a.speed {
        Some((src, speed, at)) => {
            let age = at.elapsed().as_millis();
            lines.push(kv(
                "source",
                vec![
                    Span::styled(format!("0x{src:02X}"), fg(EXT)),
                    Span::raw("  "),
                    age_span(age),
                ],
            ));
            let mps = f64::from(speed.target_speed_raw) * 0.001;
            lines.push(kv(
                "commanded",
                vec![Span::styled(
                    format!("{mps:.3} m/s  ({:.1} km/h)  {:?}", mps * 3.6, speed.direction_cmd),
                    fg(TEXT),
                )],
            ));
        }
        None => lines.push(kv(
            "—",
            vec![Span::styled(
                "no speed command — the tractor owns its speed",
                dim(),
            )],
        )),
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn section(title: &str) -> Line<'static> {
    Line::from(Span::styled(title.to_string(), bold(GOLD)))
}

fn kv(key: &str, mut value: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![Span::styled(format!("  {key:<12}"), dim())];
    spans.append(&mut value);
    Line::from(spans)
}

/// Render a `Signal<f64>` honestly: a value, or the reason there isn't one.
/// Collapsing error and not-available to `0.0` is exactly how a missing signal
/// gets mistaken for a real zero.
fn signal_span(sig: machbus::isobus::implement::Signal<f64>, unit: &str) -> Span<'static> {
    use machbus::isobus::implement::Signal;
    match sig {
        Signal::Value(v) => Span::styled(format!("{v:+.2} {unit}"), bold(TEXT)),
        Signal::Error => Span::styled("error", bold(ACCENT)),
        Signal::NotAvailable => Span::styled("not available", dim()),
    }
}

/// Age of a cached message, coloured by whether the stream is still live. The
/// command is a heartbeat, so a stalled one is itself the fault.
fn age_span(age_ms: u128) -> Span<'static> {
    let colour: Color = if age_ms > STALE_MS { ACCENT } else { OK };
    let mark = if age_ms > STALE_MS { "STALE " } else { "" };
    Span::styled(format!("{mark}{age_ms} ms ago"), fg(colour))
}
