//! RCP-04 semantic→selection integration (synvelis#293 deferred
//! residual slice)。
//!
//! `synvelis_player::rcp` は pin 済みの決定契約
//! （`ResolvedGraphSnapshot` → `ExecutionSelector::select` →
//! `PlanReconciler` → `RoutingEngine` prepare/publish）を提供するが、
//! semantic model からその入力を組み立てる配線は deferred residual
//! として残されていた。本 module がその残件を `synvelis-workspace`
//! 側で実装する:
//!
//! - [`resolve_selection`] — `Project::routing`（`RoutingModel`）と
//!   caller 提供の plugin 環境証拠・render lookup 結果から
//!   `ResolvedGraphSnapshot` / `PluginCapabilitySnapshot` /
//!   `RenderCandidateIndex` を deterministic に構築する。I/O・
//!   runtime ID・`Project` 永続化は持ち込まない。
//! - [`drive_selection_plan`] — `ExecutionSelection.receipt` を
//!   `PlanReconciler` の plan lifecycle に乗せ、`RoutingEngine` の
//!   `prepare` → `publish` 経路へ `RoutingBuildKey` が齟齬なく
//!   繋がることを headless に駆動する。
//!
//! 境界: `SessionRoutingEngine`（`synvelis-app`）adapter 配線、
//! BranchAudition/provenance 経路（#294 系）、実 plugin/OS/PCM/RT
//! 受入（RCP-GATE）は本 slice の対象外。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;

use ruviform_audio_core::AudioFormat;
use ruviform_graph::{
    Activation, Availability, ChannelMap, Direction, EdgeDelay, Multiplicity, PortFormat, PortRole,
    SignalDomain,
};
use synvelis_player::EngineConfig;
use synvelis_player::rcp::{
    ArtifactOutput, BoundaryPurpose, BoundaryRequirement, DeclaredPort, EdgeKind,
    ExecutionSelection, NodeClass, PlanReconciler, PluginCapabilitySnapshot, PluginDeclaration,
    PluginEnvironment, PluginIdentity, PluginKey, PluginStateDecl, PortEndpoint, PreparedPlan,
    RcpEdgeId, RcpNodeId, RcpPortId, RenderCandidate, RenderCandidateIndex, ResolvedEdge,
    ResolvedGraphSnapshot, ResolvedNode, ResolvedPort,
};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine, RoutingPrepare};
use synvelis_player::routing_lower::engine_format;
use synvelis_project::routing::{
    BusKind, ConnectionKind, PortBinding, PortDirection, PortOwner, PortRole as SemanticPortRole,
    ProcessorBinding, SignalDomain as SemanticSignalDomain, StripOwner,
};
use synvelis_project::{
    ExecutionEquivalence, ProcessorId, Project, ProjectRevision, RoutingActivationReceipt,
    RoutingCompileRequest, RoutingConnection, RoutingConnectionId, RoutingFault, RoutingModel,
    RuntimeClockEpochRef, RuntimeInstanceId, SemanticPortId, StripId, port_capability_generation,
    routing_fingerprint,
};
use synvelis_render::artifact::lookup::CandidateReport;

// ---------------------------------------------------------------------------
// caller-supplied evidence inputs
// ---------------------------------------------------------------------------

/// 1 processor 分の authored plugin 証拠。`descriptor` は
/// `ProcessorBinding` が保持する authored identity と一致必須
/// （不一致は fail-closed error — 別 plugin の証拠を誤適用しない）。
pub struct ProcessorPluginEvidence {
    /// `ProcessorBinding::{Resolved,Unresolved}` が保持する
    /// descriptor 文字列と一致すること。
    pub descriptor: String,
    /// Authored plugin identity（format/vendor/uid/version/
    /// binary digest）。
    pub identity: PluginIdentity,
    /// Opaque state 証拠（TIM-04 語彙をそのまま pin）。
    pub state: PluginStateDecl,
    /// Automation/parameter identity が declared layout 内で
    /// 完全解決できるか。`false` は編集・代替証拠ともに fail-closed。
    pub automation_identity_complete: bool,
}

/// semantic coverage — artifact が代替する部分グラフを semantic ID
/// で宣言する。`RcpNodeId` への写像は producer が行うため、caller
/// は runtime/snapshot-local ID を知る必要がない。
#[derive(Clone, Debug, Default)]
pub struct SemanticRenderCoverage {
    /// 代替領域に含まれる strip。
    pub strips: BTreeSet<StripId>,
    /// 代替領域に含まれる chain processor。
    pub processors: BTreeSet<ProcessorId>,
    /// 領域側の出力 port（edge の source 側 `SemanticPortId`）—
    /// artifact が再現する信号ごとに 1 件。
    pub outputs: BTreeSet<SemanticPortId>,
    /// 領域側の入力 port（edge の destination 側
    /// `SemanticPortId`）— render に pin 済みの境界入力。
    pub pinned_inputs: BTreeSet<SemanticPortId>,
}

/// 1 render candidate の semantic 宣言 + lookup 評価結果。
/// `report` は `synvelis_render::artifact::lookup` の
/// [`CandidateReport`] をそのまま運ぶ（eligibility /
/// reproducibility / access は RCP-04 語彙で一致済み）。
pub struct SemanticRenderCandidate {
    /// 代替 coverage（semantic ID）。
    pub coverage: SemanticRenderCoverage,
    /// lookup が評価した candidate report。
    pub report: CandidateReport,
    /// artifact の content hash（manifest 検証済みの値）。
    pub content_hash: String,
}

/// [`resolve_selection`] の全入力 — 全て explicit pin。
/// `Project` は revision を保持しないため `project_revision` は
/// caller（controller 観測値）から pin する。
pub struct SelectionInputs<'a> {
    /// Semantic source of truth。
    pub project: &'a Project,
    /// Commit 済み project revision（controller 観測値）。
    pub project_revision: ProjectRevision,
    /// Snapshot identity（receipt 用、caller 採番）。
    pub snapshot_id: u64,
    /// Engine geometry — 全 audio port の `PortFormat::Audio` を
    /// 決める（`engine_format` と同一規則）。
    pub engine: EngineConfig,
    /// Engine geometry generation pin（`RoutingDeps` と同じ値を
    /// 渡すこと — build key の一致に必須）。
    pub engine_config_generation: u64,
    /// Canonical source format generation pin。
    pub source_format_generation: u64,
    /// Device clock epoch pin（`RoutingCompileRequest` /
    /// `RoutingEngine::new` と同じ値）。
    pub clock_epoch: RuntimeClockEpochRef,
    /// Runtime instance pin。
    pub runtime_instance: RuntimeInstanceId,
    /// `ProcessorId` → authored plugin 証拠。chain 内全 processor
    /// （`Resolved`/`Unresolved` 双方）に必須。
    pub plugin_evidence: &'a BTreeMap<ProcessorId, ProcessorPluginEvidence>,
    /// descriptor → host 環境事実。未登録 descriptor は
    /// `PluginVerdict::Missing` として fail-closed に扱われる
    /// （推測 install しない）。
    pub environments: &'a BTreeMap<String, PluginEnvironment>,
    /// Render candidate index generation（artifact 到着で bump、
    /// graph rebuild とは独立）。
    pub render_index_generation: u64,
    /// Semantic coverage 付き render candidates。
    pub render_candidates: &'a [SemanticRenderCandidate],
}

/// [`resolve_selection`] の出力 — selection が消費する 3 入力を
/// 同一 pin で揃えて返す。
pub struct SelectionResolution {
    /// Pinned typed-graph snapshot。
    pub snapshot: ResolvedGraphSnapshot,
    /// Environment capability snapshot（generation は project 側
    /// `port_capability_generation` と一致 — reconcile の
    /// capability pin 比較が成立するよう単一情報源から導出）。
    pub capabilities: PluginCapabilitySnapshot,
    /// Pinned render candidate index。
    pub renders: RenderCandidateIndex,
}

/// Snapshot 構築の fail-closed error — semantic binding が
/// 解釈不能・証拠不整合な場合に推測で進めない。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionBuildError {
    /// Chain processor に authored plugin 証拠が無い。
    MissingPluginEvidence {
        /// 対象 processor。
        processor: ProcessorId,
    },
    /// 証拠の descriptor が binding の authored descriptor と
    /// 不一致。
    DescriptorMismatch {
        /// 対象 processor。
        processor: ProcessorId,
        /// Binding 側 descriptor。
        binding: String,
        /// 証拠側 descriptor。
        evidence: String,
    },
    /// 必要な semantic port（strip main I/O・processor main I/O）
    /// が存在しない、または方向/role/domain/channel が想定と違う。
    MissingSemanticPort {
        /// Detail（semantic port id / owner）。
        detail: String,
    },
    /// 有効な `RoutingConnection` の endpoint が port map に
    /// 無い、または方向が不正。
    InvalidConnection {
        /// 対象 connection。
        connection: RoutingConnectionId,
        /// Detail。
        detail: String,
    },
    /// Semantic port の channel 数が engine format と不一致
    /// （lowering の `ChannelMismatch` と同じ fail-closed 規則）。
    ChannelMismatch {
        /// 対象 semantic port。
        port: SemanticPortId,
    },
    /// Render coverage が snapshot に存在しない semantic entity
    /// を参照している、または方向が coverage の役割と矛盾する。
    UnknownCoverage {
        /// Detail。
        detail: String,
    },
}

impl Display for SelectionBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPluginEvidence { processor } => {
                write!(f, "missing plugin evidence for processor {processor}")
            }
            Self::DescriptorMismatch {
                processor,
                binding,
                evidence,
            } => write!(
                f,
                "plugin evidence descriptor mismatch on {processor}: binding={binding} evidence={evidence}"
            ),
            Self::MissingSemanticPort { detail } => {
                write!(f, "missing semantic port: {detail}")
            }
            Self::InvalidConnection { connection, detail } => {
                write!(f, "invalid routing connection {connection}: {detail}")
            }
            Self::ChannelMismatch { port } => {
                write!(
                    f,
                    "semantic port {port} channel count mismatches engine format"
                )
            }
            Self::UnknownCoverage { detail } => {
                write!(f, "unknown render coverage: {detail}")
            }
        }
    }
}

impl std::error::Error for SelectionBuildError {}

// ---------------------------------------------------------------------------
// semantic → typed-graph mapping
// ---------------------------------------------------------------------------

/// snapshot 内の 1 port の構築中間表現 — `RcpPortId` と edge
/// 構築用の方向フラグを保持する。
struct PortEntry {
    id: RcpPortId,
    direction: Direction,
    role: PortRole,
    /// Input port なら `true` — edge 構築の方向検査用。
    is_input: bool,
}

/// snapshot 構築中の node 表。
struct NodeBuild {
    id: RcpNodeId,
    class: NodeClass,
    ports: Vec<PortEntry>,
    plugin: Option<PluginDeclaration>,
    label: Option<String>,
}

/// strip の chain plumbing port（semantic port ではない
/// snapshot-local 構造 port）— chain 非空時のみ存在。
struct StripPlumbing {
    /// chain 先頭への feed（Output）。
    pre_chain: RcpPortId,
    /// chain 末尾からの return（Input）。
    chain_out: RcpPortId,
}

impl NodeBuild {
    fn into_node(self, audio: AudioFormat) -> ResolvedNode {
        ResolvedNode {
            id: self.id,
            class: self.class,
            ports: self
                .ports
                .into_iter()
                .map(|p| ResolvedPort {
                    id: p.id,
                    direction: p.direction,
                    role: p.role,
                    format: PortFormat::Audio(audio),
                    multiplicity: Multiplicity::unbounded(0),
                    availability: if p.role == PortRole::Sidechain {
                        Availability::Optional
                    } else {
                        Availability::Always
                    },
                    activation: Activation::Enabled,
                })
                .collect(),
            plugin: self.plugin,
            latency_frames: 0,
            label: self.label,
        }
    }
}

/// 1 owner の `port_ids` を snapshot port へ採番する。
/// input は 0 始まり・output は 1000 始まりの帯で `RcpPortId` を
/// 採番し、`SemanticPortId → RcpPortId` を `port_id_of` に記録する。
/// 戻り値は `(port entries, 次の input id, 次の output id)`。
///
/// channel 数は engine format と照合し、不一致は lowering と同じ
/// fail-closed 規則で error にする。domain は Audio のみ対応
/// （semantic model の現行語彙 — MIDI/control port は routing model
/// に存在しない）。
fn assign_ports(
    routing: &RoutingModel,
    port_ids: &[SemanticPortId],
    owner_label: &dyn Display,
    port_id_of: &mut BTreeMap<SemanticPortId, RcpPortId>,
    audio: AudioFormat,
) -> Result<(Vec<PortEntry>, u32, u32), SelectionBuildError> {
    let mut ports = Vec::new();
    let mut in_next = 0u32;
    let mut out_next = 1000u32;
    for port_id in port_ids {
        let port =
            routing
                .ports
                .get(port_id)
                .ok_or_else(|| SelectionBuildError::MissingSemanticPort {
                    detail: format!("{owner_label} references missing port {port_id}"),
                })?;
        if port.domain != SemanticSignalDomain::Audio {
            return Err(SelectionBuildError::MissingSemanticPort {
                detail: format!("port {port_id} has unsupported domain"),
            });
        }
        if port.channels != audio.layout.channels() {
            return Err(SelectionBuildError::ChannelMismatch { port: *port_id });
        }
        let (id, direction, is_input) = match port.direction {
            PortDirection::Input => {
                let id = RcpPortId(in_next);
                in_next += 1;
                (id, Direction::Input, true)
            }
            PortDirection::Output => {
                let id = RcpPortId(out_next);
                out_next += 1;
                (id, Direction::Output, false)
            }
        };
        port_id_of.insert(*port_id, id);
        let role = match port.role {
            SemanticPortRole::Main => PortRole::Main,
            SemanticPortRole::Sidechain => PortRole::Sidechain,
        };
        ports.push(PortEntry {
            id,
            direction,
            role,
            is_input,
        });
    }
    Ok((ports, in_next, out_next))
}

/// owner の `port_ids` から direction=`direction`/role=Main の
/// semantic port を探す。見つからなければ fail-closed。
fn find_main_port(
    routing: &RoutingModel,
    port_ids: &[SemanticPortId],
    direction: PortDirection,
) -> Result<SemanticPortId, SelectionBuildError> {
    port_ids
        .iter()
        .copied()
        .find(|id| {
            routing
                .ports
                .get(id)
                .is_some_and(|p| p.direction == direction && p.role == SemanticPortRole::Main)
        })
        .ok_or_else(|| SelectionBuildError::MissingSemanticPort {
            detail: format!("no {direction:?}/Main port in {port_ids:?}"),
        })
}

/// processor の plugin-advertised port を `DeclaredPort` 列へ。
/// `PluginDeclared`/`Unresolved` binding の `advertised` 名を使い、
/// `Internal` binding は plugin layout に含めない。
fn processor_declared_ports(
    routing: &RoutingModel,
    processor_id: ProcessorId,
    audio: AudioFormat,
) -> Vec<DeclaredPort> {
    let mut out = Vec::new();
    let Some(processor) = routing.processors.get(&processor_id) else {
        return out;
    };
    for port_id in &processor.port_ids {
        let Some(port) = routing.ports.get(port_id) else {
            continue;
        };
        let advertised = match &port.binding {
            PortBinding::PluginDeclared { advertised, .. }
            | PortBinding::Unresolved { advertised, .. } => advertised.clone(),
            PortBinding::Internal => continue,
        };
        out.push(DeclaredPort {
            name: advertised,
            direction: match port.direction {
                PortDirection::Input => Direction::Input,
                PortDirection::Output => Direction::Output,
            },
            domain: SignalDomain::Audio,
            format: PortFormat::Audio(audio),
        });
    }
    out
}

/// `RoutingConnection.kind` → snapshot `EdgeKind`。`Main` は
/// source strip が return bus に属する場合 `Return` とする
/// （return bus → mix への feed — send tap 側は `Send` が担う）。
fn edge_kind(routing: &RoutingModel, connection: &RoutingConnection) -> EdgeKind {
    match connection.kind {
        ConnectionKind::Send(_) => EdgeKind::Send,
        ConnectionKind::Sidechain => EdgeKind::Sidechain,
        ConnectionKind::Main => {
            let is_return = routing
                .ports
                .get(&connection.source)
                .and_then(|p| match p.owner {
                    PortOwner::Strip(strip) => routing.strips.get(&strip),
                    PortOwner::Processor(_) => None,
                })
                .and_then(|strip| match strip.owner {
                    StripOwner::Bus(bus) => routing.buses.get(&bus),
                    StripOwner::Track(_) => None,
                })
                .is_some_and(|bus| bus.kind == BusKind::Return);
            if is_return {
                EdgeKind::Return
            } else {
                EdgeKind::Main
            }
        }
    }
}

/// `ExplicitChannelMap`（destination→source index）→
/// `ChannelMap`（`(source, destination)` pairs — lowering と同じ
/// 規約）。channel index が `u16` を超える場合は fail-closed。
fn channel_map_of(
    map: &synvelis_project::routing::ExplicitChannelMap,
    connection: RoutingConnectionId,
) -> Result<ChannelMap, SelectionBuildError> {
    let mut entries = Vec::with_capacity(map.entries.len());
    for (dst, &src) in map.entries.iter().enumerate() {
        let src = u16::try_from(src).map_err(|_| SelectionBuildError::InvalidConnection {
            connection,
            detail: format!("channel map source index {src} exceeds u16"),
        })?;
        let dst = u16::try_from(dst).map_err(|_| SelectionBuildError::InvalidConnection {
            connection,
            detail: format!("channel map destination index {dst} exceeds u16"),
        })?;
        entries.push((src, dst));
    }
    Ok(ChannelMap { entries })
}

// ---------------------------------------------------------------------------
// public producer
// ---------------------------------------------------------------------------

/// `resolve_selection` の node 採番結果 — strip/processor/sink の
/// `RcpNodeId` 対応表。
struct NodeIds {
    node_of_strip: BTreeMap<StripId, RcpNodeId>,
    node_of_processor: BTreeMap<ProcessorId, RcpNodeId>,
    sink_node: RcpNodeId,
}

/// `resolve_selection` の途中状態 — port 採番・node build・
/// strip plumbing・capability entries。
#[derive(Default)]
struct GraphBuild {
    port_id_of: BTreeMap<SemanticPortId, RcpPortId>,
    nodes: BTreeMap<RcpNodeId, NodeBuild>,
    strip_plumbing: BTreeMap<StripId, StripPlumbing>,
    capabilities_entries: BTreeMap<PluginKey, PluginEnvironment>,
}

/// node id 採番: strips → processors → sink の決定順。
fn assign_node_ids(routing: &RoutingModel) -> NodeIds {
    let mut next_node = 1u64;
    let mut node_of_strip = BTreeMap::new();
    let mut node_of_processor = BTreeMap::new();
    for strip_id in routing.strips.keys() {
        node_of_strip.insert(*strip_id, RcpNodeId(next_node));
        next_node += 1;
    }
    for processor_id in routing.processors.keys() {
        node_of_processor.insert(*processor_id, RcpNodeId(next_node));
        next_node += 1;
    }
    NodeIds {
        node_of_strip,
        node_of_processor,
        sink_node: RcpNodeId(next_node),
    }
}

/// strips を `NodeClass::Builtin` node として構築する。chain 非空の
/// strip には snapshot-local plumbing port（`pre_chain` Output /
/// `chain_out` Input）を追加し、`strip_plumbing` に記録する。
fn build_strip_nodes(
    routing: &RoutingModel,
    ids: &NodeIds,
    audio: AudioFormat,
    build: &mut GraphBuild,
) -> Result<(), SelectionBuildError> {
    for (strip_id, strip) in &routing.strips {
        let node_id = ids.node_of_strip[strip_id];
        let (mut ports, mut in_next, mut out_next) = assign_ports(
            routing,
            &strip.port_ids,
            strip_id,
            &mut build.port_id_of,
            audio,
        )?;
        let chain_non_empty = routing
            .chains
            .get(&strip.chain)
            .is_some_and(|c| !c.processors.is_empty());
        if chain_non_empty {
            // chain plumbing: pre_chain output + chain_out input。
            // semantic port ではない snapshot-local 構造 port で、
            // chain 内部の cutset 境界解析を成立させる。
            let pre_chain = RcpPortId(out_next);
            out_next += 1;
            let chain_out = RcpPortId(in_next);
            in_next += 1;
            ports.push(PortEntry {
                id: pre_chain,
                direction: Direction::Output,
                role: PortRole::Main,
                is_input: false,
            });
            ports.push(PortEntry {
                id: chain_out,
                direction: Direction::Input,
                role: PortRole::Main,
                is_input: true,
            });
            build.strip_plumbing.insert(
                *strip_id,
                StripPlumbing {
                    pre_chain,
                    chain_out,
                },
            );
        }
        let _ = (in_next, out_next);
        build.nodes.insert(
            node_id,
            NodeBuild {
                id: node_id,
                class: NodeClass::Builtin,
                ports,
                plugin: None,
                label: Some(format!("strip:{strip_id}")),
            },
        );
    }
    Ok(())
}

/// chain processor を `NodeClass::Plugin` node + `PluginDeclaration`
/// として構築し、caller evidence を capability entries へ写す。
/// evidence 欠落・descriptor 不一致は fail-closed。
fn build_processor_nodes(
    routing: &RoutingModel,
    inputs: &SelectionInputs<'_>,
    ids: &NodeIds,
    audio: AudioFormat,
    build: &mut GraphBuild,
) -> Result<(), SelectionBuildError> {
    for (processor_id, processor) in &routing.processors {
        let node_id = ids.node_of_processor[processor_id];
        let binding_descriptor = match &processor.binding {
            ProcessorBinding::Resolved(plugin) => plugin.descriptor.clone(),
            ProcessorBinding::Unresolved { descriptor } => descriptor.clone(),
        };
        let evidence = inputs.plugin_evidence.get(processor_id).ok_or(
            SelectionBuildError::MissingPluginEvidence {
                processor: *processor_id,
            },
        )?;
        if evidence.descriptor != binding_descriptor {
            return Err(SelectionBuildError::DescriptorMismatch {
                processor: *processor_id,
                binding: binding_descriptor,
                evidence: evidence.descriptor.clone(),
            });
        }
        if let Some(env) = inputs.environments.get(&binding_descriptor) {
            build
                .capabilities_entries
                .insert(PluginKey::of(&evidence.identity), env.clone());
        }
        let (ports, _, _) = assign_ports(
            routing,
            &processor.port_ids,
            processor_id,
            &mut build.port_id_of,
            audio,
        )?;
        build.nodes.insert(
            node_id,
            NodeBuild {
                id: node_id,
                class: NodeClass::Plugin,
                ports,
                plugin: Some(PluginDeclaration {
                    identity: evidence.identity.clone(),
                    state: evidence.state.clone(),
                    declared_ports: processor_declared_ports(routing, *processor_id, audio),
                    automation_identity_complete: evidence.automation_identity_complete,
                }),
                label: Some(format!("processor:{processor_id}")),
            },
        );
    }
    Ok(())
}

/// `port_id_of`（`SemanticPortId` → `RcpPortId`）と node 対応表から
/// `SemanticPortId` → `PortEndpoint`（owner node + port）を構築する。
fn build_port_map(
    routing: &RoutingModel,
    port_id_of: &BTreeMap<SemanticPortId, RcpPortId>,
    ids: &NodeIds,
) -> BTreeMap<SemanticPortId, PortEndpoint> {
    let mut port_map = BTreeMap::new();
    for (port_id, rc) in port_id_of {
        let port = &routing.ports[port_id];
        let node = match port.owner {
            PortOwner::Strip(strip) => ids.node_of_strip[&strip],
            PortOwner::Processor(proc) => ids.node_of_processor[&proc],
        };
        port_map.insert(*port_id, PortEndpoint { node, port: *rc });
    }
    port_map
}

/// device sink node（`NodeClass::Sink`）— semantic model に device
/// port は存在しないため snapshot-local 構造として合成する
/// （lowering の `connect_master_sink` と同じ境界）。
fn sink_node_build(sink_node: RcpNodeId) -> (NodeBuild, PortEndpoint) {
    let sink_in = RcpPortId(0);
    (
        NodeBuild {
            id: sink_node,
            class: NodeClass::Sink,
            ports: vec![PortEntry {
                id: sink_in,
                direction: Direction::Input,
                role: PortRole::Main,
                is_input: true,
            }],
            plugin: None,
            label: Some("device-out".to_string()),
        },
        PortEndpoint {
            node: sink_node,
            port: sink_in,
        },
    )
}

/// master strip の main output semantic port — sink への feed 元。
fn master_output_port(routing: &RoutingModel) -> Result<SemanticPortId, SelectionBuildError> {
    let master_strip_id = routing
        .buses
        .get(&routing.master)
        .map(|bus| bus.strip)
        .ok_or_else(|| SelectionBuildError::MissingSemanticPort {
            detail: "master bus or its strip is missing".to_string(),
        })?;
    let master_strip = routing.strips.get(&master_strip_id).ok_or_else(|| {
        SelectionBuildError::MissingSemanticPort {
            detail: format!("master strip {master_strip_id} missing"),
        }
    })?;
    find_main_port(routing, &master_strip.port_ids, PortDirection::Output)
}

/// edge 採番器 — `RcpEdgeId` を決定順に採番して edge を蓄積する。
struct EdgeSink {
    edges: Vec<ResolvedEdge>,
    next: u64,
}

impl EdgeSink {
    fn new() -> Self {
        Self {
            edges: Vec::new(),
            next: 1,
        }
    }

    fn push(
        &mut self,
        from: PortEndpoint,
        to: PortEndpoint,
        kind: EdgeKind,
        channel_map: Option<ChannelMap>,
    ) {
        self.edges.push(ResolvedEdge {
            id: RcpEdgeId(self.next),
            from,
            to,
            kind,
            delay: EdgeDelay::Zero,
            channel_map,
            pdc_frames: 0,
        });
        self.next += 1;
    }
}

/// chain 内部 edge: `strip.pre_chain` → proc0.in → … → procN.out →
/// `strip.chain_out`（chain 順が信号順 — lowering と同一規則）。
fn chain_edges(
    routing: &RoutingModel,
    ids: &NodeIds,
    port_map: &BTreeMap<SemanticPortId, PortEndpoint>,
    strip_plumbing: &BTreeMap<StripId, StripPlumbing>,
    sink: &mut EdgeSink,
) -> Result<(), SelectionBuildError> {
    for strip in routing.strips.values() {
        let Some(plumbing) = strip_plumbing.get(&strip.id) else {
            continue;
        };
        let strip_node = ids.node_of_strip[&strip.id];
        let chain = routing.chains.get(&strip.chain).ok_or_else(|| {
            SelectionBuildError::MissingSemanticPort {
                detail: format!("strip {} references missing chain", strip.id),
            }
        })?;
        let mut prev_out = PortEndpoint {
            node: strip_node,
            port: plumbing.pre_chain,
        };
        for processor_id in &chain.processors {
            let processor = routing.processors.get(processor_id).ok_or_else(|| {
                SelectionBuildError::MissingSemanticPort {
                    detail: format!("chain references missing processor {processor_id}"),
                }
            })?;
            let in_sem = find_main_port(routing, &processor.port_ids, PortDirection::Input)?;
            let out_sem = find_main_port(routing, &processor.port_ids, PortDirection::Output)?;
            sink.push(prev_out, port_map[&in_sem], EdgeKind::Main, None);
            prev_out = PortEndpoint {
                node: ids.node_of_processor[processor_id],
                port: port_map[&out_sem].port,
            };
        }
        sink.push(
            prev_out,
            PortEndpoint {
                node: strip_node,
                port: plumbing.chain_out,
            },
            EdgeKind::Main,
            None,
        );
    }
    Ok(())
}

/// 有効な `RoutingConnection` → edge（disabled は edge に
/// ならない — semantic model の「disabled connection carries no
/// signal」に一致）。Output→Input 方向のみ許可。
fn connection_edges(
    routing: &RoutingModel,
    port_map: &BTreeMap<SemanticPortId, PortEndpoint>,
    nodes: &BTreeMap<RcpNodeId, NodeBuild>,
    sink: &mut EdgeSink,
) -> Result<(), SelectionBuildError> {
    for connection in routing.connections.values() {
        if !connection.enabled {
            continue;
        }
        let from = *port_map.get(&connection.source).ok_or_else(|| {
            SelectionBuildError::InvalidConnection {
                connection: connection.id,
                detail: format!("source port {} not in snapshot", connection.source),
            }
        })?;
        let to = *port_map.get(&connection.destination).ok_or_else(|| {
            SelectionBuildError::InvalidConnection {
                connection: connection.id,
                detail: format!(
                    "destination port {} not in snapshot",
                    connection.destination
                ),
            }
        })?;
        // 方向検査: Output→Input のみ edge になり得る。
        let direction_ok = {
            let from_is_input = nodes
                .get(&from.node)
                .and_then(|n| n.ports.iter().find(|p| p.id == from.port))
                .is_some_and(|p| p.is_input);
            let to_is_input = nodes
                .get(&to.node)
                .and_then(|n| n.ports.iter().find(|p| p.id == to.port))
                .is_some_and(|p| p.is_input);
            !from_is_input && to_is_input
        };
        if !direction_ok {
            return Err(SelectionBuildError::InvalidConnection {
                connection: connection.id,
                detail: "endpoint directions are not Output→Input".to_string(),
            });
        }
        let channel_map = connection
            .channel_map
            .as_ref()
            .map(|map| channel_map_of(map, connection.id))
            .transpose()?;
        sink.push(from, to, edge_kind(routing, connection), channel_map);
    }
    Ok(())
}

/// edge 構築: chain 内部（chain 順 = 信号順）→ 有効な
/// `RoutingConnection` → master out → device sink。
fn build_edges(
    routing: &RoutingModel,
    ids: &NodeIds,
    port_map: &BTreeMap<SemanticPortId, PortEndpoint>,
    strip_plumbing: &BTreeMap<StripId, StripPlumbing>,
    nodes: &BTreeMap<RcpNodeId, NodeBuild>,
    master_out_sem: SemanticPortId,
    sink_endpoint: PortEndpoint,
) -> Result<Vec<ResolvedEdge>, SelectionBuildError> {
    let mut sink = EdgeSink::new();
    chain_edges(routing, ids, port_map, strip_plumbing, &mut sink)?;
    connection_edges(routing, port_map, nodes, &mut sink)?;
    // master strip main out → device sink。
    sink.push(
        port_map[&master_out_sem],
        sink_endpoint,
        EdgeKind::Main,
        None,
    );
    Ok(sink.edges)
}

/// render candidate 変換に必要な snapshot 対応表の束。
struct SemanticIndex<'a> {
    node_of_strip: &'a BTreeMap<StripId, RcpNodeId>,
    node_of_processor: &'a BTreeMap<ProcessorId, RcpNodeId>,
    port_map: &'a BTreeMap<SemanticPortId, PortEndpoint>,
    nodes: &'a BTreeMap<RcpNodeId, ResolvedNode>,
}

/// Semantic model + caller evidence → RCP-04 の 3 pin 入力を構築する。
///
/// 決定性: node/port/edge ID は `BTreeMap`/`port_ids` 宣言順の
/// 走査で採番され、同一入力は常に同一 snapshot を返す。
/// `structure_digest` には `routing_fingerprint(project)` を採用する
/// — `ExecutionSelection.receipt.build_key` が `RoutingEngine` の
/// `checked_key` と field 一致するために必須（typed view 独自の
/// `compute_structure_digest` では engine の build key と乖離する）。
///
/// # Errors
/// [`SelectionBuildError`] — fail-closed。証拠欠落・binding 不整合・
/// port layout 不整合・coverage 不明を推測で埋めない。
pub fn resolve_selection(
    inputs: &SelectionInputs<'_>,
) -> Result<SelectionResolution, SelectionBuildError> {
    let project = inputs.project;
    let routing = &project.routing;
    let audio = engine_format(&inputs.engine);

    let ids = assign_node_ids(routing);
    let mut build = GraphBuild::default();
    build_strip_nodes(routing, &ids, audio, &mut build)?;
    build_processor_nodes(routing, inputs, &ids, audio, &mut build)?;

    let port_map = build_port_map(routing, &build.port_id_of, &ids);
    let master_out_sem = master_output_port(routing)?;
    let (sink_build, sink_endpoint) = sink_node_build(ids.sink_node);
    build.nodes.insert(ids.sink_node, sink_build);
    let edges = build_edges(
        routing,
        &ids,
        &port_map,
        &build.strip_plumbing,
        &build.nodes,
        master_out_sem,
        sink_endpoint,
    )?;

    // -- snapshot ---------------------------------------------------------
    let mut node_map: BTreeMap<RcpNodeId, ResolvedNode> = BTreeMap::new();
    for node_build in build.nodes.into_values() {
        node_map.insert(node_build.id, node_build.into_node(audio));
    }
    // sink の multiplicity は required feed（min=1）にする。
    if let Some(sink) = node_map.get_mut(&ids.sink_node) {
        for port in &mut sink.ports {
            port.multiplicity = Multiplicity::unbounded(1);
        }
    }

    let capability_generation = port_capability_generation(project);
    let structure_digest = routing_fingerprint(project).0;
    let snapshot = ResolvedGraphSnapshot {
        snapshot_id: inputs.snapshot_id,
        project_id: project.project_id,
        project_revision: inputs.project_revision,
        structure_digest,
        capability_generation,
        engine_config_generation: inputs.engine_config_generation,
        source_format_generation: inputs.source_format_generation,
        clock_epoch: inputs.clock_epoch,
        runtime_instance: inputs.runtime_instance,
        nodes: node_map,
        edges,
        required_outputs: vec![BoundaryRequirement {
            port: sink_endpoint,
            purpose: BoundaryPurpose::MainOut,
        }],
    };

    let capabilities = PluginCapabilitySnapshot {
        generation: capability_generation,
        entries: build.capabilities_entries,
    };

    // -- render candidates --------------------------------------------------
    let index = SemanticIndex {
        node_of_strip: &ids.node_of_strip,
        node_of_processor: &ids.node_of_processor,
        port_map: &port_map,
        nodes: &snapshot.nodes,
    };
    let mut candidates = Vec::with_capacity(inputs.render_candidates.len());
    for candidate in inputs.render_candidates {
        candidates.push(build_render_candidate(
            candidate,
            &index,
            inputs.project_revision,
            structure_digest,
            audio,
        )?);
    }
    candidates.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
    let renders = RenderCandidateIndex {
        generation: inputs.render_index_generation,
        candidates,
    };

    Ok(SelectionResolution {
        snapshot,
        capabilities,
        renders,
    })
}

/// `SemanticRenderCandidate` → `RenderCandidate`。coverage の
/// semantic ID を snapshot-local ID へ写像し、lookup report の
/// eligibility/reproducibility/access を verbatim で pin する
/// （`reason_of_eligibility`/`reason_of_lookup` の表示契約と
/// 同一語彙 — 変換による乖離を作らない）。output の format/domain
/// は snapshot 側 port の宣言値を使う（cutset の
/// FormatMismatch/DomainMismatch と整合）。latency は semantic
/// model が保持しないため 0。
fn build_render_candidate(
    candidate: &SemanticRenderCandidate,
    index: &SemanticIndex<'_>,
    revision: ProjectRevision,
    digest: u64,
    audio: AudioFormat,
) -> Result<RenderCandidate, SelectionBuildError> {
    let port_dir = |endpoint: PortEndpoint| -> Option<Direction> {
        index
            .nodes
            .get(&endpoint.node)
            .and_then(|n| n.ports.iter().find(|p| p.id == endpoint.port))
            .map(|p| p.direction)
    };
    let mut node_set = BTreeSet::new();
    for strip in &candidate.coverage.strips {
        node_set.insert(*index.node_of_strip.get(strip).ok_or_else(|| {
            SelectionBuildError::UnknownCoverage {
                detail: format!("strip {strip} not in snapshot"),
            }
        })?);
    }
    for processor in &candidate.coverage.processors {
        node_set.insert(*index.node_of_processor.get(processor).ok_or_else(|| {
            SelectionBuildError::UnknownCoverage {
                detail: format!("processor {processor} not in snapshot"),
            }
        })?);
    }
    let mut outputs = Vec::new();
    for port_id in &candidate.coverage.outputs {
        let endpoint =
            *index
                .port_map
                .get(port_id)
                .ok_or_else(|| SelectionBuildError::UnknownCoverage {
                    detail: format!("output port {port_id} not in snapshot"),
                })?;
        // output port は Output direction 必須 — 入力 port を
        // artifact output として宣言するのは coverage の誤り。
        if port_dir(endpoint) != Some(Direction::Output) {
            return Err(SelectionBuildError::UnknownCoverage {
                detail: format!("output port {port_id} is not an Output port"),
            });
        }
        outputs.push(ArtifactOutput {
            source: endpoint,
            domain: SignalDomain::Audio,
            format: PortFormat::Audio(audio),
            latency_frames: 0,
        });
    }
    outputs.sort_by_key(|o| o.source);
    let mut pinned_inputs = BTreeSet::new();
    for port_id in &candidate.coverage.pinned_inputs {
        let endpoint =
            *index
                .port_map
                .get(port_id)
                .ok_or_else(|| SelectionBuildError::UnknownCoverage {
                    detail: format!("pinned input port {port_id} not in snapshot"),
                })?;
        if port_dir(endpoint) != Some(Direction::Input) {
            return Err(SelectionBuildError::UnknownCoverage {
                detail: format!("pinned input port {port_id} is not an Input port"),
            });
        }
        pinned_inputs.insert(endpoint);
    }
    Ok(RenderCandidate {
        artifact_id: candidate.report.artifact.0.to_string(),
        content_hash: candidate.content_hash.clone(),
        node_set,
        outputs,
        pinned_inputs,
        eligibility: candidate.report.eligibility,
        reproducibility: candidate.report.reproducibility,
        snapshot_revision: revision,
        snapshot_digest: digest,
        access: candidate.report.access.clone(),
    })
}

// ---------------------------------------------------------------------------
// selection → plan → engine wiring
// ---------------------------------------------------------------------------

/// [`drive_selection_plan`] の結果。payload は box 済み —
/// receipt 系の大きな struct を variant 間で均す。
pub enum SelectionPlanDrive {
    /// Engine が `NoRebuild` equivalence を返した — live plan が
    /// configured revision を semantic にカバー済み。publish も
    /// reconciler への記録も行わない（engine の `Equivalent`
    /// 契約どおり receipt 不変）。
    Equivalent {
        /// Reconciler が採番した candidate（publish されなかった
        /// ため evidence のみ）。
        plan: Box<PreparedPlan>,
        /// Engine の equivalence 証拠。
        equivalence: Box<ExecutionEquivalence>,
    },
    /// prepare → publish が完走し reconciler も `Published` を
    /// 記録した。
    Published {
        /// Reconciler plan。
        plan: Box<PreparedPlan>,
        /// Engine 側 activation receipt。
        engine_receipt: Box<RoutingActivationReceipt>,
        /// Reconciler 側 activation receipt。
        reconcile_receipt: Box<RoutingActivationReceipt>,
    },
    /// prepare または publish が fault で失敗 — last-good 保持、
    /// active plan 不変、receipt は `Failed`/`Blocked`。
    Failed {
        /// Reconciler plan。
        plan: Box<PreparedPlan>,
        /// 発生した fault。
        fault: RoutingFault,
        /// Reconciler 側 receipt（pin 再検証・last-good 証拠込み）。
        reconcile_receipt: Box<RoutingActivationReceipt>,
    },
}

/// `PlanReconciler` の private `check_pins` と同一規則の pre-publish
/// stale gate — selection が解決した pin が live snapshot /
/// capability snapshot と既に乖離している場合、engine の
/// prepare/publish を一切呼ばずに拒否する（stale な selection 結果
/// の plan 公開は禁止 — reconciler の publish 時再検証だけでは
/// engine 側の activation が先に起きてしまうため、境界側で
/// 同じ規則を先に適用する）。
fn selection_pin_fault(
    key: &synvelis_project::RoutingBuildKey,
    live: &ResolvedGraphSnapshot,
    capabilities: &PluginCapabilitySnapshot,
) -> Option<RoutingFault> {
    if key.project_id != live.project_id || key.project_revision != live.project_revision {
        return Some(RoutingFault::StaleProjectSnapshot);
    }
    if key.routing_fingerprint.0 != live.structure_digest {
        return Some(RoutingFault::SpeculativeMismatch);
    }
    if key.capability_generation != live.capability_generation
        || capabilities.generation != live.capability_generation
    {
        return Some(RoutingFault::PluginCapabilityChanged);
    }
    if key.engine_config_generation != live.engine_config_generation
        || key.source_format_generation != live.source_format_generation
    {
        return Some(RoutingFault::UnsupportedFormatOrTimeMap);
    }
    if key.device_clock_epoch != live.clock_epoch || key.runtime_instance != live.runtime_instance {
        return Some(RoutingFault::DeviceEpochChanged);
    }
    None
}

/// [`drive_selection_plan`] の入力一式 — engine/reconciler の
/// mutable 借用と、pin 済み snapshot/capability/selection/request/
/// deps を 1 つの context に束ねる。
pub struct PlanDrive<'a> {
    /// Plan 構築先の routing engine。
    pub engine: &'a mut RoutingEngine,
    /// Plan receipt を記録する reconciler。
    pub reconciler: &'a mut PlanReconciler,
    /// semantic project（revision 時点の live model）。
    pub project: &'a Project,
    /// `project` の commit 済み revision。
    pub revision: ProjectRevision,
    /// reconciler pin 再検証が比較する現在 snapshot —
    /// [`resolve_selection`] の出力をそのまま渡すこと。
    pub live: &'a ResolvedGraphSnapshot,
    /// 現在の plugin capability snapshot。
    pub capabilities: &'a PluginCapabilitySnapshot,
    /// 駆動する selection 結果。
    pub selection: &'a ExecutionSelection,
    /// engine prepare への compile request。
    pub request: &'a RoutingCompileRequest,
    /// engine prepare への deps（canonical resolver・clips・
    /// generation pins）。
    pub deps: RoutingDeps<'a>,
}

/// `ExecutionSelection` → `PlanReconciler` → `RoutingEngine`
/// prepare/publish の headless 配線。
///
/// 流れ:
/// 0. stale gate — `selection.receipt.build_key` の pin が `live`
///    snapshot / `capabilities` と乖離していれば engine を呼ばず
///    `Failed` で返す（stale selection の公開禁止）。
/// 1. `PlanReconciler::prepare(&selection.receipt)` — receipt の
///    `build_key` / `render_index_generation` / `selection_digest`
///    を pin した `PreparedPlan` を採番。
/// 2. `RoutingEngine::prepare(request, project, revision, deps)` —
///    engine 自身の `checked_key`（`routing_fingerprint` +
///    `port_capability_generation` + deps/request pins）で
///    stale/capability/epoch を拒否。`Equivalent` なら publish せず
///    `NoRebuild` 証拠を返す。
/// 3. `RoutingEngine::publish` → `Result<(), RoutingFault>` を
///    `PlanReconciler::publish` へ渡し、pin 再検証・last-good 保持・
///    stale activation 拒否を receipt として記録する。
///
/// `live`/`capabilities` は reconciler の pin 再検証が比較する
/// 現在 snapshot — [`resolve_selection`] の出力をそのまま渡すこと
/// （selection と異なる snapshot を渡すと stale として正しく拒否
/// される）。`selection.verdict == Blocked` の selection をここへ
/// 渡すこと自体は妨げない — plan 構築は機構であり、exact 実行の
/// 可否は selection verdict が別層で担う。ただし呼び出し側は
/// 通常 verdict を確認してから drive する。
///
/// fault は panic せず [`SelectionPlanDrive::Failed`] に収める。
#[must_use]
pub fn drive_selection_plan(drive: PlanDrive<'_>) -> SelectionPlanDrive {
    let PlanDrive {
        engine,
        reconciler,
        project,
        revision,
        live,
        capabilities,
        selection,
        request,
        deps,
    } = drive;
    let plan = reconciler.prepare(&selection.receipt);
    // Pre-publish stale gate: selection pins that already diverged
    // from the live snapshot must never reach the engine — plan
    // publication for stale selection results is forbidden.
    if let Some(fault) = selection_pin_fault(&selection.receipt.build_key, live, capabilities) {
        let reconcile_receipt = reconciler.publish(&plan, live, capabilities, Err(fault));
        return SelectionPlanDrive::Failed {
            plan: Box::new(plan),
            fault,
            reconcile_receipt: Box::new(reconcile_receipt),
        };
    }
    match engine.prepare(request, project, revision, deps) {
        Err(fault) => {
            let reconcile_receipt = reconciler.publish(&plan, live, capabilities, Err(fault));
            SelectionPlanDrive::Failed {
                plan: Box::new(plan),
                fault,
                reconcile_receipt: Box::new(reconcile_receipt),
            }
        }
        Ok(RoutingPrepare::Equivalent(equivalence)) => SelectionPlanDrive::Equivalent {
            plan: Box::new(plan),
            equivalence: Box::new(equivalence),
        },
        Ok(RoutingPrepare::Built(prepared)) => match engine.publish(*prepared, project, revision) {
            Ok(engine_receipt) => {
                let reconcile_receipt = reconciler.publish(&plan, live, capabilities, Ok(()));
                SelectionPlanDrive::Published {
                    plan: Box::new(plan),
                    engine_receipt: Box::new(engine_receipt),
                    reconcile_receipt: Box::new(reconcile_receipt),
                }
            }
            Err(fault) => {
                let reconcile_receipt = reconciler.publish(&plan, live, capabilities, Err(fault));
                SelectionPlanDrive::Failed {
                    plan: Box::new(plan),
                    fault,
                    reconcile_receipt: Box::new(reconcile_receipt),
                }
            }
        },
    }
}
