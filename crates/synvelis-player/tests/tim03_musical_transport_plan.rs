//! TIM-03 production plan-owned musical transport tests.
//!
//! These tests verify Synvelis's supply responsibility at the production
//! `ParamDispatch` boundary. They do not retest ruviform's tempo math: the
//! oracle is that the exact project-owned map identity, revision, and PPQN
//! are present together, while invalid input remains honestly absent.

use ruviform_audio_core::{ChannelLayout, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, PluginNode, attach};
use ruviform_graph::{GraphBuilder, NodeDef, NodeKind};
use ruviform_media_core::{
    DeliveryPolicy, MediaTime, MidiChannel, MidiEvent, MidiGroup, MidiKind, MidiValue, NoteKey,
    ScheduleBudget, TimeBase,
};
use ruviform_plugin_core::{
    BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind, ParamEvent, ParamFlags, ParamId,
    ParamInfo, ParamValue, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, ProcessConfig,
    ProcessContext, ProcessResult, TimedParamEvent, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use synvelis_player::{MusicalTransportError, ParamDispatch, compile_project_musical_transport};
use synvelis_project::tempo::{SignatureDto, TempoEventDto};
use synvelis_project::{ParameterTargetId, Project, ProjectId};

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim03-musical-plan",
    name: "TIM-03 Musical Plan Dummy",
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

struct Dummy;

impl PluginProcessor for Dummy {
    fn prepare(
        &mut self,
        _config: &ProcessConfig,
        _context: &mut ProcessContext,
    ) -> Result<(), ruviform_plugin_core::PrepareError> {
        Ok(())
    }

    fn process<I, O>(
        &mut self,
        _inputs: &[I],
        _outputs: &mut [O],
        _events: ruviform_plugin_core::EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: ruviform_plugin_core::BufferView,
        O: ruviform_plugin_core::BufferViewMut,
    {
        ProcessResult::CONTINUE
    }
}

impl Plugin for Dummy {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &PARAMS
    }
    fn buses(&self) -> Option<&[BusDescriptor]> {
        Some(&BUSES)
    }
}

struct Factory;

impl PluginFactory for Factory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(Dummy)
    }

    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(Dummy))
    }
}

struct Hosted {
    _host: PluginHost,
    control: ruviform_audio_plugin_bridge::PluginNodeControl,
    _node: PluginNode,
}

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

fn host_plugin() -> Hosted {
    let mut host = PluginHost::new();
    host.register_factory(Box::new(Factory));
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
            port_format: ruviform_audio_core::AudioFormat {
                sample_rate: SampleRate::new(48_000).unwrap(),
                layout: ChannelLayout::Stereo,
                sample_format: ruviform_audio_core::SampleFormat::F32,
            },
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
    .unwrap();
    Hosted {
        _host: host,
        control: attached.control,
        _node: attached.node,
    }
}

fn project_480_150bpm() -> Project {
    let mut project = Project::new(ProjectId::new(1), "tim03 musical");
    project.ppqn.0 = 480;
    project.tempo_map.ppqn = 480;
    project.tempo_map.revision = 9;
    project.tempo_map.tempo_events = vec![
        TempoEventDto {
            tick: 0,
            bpm_milli: 150_000,
        },
        TempoEventDto {
            tick: 960,
            bpm_milli: 90_000,
        },
    ];
    project.tempo_map.signature_events = vec![SignatureDto {
        tick: 0,
        numerator: 3,
        denominator: 8,
    }];
    project
}

fn param_write(pos: i64, value: f64) -> (ParameterTargetId, TimedParamEvent) {
    (
        ParameterTargetId::new(1),
        TimedParamEvent {
            position: MediaTime::new(pos, frame_base()),
            event: ParamEvent {
                param_id: ParamId(7),
                value: ParamValue::new(value).unwrap(),
            },
        },
    )
}

fn note_on(key: u8) -> MidiEvent {
    MidiEvent {
        group: MidiGroup::ZERO,
        channel: MidiChannel::new(0).unwrap(),
        kind: MidiKind::NoteOn {
            key: NoteKey::new(key).unwrap(),
            velocity: MidiValue::from_u7(100),
        },
    }
}

fn issued_node() -> ruviform_graph::NodeId {
    let mut builder = GraphBuilder::new();
    builder
        .add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: Vec::new(),
        })
        .0
}

fn dispatch_with_source() -> (ParamDispatch, synvelis_player::PlanMusicalTransport) {
    let hosted = host_plugin();
    let node = issued_node();
    let mut dispatch = ParamDispatch::new(hosted.control, node, frame_base()).unwrap();
    dispatch
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .unwrap();
    let project = project_480_150bpm();
    let source = compile_project_musical_transport(&project).unwrap();
    assert!(dispatch.set_musical_transport(Some(source.clone())));
    (dispatch, source)
}

fn assert_complete_musical(
    plan: &ruviform_media_core::TransportPlan<EventKind>,
    source: &synvelis_player::PlanMusicalTransport,
) {
    assert!(
        plan.tempo_map.is_some(),
        "plan must own a compiled tempo map"
    );
    assert_eq!(plan.tempo_revision, source.tempo_revision());
    assert_eq!(plan.musical_ppqn, Some(source.ppqn()));
}

#[test]
fn production_plan_mints_carry_complete_project_musical_source() {
    let (mut dispatch, source) = dispatch_with_source();

    let param_plan = dispatch.dispatch(&[param_write(64, 0.25)]).unwrap();
    assert_complete_musical(&param_plan, &source);

    let midi = [(32, note_on(60)), (96, note_on(64))];
    let midi_plan = dispatch.dispatch_midi(&midi).unwrap();
    assert_complete_musical(&midi_plan, &source);
    assert_eq!(midi_plan.events.len(), 2);
    assert!(
        midi_plan
            .events
            .iter()
            .all(|event| matches!(event.payload, EventKind::Midi(_)))
    );

    let empty_plan = dispatch.clear_midi_plan().unwrap();
    assert_complete_musical(&empty_plan, &source);
    assert!(empty_plan.events.is_empty());
}

#[test]
fn invalid_project_state_is_honestly_absent_not_fixed_fallback() {
    let mut project = project_480_150bpm();
    project.tempo_map.ppqn = 960;
    let err = compile_project_musical_transport(&project).unwrap_err();
    assert!(matches!(
        err,
        MusicalTransportError::PpqnMismatch {
            project_ppqn: 480,
            tempo_map_ppqn: 960
        }
    ));

    let hosted = host_plugin();
    let mut dispatch = ParamDispatch::new(hosted.control, issued_node(), frame_base()).unwrap();
    dispatch
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .unwrap();
    let plan = dispatch.dispatch(&[param_write(64, 0.25)]).unwrap();
    assert!(plan.tempo_map.is_none());
    assert_eq!(plan.musical_ppqn, None, "no fixed 960 fallback may appear");
}

#[test]
fn musical_refresh_preserves_event_payload_positions_and_order() {
    let (mut dispatch, first) = dispatch_with_source();
    let midi = [(32, note_on(60)), (96, note_on(64))];
    let before = dispatch.dispatch_midi(&midi).unwrap();
    assert!(dispatch.last_plan_musical_is_current());

    let mut project = project_480_150bpm();
    project.tempo_map.revision = 10;
    project.tempo_map.tempo_events[0].bpm_milli = 144_000;
    let second = compile_project_musical_transport(&project).unwrap();
    assert!(!first.same_project_basis(&second));
    assert!(dispatch.set_musical_transport(Some(second.clone())));
    assert!(!dispatch.last_plan_musical_is_current());

    let after = dispatch.refresh_musical_plan().unwrap().unwrap();
    assert_complete_musical(&after, &second);
    assert_eq!(after.events.len(), before.events.len());
    for (old, new) in before.events.iter().zip(&after.events) {
        assert_eq!(new.position, old.position);
        assert_eq!(new.source_sequence, old.source_sequence);
        assert_eq!(new.payload, old.payload);
        assert_eq!(new.transport_generation, old.transport_generation);
    }
    assert_eq!(dispatch.staged_midi(), &midi);
    assert!(dispatch.last_plan_musical_is_current());
}
