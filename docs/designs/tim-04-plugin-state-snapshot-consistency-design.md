---
id: "DESIGN-034UC0ixTsN58rRlRctgAH"
type: "design"
status: "approved"
title: "TIM-04 plugin state snapshot consistency design — quality receipt/store/session/offline recipe"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/ruviform/issues/196"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/designs/tim03-process-boundary-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/tim03-process-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  defers: []
  deferred-by: []
---

# TIM-04 plugin state snapshot consistency design — quality receipt/store/session/offline recipe

## Context

`synvelis-project` は Project Save（`ProjectController::save` →
`ProjectStore` staged publish → HEAD）と VCS-01 の read-only
`SnapshotResolver` を持つが、plugin の opaque state を capture/
restore する経路・品質分類・receipt を持たない。`OpaqueStateRef`
は `Unknown` 固定で、実 plugin host への probe は未接続
（VCS-01 spec の deferred concern）。Ruviform #196 が
`HostCommand::SaveState`/`RestoreState` と `Plugin::save_state`/
`restore_state`（default `HostError::Unsupported`）を Own して
おり、Synvelis はそれを consumer として品質 Receipt/Reload/
Offline Recipe を実装する。RT callback からの state save/FS IO
は禁止であり、全経路は非 RT。

## Scope

### In Scope

- `crates/synvelis-project/src/plugin_state/` 新設
  （`mod.rs`/`types.rs`/`port.rs`/`store.rs`/`session.rs`）。
- `crates/synvelis-project/src/lib.rs` への additive export。
- `crates/synvelis-player/src/plugin_state.rs` 新設 +
  `host.rs`/`routing_engine.rs` への additive vend 経路
  （`ProcessorConnection.state_handle`・`RetainedConnection`・
  `RoutingEngine::plugin_state_handles`）。
- `crates/synvelis-render/src/spec.rs`/`preflight.rs`/
  `job.rs`/`lib.rs` への `PluginStateRecipe` 追加と品質ゲート。
- `crates/synvelis-project/tests/plugin_state.rs`・
  `crates/synvelis-player/tests/tim04_plugin_state.rs`・
  `crates/synvelis-render/tests/tim04_offline_recipe.rs`。

### Out of Scope

- 実 CLAP/VST3/AU・OS Plugin capability 実測、実 OS FS
  atomicity、L1 out-of-process state 経路、GUI、RT 経路、
  VCS snapshot への manifest 統合、`synvelis-app`/
  `workspace`/`library`/`gateway`/`recording`/`theme`/`poc`、
  `Cargo.toml`/`Cargo.lock`/`mise.toml`。

## Components and Boundaries

- **`plugin_state::types`** — 純粋な値型:
  `StateCaptureQuality`（`Coordinated`>`Quiescent`>`BestEffort`>
  `Unsupported` の序列を `rank()`/`exact_eligible()` で表現）、
  `CaptureMode`（caller の要求保証度）、
  `StateQualityPolicy`（`Standard`/`Strict`）、
  `PluginStateFailureKind`（`CaptureFailed`/`Timeout`/
  `PermissionDenied`/`Crashed`/`Corrupt`/`Unavailable`/`Missing`）、
  `PluginStateBinding`（descriptor/version/
  capability_generation/host_runtime_instance/graph_generation）、
  `CaptureObservation`（`processing`/`stable_boundary`）、
  `PluginStateCapture`（blob + `achieved` 品質 + detail）、
  `PluginStateCaptureError`（`Unsupported`/`Failed(kind)`）、
  `PluginStateOutcome`（`Captured{quality,blob_digest,blob_len}`/
  `Unsupported`/`Failed(kind)`）、`PluginStateRecord`、
  `PluginStateManifest`（`PLUGIN_STATE_SCHEMA_VERSION = 1`、
  `deny_unknown_fields`）、`PluginStateQualitySummary`
  （`exact_eligible()` = 全 record が `Captured{Coordinated|
  Quiescent}`、空は vacuous true）、`PluginStateReceipt`、
  `PluginStateRestoreReceipt`、`PluginStateError`。
- **`plugin_state::port`** — `PluginStatePort` trait:
  `describe(&self, ProcessorId) -> Option<PluginStateBinding>`、
  `capture(&mut self, ProcessorId, CaptureMode) ->
  Result<PluginStateCapture, PluginStateCaptureError>`、
  `restore(&mut self, ProcessorId, &[u8]) ->
  Result<(), PluginStateCaptureError>`。`ProcessorId` のみを
  キーにし、project 型への依存は session 側に限定する。
- **`plugin_state::store`** — `PluginStateStore` trait:
  `save_manifest(bytes) -> Result<String, PluginStateStoreError>`
  （content-addressed 名 `ps-<digest16>.json` を返し HEAD を
  repoint）、`head()`、`read_manifest(name)`、
  `put_blob(digest, bytes)`、`read_blob(digest)`。
  `PluginStateStoreError::{NotPublished, Indeterminate, Failed}`
  — `Indeterminate` は publish 効果が確定しない場合で、
  session が HEAD/blob readback で reconcile する。
  `InMemoryPluginStateStore`（atomic・常に NotPublished か
  Ok）と `DirectoryPluginStateStore`（`<root>/manifests/`・
  `<root>/blobs/`・`<root>/HEAD`、publish は
  `ruviform_io_file` staged publish `FileAndDirectorySync`
  再利用、blob は digest 名で immutable `CreateNew`、
  manifest 衝突は byte-identical のみ受理）。
- **`plugin_state::session`** — `PluginStateSession{port, store}`
  + `save(&PluginStateSaveRequest)`/`restore(
  &PluginStateRestoreRequest)`:
  - save: `project.routing.processors` を `ProcessorId` 順に
    走査 → `Unresolved`/describe None → `Failed(Unavailable)`、
    `capture` の `Unsupported` → `Unsupported`、
    `Failed(kind)` → `Failed(kind)`、`Ok` → digest 計算。
    manifest 構築 → `Strict` なら `exact_eligible()` を検査し
    違反は `QualityRejected`（store 未接触）→ blob put
    （`Indeterminate` は read_blob+digest 照合で reconcile、
    不一致は `Store` error で manifest 未 publish = HEAD 保護）
    → manifest publish → `Indeterminate` は head/read_manifest
    照合で `reconciled` receipt。
  - restore: manifest 名（明示 or HEAD）→ read → schema/digest
    検証 → record 毎に `read_blob`+sha256 照合（`Corrupt`）→
    `port.restore`。per-record outcome を receipt に記録し、
    project に無い processor は `missing_states` に列挙。
- **player `plugin_state`** — `PluginStateSession`（host
  adapter）: `Rc<RefCell<PluginHost>>` を共有し、
  `bind(processor_id, handle, plugin: &PluginRef,
  graph_generation)` で live instance を記録。
  `describe` は bind 情報を返し、`capture` は
  `lifecycle_state` 前後観測で `processing`/
  `stable_boundary` を求め、`SaveState`/`RestoreState` を
  `HostCommand` で発行。`achieved` 品質は
  `processing||!stable → BestEffort`、`stable && mode==
  Coordinated → Coordinated`、`stable && mode!=Coordinated →
  Quiescent` とする（mode=BestEffort でも stable non-
  processing なら実測 Quiescent を記録 — 過小申告しない）。
  `HostError` 写像: `Unsupported`→Unsupported、
  `Failed`/`Ipc`→`Crashed`、`PluginFailed`→`CaptureFailed`、
  その他→`CaptureFailed`。
- **engine vend 経路** — `ProcessorConnection` に
  `state_handle: Option<PluginHandle>`（plugin host のみ
  `Some`）、`RetainedConnection` に同 field（Copy・drop 順に
  無関係）、`RoutingEngine::plugin_state_handles(generation)
  -> Vec<PluginStateEndpoint{processor,node,handle}>`（`&self`、
  handle は generation カウンタで stale 拒否されるため copy
  vend で安全）。
- **render `PluginStateRecipe`** — `ExportRequest.
  plugin_state: Option<PluginStateRecipe{policy, evidence:
  PluginStateQualitySummary, manifest_digest: Option<
  ContentDigest>}>`。preflight_static で `Strict` かつ
  `!evidence.exact_eligible()` → `PreflightError::
  PluginStateQuality`。`ExportReceipt.plugin_state` に
  recipe を記録（`Standard` でも evidence を偽らない）。

## Data and Control Flow

- save: caller が `ProjectController.live()` +
  `live_revision()` + `saved().head` + host/graph generation
  を `PluginStateSaveRequest` に詰め、`PluginStateSession::
  save` が capture→manifest→gate→publish→reconcile を実行。
  返る `PluginStateReceipt` は `SavedState`（project HEAD）
  とは別物で、`project_revision`/`project_head` で対応付く。
- restore: caller が manifest 名 or HEAD を指定し、
  `PluginStateRestoreReceipt` の per-record outcome を
  live project の processor と照合する。
- offline: caller が save/restore receipt または manifest
  の `summarize()` から `PluginStateQualitySummary` を得て
  `ExportRequest.plugin_state` に載せる。render は
  evidence を消費せず gate と記録のみ行う（実 plugin の
  render graph 組込みは deferred）。

## Non-Functional Constraints

- 全経路非 RT: `PluginStatePort`・`PluginStateStore`・
  session・player adapter・render preflight は RT
  callback から呼ばない。blob/manifest サイズは
  `SnapshotLimits` 相当の bound を設けず、store 層の
  `PublishOptions`/FS 制約に委ねる（headless blob は小さい）。
- 依存追加なし: `serde`/`serde_json`/`ruviform-io-file`/
  `ruviform-plugin-host`（player のみ既存 dep）/
  `ruviform-plugin-core`（project 既存 dep）のみ。
- `unsafe_code` forbid・`missing_docs` warn・clippy
  pedantic warn の workspace lint に従う。

## Risks and Trade-offs

- **manifest/project revision の乖離**: plugin state は
  project schema に field を追加せず別 store に置くため、
  project undo/redo は plugin state に影響しない（Issue の
  「Undo/History/Copy Restore で品質維持」は immutable
  manifest+blob の copy/readback で検証）。project_revision
  + project_head で対応付け、ズレは receipt で可視化。
- **内容 digest 名の衝突**: sha256 衝突は仮定しないが、
  byte-identical manifest の再 save は同名 idempotent で
  問題ない。異内容は別名になるため immutable 衝突は
  起きない（ProjectStore の watermark 衝突分類は不要）。
- **blob orphan**: capture 後に manifest publish 前の失敗で
  blob が orphan になり得る — content-addressed で無害、
  GC は deferred。
- **`stable_boundary` の保証度**: player adapter は
  lifecycle 前後一致のみを attest し、plugin 内部状態の
  真の一致点は保証しない — `Coordinated` は「host が観測
  した一致点」として記録し、実 plugin×format の強い保証は
  別受入 Gate（Issue 本文どおり）。

## Deferred Design Concerns

- 実 CLAP/VST3/AU・OS 別 capability 実測 | 実 plugin/OS が
  必要 | 実 plugin 受入 slice | Mock 証拠の実測換算。
- L1 out-of-process state 経路 | Ruviform L1 supervisor
  の責務 | L1 受入時 | In-process 成功の L1 換算。
- VCS snapshot への manifest 統合（`OpaqueStateRef` 昇格）
  | VCS-02/03 | VCS slice | `Unknown` 固定のまま。
- GUI/app/workspace 接続・render への実 plugin 組込み |
  別 Issue/Owner | app slice | headless receipt の UI
  不在。

## Verification Notes

- [ ] `synvelis-project` mock port/store tests: 全品質・
  失敗 kind・Strict fail-closed・indeterminate reconcile・
  copy restore。
- [ ] `synvelis-player` adapter tests: 実 `PluginHost` +
  mock plugin の SaveState/RestoreState round-trip、
  Processing 中 capture の BestEffort、stable capture の
  Coordinated/Quiescent。
- [ ] `synvelis-render` tests: Strict+非 exact evidence の
  preflight 拒否、Standard の receipt evidence。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`
  等の正規タスク。
