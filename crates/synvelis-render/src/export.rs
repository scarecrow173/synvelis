//! Export orchestration: snapshot → preflight → verified prepare →
//! lowered graph + fresh processors → `PreparedRender` → sequential
//! `render()` over a real io-wav encoder/muxer → reopen verify →
//! safe publish → typed outcome.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use synvelis_player::error::ResolveError;
use synvelis_player::lowering::{EngineConfig, NodeContent, SkippedPlayback};
use synvelis_player::routing_lower::{
    GainProcessor, SemanticNodeRole, SemanticOutput, StripProcessor, engine_format,
    lower_semantic_offline,
};
use synvelis_player::{ProcessorConnectRequest, ProcessorHost};
use synvelis_project::{
    BuildBudget, BuildMode, LOWERING_POLICY_VERSION, RoutingBuildKey, RoutingFault,
    RuntimeClockEpochRef, RuntimeInstanceId, SourceId, port_capability_generation,
    routing_fingerprint,
};

use ruviform_audio::render::{
    PreparedRender, RenderError, RenderOutput, RenderSpec, RenderTransport, SilentFeeder,
    StagedCommand, TailSpec, render,
};
use ruviform_audio::typed::{
    CompilerConfig, FactoryError, GraphProcessor, InstalledInfo, NodeEvent, OfflineRenderSupport,
    PrepareOutcome,
};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleRate};
use ruviform_graph::{GraphProject, NodeSpec};
use ruviform_io_file::{Durability, PublishMode, PublishOptions, SealedFile, StagedFile};
use ruviform_io_wav::WavOutputSpec;
use ruviform_media_core::{
    BlockContract, DeliveryPolicy, EngineFrame, InBlockTransportSupport, MediaTime, ScheduleBudget,
    ScheduledEvent, TransportCommand, TransportScheduler,
};
use ruviform_media_io::{Error as MediaError, ErrorKind, Muxer};

use crate::error::{ExportError, PreflightError};
use crate::job::{
    CapabilityEvidence, ExportJob, ExportOutcome, ExportReceipt, ExportStage, stage_of,
};
use crate::preflight::{canonical_of, preflight_canonical, preflight_static};
use crate::prepare::{decode_window, open_sources};
use crate::processor::{ClipRenderer, DelayProc, GainProc, OnePoleProc};
use crate::snapshot::RenderSnapshot;
use crate::spec::{DynPublish, DynVerify, ExportRequest, Insert, OutputHooks, TailPolicy};
use crate::verify::{VerifiedOutput, facts_evidence, reopen_check};

fn no_events(_: &ScheduledEvent<u32>, _: u64) -> Option<NodeEvent> {
    None
}

fn failed(stage: ExportStage, error: impl Into<ExportError>) -> ExportOutcome {
    ExportOutcome::Failed {
        stage,
        error: error.into(),
    }
}

/// Runs one export against a frozen snapshot.
///
/// The render input is `snapshot` alone — live project edits cannot
/// reach it, and no processor instance is shared with any live
/// driver. All failures before publish keep an existing destination
/// byte-for-byte intact and discard the staged temp. Chain processors
/// fail closed ([`RoutingFault::PluginUnavailable`]) — use
/// [`export_with_host`] to provision them.
#[must_use]
pub fn export(
    snapshot: &RenderSnapshot,
    request: &ExportRequest,
    hooks: OutputHooks,
    job: &ExportJob,
) -> ExportOutcome {
    match run(snapshot, request, hooks, job, None) {
        Ok(outcome) | Err(outcome) => outcome,
    }
}

/// [`export`] with a [`ProcessorHost`] for semantic chain processors —
/// the same provisioning boundary the realtime routing engine uses,
/// always in [`BuildMode::Offline`].
#[must_use]
pub fn export_with_host(
    snapshot: &RenderSnapshot,
    request: &ExportRequest,
    hooks: OutputHooks,
    job: &ExportJob,
    host: &mut dyn ProcessorHost,
) -> ExportOutcome {
    match run(snapshot, request, hooks, job, Some(host)) {
        Ok(outcome) | Err(outcome) => outcome,
    }
}

/// Fixed lowering bounds for the offline path — the realtime engine
/// takes caller budgets; an export has no caller budget channel, so
/// the offline lowering gets generous fixed ceilings that still fail
/// closed on pathological graphs.
const LOWER_MAX_NODES: usize = 1 << 16;
const LOWER_MAX_CONNECTIONS: usize = 1 << 17;
/// The offline build's single plan generation — a stateless export
/// has no generation lineage; `1` is its first and only generation.
const OFFLINE_GENERATION: u64 = 1;

#[expect(clippy::too_many_lines, reason = "one orchestration pass")]
fn run(
    snapshot: &RenderSnapshot,
    request: &ExportRequest,
    hooks: OutputHooks,
    job: &ExportJob,
    mut host: Option<&mut dyn ProcessorHost>,
) -> Result<ExportOutcome, ExportOutcome> {
    // ---- preflight (static) ----------------------------------------
    preflight_static(snapshot, request).map_err(|e| failed(ExportStage::Preflight, e))?;

    // ---- verified prepare ------------------------------------------
    // An unsupported codec is a *preflight* refusal — the codec is a
    // known source fact the request cannot satisfy — not a prepare
    // corruption. `Cancelled` is an outcome, not a failure.
    let mut opened = open_sources(&snapshot.project, &snapshot.media_root, &job.cancel).map_err(
        |e| match e {
            ResolveError::Cancelled => ExportOutcome::Cancelled { phase: "prepare" },
            ResolveError::UnsupportedCodec { source_id, detail } => failed(
                ExportStage::Preflight,
                PreflightError::UnsupportedCodec { source_id, detail },
            ),
            e => failed(ExportStage::Prepare, e),
        },
    )?;
    preflight_canonical(snapshot, request, &opened)
        .map_err(|e| failed(ExportStage::Preflight, e))?;

    // ---- semantic lowering (BuildMode::Offline) ----------------------
    // The semantic graph runs at the project's declared port channel
    // geometry — `request.engine.channels` is the *output tap* count,
    // bridged only by the explicit channel map on the sink edge (an
    // explicit output declaration, never an implicit downmix).
    let engine = request.engine;
    let Some(port_channels) = snapshot.project.routing.port_channel_count() else {
        return Err(failed(
            ExportStage::Preflight,
            PreflightError::RoutingLowering {
                fault: RoutingFault::UnsupportedFormatOrTimeMap,
                detail: "routing ports declare mixed or no channel counts".into(),
            },
        ));
    };
    let Ok(graph_channels) = u16::try_from(port_channels) else {
        return Err(failed(
            ExportStage::Preflight,
            PreflightError::RoutingLowering {
                fault: RoutingFault::UnsupportedFormatOrTimeMap,
                detail: format!("port channel count {port_channels} out of range"),
            },
        ));
    };
    let lower_engine = EngineConfig {
        channels: graph_channels,
        ..engine
    };
    let graph_format = engine_format(&lower_engine);
    let sink_format = AudioFormat {
        layout: if engine.channels == 1 {
            ChannelLayout::Mono
        } else {
            ChannelLayout::Stereo
        },
        ..graph_format
    };
    // A stateless export has no engine/source-format generation
    // lineage — `0` is the honest sentinel; coherence evidence lives
    // in `project_revision` + `routing_fingerprint` +
    // `capability_generation`, all computed from the frozen snapshot.
    let key = RoutingBuildKey {
        project_id: snapshot.project.project_id,
        project_revision: snapshot.ids.live_revision,
        routing_fingerprint: routing_fingerprint(&snapshot.project),
        capability_generation: port_capability_generation(&snapshot.project),
        engine_config_generation: 0,
        source_format_generation: 0,
        lowering_policy_version: LOWERING_POLICY_VERSION,
        build_mode: BuildMode::Offline,
        device_clock_epoch: RuntimeClockEpochRef(0),
        runtime_instance: RuntimeInstanceId(0),
    };
    let budget = BuildBudget {
        max_nodes: LOWER_MAX_NODES,
        max_connections: LOWER_MAX_CONNECTIONS,
        max_frames: usize::try_from(request.max_block).unwrap_or(usize::MAX),
    };
    let lowered = lower_semantic_offline(
        &snapshot.project,
        &|id: SourceId| canonical_of(&opened, id),
        &lower_engine,
        &key,
        OFFLINE_GENERATION,
        &budget,
        SemanticOutput {
            insert_count: request.inserts.len(),
            format: Some(sink_format),
        },
    )
    .map_err(|e| {
        failed(
            ExportStage::Preflight,
            PreflightError::RoutingLowering {
                fault: e.fault(),
                detail: format!("{e:?}"),
            },
        )
    })?;
    // Every skip is explicit: `Muted`, `CompSilence` and
    // `SupersededByComp` are intentional silence — anything else is a
    // refused render, never a silent drop.
    for skipped in &lowered.skipped {
        match skipped {
            SkippedPlayback::Muted { .. }
            | SkippedPlayback::CompSilence { .. }
            | SkippedPlayback::SupersededByComp { .. } => {}
            SkippedPlayback::RateMismatch { clip_id, .. }
            | SkippedPlayback::UnresolvableAnchor { clip_id }
            | SkippedPlayback::MissingSource { clip_id, .. }
            | SkippedPlayback::UnsupportedMapping { clip_id } => {
                return Err(failed(
                    ExportStage::Preflight,
                    PreflightError::UnrenderableClip {
                        clip_id: *clip_id,
                        reason: format!("{skipped:?}"),
                    },
                ));
            }
            SkippedPlayback::CompProjection { comp_id, .. }
            | SkippedPlayback::CompSegment { comp_id, .. }
            | SkippedPlayback::CompOverlap { comp_id, .. } => {
                return Err(failed(
                    ExportStage::Preflight,
                    PreflightError::UnrenderableComp {
                        comp_id: *comp_id,
                        detail: format!("{skipped:?}"),
                    },
                ));
            }
        }
    }
    // Zero playback nodes — an all-muted or clip-free export — is a
    // refusal, not a silently empty file.
    if lowered.playbacks.is_empty() {
        return Err(failed(
            ExportStage::Preflight,
            PreflightError::NothingToRender,
        ));
    }
    let tail = request.tail.unwrap_or(TailPolicy::None);
    let play_pos = request.range_start.saturating_sub(request.preroll);
    let play_pos_i64 = i64::try_from(play_pos).unwrap_or(i64::MAX);
    // The drive touches positions `[play_pos, end + tail + latency)` —
    // declared insert latency is the only latency source in the graph,
    // checked against `total_latency` after compile below.
    let declared_latency: u64 = request.inserts.iter().map(Insert::declared_latency).sum();
    let needed = play_pos
        ..request
            .range_end
            .saturating_add(tail.frames())
            .saturating_add(declared_latency);

    let mut pending: BTreeMap<ruviform_graph::NodeId, Box<dyn GraphProcessor>> = BTreeMap::new();
    for binding in &lowered.playbacks {
        let opened_src = opened.get_mut(&binding.source_id).ok_or_else(|| {
            let error = match binding.content {
                NodeContent::Clip(clip_id) => PreflightError::MissingSource {
                    clip_id,
                    source_id: binding.source_id,
                },
                content => PreflightError::UnrenderableComp {
                    comp_id: content.comp_id().expect("comp content carries a comp id"),
                    detail: format!(
                        "{content:?} references missing source {}",
                        binding.source_id
                    ),
                },
            };
            failed(ExportStage::Prepare, error)
        })?;
        let decoded = decode_window(
            &mut opened_src.reader,
            binding.canonical,
            binding.window,
            needed.clone(),
            &job.cancel,
        )
        .map_err(|e| match e {
            ResolveError::Cancelled => ExportOutcome::Cancelled { phase: "prepare" },
            e => failed(ExportStage::Prepare, e),
        })?;
        pending.insert(
            binding.node_id,
            Box::new(ClipRenderer::new(
                decoded.planes,
                decoded.base,
                binding.window,
                binding.fade,
                play_pos_i64,
                request.clip_support,
            )),
        );
    }
    // Semantic graph nodes the lowering emitted beyond playbacks:
    // strip faders, send gains and chain processors. Fader/gain
    // stages are deterministic Synvelis processors; chain processors
    // go through the same [`ProcessorHost`] boundary the realtime
    // engine uses — absent a host they fail closed, never bypassed.
    // `held` retains every connection's lease/control/state halves
    // until the render completes — dropping a lease early would tear
    // the plugin instance down mid-render.
    let mut held = Vec::new();
    let block = request.max_block.max(1);
    for (node, role) in &lowered.node_roles {
        match role {
            SemanticNodeRole::Playback(_) => {}
            SemanticNodeRole::StripFader { gains, .. } => {
                pending.insert(*node, Box::new(StripProcessor::new(*gains)));
            }
            SemanticNodeRole::SendGain { gain, .. } => {
                pending.insert(*node, Box::new(GainProcessor::new(*gain)));
            }
            SemanticNodeRole::ChainProcessor { processor } => {
                let Some(host) = host.as_deref_mut() else {
                    return Err(failed(
                        ExportStage::Preflight,
                        PreflightError::RoutingLowering {
                            fault: RoutingFault::PluginUnavailable,
                            detail: format!("chain processor {processor} needs a ProcessorHost"),
                        },
                    ));
                };
                let Some(semantic) = snapshot.project.routing.processors.get(processor) else {
                    return Err(failed(
                        ExportStage::Preflight,
                        PreflightError::RoutingLowering {
                            fault: RoutingFault::InvalidRouting,
                            detail: format!("chain processor {processor} missing from model"),
                        },
                    ));
                };
                let mut ports = Vec::with_capacity(semantic.port_ids.len());
                for port_id in &semantic.port_ids {
                    let Some(port) = snapshot.project.routing.ports.get(port_id) else {
                        return Err(failed(
                            ExportStage::Preflight,
                            PreflightError::RoutingLowering {
                                fault: RoutingFault::PortUnresolved,
                                detail: format!(
                                    "chain processor {processor} port {port_id} unresolved"
                                ),
                            },
                        ));
                    };
                    ports.push(port.clone());
                }
                let connection = host
                    .connect(ProcessorConnectRequest {
                        processor: semantic.clone(),
                        ports,
                        mode: BuildMode::Offline,
                        generation: OFFLINE_GENERATION,
                        node: *node,
                        port_format: graph_format,
                        max_frames: block,
                    })
                    .map_err(|e| {
                        failed(
                            ExportStage::Preflight,
                            PreflightError::RoutingLowering {
                                fault: e.fault(),
                                detail: format!("{e:?}"),
                            },
                        )
                    })?;
                // Offline builds accept only certified-exact instances —
                // `Unknown`/`Declared` are never treated as verified.
                if connection.processor.offline_support() != OfflineRenderSupport::Verified {
                    return Err(failed(
                        ExportStage::Preflight,
                        PreflightError::RoutingLowering {
                            fault: RoutingFault::OfflineCapabilityMismatch,
                            detail: format!(
                                "chain processor {processor} is not OfflineRenderSupport::Verified"
                            ),
                        },
                    ));
                }
                let synvelis_player::ProcessorConnection {
                    processor: instance,
                    lease,
                    control,
                    state_handle,
                } = connection;
                pending.insert(*node, instance);
                held.push((lease, control, state_handle));
            }
        }
    }
    for (insert, spec) in lowered.inserts.iter().zip(request.inserts.iter()) {
        // Insert nodes run at the *graph* format inside the semantic
        // graph — channel state must match the node's port format,
        // which is the project geometry, not the output tap count.
        let channels = usize::from(graph_channels);
        let proc: Box<dyn GraphProcessor> = match *spec {
            Insert::Gain { gain_db } => Box::new(GainProc::new(gain_db)),
            Insert::OnePole { a, latency } => Box::new(OnePoleProc::new(a, latency, channels)),
            Insert::Delay { frames } => Box::new(DelayProc::new(frames, channels)),
        };
        pending.insert(insert.node_id, proc);
    }
    let mut factory = |node: &NodeSpec,
                       _existing: Option<&InstalledInfo>|
     -> Result<PrepareOutcome, FactoryError> {
        pending
            .remove(&node.id())
            .map_or(Ok(PrepareOutcome::Boundary), |p| Ok(PrepareOutcome::New(p)))
    };

    // ---- validate + compile ------------------------------------------
    let mut graph_project = GraphProject::new();
    graph_project.prepare(lowered.spec.clone()).map_err(|e| {
        failed(
            ExportStage::Compile,
            ExportError::Graph {
                detail: format!("{e}"),
            },
        )
    })?;
    let validated = graph_project
        .published()
        .expect("prepare just published")
        .graph();
    let config = CompilerConfig {
        max_frames: usize::try_from(block).unwrap_or(usize::MAX),
        ..CompilerConfig::default()
    };
    let mut prepared = PreparedRender::compile(validated, &mut factory, config, 64, 64)
        .map_err(|e| failed(ExportStage::Compile, e))?;
    let node_support = prepared.capabilities().clone();
    let total_latency = prepared.total_latency();
    // The decode bound assumed declared insert latency is the only
    // latency source — if the compiled graph reports more, the bound
    // under-decoded and continuing would emit silence where content
    // belongs. Refuse rather than ship wrong output.
    if total_latency > declared_latency {
        return Err(failed(
            ExportStage::Compile,
            ExportError::Graph {
                detail: format!(
                    "graph latency {total_latency} exceeds declared insert \
                     latency {declared_latency}"
                ),
            },
        ));
    }

    // ---- transport + render geometry ----------------------------------
    let rate = SampleRate::new(engine.sample_rate).map_err(|_| {
        failed(
            ExportStage::Preflight,
            PreflightError::InvalidRange {
                detail: "engine rate must be non-zero".into(),
            },
        )
    })?;
    let base = rate.time_base();
    let mut transport = RenderTransport::<u32> {
        scheduler: TransportScheduler::new(
            base,
            BlockContract::new(1, block, InBlockTransportSupport::Unknown),
            DeliveryPolicy::Exact,
            ScheduleBudget::default(),
        ),
        commands: Vec::new(),
        plans: Vec::new(),
    };
    let epoch = transport.scheduler.epoch();
    if play_pos > 0 {
        transport.commands.push(StagedCommand {
            command: TransportCommand::Seek(MediaTime::new(play_pos_i64, base)),
            due: EngineFrame::new(epoch, 0),
        });
    }
    transport.commands.push(StagedCommand {
        command: TransportCommand::Play,
        due: EngineFrame::new(epoch, 0),
    });
    // Emitted position = position − total_latency ∈ [start, end+tail);
    // the job covers positions [play_pos, end+tail+latency).
    let job_end = request
        .range_end
        .saturating_add(tail.frames())
        .saturating_add(total_latency)
        .saturating_sub(play_pos);
    let layout = if engine.channels == 1 {
        ChannelLayout::Mono
    } else {
        ChannelLayout::Stereo
    };
    let spec = RenderSpec {
        sample_rate: rate,
        layout,
        output_port: lowered.device_in,
        range_start: request.range_start,
        range_end: request.range_end,
        tail: match tail {
            TailPolicy::None => TailSpec::None,
            TailPolicy::Known(n) => TailSpec::Known(n),
            TailPolicy::Estimated(n) => TailSpec::Estimated(n),
        },
        job_start: 0,
        job_end,
        callback_chunk: block,
    };

    // ---- staged output + verify/publish --------------------------------
    let OutputHooks {
        wrap_sink,
        verify,
        publish,
        expect_pcm,
    } = hooks;
    let verified_slot = Arc::new(Mutex::new(None::<VerifiedOutput>));
    let slot = Arc::clone(&verified_slot);
    let custom_verify = verify.is_some();
    let verify_fn: DynVerify = verify.unwrap_or_else(|| {
        Box::new(move |sealed, facts| {
            let observed = reopen_check(sealed, facts, expect_pcm.as_deref())?;
            *slot
                .lock()
                .map_err(|_| MediaError::from_kind(ErrorKind::Io))? = Some(observed);
            Ok(())
        })
    });
    let publish_fn: DynPublish =
        publish.unwrap_or_else(|| Box::new(SealedFile::publish) as DynPublish);
    let info = WavOutputSpec {
        sample_rate: rate,
        layout,
        sample_format: request.wire_format,
    }
    .stream_info();
    let encoder = ruviform_io_wav::make_encoder(&info).map_err(|e| {
        failed(
            ExportStage::Preflight,
            PreflightError::InsufficientCapability {
                detail: format!("wav encoder rejected output shape: {e}"),
            },
        )
    })?;
    let muxer_info = info.clone();
    let make_muxer: crate::spec::DynMuxerCtor = match wrap_sink {
        Some(wrap) => Box::new(move |staged: &mut StagedFile| {
            ruviform_io_wav::open_muxer(wrap(staged), std::slice::from_ref(&muxer_info))
                .map(|m| Box::new(m) as Box<dyn Muxer + '_>)
        }),
        None => Box::new(move |staged: &mut StagedFile| {
            ruviform_io_wav::open_muxer(staged, std::slice::from_ref(&muxer_info))
                .map(|m| Box::new(m) as Box<dyn Muxer + '_>)
        }),
    };
    let staged = StagedFile::create(&request.destination).map_err(|e| {
        failed(
            ExportStage::Preflight,
            ExportError::Io {
                detail: format!("staged output create: {e}"),
            },
        )
    })?;
    let output = RenderOutput {
        staged,
        make_muxer,
        encoder: Box::new(encoder),
        // `replace_existing` maps to the publish mode; the old
        // `sync_before_publish: true` contract is `Durability::FileSync`
        // — staged content is synced before the atomic publish, the
        // directory entry itself is not additionally synced.
        publish_options: PublishOptions::new(
            if request.replace_existing {
                PublishMode::Replace
            } else {
                PublishMode::CreateNew
            },
            Durability::FileSync,
        ),
        verify: Some(verify_fn),
        publish: publish_fn,
    };

    // ---- sequential drive ------------------------------------------------
    let mut feeder = SilentFeeder;
    let caps = ruviform_io_wav::wav_capabilities();
    match render(
        &mut prepared,
        &mut transport,
        &spec,
        &mut feeder,
        output,
        &job.progress,
        &job.cancel,
        &mut no_events,
    ) {
        Ok(render_receipt) => {
            let verified = verified_slot
                .lock()
                .ok()
                .and_then(|v| v.clone())
                .unwrap_or_else(|| {
                    facts_evidence(
                        usize::from(engine.channels),
                        rate.get(),
                        render_receipt.frames_emitted,
                    )
                });
            debug_assert!(
                verified.reopened || custom_verify,
                "default verify always reopens"
            );
            let tail_evidence = render_receipt.tail_evidence;
            Ok(ExportOutcome::Completed {
                receipt: Box::new(ExportReceipt {
                    render: render_receipt,
                    snapshot: snapshot.ids.clone(),
                    source_map: lowered.source_map,
                    capability: CapabilityEvidence {
                        wav_sample_formats: caps
                            .sample_formats
                            .iter()
                            .map(|f| format!("{f:?}"))
                            .collect(),
                        node_support,
                        total_latency,
                        tail_evidence,
                        wav_rf64: caps.rf64,
                        wav_custom_layouts: caps.custom_layouts,
                    },
                    verified,
                    plugin_state: request.plugin_state.clone(),
                }),
            })
        }
        Err(RenderError::Cancelled { phase }) => Ok(ExportOutcome::Cancelled { phase }),
        Err(e) => Err(failed(stage_of(&e), e)),
    }
}
