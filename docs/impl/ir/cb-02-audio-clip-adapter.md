---
id: "IMPL-cb02-audio-clip-adapter"
type: "impl"
status: "completed"
title: "CB-02 Audio Clip/Take/Comp 候補 Adapter — Raw Source 不変・加工比較・Adopt（synvelis#279）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/279"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/variant/audio.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb02_audio_take.rs"
      - type: "file"
        path: "docs/impl/ir/cb-02-audio-clip-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-02-audio-clip-adapter.md"
      - type: "file"
        path: "docs/test-specs/cb-02-audio-clip-adapter.md"
      - type: "file"
        path: "evidence/sv279/verification.md"
      - type: "file"
        path: "evidence/sv279/acceptance-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/error.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/scope.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-02-audio-clip-adapter.md"
    - "docs/impl/ir/cb-01-variant-set.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/creative-branch.md"
    - "docs/designs/creative-branch-design.md"
    - "docs/adr/cb-00-creative-branch.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/cb-02-audio-clip-adapter.md"
    rejected: []
---

# Implementation Record: CB-02 Audio Clip/Take/Comp 候補 Adapter（synvelis#279）

## Summary

Issue #279 の CB-02 スライスとして、`VariantScope::AudioClip` と
`VariantScope::TakeUsage` に対する実ドメイン
`DomainAdapter`（`AudioClipAdapter`/`TakeUsageAdapter`）を
`crates/synvelis-project/src/variant/audio.rs` に実装した。
Raw media（`Source`/`Asset`/`RecordingSession`/`Take`/capture
系）は共有・不変のまま、clip-local 編集と comp registry
（`TakeGroup`/`Comp`/`CompRegion`/`CompTransition`）の
候補編集・比較・adopt materialize を headless fixture で
回帰固定した。`tests/cb02_audio_take.rs` に 10 件の Case ID
付き fixture を追加した。

## Context

- 対象 Issue: https://github.com/scarecrow173/synvelis/issues/279
  （CB-02 — CB-03〜08（#280〜285）・GPUI（#286）・Action/Gateway
  統合（#287）の前提）。
- CB-01（#278/PR #425）の `DomainAdapter` 契約と
  `FakeDomainAdapter` headless 先例の上に、実ドメイン Adapter
  を載せるスライス。engine 側の契約は変えない。
- `synvelis-gateway`/`synvelis-render` が `ProjectOp` に対して
  網羅 match を持つため、`ProjectOp` への variant 追加は禁止。
  `synvelis-render/tests/rcp01_recipe.rs` が 4 引数
  `adopt_branch` と `BranchEditIntent { op: ProjectOp }` を
  使用するため、既存 API シグネチャも source 互換を維持する。
- doc-driven-dev-graph の route は起動したが、リポジトリ全体の
  pre-existing `artifact-graph` blocker（本タスク無関係の docs
  関係）で fail-closed。CB-01 と同じく route 結果を evidence
  に記録し、spec/design/plan/task 相当の内容はリース済み 3
  文書（ir/exp/test-specs）に集約した。

## Implementation

### Changes

- `src/variant/audio.rs` 新設:
  - `AudioClipAdapter` — `VariantScope::AudioClip` の
    `ProjectOp` 候補編集（extent/placement/mute/note/automation
    等の clip-local op）を scope 閉じ込め付きで適用。
    `AudioClipCapabilities`/`ProcessingCapability` で
    Trim/Placement/Mute/ClipNotes=Supported、
    Loop/SimpleStretch/Pitch/Fade/ExternalRecipe=Unsupported を
    正直に報告し、edit guard と同一判定関数
    （`playback_mapping_capability`）を共有して表示と強制を
    一致させた（CB-CLIP-02）。
  - `TakeUsageAdapter` — `VariantScope::TakeUsage` の
    `BranchEditOp::Comp(CompOp)` 候補編集。take 選択
    （`SetCompRegionCandidate`）と processing（range/transition
    等）を区別し、comp op の対象 take group 閉じ込め・
    mapping capability・scope 外拒否を `check_comp_op` で
    fail-closed。`CompOp::apply` は自己 validate しないため
    adapter が `Operation::validate` を明示呼出。
  - adopt preflight: 録音中 session が対象 take group に関与
    する場合 `VariantError::RecordingActive`、候補 comp closure
    が参照する shared material（source/take/segment）を live が
    失っている場合 typed `NotFound` で adopt 前に拒否。
- `src/variant/adapter.rs`: `BranchEditOp` union
  （`Project(ProjectOp)`/`Comp(CompOp)`）追加、
  `DomainAdapter::edit_candidate` を `BranchEditOp` 受け取りに
  汎化、`fork_candidate_project` 共有 helper、
  `edit_scoped_project_op`（capability guard + scope 対象照合 +
  apply + scope removal/creation 閉じ込め）を fake/audio で
  共用する `OpGuard` 型 alias 付きで集約。
- `src/variant/engine.rs`: `EditAdopted.inner` を
  `BranchEditOp` 化、`BranchEditOpIntent` + `edit_branch_op`
  追加（`edit_branch` は `ProjectOp`→`BranchEditOp::Project`
  変換の互換 wrapper として温存）、4 引数 `adopt_branch` を
  復元し adapter 対応の `adopt_branch_adapted` を追加。
  内部 adopt 経路で `DomainAdapter::preflight_adopt` を
  candidate 検証後・write plan/CAS 変更前に呼出。
- `src/variant/error.rs`: `VariantError::RecordingActive` 追加。
- `src/variant/mod.rs`: audio module 公開 + re-export。
- `src/variant/scope.rs`: TakeUsage scope の所有規則
  （take group + comp closure を所有、raw take/source は共有
  material のまま）の doc/comment 追従。
- `src/apply/entity.rs`: comping 4 kind
  （`TakeGroup`/`Comp`/`CompRegion`/`CompTransition`）の
  `entity_value`/`write_entity` 対応を追加。`supported_kind`
  （VCS apply surface）は据置とし、variant adopt 専用の
  `adopt_supported_kind` を新設 — VCS `apply_selected`/
  `target_fingerprint` は comp kind を引き続き fail-closed で
  拒否する（部分選択 apply が comp ドメイン不変条件を迂回
  しない境界を維持。`rcd_04_vcs_stable_id` の契約を保全）。
- `tests/cb02_audio_take.rs` 新設: CB-CLIP-01/02・CB-TAKE-01〜04・
  CB-AUDIO-RCP-01 の 10 fixture。

### Decisions During Implementation

- **`BranchEditOp` union で `ProjectOp` を不変に保つ**:
  `CompOp` は `ProjectOp` に追加できない（leased crate の網羅
  match を壊す）。代わりに additive union + `edit_branch_op` +
  `BranchEditOpIntent` を導入し、`edit_branch`・4 引数
  `adopt_branch`・`BranchEditIntent { op: ProjectOp }` は
  source 互換のまま残した。adapter 対応 adopt が必要な側だけ
  `adopt_branch_adapted` を呼ぶ。
- **VCS apply surface と adopt materialize surface の分離**:
  `supported_kind` をそのまま広げると `apply_selected` が comp
  entity を生 JSON transplant でき、region overlap・handle
  window・transition 整合といったドメイン検証を迂回する。
  `adopt_supported_kind` に分離し、adopt は domain-validated
  candidate のみを materialize する権威境界に固定した
  （詳細は exp E3）。
- **capability guard を adapter に置く**: `PlaybackMapping` の
  `Loop`/`SimpleStretch` は project model 上表現可能だが、
  player/comp lowering が `Natural` のみ受理し他は
  `UnsupportedMapping`/skip に落ちる。adapter が edit 時に
  typed `Unsupported` で拒否し、`capabilities()` の報告と
  同一関数で判定を共有 — 「表示 Capability と実処理
  Capability の一致」（CB-CLIP-02）を構造で保証。
- **録音中判定は adopt preflight**: 録音中 session の pass/
  stream/segment が対象 take group の take を含む場合、
  candidate 検証後・live 変更前に `RecordingActive` で拒否。
  live project・undo 履歴・branch content に部分変更なし。

## Related Experiments

- `docs/impl/exp/cb-02-audio-clip-adapter.md` — `BranchEditOp`
  union 採用理由・apply surface 分離・capability guard 配置・
  adopt preflight 境界の検討記録（adopted）。

## Validation

- `mise run fmt-check` — Pass。
- `mise run check synvelis-project` — Pass（`--locked
  --all-targets`）。
- `mise run lint synvelis-project` — Pass（clippy `-D warnings`）。
- `mise run test synvelis-project` — Pass（全 suite green。
  `cb01_variant.rs` 17 件・`cb02_audio_take.rs` 10 件・
  `rcd_04_vcs_stable_id.rs` 4 件を含む）。
- `mise run check synvelis-workspace` — Pass。
- `mise run check synvelis-render` — Pass（leased crate の
  source 互換を確認）。
- 詳細は `evidence/sv279/verification.md`・
  `evidence/sv279/acceptance-ledger.md`。

## Risks

- comp entity の VCS `apply_selected` 経路は依然 fail-closed。
  Semantic Diff → Apply で comp 変更を適用したい場合は、
  生 entity write ではなく `CompOp` replay 経路の設計が別途
  必要（後続スライス/Issue の範囲）。
- `Loop`/`SimpleStretch` mapping を lowering が受理するように
  なった時点で capability guard の前提が変わる — adapter の
  `playback_mapping_capability` を単一権威として更新すれば
  表示・強制の両方が追従する。
- 候補の音響比較は derived render 経由のみ。実 render/RT での
  波形比較は本スライス対象外（NotRun 記録済み）。

## Follow-ups

- 残ドメイン Adapter（CB-03〜08 = Issue #280〜285）、
  Compare Orchestrator（#285）、GPUI（#286）、Action/Gateway
  統合（#287）、Cross-Gate/実機受入（#288）。
- comp 変更の VCS apply 経路（`CompOp` replay 型）が必要に
  なった時点での設計。
- 実 OS/実 Plugin/実 RT/実 Render・実録音/Import pipeline での
  受入（`evidence/sv279/` の NotRun/Deferred を後続 Issue で
  Pass/NotRun 記録）。
