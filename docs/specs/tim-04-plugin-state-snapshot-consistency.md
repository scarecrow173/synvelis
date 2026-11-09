---
id: "SPEC-034UBZ7xNf0yey5lCLjVNT"
type: "spec"
status: "approved"
title: "TIM-04 Plugin State Snapshot 一貫性 — quality receipt・save/reload/HEAD/offline recipe（headless mock slice）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/ruviform/issues/196"
    - "https://github.com/scarecrow173/ruviform/issues/253"
  defers: []
  deferred-by: []
---

# TIM-04 Plugin State Snapshot 一貫性 — quality receipt・save/reload/HEAD/offline recipe（headless mock slice）

## What Should Be Built

Issue #128（TIM-04 Plugin State Snapshot 一貫性・保存品質 Receipt と
Reload/Offline 対応）のうち、headless/mock plugin で検証可能な
slice を実装する。Project Save と Plugin State Snapshot の整合を
構造化し、「File Save 成功」を「Plugin State exact」と誤表示
しない品質モデルを `synvelis-project` に新設し、
`synvelis-player` の非 RT State Snapshot Plugin Integration と
`synvelis-render` の Offline Recipe 品質ゲートを接続する。

- **`StateCaptureQuality`**: Issue 語彙どおり
  `Coordinated`/`Quiescent`/`BestEffort`/`Unsupported`。
  `Coordinated` は plugin/host 一致点確認、`Quiescent` は実行
  停止または安全な静止状態、`BestEffort` は時点一貫性未保証、
  `Unsupported` は state capability 非対応。
- **`PluginStatePort`**: `synvelis-project` が所有する非 RT
  capture/restore 境界。実装側（`synvelis-player` の
  `PluginHost` adapter、test mock）が `describe`/`capture`/
  `restore` を提供し、capture は blob と観測事実
  （`processing`・`stable_boundary`）を返す。品質の最終分類は
  port が返す `achieved` を記録し、project 側は policy を
  強制する — Generic Plugin Host/Lifecycle/State API は
  Ruviform（#196）が所有し複製しない。
- **`PluginStateStore`**: content-addressed blob
  （`blobs/<digest>`）+ content-addressed manifest
  （`manifests/ps-<digest>.json`）+ HEAD pointer を持つ
  persistence port。`InMemoryPluginStateStore` と
  `DirectoryPluginStateStore` を提供し、publish は
  `ruviform-io-file` の staged publish を再利用する。
  manifest 名は内容 digest 由来で immutable、HEAD が最新を指す。
- **`PluginStateSession`**: save/restore オーケストレーション。
  save は全 processor の capture → manifest 構築 → 品質
  policy ゲート → blob/manifest publish → HEAD/blob readback
  reconcile の順。`Standard` policy は品質を可視化した条件付き
  保存を許容し、`Strict`（Exact 要求）は
  `BestEffort`/`Unsupported`/失敗を `QualityRejected` で
  fail-closed する（何も publish しない）。
- **`PluginStateReceipt` / `PluginStateRestoreReceipt`**:
  per-processor の `PluginStateRecord`（processor_id・
  plugin binding 情報・project_revision・stored_at・
  outcome）と品質集計 `PluginStateQualitySummary` を持つ
  receipt。Saved Project/HEAD（`SavedState`）・RT Active
  （`RoutingActivationReceipt`）とは別 receipt であり、
  Save Completed ≠ Plugin State Exact ≠ RT Active を構造的に
  分離する。
- **`PluginStateManifest`**: project_revision・project_head
  （任意）・per-processor record を持つ versioned
  （`PLUGIN_STATE_SCHEMA_VERSION`）manifest。blob は
  `ContentDigest`（sha256）で参照し、record には
  `blob_digest`/`blob_len` を記録する。
- **player `PluginStateSession` adapter**: `PluginHost` の
  `HostCommand::SaveState`/`RestoreState`（Ruviform #196
  contract）を `PluginStatePort` として実装。`PluginHandle`
  は `ProcessorConnection` → `RetainedConnection` →
  `RoutingEngine::plugin_state_handles` で generation
  スコープに vend し、bind 時に descriptor/version/
  capability_generation/host_runtime_instance/graph_generation
  を記録する。capture の `processing` は `lifecycle_state` で
  観測し、`Coordinated` は lifecycle が capture 前後で
  non-Processing のまま安定した場合のみ attest する。
- **render Offline Recipe**: `ExportRequest.plugin_state` に
  `PluginStateRecipe { policy, evidence, manifest_digest }` を
  追加し、preflight が `Strict` policy で evidence の
  `exact_eligible()` を fail-closed 検査する。receipt は
  recipe evidence をそのまま記録し、条件付き保存を
  完璧な再現保証と偽らない。

## Why Now

Issue #128 は Project Save と Plugin State Snapshot の整合・
保存品質 Receipt・Reload/Offline 対応を要求する。VCS-01（#92）
の `OpaqueStateRef` は plugin opaque state を `Unknown` 固定で
記録するのみで、実 capture/restore・品質分類・HEAD reconcile
を持たない。TIM-03（#127）は process boundary の capability/
degraded policy を確立したが、state snapshot の保存品質は
未扱い。Ruviform #196 が Generic Plugin Adapter の非 RT
Save/Restore・Stateful Lifecycle を Own しており、本 slice は
その consumer として品質 Receipt/Reload/Offline Recipe を
Synvelis 側に実装する。実 CLAP/VST3/OS Plugin 別 capability
実測は本 Issue の独立受入 Gate であり、headless/mock で
検証可能な行を先行実装する（Issue 本文「#253/#92 の全面完成
待ちで基本 State Receipt の実装を止めない」に従う）。

## Users and Value

- **Project 利用者**: save 時に各 plugin の state が
  どの品質（Coordinated/Quiescent/BestEffort/Unsupported）で
  保存されたかを receipt で確認でき、Strict 保存では
  不十分な品質を fail-closed で拒否できる。
- **Reload/Restore を行う caller**: manifest + content-
  addressed blob から per-processor の restore 結果
  （Restored/NotCaptured/Unsupported/Failed）を receipt で
  受け取り、blob 破損・crash・timeout を typed に識別できる。
- **Offline Export を行う caller**: render recipe に plugin
  state 品質 evidence を添付し、offline exact 要求で
  BestEffort/Unsupported を preflight で拒否できる。
- **VCS-02 以降の差分・履歴 consumer**: manifest digest と
  per-processor quality を immutable snapshot と並置して
  比較・監査できる（比較自体は別 Issue の責務）。

## Scope

### In Scope

- `synvelis-project` への additive module `plugin_state`:
  `StateCaptureQuality`/`CaptureMode`/`StateQualityPolicy`/
  `PluginStateFailureKind`/`PluginStateBinding`/
  `CaptureObservation`/`PluginStateCapture`/
  `PluginStateCaptureError`/`PluginStatePort`/
  `PluginStateOutcome`/`PluginStateRecord`/
  `PluginStateManifest`/`PluginStateQualitySummary`/
  `PluginStateReceipt`/`PluginStateRestoreReceipt`/
  `PluginStateError`/`PluginStateStore`/
  `PluginStateStoreError`/`InMemoryPluginStateStore`/
  `DirectoryPluginStateStore`/`PluginStateSession`/
  `PluginStateSaveRequest`/`PluginStateRestoreRequest`/
  `plugin_state_manifest_name`。
- `synvelis-player` への `plugin_state` module:
  `PluginStateSession`（host adapter、`PluginStatePort`
  実装）と `ProcessorConnection.state_handle` →
  `RetainedConnection` → `RoutingEngine::plugin_state_handles`
  の generation-scoped vend 経路。
- `synvelis-render` への `ExportRequest.plugin_state`
  （`PluginStateRecipe`）と preflight 品質ゲート・
  `ExportReceipt` への evidence 記録。
- headless fixture tests: mock `PluginStatePort` による
  全品質・Crash/Timeout/Permission/Corrupt/Unsupported/
  Strict fail-closed/indeterminate publish reconcile/Undo・
  History・Copy Restore 品質維持、および player adapter の
  実 `PluginHost` + mock plugin による SaveState/RestoreState
  round-trip。
- `docs/` の spec/design/plan/task/test-spec/impl 文書チェーン。

### Out of Scope

- 実 CLAP/VST3/AU OS Plugin 別 capability 実測
  （Plugin×OS×Format の品質 Gate — 本 Issue の独立受入行、
  headless slice では `NotRun`/`Unsupported` として記録）。
- 実 OS filesystem atomicity・実 plugin crash の process
  分離検証（Ruviform L1 supervisor の責務を含む）。
- GUI・RT callback からの Plugin State Save/FS IO
  （非 RT のみ — RT callback からは呼ばない契約を維持）。
- VCS-02 Semantic Diff・VCS-03 Git Provider との統合
  （manifest/receipt は VCS Bytes Port にそのまま載る
  opaque bytes であり、本 slice では接続しない）。
- `synvelis-app`/`synvelis-workspace`/`synvelis-library`/
  `synvelis-gateway`/`synvelis-recording`/`synvelis-theme`/
  `recording-poc`/`mcp-poc` の変更。
- `Cargo.toml`/`Cargo.lock`/`mise.toml` の変更
  （依存追加なし — 既存依存のみで実装）。

## Requirements

- `StateCaptureQuality` は `Coordinated`/`Quiescent`/
  `BestEffort`/`Unsupported` の 4 値。`Coordinated` は
  plugin/host 一致点が確認できた capture、`Quiescent` は
  実行停止または安全な静止状態での capture、`BestEffort` は
  時点一貫性を保証しない capture、`Unsupported` は state
  capability 非対応。
- `PluginStatePort` は非 RT のみ: `describe(processor_id)`
  が live binding の plugin identity（descriptor/version/
  capability_generation/host_runtime_instance/
  graph_generation）を返し、`capture(processor_id, mode)` が
  opaque blob と `CaptureObservation{processing,
  stable_boundary}` を返し、`restore(processor_id, blob)` が
  以前の `SaveState` bytes を適用する。失敗は
  `PluginStateCaptureError::{Unsupported, Failed(kind)}` で
  typed。
- `PluginStateSession::save` は project の全 processor を
  決定的順序（`ProcessorId` 順）で capture し、
  `Unresolved` binding・live instance 不在を
  `Failed(Unavailable)`、plugin の state 非対応を
  `Unsupported`、その他の失敗を `Failed(kind)` として
  blob なしで明示記録する — 空 blob を正しい state と
  みなさない。
- 品質 policy: `Standard` は全 outcome を記録して保存を
  許容し、`Strict` は `BestEffort`/`Unsupported`/`Failed`
  を含む save を `QualityRejected` で拒否し、blob・
  manifest・HEAD のいずれも publish しない（旧 state・
  recovery backup を保護）。
- manifest は `schema_version`/`project_id`/
  `project_revision`/`project_head`（任意）/`entries`
  を持ち、blob は content-addressed で `blob_digest`
  （`ContentDigest` sha256）+`blob_len` を記録する。
  `stored_at` は save 単位で caller が供給する。
- save は blob put（content-addressed・idempotent）→
  manifest publish（content-addressed 名・HEAD repoint）
  → HEAD/blob readback reconcile の順。publish が
  `Indeterminate` の場合は HEAD 名と manifest bytes・
  blob digest を readback で照合し、一致すれば
  `reconciled: true` の receipt、一致しなければ typed
  error とする — indeterminate を成功とも失敗とも
  断定しない。
- restore は manifest（明示名または HEAD）を読み、
  schema 検証・digest 記録の後、record ごとに blob を
  read_blob → sha256 照合（不一致は `Failed(Corrupt)`）→
  `port.restore` を実行し、per-record outcome を receipt
  に記録する。manifest に無い live processor は
  `missing_states` として報告し、無言で skip しない。
- Undo/History/Copy Restore: manifest+blob は immutable
  で revision に紐づき、project の undo/redo・load・
  store copy をまたいでも record の品質・digest・
  evidence が維持されることを fixture で検証する。
  v5 等の旧 Project には存在しない Plugin State を
  migration で作らない（`PluginStateManifest` は
  Project schema に依存せず、project 側に新規永続
  field を追加しない）。
- `synvelis-render` の Offline Recipe は
  `ExportRequest.plugin_state: Option<PluginStateRecipe>`
  で policy + quality evidence + manifest digest を運び、
  preflight が `Strict` で `exact_eligible()` を検査、
  receipt が evidence を記録する。実 plugin を render
  graph に組み込むことは本 slice の対象外。
- RT callback から Plugin State Save/restore・FS IO を
  呼ばない。player adapter の全経路は非 RT。
- 実 CLAP/VST3/AU・実 OS の state 保存品質は本 slice
  の対象外 — `NotRun`/`Unsupported` として PR・impl
  record に明記する。

## Acceptance Criteria

Issue #128 本文の受入条件を headless/mock で検証可能な
行に写像して採用する。実 VST3・実 OS・GUI は対象外で
`NotRun`/`Unsupported` を明記する。

- [ ] Coordinated/Quiescent/BestEffort/Unsupported 別の
  Save/Reload・Offline Recipe を fixture で検証。
  Plugin Crash/Timeout/Permission/Blob Corruption を
  typed failure として検証。
- [ ] Save Completed ≠ Plugin State Exact ≠ RT Active:
  `SavedState`・`PluginStateReceipt`・
  `RoutingActivationReceipt` が別 receipt であり、
  Standard save が BestEffort 品質を正直に記録する
  ことを検証。
- [ ] Indeterminate Publish は HEAD/Blob の readback で
  Reconcile し、成功・失敗を断定しない。
- [ ] Undo/History/Copy Restore で Plugin State 品質が
  維持されることを fixture で検証。
- [ ] Strict/Exact 要求の Save/Offline Export/Restore で
  BestEffort/Unsupported が fail-closed になることを
  検証。Standard save は品質を可視化した条件付き保存を
  許容し、完璧な再現保証を偽らない。
- [ ] `cargo`/`mise` 正規タスク（fmt-check/check/lint/
  test/test-rt 等、存在するもの全て）が pass。
  RT 非回帰・依存非回帰を確認。

## Deferred Design Concerns

- 実 CLAP/VST3/AU・OS 別 Plugin State capability 実測
  （Plugin×OS×Format の品質 Gate）| headless slice の
  範囲外・実 plugin/OS が必要 | 実 plugin 受入 slice
  着手時 | Mock 証拠を実 plugin 証拠に換算する誤り。
- L1 out-of-process plugin の state save/restore
  timeout・crash recovery 統合 | Ruviform #196/L1
  supervisor の責務境界・headless では in-process
  mock のみ | L1 state 経路の受入時 | In-process
  成功を L1 成功と誤認する誤り。
- VCS snapshot への plugin-state manifest 統合
  （`OpaqueStateRef` の `Known` 昇格）| VCS-02/03 の
  snapshot reader 統合は別 Issue | VCS 系 slice
  着手時 | Opaque state が恒久的に `Unknown` のまま。
- GUI からの save/restore UX・offline export への
  実 plugin 組み込み | `synvelis-app`/`workspace`
  （sv102）・render の plugin graph 組込みは別
  Issue/Owner | app/workspace slice 着手時 |
  headless receipt が UI に表示されない。

## Sources and Relations

- Issue #128（対象 Issue、品質 enum・evidence・policy・
  受入条件の正本）。
- Ruviform #196（Generic Plugin Adapter の Stateful
  Lifecycle/非 RT Save/Restore — `HostCommand::
  SaveState`/`RestoreState`・`Plugin::save_state`/
  `restore_state`・`HostError::Unsupported` を消費。
  複製しない）。
- Ruviform #253（OS MIDI backend/VST3 MIDI 翻訳 —
  Plugin State Save/Restore の Owner ではない。
  Issue #128 本文の Owner 訂正どおり参照のみ）。
- Issue #17 / `docs/designs/project-asset-persistence-architecture.md`
  （Project Save/HEAD・Saved/Recovery/VCS 境界）。
- Issue #84 / `docs/specs/syn-io-02-snapshot-head-staged-publish.md`
  （ProjectStore/HEAD staged publish 契約 — plugin-state
  store も同じ staged publish を再利用）。
- Issue #92 / `docs/specs/vcs-01-immutable-snapshot-resolver.md`
  （`ContentDigest`/`OpaqueStateRef`/`SnapshotResolver` —
  read-only snapshot 側の既存契約を再利用し変更しない）。
- Issue #127 / `docs/specs/tim03-process-boundary.md`
  （process boundary capability/degraded policy —
  非 RT preflight oracle の設計先例）。
- Issue #126（SMF import/export — project schema v9・
  `Project::smf_documents` の前例。plugin state は
  project schema に field を追加しない設計とした根拠）。
- `crates/synvelis-project/src/{store,snapshot,controller,routing}.rs`
  （既存の staged publish・digest・Single Writer・
  Processor/PluginRef — すべて再利用し変更しない）。
- `crates/synvelis-player/src/{host,routing_engine}.rs`
  （`ProcessorHost`/`ProcessorConnection`/
  `RetainedConnection`/`take_processor_control` —
  generation-scoped non-RT vend の先例）。
