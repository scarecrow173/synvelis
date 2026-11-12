//! SV-GUI-03 slice 5 (synvelis#112 AC5, ruviform#464) — driver-neutral
//! `SemanticGuiTest` scenario helpers shared by the Fake lane
//! (`tests/screen_scenario.rs`) and the GPUI-simulated lane
//! (`tests/scenario_gpui.rs`, `gpui-test` feature).
//!
//! Every function is an ordinary Rust function over the
//! `SemanticGuiTest` port — no adapter types appear here, so the same
//! scenario body runs identically on `FakeGuiTest` and `GpuiGuiTest`.
//! `ScenarioReport`s record the adapter's own `driver()`/
//! `capabilities()`: a semantic pass never upgrades into simulated
//! pointer, real-OS, IME, pixel, or native-dock evidence — those stay
//! `Unsupported`/`NotRun` (external gates: ruviform#313/#314,
//! synvelis#114).

use std::collections::BTreeSet;
use std::path::Path;

use ruviform_ui::WindowId;
use ruviform_ui::behavior::{CommandIntent, CommandPhase, Revision, ValueInput};
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::ipc::Event;
use ruviform_ui::view::{ComponentKey, ComponentSnapshot, TAB_SELECT_ACTION, TABLE_SELECT_ACTION};
use ruviform_ui_test::golden::{BaselineStore, GoldenKey, GoldenVerdict};
use ruviform_ui_test::{
    CaptureOutcome, CaseId, GuiAction, MintedIntent, RunId, ScenarioCapability, ScenarioError,
    ScenarioId, ScenarioReport, ScenarioTarget, ScenarioVerdict, SemanticGuiTest, TargetQuery,
    VisualGuiTest, parity_gesture_scenario,
};
use serde_json::{Value, json};
use synvelis_app::binding::VIEW_STATE_TARGET;
use synvelis_app::{InspectorRow, InspectorScreenState, MainScreenState, mixer_gain_target};
use synvelis_workspace::ArrangementRow;
use synvelis_workspace::controls_ui::{TRANSPORT_POSITION_TARGET, TRANSPORT_TARGET};

// ---------------------------------------------------------------------
// scenario / case ids (synvelis product namespace)
// ---------------------------------------------------------------------

/// Main screen scenario id — the `main_screen_spec` surface.
pub const SCENARIO_MAIN_SCREEN: &str = "synvelis.main.screen";
/// Secondary window scenario id — the `secondary_screen_spec` deck.
pub const SCENARIO_SECONDARY_SCREEN: &str = "synvelis.secondary.screen";

pub const CASE_SEEK_PARITY: &str = "seek-position-parity";
pub const CASE_FADER_PARITY: &str = "fader-gain-parity";
pub const CASE_TRANSPORT_PLAY: &str = "transport-play-invoke";
pub const CASE_ARRANGEMENT_SELECT: &str = "arrangement-select-row";
pub const CASE_TAB_CYCLE: &str = "tab-cycle";
pub const CASE_SECONDARY_SEEK: &str = "secondary-seek-parity";
pub const CASE_READ_ONLY_INSPECTOR: &str = "inspector-read-only";

// ---------------------------------------------------------------------
// component keys (must match `screens.rs` literals)
// ---------------------------------------------------------------------

pub const KEY_SEEK_POSITION: &str = "seek.position";
pub const KEY_FADER_GAIN: &str = "fader.gain";
pub const KEY_TRANSPORT_PLAY: &str = "transport.play";
pub const KEY_SCREEN_TABS: &str = "screen.tabs";
pub const KEY_ARRANGEMENT_TABLE: &str = "arrangement.table";
pub const KEY_INSPECTOR_TABLE: &str = "inspector.table";
pub const KEY_SEEK_POSITION_SECONDARY: &str = "seek2.position";
pub const KEY_TRANSPORT_PLAY_SECONDARY: &str = "transport2.play";

/// Tabs toured by the cycle scenario — every tab except the spec's
/// initial active index 0 (Arrangement).
pub const TAB_CYCLE_INDICES: [usize; 4] = [1, 2, 3, 4];

/// Plain-domain values the gesture scenarios commit.
const SEEK_PLAIN: f64 = 8.0;
const FADER_PLAIN: f64 = 0.4;
/// Authoritative projection revision the scenarios post back.
const PROJECTION_REVISION: u64 = 1;

/// Capabilities no semantic adapter may claim — asserted absent from
/// every report this lane produces.
pub const HONESTY_ABSENT: [ScenarioCapability; 5] = [
    ScenarioCapability::SimulatedPointerOrKeyboard,
    ScenarioCapability::RealOsPointerOrKeyboard,
    ScenarioCapability::Ime,
    ScenarioCapability::VisualCapture,
    ScenarioCapability::NativePluginDock,
];

// ---------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------

/// The shared screen fixture — 120 s project length (Seek range), two
/// arrangement rows so `arrangement.table` is selectable, and a
/// populated Inspector breakdown so the read-only table has rows.
#[must_use]
pub fn screen_state() -> MainScreenState {
    MainScreenState {
        project_length_seconds: 120.0,
        arrangement_rows: vec![
            ArrangementRow {
                track: "t1".to_string(),
                lane: "lane-1".to_string(),
                clip: "clip-1".to_string(),
                source: "src-1".to_string(),
                start_seconds: Some(0.0),
                end_seconds: Some(2.0),
                muted: false,
            },
            ArrangementRow {
                track: "t1".to_string(),
                lane: "lane-1".to_string(),
                clip: "clip-2".to_string(),
                source: "src-2".to_string(),
                start_seconds: Some(4.0),
                end_seconds: Some(6.5),
                muted: true,
            },
        ],
        inspector: InspectorScreenState {
            target: Some("param-abc".to_string()),
            rows: vec![InspectorRow {
                field: "mode".to_string(),
                value: "Manual".to_string(),
            }],
        },
        ..MainScreenState::default()
    }
}

// ---------------------------------------------------------------------
// scenario plumbing (driver-neutral)
// ---------------------------------------------------------------------

fn key(name: &str) -> ComponentKey {
    ComponentKey(name.to_string())
}

/// Resolves `component` on `view`/`window` into a pinned
/// [`ScenarioTarget`] narrowed to the component, then pre-flights the
/// `SemanticAction` capability every scenario here requires.
///
/// # Errors
/// target 解決・capability pre-flight の `ScenarioError`。
pub fn resolve_component(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
    component: &str,
) -> Result<(ScenarioTarget, ComponentKey), ScenarioError> {
    let component_key = key(component);
    let target =
        gui.resolve_target(&TargetQuery::new(window, view).component(component_key.clone()))?;
    gui.require_capabilities(&BTreeSet::from([ScenarioCapability::SemanticAction]))?;
    Ok((target, component_key))
}

/// Posts an `Applied` receipt for every minted intent — the
/// consumer→view direction every driven control settles through.
fn post_applied_receipts(
    gui: &mut impl SemanticGuiTest,
    target: &ScenarioTarget,
    intents: &[MintedIntent],
) -> Result<(), ScenarioError> {
    for minted in intents {
        gui.post_event(
            target,
            &Event::Result {
                id: minted.command_id.clone(),
                ok: true,
                data: Some(json!({ "status": "applied" })),
                error: None,
            },
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
// outcomes
// ---------------------------------------------------------------------

/// Evidence one gesture scenario returns: the pinned target, the
/// minted intents, and the post-settle component snapshot.
pub struct GestureOutcome {
    /// Pin 済み target(component へ narrow)。
    pub target: ScenarioTarget,
    /// gesture が発行した intent 列(Begin/Preview/Commit)。
    pub intents: Vec<MintedIntent>,
    /// receipt + projection 投稿後の component snapshot。
    pub snapshot: ComponentSnapshot,
}

/// Evidence one discrete-action scenario returns (Invoke / row
/// select / tab select share the shape).
pub struct ActionOutcome {
    /// Pin 済み target(component へ narrow)。
    pub target: ScenarioTarget,
    /// 発行された intent 列。
    pub intents: Vec<MintedIntent>,
    /// Applied receipt 着弾後の component snapshot。
    pub snapshot: ComponentSnapshot,
}

/// Tab-cycle evidence: per-press intents plus the final displayed
/// selection.
pub struct TabCycleOutcome {
    /// Pin 済み target(`screen.tabs` へ narrow)。
    pub target: ScenarioTarget,
    /// 各 press が発行した intent 列(巡回順)。
    pub intents: Vec<MintedIntent>,
    /// 最終 snapshot の表示 tab index。
    pub selected: Option<usize>,
}

// ---------------------------------------------------------------------
// scenarios — identical bodies on every adapter
// ---------------------------------------------------------------------

/// `parity_gesture_scenario`(Begin→Preview→Commit)on `component`
/// に、Applied receipt と authoritative projection の post を続ける
/// driver-neutral scenario。`control_target` は projection の
/// `control_id`(seek は `transport/position`、fader は tagged param
/// id)。戻り値は gesture intents と最終 snapshot。
///
/// # Errors
/// target 解決・capability pre-flight・各 port 操作の
/// `ScenarioError`。commit intent が発行されなければ `Driver` error。
pub fn gesture_parity_scenario(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
    component: &str,
    gesture_id: &str,
    plain: f64,
    control_target: &str,
) -> Result<GestureOutcome, ScenarioError> {
    let (target, component_key) = resolve_component(gui, window, view, component)?;
    let (intents, _) = parity_gesture_scenario(
        gui,
        &target,
        &component_key,
        gesture_id,
        ValueInput::Plain(plain),
    )?;

    // The commit's authoritative receipt + projection post back
    // through the consumer→view direction — the same `deliver_event`
    // decode every driver uses.
    let commit_id = intents
        .iter()
        .rev()
        .find_map(|m| match &m.intent {
            CommandIntent::Commit { .. } => Some(m.command_id.clone()),
            _ => None,
        })
        .ok_or_else(|| ScenarioError::Driver("gesture minted no commit intent".to_string()))?;
    gui.post_event(
        &target,
        &Event::Result {
            id: commit_id,
            ok: true,
            data: Some(json!({ "status": "applied", "revision": PROJECTION_REVISION })),
            error: None,
        },
    )?;
    gui.post_event(
        &target,
        &Event::Notify {
            seq: PROJECTION_REVISION,
            data: json!({
                "control_id": control_target,
                "revision": PROJECTION_REVISION,
                "value": plain,
            }),
        },
    )?;
    gui.settle()?;
    let snapshot = gui.observe(&target, &component_key)?;
    Ok(GestureOutcome {
        target,
        intents,
        snapshot,
    })
}

/// Main screen `seek.position` の gesture parity — commit 値 8.0 秒、
/// projection target は `transport/position`。
///
/// # Errors
/// [`gesture_parity_scenario`] と同じ。
pub fn seek_parity_scenario(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
) -> Result<GestureOutcome, ScenarioError> {
    gesture_parity_scenario(
        gui,
        window,
        view,
        KEY_SEEK_POSITION,
        "sv112-seek",
        SEEK_PLAIN,
        TRANSPORT_POSITION_TARGET,
    )
}

/// Secondary window `seek2.position` の gesture parity — 同一の
/// `transport/position` control target を別 view instance で駆動する
/// (`MultiWindow` 証拠)。
///
/// # Errors
/// [`gesture_parity_scenario`] と同じ。
pub fn secondary_seek_parity_scenario(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
) -> Result<GestureOutcome, ScenarioError> {
    gesture_parity_scenario(
        gui,
        window,
        view,
        KEY_SEEK_POSITION_SECONDARY,
        "sv112-seek2",
        SEEK_PLAIN,
        TRANSPORT_POSITION_TARGET,
    )
}

/// `fader.gain` の gesture parity — commit 値 0.4(normalized==plain
/// domain [0,1])、projection target は bootstrap master-strip の
/// tagged param id([`mixer_gain_target`])。
///
/// # Errors
/// [`gesture_parity_scenario`] と同じ。
pub fn fader_parity_scenario(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
) -> Result<GestureOutcome, ScenarioError> {
    gesture_parity_scenario(
        gui,
        window,
        view,
        KEY_FADER_GAIN,
        "sv112-fader",
        FADER_PLAIN,
        &mixer_gain_target(),
    )
}

/// Button `Invoke` + Applied receipt の driver-neutral scenario。
/// `transport.play` は `transport` target に `play` action を 1 件
/// mint する(discrete `Action` intent)。
///
/// # Errors
/// target 解決・capability pre-flight・port 操作の `ScenarioError`。
pub fn invoke_scenario(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
    component: &str,
) -> Result<ActionOutcome, ScenarioError> {
    let (target, component_key) = resolve_component(gui, window, view, component)?;
    gui.perform(
        &target,
        GuiAction::Invoke {
            key: component_key.clone(),
        },
    )?;
    gui.settle()?;
    let intents = gui.drain_intents();
    post_applied_receipts(gui, &target, &intents)?;
    gui.settle()?;
    let snapshot = gui.observe(&target, &component_key)?;
    Ok(ActionOutcome {
        target,
        intents,
        snapshot,
    })
}

/// Selectable `Table` の row 選択 scenario — `select_row` が view-local
/// `select` Action(`{"row": n}` on `VIEW_STATE_TARGET`)を mint し、
/// Applied receipt で表示選択が確定する。
///
/// # Errors
/// target 解決・capability pre-flight・port 操作の `ScenarioError`。
pub fn select_row_scenario(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
    component: &str,
    row: usize,
) -> Result<ActionOutcome, ScenarioError> {
    let (target, component_key) = resolve_component(gui, window, view, component)?;
    gui.select_row(&target, &component_key, row)?;
    gui.settle()?;
    let intents = gui.drain_intents();
    post_applied_receipts(gui, &target, &intents)?;
    gui.settle()?;
    let snapshot = gui.observe(&target, &component_key)?;
    Ok(ActionOutcome {
        target,
        intents,
        snapshot,
    })
}

/// `Tabs` の tab 巡回 scenario — `indices` の各 tab を順に
/// `select_tab` し、press ごとに Applied receipt を着弾させる。
/// 各 press は view-local `tab-select` Action(`{"index": i}` on
/// `VIEW_STATE_TARGET`)を 1 件 mint する。
///
/// # Errors
/// target 解決・capability pre-flight・port 操作の `ScenarioError`。
pub fn tab_cycle_scenario(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
    component: &str,
    indices: &[usize],
) -> Result<TabCycleOutcome, ScenarioError> {
    let (target, component_key) = resolve_component(gui, window, view, component)?;
    let mut intents = Vec::new();
    for &index in indices {
        gui.select_tab(&target, &component_key, index)?;
        gui.settle()?;
        let minted = gui.drain_intents();
        post_applied_receipts(gui, &target, &minted)?;
        gui.settle()?;
        intents.extend(minted);
    }
    let snapshot = gui.observe(&target, &component_key)?;
    Ok(TabCycleOutcome {
        target,
        intents,
        selected: snapshot.selected,
    })
}

/// Inspector breakdown `Table` の read-only 検証 — mounted・選択なし・
/// row select が reject されること( silent accept しない)。
///
/// # Errors
/// target 解決・capability pre-flight・observe の `ScenarioError`。
pub fn inspector_table_is_read_only(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
) -> Result<bool, ScenarioError> {
    let (target, component_key) = resolve_component(gui, window, view, KEY_INSPECTOR_TABLE)?;
    let table = gui.observe(&target, &component_key)?;
    if !table.mounted || table.selected.is_some() {
        return Ok(false);
    }
    Ok(gui.select_row(&target, &component_key, 0).is_err())
}

// ---------------------------------------------------------------------
// product oracle — identical assertions on every adapter
// ---------------------------------------------------------------------

/// `seek.position`/`seek2.position` の gesture outcome oracle —
/// Begin/Preview/Commit が `transport/position` を target に mint
/// され、receipt+projection 後に `Observed` phase・revision 1・
/// 8.0 秒の authoritative display に settle する。
#[track_caller]
pub fn assert_seek_parity(outcome: &GestureOutcome) {
    assert_eq!(
        outcome.intents.len(),
        3,
        "begin+preview+commit: {:?}",
        outcome.intents
    );
    for minted in &outcome.intents {
        let intent_target = match &minted.intent {
            CommandIntent::Begin { target, .. }
            | CommandIntent::Preview { target, .. }
            | CommandIntent::Commit { target, .. } => target,
            other => panic!("gesture intent expected, got {other:?}"),
        };
        assert_eq!(intent_target, TRANSPORT_POSITION_TARGET);
    }
    let snap = &outcome.snapshot;
    assert_eq!(snap.phase, CommandPhase::Observed);
    let observed = snap.observed.as_ref().expect("projection observed");
    assert_eq!(observed.revision, Revision::Num(PROJECTION_REVISION));
    assert_eq!(observed.value, json!(SEEK_PLAIN));
    let display = snap.display.as_ref().expect("controlled display");
    assert!(
        (display.plain - SEEK_PLAIN).abs() < 1e-9,
        "display settles on the seconds position: {display:?}"
    );
}

/// `fader.gain` の gesture outcome oracle — Begin/Preview/Commit が
/// master-strip param target を target に mint され、receipt+
/// projection 後に `Observed` phase・0.4 の authoritative display に
/// settle する。
#[track_caller]
pub fn assert_fader_parity(outcome: &GestureOutcome) {
    let target = mixer_gain_target();
    assert_eq!(
        outcome.intents.len(),
        3,
        "begin+preview+commit: {:?}",
        outcome.intents
    );
    for minted in &outcome.intents {
        let intent_target = match &minted.intent {
            CommandIntent::Begin { target, .. }
            | CommandIntent::Preview { target, .. }
            | CommandIntent::Commit { target, .. } => target,
            other => panic!("gesture intent expected, got {other:?}"),
        };
        assert_eq!(intent_target, &target);
    }
    let snap = &outcome.snapshot;
    assert_eq!(snap.phase, CommandPhase::Observed);
    let observed = snap.observed.as_ref().expect("projection observed");
    assert_eq!(observed.revision, Revision::Num(PROJECTION_REVISION));
    assert_eq!(observed.value, json!(FADER_PLAIN));
    let display = snap.display.as_ref().expect("controlled display");
    assert!(
        (display.plain - FADER_PLAIN).abs() < 1e-9,
        "display settles on the committed gain: {display:?}"
    );
}

/// `transport.play`/`transport2.play` の invoke outcome oracle —
/// `transport` target に `play` action の discrete `Action` intent が
/// 1 件 mint され、Applied receipt で button phase が `Applied` に
/// settle する。
#[track_caller]
pub fn assert_transport_play(outcome: &ActionOutcome) {
    assert_eq!(
        outcome.intents.len(),
        1,
        "one action intent: {:?}",
        outcome.intents
    );
    match &outcome.intents[0].intent {
        CommandIntent::Action {
            target,
            action,
            params,
            ..
        } => {
            assert_eq!(target, TRANSPORT_TARGET);
            assert_eq!(action, "play");
            assert_eq!(*params, None);
        }
        other => panic!("action intent expected: {other:?}"),
    }
    let snap = &outcome.snapshot;
    assert!(snap.mounted);
    assert_eq!(snap.phase, CommandPhase::Applied);
}

/// Arrangement row 選択の outcome oracle — `select` Action
/// (`{"row": row}` on `VIEW_STATE_TARGET`)が 1 件 mint され、Applied
/// receipt で表示選択が `row` に settle する。
#[track_caller]
pub fn assert_row_select(outcome: &ActionOutcome, row: usize) {
    assert_eq!(
        outcome.intents.len(),
        1,
        "one select intent: {:?}",
        outcome.intents
    );
    match &outcome.intents[0].intent {
        CommandIntent::Action {
            target,
            action,
            params,
            ..
        } => {
            assert_eq!(target, VIEW_STATE_TARGET);
            assert_eq!(action, TABLE_SELECT_ACTION);
            assert_eq!(*params, Some(json!({ "row": row })));
        }
        other => panic!("select intent expected: {other:?}"),
    }
    let snap = &outcome.snapshot;
    assert!(snap.mounted);
    assert_eq!(snap.phase, CommandPhase::Applied);
    assert_eq!(snap.selected, Some(row));
}

/// Tab 巡回の outcome oracle — `indices` と同数の `tab-select`
/// Action(`{"index": i}` on `VIEW_STATE_TARGET`)が巡回順に mint
/// され、最終表示 tab が最後の index に settle する。
#[track_caller]
pub fn assert_tab_cycle(outcome: &TabCycleOutcome, indices: &[usize]) {
    assert_eq!(
        outcome.intents.len(),
        indices.len(),
        "one tab-select intent per press: {:?}",
        outcome.intents
    );
    for (minted, &index) in outcome.intents.iter().zip(indices) {
        match &minted.intent {
            CommandIntent::Action {
                target,
                action,
                params,
                ..
            } => {
                assert_eq!(target, VIEW_STATE_TARGET);
                assert_eq!(action, TAB_SELECT_ACTION);
                assert_eq!(*params, Some(json!({ "index": index })));
            }
            other => panic!("tab-select intent expected: {other:?}"),
        }
    }
    assert_eq!(outcome.selected, indices.last().copied());
}

// ---------------------------------------------------------------------
// honesty checks — identical on every adapter
// ---------------------------------------------------------------------

/// Generation pin の stale 検証 — `component` へ resolve したあと、
/// `generation + 1` pin の再 resolve が `StaleGeneration` で reject
/// されることを確認する(全 adapter 共通の stale-pin gate)。
///
/// # Errors
/// resolve/pin 検証の `ScenarioError`。pin が通ってしまった場合は
/// panic(assert 内)。
pub fn assert_stale_generation_pin_rejected(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
    component: &str,
) -> Result<(), ScenarioError> {
    let (target, component_key) = resolve_component(gui, window, view, component)?;
    let err = gui
        .resolve_target(
            &TargetQuery::new(window, view)
                .component(component_key)
                .generation(target.generation + 1),
        )
        .expect_err("wrong generation pin must fail");
    assert!(
        matches!(err, ScenarioError::StaleGeneration { .. }),
        "expected StaleGeneration, got {err:?}"
    );
    Ok(())
}

/// Capability pre-flight の honesty 検証 — semantic lane が申告
/// しない capability は全て `Unsupported` error で reject される。
#[track_caller]
pub fn assert_unclaimed_capabilities_rejected(gui: &mut impl SemanticGuiTest) {
    for capability in HONESTY_ABSENT {
        let err = gui
            .require_capabilities(&BTreeSet::from([capability]))
            .expect_err("capability must be unsupported");
        assert!(
            matches!(err, ScenarioError::Unsupported { .. }),
            "{capability:?}: {err:?}"
        );
    }
}

/// Visual honesty 検証 — pixel capture は `Unsupported` outcome、
/// golden step も `Unsupported` verdict を返す(draw plan や semantic
/// snapshot を pixel 証拠に昇格させない)。`driver_class` は lane が
/// 正直に名乗る adapter class(`"fake"`/`"gpui-test"`)。
#[track_caller]
pub fn assert_visual_honesty<G>(
    gui: &mut G,
    target: &ScenarioTarget,
    driver_class: &str,
    baseline_dir: &Path,
) where
    G: SemanticGuiTest + VisualGuiTest,
{
    let outcome = gui
        .capture_frame(target)
        .expect("capture_frame returns an honest outcome");
    assert!(
        matches!(outcome, CaptureOutcome::Unsupported { .. }),
        "capture must be Unsupported, not pixel evidence: {outcome:?}"
    );

    let store = BaselineStore::new(baseline_dir.to_path_buf());
    let key = GoldenKey {
        story_id: SCENARIO_MAIN_SCREEN.to_string(),
        theme: "test".to_string(),
        state: "idle".to_string(),
        driver_class: driver_class.to_string(),
        os_class: std::env::consts::OS.to_string(),
        gpu_class: None,
        scale_factor: 1.0,
        font_class: None,
        revision: "test".to_string(),
    };
    let comparison = gui
        .golden_check(target, &key, None, &store)
        .expect("golden_check returns a report");
    assert_eq!(comparison.report.verdict, GoldenVerdict::Unsupported);
}

// ---------------------------------------------------------------------
// ScenarioReport — the adapter's own evidence level, never upgraded
// ---------------------------------------------------------------------

/// `ScenarioReport` が adapter の正直な証拠水準を保持していること
/// を検証する oracle — `capabilities` は `{SemanticAction,
/// MultiWindow}` のみで、pointer/real-OS/IME/pixel/native-dock の
/// 強い証拠は一切申告されない。全 lane で同一。
#[track_caller]
pub fn assert_honest_capabilities(report: &ScenarioReport) {
    assert_eq!(
        report.capabilities,
        BTreeSet::from([
            ScenarioCapability::SemanticAction,
            ScenarioCapability::MultiWindow,
        ])
    );
    for absent in HONESTY_ABSENT {
        assert!(
            !report.capabilities.contains(&absent),
            "{absent:?} must not be claimed by a semantic adapter"
        );
    }
}

/// `ScenarioReport` の骨格 — adapter 実測の `driver()`/`capabilities()`
/// をそのまま記録する。`verdict`/`expected`/`observed` は証拠に応じて
/// caller が設定する — fake/gpui-simulated の pass を実機・pixel
/// pass へ昇格させない。
#[must_use]
pub fn screen_scenario_report(
    gui: &impl SemanticGuiTest,
    run: &str,
    scenario: &str,
    case: &str,
) -> ScenarioReport {
    let mut report = ScenarioReport::new(
        RunId::new(run),
        ScenarioId::new(scenario),
        Some(CaseId::new(case)),
        gui.driver(),
    );
    report.capabilities = gui.capabilities();
    report.required = BTreeSet::from([ScenarioCapability::SemanticAction]);
    report.environment = Some(json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    }));
    report
}

/// `Unsupported`/`NotRun` を正直に記録する report 骨格 — required
/// capability を caller が引き上げ、verdict を `Unsupported` に
/// 固定する外部 gate 記録用。
#[must_use]
pub fn gated_scenario_report(
    gui: &impl SemanticGuiTest,
    run: &str,
    scenario: &str,
    case: &str,
    required: BTreeSet<ScenarioCapability>,
    verdict: ScenarioVerdict,
    detail: &str,
) -> ScenarioReport {
    let mut report = screen_scenario_report(gui, run, scenario, case);
    report.required = required;
    report.verdict = verdict;
    report.detail = Some(detail.to_string());
    report
}

/// Minted intent 列を `CommandIntent::Action` の
/// `(target, action, params)` へ射影する(product oracle)。
#[must_use]
pub fn command_actions(intents: &[MintedIntent]) -> Vec<(String, String, Option<Value>)> {
    intents
        .iter()
        .filter_map(|m| match &m.intent {
            CommandIntent::Action {
                target,
                action,
                params,
                ..
            } => Some((target.clone(), action.clone(), params.clone())),
            _ => None,
        })
        .collect()
}
