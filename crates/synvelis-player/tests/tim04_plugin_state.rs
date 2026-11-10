//! TIM-04 (issue #128): `PluginStateHost` — the non-RT
//! `PluginHost` → `PluginStatePort` adapter — exercised against a
//! real `PluginHost` with stateful/stateless mock plugins.
//!
//! Coverage: describe/bind identity, `SaveState`/`RestoreState`
//! round-trip through `HostCommand`, lifecycle-observed quality
//! (Processing → `BestEffort`, stable non-processing →
//! `Coordinated`/`Quiescent`), `Unsupported` default hooks, and
//! stale-handle fail-closed after `Destroy`.

use std::cell::RefCell;
use std::rc::Rc;

use ruviform_audio_core::{ChannelLayout, SampleRate};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList,
    HostError, ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError,
    ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase,
};
use ruviform_plugin_host::{HostCommand, HostReply, PluginHandle, PluginHost};
use synvelis_player::lowering::EngineConfig;
use synvelis_player::plugin_state::PluginStateHost;
use synvelis_player::routing_engine::RoutingEngine;
use synvelis_project::{
    CaptureMode, Command, CounterIdGenerator, InMemoryPluginStateStore, PluginRef,
    PluginStateCaptureError, PluginStateFailureKind, PluginStatePort, PluginStateRestoreRequest,
    PluginStateSaveRequest, PluginStateSession, ProcessorId, Project, ProjectController, ProjectId,
    ProjectOp, RuntimeClockEpochRef, RuntimeInstanceId, StateCaptureQuality, StateQualityPolicy,
};

// ── Mock plugins ─────────────────────────────────────────────────

fn descriptor(id: &'static str) -> PluginDescriptor {
    PluginDescriptor {
        id,
        name: id,
        vendor: "Synvelis",
        version: "0.1.0",
        features: &[],
    }
}

const STATE_BUSES: [BusDescriptor; 2] = [
    BusDescriptor {
        direction: BusDirection::Input,
        layout: ChannelLayout::Stereo,
        name: "In",
        role: BusRole::Main,
        enabled: true,
    },
    BusDescriptor {
        direction: BusDirection::Output,
        layout: ChannelLayout::Stereo,
        name: "Out",
        role: BusRole::Main,
        enabled: true,
    },
];

/// Stateful plugin: `save_state` emits its gain; `restore_state`
/// applies it back. Records restore calls for assertions.
struct StatePlugin {
    gain: f32,
    restored: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl PluginProcessor for StatePlugin {
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
        _outputs: &mut [O],
        _events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        ProcessResult::CONTINUE
    }
}

impl Plugin for StatePlugin {
    fn descriptor(&self) -> PluginDescriptor {
        descriptor("org.synvelis.test.tim04-state")
    }
    fn params(&self) -> &[ParamInfo] {
        &[]
    }
    fn buses(&self) -> &[BusDescriptor] {
        &STATE_BUSES
    }
    fn save_state(&mut self) -> Result<Vec<u8>, HostError> {
        Ok(self.gain.to_le_bytes().to_vec())
    }
    fn restore_state(&mut self, state: &[u8]) -> Result<(), HostError> {
        let bytes: [u8; 4] = state.try_into().map_err(|_| HostError::PluginFailed)?;
        self.gain = f32::from_le_bytes(bytes);
        self.restored.borrow_mut().push(state.to_vec());
        Ok(())
    }
}

/// Plugin without state hooks — the default `Unsupported` surface.
struct BarePlugin;

impl PluginProcessor for BarePlugin {
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
        _outputs: &mut [O],
        _events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        ProcessResult::CONTINUE
    }
}

impl Plugin for BarePlugin {
    fn descriptor(&self) -> PluginDescriptor {
        descriptor("org.synvelis.test.tim04-bare")
    }
    fn params(&self) -> &[ParamInfo] {
        &[]
    }
}

struct StateFactory {
    restored: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl PluginFactory for StateFactory {
    fn descriptor(&self) -> PluginDescriptor {
        descriptor("org.synvelis.test.tim04-state")
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(StatePlugin {
            gain: 0.5,
            restored: Rc::clone(&self.restored),
        })
    }
}

struct BareFactory;

impl PluginFactory for BareFactory {
    fn descriptor(&self) -> PluginDescriptor {
        descriptor("org.synvelis.test.tim04-bare")
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(BarePlugin)
    }
}

// ── Host fixture ─────────────────────────────────────────────────

struct Fixture {
    host: Rc<RefCell<PluginHost>>,
    state: PluginStateHost,
    state_handle: PluginHandle,
    bare_handle: PluginHandle,
    restored: Rc<RefCell<Vec<Vec<u8>>>>,
}

/// A `PluginHost` with both factories; both instances driven to
/// `Processing` (the connect-path lifecycle).
fn fixture() -> Fixture {
    let restored = Rc::new(RefCell::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(StateFactory {
        restored: Rc::clone(&restored),
    }));
    host.register_factory(Box::new(BareFactory));

    let state_handle = instantiate(&mut host, "org.synvelis.test.tim04-state");
    let bare_handle = instantiate(&mut host, "org.synvelis.test.tim04-bare");
    for handle in [state_handle, bare_handle] {
        exec(&mut host, HostCommand::Initialize { handle });
        exec(
            &mut host,
            HostCommand::Activate {
                handle,
                config: ProcessConfig::new(SampleRate::new(48_000).unwrap(), 1, 64).unwrap(),
            },
        );
        exec(&mut host, HostCommand::StartProcessing { handle });
    }

    let host = Rc::new(RefCell::new(host));
    let mut state = PluginStateHost::new(Rc::clone(&host));
    let state_pid = ProcessorId::new(1);
    let bare_pid = ProcessorId::new(2);
    state.bind(
        state_pid,
        state_handle,
        &PluginRef {
            descriptor: "org.synvelis.test.tim04-state".into(),
            version: "0.1.0".into(),
            capability_revision: 1,
        },
        42,
    );
    state.bind(
        bare_pid,
        bare_handle,
        &PluginRef {
            descriptor: "org.synvelis.test.tim04-bare".into(),
            version: "0.1.0".into(),
            capability_revision: 1,
        },
        42,
    );
    Fixture {
        host,
        state,
        state_handle,
        bare_handle,
        restored,
    }
}

fn instantiate(host: &mut PluginHost, plugin_id: &str) -> PluginHandle {
    match host.execute(HostCommand::Instantiate {
        plugin_id: plugin_id.into(),
    }) {
        Ok(HostReply::Instantiated { handle }) => handle,
        other => panic!("instantiate failed: {other:?}"),
    }
}

fn exec(host: &mut PluginHost, command: HostCommand) {
    match host.execute(command) {
        Ok(HostReply::Ok | HostReply::State { .. }) => {}
        other => panic!("command failed: {other:?}"),
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[test]
fn describe_reports_bind_identity() {
    let f = fixture();
    let binding = f.state.describe(ProcessorId::new(1)).unwrap();
    assert_eq!(binding.descriptor, "org.synvelis.test.tim04-state");
    assert_eq!(binding.version, "0.1.0");
    assert_eq!(binding.capability_revision, 1);
    assert_eq!(binding.graph_generation, 42);
    assert_ne!(binding.host_runtime_instance, "");
    assert!(f.state.describe(ProcessorId::new(999)).is_none());
}

#[test]
fn capture_while_processing_is_best_effort() {
    let mut f = fixture();
    let capture = f
        .state
        .capture(ProcessorId::new(1), CaptureMode::Coordinated)
        .unwrap();
    // The instance is Processing — no point-in-time guarantee.
    assert_eq!(capture.achieved, StateCaptureQuality::BestEffort);
    assert!(capture.observation.processing);
    assert!(!capture.observation.stable_boundary);
    assert_eq!(capture.blob.len(), 4);
}

#[test]
fn capture_after_stop_processing_is_coordinated() {
    let mut f = fixture();
    let pid = ProcessorId::new(1);
    exec(
        &mut f.host.borrow_mut(),
        HostCommand::StopProcessing {
            handle: f.state_handle,
        },
    );
    let capture = f.state.capture(pid, CaptureMode::Coordinated).unwrap();
    assert_eq!(capture.achieved, StateCaptureQuality::Coordinated);
    assert!(!capture.observation.processing);
    assert!(capture.observation.stable_boundary);
}

#[test]
fn capture_with_quiescent_mode_records_quiescent() {
    let mut f = fixture();
    exec(
        &mut f.host.borrow_mut(),
        HostCommand::StopProcessing {
            handle: f.state_handle,
        },
    );
    let capture = f
        .state
        .capture(ProcessorId::new(1), CaptureMode::Quiescent)
        .unwrap();
    assert_eq!(capture.achieved, StateCaptureQuality::Quiescent);
}

#[test]
fn unsupported_plugin_reports_unsupported() {
    let mut f = fixture();
    exec(
        &mut f.host.borrow_mut(),
        HostCommand::StopProcessing {
            handle: f.bare_handle,
        },
    );
    let error = f
        .state
        .capture(ProcessorId::new(2), CaptureMode::Coordinated)
        .unwrap_err();
    assert_eq!(error, PluginStateCaptureError::Unsupported);
    let error = f
        .state
        .restore(ProcessorId::new(2), b"anything")
        .unwrap_err();
    assert_eq!(error, PluginStateCaptureError::Unsupported);
}

#[test]
fn save_restore_roundtrip_through_session() {
    let mut f = fixture();
    exec(
        &mut f.host.borrow_mut(),
        HostCommand::StopProcessing {
            handle: f.state_handle,
        },
    );
    exec(
        &mut f.host.borrow_mut(),
        HostCommand::StopProcessing {
            handle: f.bare_handle,
        },
    );

    // Minimal project carrying one resolved processor — the session
    // only reads `routing.processors`.
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTrack { name: "t".into() },
    })
    .unwrap();
    let track = *c.live().tracks.keys().next_back().unwrap();
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTrackStrip { track_id: track },
    })
    .unwrap();
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "state".into(),
            binding: synvelis_project::ProcessorBinding::Resolved(PluginRef {
                descriptor: "org.synvelis.test.tim04-state".into(),
                version: "0.1.0".into(),
                capability_revision: 1,
            }),
            sidechain_input: false,
        },
    })
    .unwrap();
    let pid = *c.live().routing.processors.keys().next_back().unwrap();
    // Rebind the adapter to the project's actual processor id.
    f.state.unbind(ProcessorId::new(1));
    f.state.bind(
        pid,
        f.state_handle,
        &PluginRef {
            descriptor: "org.synvelis.test.tim04-state".into(),
            version: "0.1.0".into(),
            capability_revision: 1,
        },
        42,
    );

    let request = PluginStateSaveRequest {
        project: c.live(),
        project_revision: c.live_revision().0,
        project_head: None,
        mode: CaptureMode::Coordinated,
        policy: StateQualityPolicy::Strict,
        stored_at: "t".into(),
    };
    let mut session = PluginStateSession::new(f.state, InMemoryPluginStateStore::new());
    let receipt = session.save(&request).unwrap();
    assert!(receipt.summary.exact_eligible());
    assert_eq!(receipt.summary.exact, 1);

    let restore = session
        .restore(&PluginStateRestoreRequest {
            manifest: None,
            project: c.live(),
        })
        .unwrap();
    assert!(restore.records.iter().all(|r| matches!(
        r.outcome,
        synvelis_project::PluginStateRestoreOutcome::Restored
    )));
    // The plugin saw the 4-byte gain blob.
    assert_eq!(f.restored.borrow().len(), 1);
    assert_eq!(f.restored.borrow()[0].len(), 4);
}

#[test]
fn destroyed_instance_fails_closed() {
    let mut f = fixture();
    exec(
        &mut f.host.borrow_mut(),
        HostCommand::StopProcessing {
            handle: f.state_handle,
        },
    );
    exec(
        &mut f.host.borrow_mut(),
        HostCommand::Deactivate {
            handle: f.state_handle,
        },
    );
    exec(
        &mut f.host.borrow_mut(),
        HostCommand::Destroy {
            handle: f.state_handle,
        },
    );
    let error = f
        .state
        .capture(ProcessorId::new(1), CaptureMode::Coordinated)
        .unwrap_err();
    match error {
        PluginStateCaptureError::Failed(kind) => {
            assert!(
                [
                    PluginStateFailureKind::Unavailable,
                    PluginStateFailureKind::Crashed
                ]
                .contains(&kind)
            );
        }
        other @ PluginStateCaptureError::Unsupported => {
            panic!("expected typed failure, got {other:?}")
        }
    }
}

#[test]
fn unknown_processor_is_unavailable() {
    let mut f = fixture();
    let error = f
        .state
        .capture(ProcessorId::new(77), CaptureMode::Coordinated)
        .unwrap_err();
    assert_eq!(
        error,
        PluginStateCaptureError::Failed(PluginStateFailureKind::Unavailable)
    );
}

#[test]
fn engine_vends_empty_state_handles_for_unknown_generation() {
    // `plugin_state_handles` on an unknown generation returns an
    // empty vec — the vend path fails closed.
    let engine = RoutingEngine::new(
        EngineConfig {
            sample_rate: 48_000,
            channels: 2,
            block_frames: 256,
        },
        RuntimeInstanceId(1),
        RuntimeClockEpochRef(1),
    );
    assert!(engine.plugin_state_handles(999).is_empty());
}
