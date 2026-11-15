//! TIM-03 driver-side loop-wrap boundary derivation acceptance tests —
//! Issue #127 driver slice (headless mock; spec
//! `docs/specs/tim-03-driver-loop-boundary.md`, test spec rules
//! DB-01..DB-08 in `docs/test-specs/tim-03-driver-loop-boundary.md`).
//!
//! [`HeadlessDriver`] keeps a scheduler-faithful [`WrapSim`] per
//! attached node (`AttachedDispatch::wrap_sim`), advanced through
//! every observed window by the same `due = 0` transport commands
//! the node scheduler applies. Each `stage_window` call site —
//! activation/resume anchor, regular transport staging, seek
//! re-anchor — previews the upcoming window and supplies the derived
//! `LoopWrap` [`BoundaryPoint`]s so the session oracle evaluates
//! legality against the *real* wrap trajectory.
//!
//! DB-08 evidence (the R9 analogue): the recording plugin's
//! `TransportInfo.position` is the real `TransportScheduler`'s
//! folded position at each physical call head, and calls partition
//! each block at transport-span boundaries — command applies and
//! loop wraps. Accumulating `frames` from a known window head
//! reconstructs the exact frame of every wrap the scheduler
//! performed; the tests require the oracle-evaluated
//! `attached_window_plan(..).boundaries` to equal those observed
//! interior cuts, including across `SetLoop`/`Seek`/`ClearLoop`
//! where the lazy-fold `TransportMirror` would diverge.
//!
//! Node cursor domain: the node's own cursor starts at 0, advances
//! one block per processed callback and re-anchors on `set_clock` —
//! it equals the driver cursor except inside `seek`'s quiesce
//! passes, which the tests drain away before comparing.

use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, attach};
use ruviform_media_core::{
    BlockContract, DeliveryPolicy, InBlockTransportSupport, ScheduleBudget, TimeBase,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, Event,
    EventList, ParamFlags, ParamId, ParamInfo, Plugin, PluginDescriptor, PluginFactory,
    PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo,
    erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::lowering::{EngineConfig, lower_with_inserts};
use synvelis_player::process_boundary::{
    BoundarySessionSpec, ExecutionMode, ProcessCapability, TimingPrecision,
};
use synvelis_player::{AttachedNode, HeadlessDriver, ParamDispatch};
use synvelis_project::{Project, ProjectId};

const BLOCK: u64 = 128;
const BLOCK_U32: u32 = 128;

// ---------------------------------------------------------------------
// recording plugin fixture (tim03_loop_boundary CallPos shape)
// ---------------------------------------------------------------------

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim03-dlb",
    name: "TIM-03 DLB Recorder",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const BUSES: [BusDescriptor; 2] = [
    BusDescriptor {
        direction: BusDirection::Input,
        layout: ChannelLayout::Stereo,
        name: "Main In",
        role: BusRole::Main,
        enabled: true,
    },
    BusDescriptor {
        direction: BusDirection::Output,
        layout: ChannelLayout::Stereo,
        name: "Main Out",
        role: BusRole::Main,
        enabled: true,
    },
];

const PARAMS: [ParamInfo; 1] = [ParamInfo {
    id: ParamId(7),
    name: "Gain",
    unit: "",
    min_value: 0.0,
    max_value: 1.0,
    default_value: 0.0,
    flags: ParamFlags::AUTOMATABLE,
}];

/// One physical `process` call's scheduler evidence: the folded
/// transport position at the call head (`-1` when transport info was
/// absent) and the call's frame count. Calls partition each block at
/// transport-span boundaries — command applies and loop wraps — so
/// accumulating `frames` from a known head reconstructs the exact
/// frame of every wrap the real scheduler performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallPos {
    position: i64,
    frames: u32,
}

type Positions = Arc<Mutex<Vec<CallPos>>>;

struct RecPlugin {
    positions: Positions,
}

impl PluginProcessor for RecPlugin {
    fn prepare(
        &mut self,
        _config: &ProcessConfig,
        _context: &mut ProcessContext,
    ) -> Result<(), PrepareError> {
        Ok(())
    }

    fn process<I, O>(
        &mut self,
        _inputs: &[I],
        outputs: &mut [O],
        _events: EventList<'_>,
        transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        self.positions.lock().unwrap().push(CallPos {
            position: transport.map_or(-1, |t| t.position().value()),
            frames: outputs
                .first()
                .map_or(0, |b| u32::try_from(b.frames()).unwrap_or(u32::MAX)),
        });
        ProcessResult::CONTINUE
    }

    fn drain_output_reports(&mut self, _out: &mut Vec<Event>) {}
}

impl Plugin for RecPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &PARAMS
    }
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
    }
}

struct RecFactory {
    positions: Positions,
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(RecPlugin {
            positions: Arc::clone(&self.positions),
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
            positions: Arc::clone(&self.positions),
        }))
    }
}

fn fmt() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

fn engine_config() -> EngineConfig {
    EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: usize::try_from(BLOCK).unwrap(),
    }
}

/// A hosted plugin attach — the `Attached { node, control, ports }`
/// triple the driver consumes, plus the call-position ledger.
struct Hosted {
    host: PluginHost,
    attached: ruviform_audio_plugin_bridge::Attached,
    positions: Positions,
}

fn host_plugin() -> Hosted {
    let positions = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        positions: Arc::clone(&positions),
    }));
    let HostReply::Instantiated { handle } = host
        .execute(HostCommand::Instantiate {
            plugin_id: DESCRIPTOR.id.to_owned(),
        })
        .unwrap()
    else {
        panic!("Instantiate")
    };
    host.execute(HostCommand::Initialize { handle }).unwrap();
    let process_config = ProcessConfig::new(SampleRate::new(48_000).unwrap(), 1, 512).unwrap();
    host.execute(HostCommand::Activate {
        handle,
        config: process_config,
    })
    .unwrap();
    host.execute(HostCommand::StartProcessing { handle })
        .unwrap();
    let attached = attach(
        &mut host,
        handle,
        &AttachConfig {
            buses: BUSES.to_vec(),
            port_format: fmt(),
            bounds: EndpointBounds {
                max_frames: 512,
                channels: 2,
            },
            process_config,
            frame_base: frame_base(),
            policy: DeliveryPolicy::Exact,
            budget: ScheduleBudget::default(),
            input: None,
        },
    )
    .expect("attach");
    Hosted {
        host,
        attached,
        positions,
    }
}

/// The attach-resolved boundary inputs a declared processor's
/// composition root would vend — Exact realtime delivery, no consent.
fn spec() -> BoundarySessionSpec {
    BoundarySessionSpec {
        capability: ProcessCapability::new(
            BlockContract::new(1, 512, InBlockTransportSupport::Unknown),
            TimingPrecision::FrameAccurateEvents,
        ),
        mode: ExecutionMode::Realtime,
        consent: None,
        budget: ScheduleBudget::default(),
    }
}

/// Lowered insert slots for one attached node on an empty project.
fn lowered_inserts() -> (synvelis_player::lowering::Lowered, ruviform_graph::NodeId) {
    let project = Project::new(ProjectId::new(0), "tim03-dlb");
    let lowered = lower_with_inserts(
        &project,
        &|_: synvelis_project::SourceId| -> Option<AudioFormat> { None },
        &engine_config(),
        1,
    );
    let node_id = lowered.inserts[0].node_id;
    (lowered, node_id)
}

/// Activates a driver with one attached node — `boundary: Some(spec)`
/// opens the TIM-03 session, `None` keeps the legacy lenient path.
/// Returns the driver, the node id, the position ledger and the host
/// (kept alive for the driver's lifetime).
fn driver_with(
    boundary: Option<BoundarySessionSpec>,
) -> (
    HeadlessDriver,
    ruviform_graph::NodeId,
    Positions,
    PluginHost,
) {
    let (lowered, node_id) = lowered_inserts();
    let hosted = host_plugin();
    let positions = Arc::clone(&hosted.positions);
    let node = hosted.attached.node;
    let control = hosted.attached.control;
    let host = hosted.host;
    let mut driver = HeadlessDriver::new(engine_config());
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node,
                dispatch: ParamDispatch::new(control, node_id, frame_base()).unwrap(),
                boundary,
            }],
        )
        .expect("activate");
    (driver, node_id, positions, host)
}

fn drain(positions: &Positions) -> Vec<CallPos> {
    std::mem::take(&mut *positions.lock().unwrap())
}

/// Folds recorded calls into `(absolute_start_frame, position,
/// frames)` — the cursor starts at `first_head` (the node frame the
/// first recorded call covered) and each call advances it by its
/// frame count, so call starts land exactly on transport-span
/// boundaries: command applies and loop wraps.
fn call_starts(records: &[CallPos], first_head: u64) -> Vec<(u64, i64, u32)> {
    let mut frame = first_head;
    records
        .iter()
        .map(|r| {
            let start = frame;
            frame += u64::from(r.frames);
            (start, r.position, r.frames)
        })
        .collect()
}

/// The window-interior span cuts `(window offset)` the plugin
/// observed inside `[head, head + BLOCK)` — head applies (offset 0)
/// never split a span, so every interior cut is a real scheduler
/// wrap (driver commands are all `due = 0`).
fn interior_cuts(calls: &[(u64, i64, u32)], head: u64) -> Vec<u32> {
    calls
        .iter()
        .filter(|(start, _, _)| *start > head && *start < head + BLOCK)
        .map(|(start, _, _)| u32::try_from(start - head).unwrap_or(u32::MAX))
        .collect()
}

/// The oracle-evaluated boundary offsets the session last saw for
/// the upcoming window — sorted, deduplicated, all `< BLOCK`.
fn plan_boundaries(driver: &HeadlessDriver, node: ruviform_graph::NodeId) -> Vec<u32> {
    let plan = driver
        .attached_window_plan(node)
        .expect("sessioned node has a window plan");
    for &b in &plan.boundaries {
        assert!(b < BLOCK_U32, "boundary {b} outside [0, {BLOCK_U32})");
    }
    plan.boundaries.clone()
}

/// Runs one block covering node window `[head, head + BLOCK)` and
/// returns the plugin-observed `(start, position, frames)` calls.
fn run_window(
    driver: &mut HeadlessDriver,
    positions: &Positions,
    head: u64,
) -> Vec<(u64, i64, u32)> {
    assert!(drain(positions).is_empty(), "stale recorded calls");
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    call_starts(&drain(positions), head)
}

/// DB-01/DB-05/DB-08: activation anchor, then steady loop wraps
/// across consecutive windows — every staged window's oracle-evaluated
/// `boundaries` equal the interior cuts the real scheduler produced,
/// including a same-window double wrap.
#[test]
fn tim03_dlb_01_activation_anchor_and_steady_loop() {
    let (mut driver, node, positions, _hosted) = driver_with(Some(spec()));

    // Window [0,128): the activation anchor (Seek(0)+Play at head)
    // leaves a single unsplit span at position 0.
    let calls = run_window(&mut driver, &positions, 0);
    assert_eq!(calls, vec![(0, 0, BLOCK_U32)]);
    assert_eq!(interior_cuts(&calls, 0), Vec::<u32>::new());
    // The anchor plan covered [0,128): head applies are offset-0
    // boundaries — filtered out of the applied set, so empty.
    assert_eq!(plan_boundaries(&driver, node), Vec::<u32>::new());

    // `set_loop` staged at cursor 128 → plan covers [128,256): the
    // wrap at node frame 200 is offset 72.
    driver.set_loop(100, 200).expect("set_loop");
    assert_eq!(plan_boundaries(&driver, node), vec![72]);
    let calls = run_window(&mut driver, &positions, 128);
    assert_eq!(interior_cuts(&calls, 128), vec![72]);
    assert_eq!(calls, vec![(128, 128, 72), (200, 100, 56)]);

    // Same loop re-staged → [256,384): position 156 wraps at offset 44.
    driver.set_loop(100, 200).expect("set_loop");
    assert_eq!(plan_boundaries(&driver, node), vec![44]);
    let calls = run_window(&mut driver, &positions, 256);
    assert_eq!(interior_cuts(&calls, 256), vec![44]);
    assert_eq!(calls, vec![(256, 156, 44), (300, 100, 84)]);

    // [384,512): two wraps — offsets 16 and 116.
    driver.set_loop(100, 200).expect("set_loop");
    assert_eq!(plan_boundaries(&driver, node), vec![16, 116]);
    let calls = run_window(&mut driver, &positions, 384);
    assert_eq!(interior_cuts(&calls, 384), vec![16, 116]);
    assert_eq!(calls, vec![(384, 184, 16), (400, 100, 100), (500, 100, 12)]);

    // [512,640): position 112 wraps once at offset 88.
    driver.set_loop(100, 200).expect("set_loop");
    assert_eq!(plan_boundaries(&driver, node), vec![88]);
    let calls = run_window(&mut driver, &positions, 512);
    assert_eq!(interior_cuts(&calls, 512), vec![88]);
    assert_eq!(calls, vec![(512, 112, 88), (600, 100, 40)]);
}

/// DB-02/DB-04/DB-06: a wrap landing exactly on the window end is the
/// *next* window's boundary — excluded from the current plan, then
/// delivered as a head wrap (offset 0) there. Reached through the
/// seek re-anchor path: `seek(72)` leaves the raw position below the
/// loop start so it climbs to the loop end exactly at the window end.
#[test]
fn tim03_dlb_02_window_end_wrap_belongs_to_next_window() {
    let (mut driver, node, positions, _hosted) = driver_with(Some(spec()));
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let _ = drain(&positions);
    driver.set_loop(100, 200).expect("set_loop");
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let _ = drain(&positions);

    // Seek re-anchor at frame 72: quiesce passes drain, then
    // set_clock+Seek re-anchor the node — window [72,200) starts at
    // raw position 72, climbing to the loop end 200 exactly at the
    // window end (offset 128).
    driver.seek(72).expect("seek");
    // The quiesce passes recorded their own calls — discard them;
    // the re-anchored window starts at node frame 72.
    let _ = drain(&positions);
    let calls = run_window(&mut driver, &positions, 72);
    // The stage_seek plan covered [72,200): Seek's head point and
    // the window-end wrap are both offset-0/later-window boundaries —
    // correctly absent from the applied set.
    assert_eq!(plan_boundaries(&driver, node), Vec::<u32>::new());
    // One unsplit span: position 72 → 200 with no interior wrap.
    assert_eq!(calls, vec![(72, 72, BLOCK_U32)]);

    // Next window [200,328): the deferred wrap fires at the head
    // (offset 0 — a head boundary, never a cut), then the position
    // wraps again at offset 100.
    driver.set_loop(100, 200).expect("set_loop");
    // Head wrap (offset 0) filtered; the offset-100 wrap remains.
    assert_eq!(plan_boundaries(&driver, node), vec![100]);
    let calls = run_window(&mut driver, &positions, 200);
    assert_eq!(interior_cuts(&calls, 200), vec![100]);
    assert_eq!(calls, vec![(200, 100, 100), (300, 100, 28)]);
}

/// DB-03: with no loop installed and after `panic` (Stopped freezes
/// the position), derived boundaries contain no interior offsets and
/// the plugin observes unsplit spans.
#[test]
fn tim03_dlb_03_no_loop_and_stopped_have_no_wraps() {
    let (mut driver, node, positions, _hosted) = driver_with(Some(spec()));
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let _ = drain(&positions);

    // `clear_loop` with no region stages a window evaluation: the
    // preview derives nothing — no loop, no wraps.
    driver.clear_loop().expect("clear_loop");
    assert_eq!(plan_boundaries(&driver, node), Vec::<u32>::new());
    let calls = run_window(&mut driver, &positions, 128);
    assert_eq!(calls, vec![(128, 128, BLOCK_U32)]);

    // `panic` → Stop at the next head: position freezes at 0, no
    // wraps can derive.
    driver.panic().expect("panic");
    assert_eq!(plan_boundaries(&driver, node), Vec::<u32>::new());
    let calls = run_window(&mut driver, &positions, 256);
    assert_eq!(calls, vec![(256, 0, BLOCK_U32)]);
}

/// DB-04/DB-07: a mid-stream `SetLoop` region change shifts later
/// wraps exactly like the scheduler (position below the new start
/// climbs to the new end), and `ClearLoop` removes them — the oracle
/// sees both transitions through the staged previews.
#[test]
fn tim03_dlb_04_set_loop_change_and_clear_loop() {
    let (mut driver, node, positions, _hosted) = driver_with(Some(spec()));
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let _ = drain(&positions);
    driver.set_loop(100, 200).expect("set_loop");
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let _ = drain(&positions);
    // After [128,256): position folded to 156 at cursor 256.

    // Region change to [200,260): position 156 sits below the new
    // start — the wrap moves to offset 104 (frame 360).
    driver.set_loop(200, 260).expect("set_loop");
    assert_eq!(plan_boundaries(&driver, node), vec![104]);
    let calls = run_window(&mut driver, &positions, 256);
    assert_eq!(interior_cuts(&calls, 256), vec![104]);
    assert_eq!(calls, vec![(256, 156, 104), (360, 200, 24)]);

    // `ClearLoop` staged at cursor 384: position 224 unfolds raw —
    // no wraps derive, one unsplit span.
    driver.clear_loop().expect("clear_loop");
    assert_eq!(plan_boundaries(&driver, node), Vec::<u32>::new());
    let calls = run_window(&mut driver, &positions, 384);
    assert_eq!(calls, vec![(384, 224, BLOCK_U32)]);
}

/// DB-06: `seek` into the middle of an active loop cascades same-head
/// wraps algebraically — the staged preview derives the head wrap
/// (offset 0) plus the next interior wrap, matching the scheduler.
#[test]
fn tim03_dlb_05_seek_into_active_loop_cascades() {
    let (mut driver, node, positions, _hosted) = driver_with(Some(spec()));
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let _ = drain(&positions);
    driver.set_loop(100, 200).expect("set_loop");
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let _ = drain(&positions);

    // Seek to frame 500 with loop [100,200) still active: the raw
    // position 500 sits far past the end — same-head cascade folds
    // it to 100 + (500-200) % 100 = 100 at the window head, then the
    // position wraps again at offset 100 (frame 600).
    driver.seek(500).expect("seek");
    let _ = drain(&positions);
    // Head cascade wrap is offset 0 — filtered; offset 100 remains.
    assert_eq!(plan_boundaries(&driver, node), vec![100]);
    let calls = run_window(&mut driver, &positions, 500);
    assert_eq!(interior_cuts(&calls, 500), vec![100]);
    assert_eq!(calls, vec![(500, 100, 100), (600, 100, 28)]);
}

/// DB-07: a node attached without a boundary declaration keeps the
/// legacy lenient path — no session, no window plan, and the driver
/// still runs it (the scheduler's own wrap splitting is unchanged).
#[test]
fn tim03_dlb_06_sessionless_node_stays_legacy() {
    let (mut driver, node, positions, _hosted) = driver_with(None);
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let _ = drain(&positions);
    driver.set_loop(100, 200).expect("set_loop");
    driver.run(1, PumpPolicy::Every(1)).expect("run");

    assert!(driver.attached_window_plan(node).is_none());
    // The scheduler still splits at the real wrap — legacy behavior
    // is transport-faithful, only the oracle evaluation is absent.
    let calls = call_starts(&drain(&positions), 128);
    assert_eq!(interior_cuts(&calls, 128), vec![72]);
    assert_eq!(calls, vec![(128, 128, 72), (200, 100, 56)]);
}
