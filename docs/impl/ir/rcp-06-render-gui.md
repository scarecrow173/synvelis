---
id: "IMPL-rcp06-render-gui"
type: "impl"
status: "completed"
title: "RCP-06 Missing Plugin/Render Availability・Policy UI・Action 契約 — Headless/Driver-neutral 投影スライス"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/295"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/render_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/rcp06_render_ui.rs"
      - type: "file"
        path: "crates/synvelis-app/src/render.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/rcp06_render.rs"
      - type: "file"
        path: "docs/impl/ir/rcp-06-render-gui.md"
      - type: "file"
        path: "docs/impl/exp/rcp-06-render-gui.md"
      - type: "file"
        path: "docs/test-specs/rcp-06-render-gui.md"
      - type: "file"
        path: "evidence/sv295/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv295/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-theme/src/status.rs"
      - type: "file"
        path: "Cargo.lock"
    supersedes: []
    superseded_by: []
  verifies:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCP-06 render GUI headless implementation record

## Summary

synvelis#295 RCP-06 の **headless-first 投影スライス**。
Missing Plugin 環境で「semantic edit 可能」と「正確な再生/実行可否」
を独立した軸として分離し、Branch Difference Explorer / Signal Flow
系 overlay / Render Policy / Render Request / Job monitor の
presentation model と driver-neutral `ViewSpec` + binding decode を
実装する。authority は RCP-01..RCP-04（recipe/lookup、
`ExecutionSelector`、render policy/job orchestrator）の既存契約を
消費するのみで、第二正本・再判定・独自 Gateway は作らない。

## 実装内容

### `crates/synvelis-workspace/src/render_ui.rs`（新規）

presentation model の正本。5 軸を独立 enum として投影:

- `Editability`（`edit-possible`/`edit-blocked`）— semantic 編集可否。
  `AutomationIdentityIncomplete` のみが編集を block し、Missing/
  Denied/Drift plugin は編集可能のまま（project が canonical、
  RCP-00 invariant）。
- `Executability`（`live`/`blocked`）— live 実行可否。
  `PluginVerdict`/`UnresolvedNode`/`ArtifactEligibility` から
  最も具体的な [`RenderReason`] を fail-closed で選択。
- `RenderValidity`（`render-valid`/`render-preview`/`render-stale`/
  `render-missing`/`render-corrupt`/`render-unverified`/
  `render-unsupported`/`render-denied`/`render-unknown`）—
  `ArtifactEligibility`/`LookupOutcome` の display 投影。
  `EligiblePreview` は `Preview` に落ち、Exact を偽らない。
- `PlaybackQuality`（`quality-exact-artifact`/`quality-verified-
  tolerance`/`quality-preview`/`quality-unavailable`）—
  eligibility + `ReproducibilityClass` + preview_class から投影。
- `RuntimeState`（`runtime-configured`/`runtime-prepared`/
  `runtime-published`/`runtime-active`/`runtime-last-good-mismatch`）。

その他の投影:

- `RenderReason` — `capability.*`/`state.*`/`source.*`/`boundary.*`/
  `render.*`/`policy.*`/`job.*` の category prefix 付き安定 code。
  GUI label・headless 出力・Workflow/Script receipt が同一 code を
  共有する契約（RCP-UX-02）。
- `project_render_node` — `NodeRenderEvidence`（verdict/
  eligibility/reproducibility/substitution/unresolved/live_region/
  last_good）→ `RenderNodePresentation`。Substitution overlay
  （cutset artifact + preview_class）は node 実体を隠さず重畳表示。
- `project_selection` — `ExecutionSelection` → verdict label +
  live/subst/skipped/unresolved/degraded count + snapshot pin
  （`SelectionPins`）。`non-equivalent-preview` は `exact` と
  別 label。
- `SnapshotPinView::pin_text` — `locator @ sha256:<12hex>` の
  UI/CLI 共通 pin 表記。
- `project_branch_diff` — `BranchDiffInput`（両側 pin/demands/
  resolutions + changed_parameters/missing_plugins/semantic_diffs/
  preview_markers/warnings）→ `BranchDiffView`。境界ごとに
  resolution kind・recipe key prefix・coverage・reason を表示し、
  音源欠落時も semantic diff を表示。stale waveform は出さない。
- `project_render_policy` — `EffectivePolicy` + `CommitCapability`
  → trigger 5 行（enabled/provider availability/blocked reason を
  分離）・scope/completion/quality/budget/retention・field ごとの
  adopted-layer provenance・`recipe_invariant`（trigger-only 変更で
  RecipeKey 不変、RCP-POL-02）。
- `project_render_request` / `render_request_candidates` —
  boundary candidate・coverage・expected bytes・missing plugins・
  owner-machine・invalidation・action candidates。
  `ACTION_RENDER_{PLAN,INSPECT,REQUEST,CANCEL,RETRY,VERIFY,ATTACH,
  PUBLISH,REMOTE}` の共有 action id 語彙。Publish/Remote は
  `requires_grant`、非 executable candidate は `disabled_reason`
  に code を保持。
- `project_render_job` / `render_job_candidates` / `job_control_allowed` —
  `JobRecord` → state label・attempt・pending/failure（typed
  reason + retryable）・completeness gaps・receipt tail・
  entry surface・key summary。Cancel/Retry/Attach は job key で
  冪等、state gate は orchestrator semantics と一致。
- `render_request_report` — surface request → `TriggerReport` の
  **唯一の lowering**。Demand event と Publish/Remote action は
  grant 必須で fail-closed（`RenderUiError::GrantRequired`）。
  `entry` は provenance のみで job key に入らない（RCP-POL-07）。
- `render_job_command` / `JobControlOp` — cancel/retry/attach の
  typed op + job key 運搬（冪等性は key が保証）。
- `RenderViewState` + `RenderViewAction` + `apply_render_view_action`
  + `render_policy_draft_patch` — view-local の選択状態と policy
  draft intent。draft は `PolicyPatch`（operation-intent 層）にのみ
  lowering され、authority を直接変更しない。Manual trigger は
  無条件 enabled で toggle off 不可。
- `RenderCommand` — `ViewLocal` / `Request` / `JobControl` の
  decode 済み command 型。

### `crates/synvelis-workspace/src/state.rs`

`ViewState` に `render: Option<RenderViewState>` を追加
（tolerant deserialize、view-local で project/render/job/policy
権限を持たないことを doc comment で明記）。

### `crates/synvelis-app/src/render.rs`（新規）

AUTX-07 パターンの driver-neutral `ViewSpec` 層:

- `RCP_RENDER_TARGET` / `RCP_BRANCH_TARGET` / `RCP_REQUEST_TARGET`
  の論理 target。
- `render_status_screen_spec` — 5 軸 legend（色非依存の意味付けを
  label として明示）・nodes table（read-only）・selection verdict +
  pin generations・policy summary/provenance/draft 注記・jobs table
  （選択可）+ 選択 job の executable candidate のみ button 化
  （fail-closed by spec）+ disabled reason text。
- `branch_diff_screen_spec` — pin 表示・warnings・boundary table
  （選択可）・missing plugins・semantic diff table・preview markers。
- `render_request_screen_spec` — pin/purpose/coverage/expected/
  owner-machine/invalidation・boundary candidates table（選択可）・
  executable candidate のみ button・disabled reasons text。
- `decode_render_command` — `(target, action, params)` →
  `RenderCommand` の単一 decode 経路。未知 target/action・malformed
  params は `None`（fail-closed）。table `select`/`{"row":n}`、
  `job-control`/`{"op":...}`、`toggle-trigger`/`set-completion`/
  `set-scope`/`clear-draft`/`request` を decode。
- `apply_render_view_command` — `ViewLocal` のみ
  `RenderViewState` に適用。domain request/job control は authority
  port へ route するためここでは no-op。
- `mount_{render_status,branch_diff,render_request}_view` —
  attach-epoch 契約は `mount_action_palette_view` と同一。

GPUI/OS/DOM 型は一切含まず、`synvelis-render`/`synvelis-gateway`
型も名指ししない（authority 語彙は display 文字列として到達）。

### `crates/synvelis-theme/src/status.rs`

`StatusTone::of_label` に RCP-06 の 5 軸 + job state label の
mapping を追加。色は副次 cue であり、意味は label/reason code が
担う（RCP-UX-01）。未知 label は従来どおり `Unknown` に fail-closed。

## 検証

`evidence/sv295/verification.md` と `docs/test-specs/
rcp-06-render-gui.md` を参照。headless/Fake-driver の PASS は
実 GPUI/実 OS Gate（RCP-UX-06）へ昇格しない。

## 対象外（本スライスで実施しない）

- 実 GPUI/実 OS/IME/DPI/2Window/pixel/RT lane（RCP-UX-06 — Case ID
  付き NotRun）。
- 実 Plugin・実 VCS provider・PCM Golden・AudioLens Consumer。
- CLI/Workflow/Luau/Rhai/Fake AI の実 surface（sv#273 他 lease —
  本スライスは共有 action id 語彙と `TriggerReport` lowering の
  contract test のみ）。
- cross-gate RCP-07、adopt（RCP-05 scope）。
