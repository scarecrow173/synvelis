---
id: "EXP-rcp06-render-gui"
type: "impl"
status: "completed"
title: "RCP-06 render GUI — 実装中の判断と却下した代替案"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/295"
  verifies:
    - "docs/impl/ir/rcp-06-render-gui.md"
  supersedes: []
  superseded_by: []
metadata:
  experiments:
    adopted:
      - "AUTX-07 と同型の workspace read-model → app ViewSpec 2 段投影"
      - "5 軸を独立 enum + label() で投影し色非依存の意味付けを保証"
      - "RenderReason を category prefix 付き安定 code として単一語彙化"
      - "executable candidate のみ button 化する fail-closed spec"
    rejected:
      - "GPUI 直接結合の View 実装 — driver-neutral 契約に反し他 driver を阻害"
      - "LookupOutcome/PluginVerdict の display 側再判定 — authority の二重実装になる"
      - "nodes table の選択式化 — view-local state に node 選択権限がなく YAGNI"
      - "JobRecord を app 層で構築する fixture — authority 型を app が名指しできない境界に反する"
---

# RCP-06 render GUI experiment log

## 採用した設計

### 2 段投影（workspace read-model → app ViewSpec）

AUTX-07（`automation_ui.rs` + `automation.rs`）と同じ構造を採用:
`synvelis-workspace::render_ui` が authority 型
（`PluginVerdict`/`ArtifactEligibility`/`LookupOutcome`/
`ExecutionSelection`/`EffectivePolicy`/`JobRecord`/`TriggerReport`）
を display read-model に投影し、`synvelis-app::render` がそれを
`ruviform_ui::view` spec に落とす。app 層は `synvelis-render`/
`synvelis-gateway` に依存せず、authority 語彙は display 文字列
（label/reason code/pin text）としてのみ到達する。これにより
CLI/Workflow/Script が同じ read-model/code を消費でき、GUI 側に
第二正本が生まれない。

### 5 軸の独立 enum

`Editability`/`Executability`/`RenderValidity`/`PlaybackQuality`/
`RuntimeState` を独立 enum とし、それぞれ `label()` で安定文字列
（`edit-possible`/`render-stale`/`quality-preview`/…）を返す。
Missing Plugin で `edit-possible` + `blocked` + `render-missing` +
`quality-unavailable` が同時に成立し、「編集できるが正確には
再生できない」を色なしで表現できる（RCP-UX-01）。`StatusTone::
of_label` はこの label 語彙を tone に写すだけで、意味は label が
担う。

### RenderReason 単一語彙

`capability.*`/`state.*`/`source.*`/`boundary.*`/`render.*`/
`policy.*`/`job.*` の category prefix 付き code を
`PluginVerdict`/`UnresolvedReason`/`LookupOutcome`/
`UnknownCause`/`UnsupportedReason`/`JobFailureReason`/
`CompletenessGapKind`/`CommitCapability` から一意に写像する
`reason_of_*` 群を projection 内に集約。UI table cell・disabled
reason・CLI 出力が同一 `code()` を表示する契約（RCP-UX-02）。

### fail-closed spec

`render_request_candidates`/`render_job_candidates` が executable/
disabled_reason を計算し、spec builder は executable candidate の
み button を生成する。disabled reason は text として残るため
「なぜ押せないか」が消えない。decode 側も未知 target/action・
malformed params を `None` で reject し、view-local command は
`RenderViewState` のみに適用される。

### TriggerReport の単一 lowering

`render_request_report` が `TriggerEvent` + `EntrySurface` +
`GatewayActionRef` + `Option<GrantRef>` → `TriggerReport` のみを
行い、Demand event と Publish/Remote action の grant 必須をここで
fail-closed 検査する。`entry` は receipt provenance で job key に
含まれないため、GUI/CLI/Workflow/Script が同一 request で同一
`RenderJobKey` に coalesce する（RCP-POL-07/RCP-UX-04）。

## 却下した代替案

- **GPUI 直接 View 実装**: `ruviform_ui::view` の driver-neutral
  契約と fake driver fixture パターン（AUTX-07）に反し、非 GPUI
  driver へ DAW 固有実装を強制することになる（Issue 対象外条件）。
- **display 側での eligibility/verdict 再判定**: `ArtifactEligibility`
  `Unknown`/`Unverified` を valid 扱いする等の再解釈は authority の
  二重実装。projection は写像のみ行い判定しない。
- **nodes table の行選択**: `RenderViewState` に node 選択権限が
  なく、現行 surface 要件でも未使用。YAGNI で read-only table。
- **app 層での JobRecord fixture 構築**: `JobRecord` は
  `synvelis-render` 型を多数含み、app が名指しできない境界に反する。
  job 系の検証は workspace 側 fixture で行う。

## 発見した問題と修正

- `render_validity_of_lookup` が `EligiblePreview` を `Valid` に
  写像していた（`render_validity_of(ArtifactEligibility)` は
  `Preview`）。lookup 経路でも preview-class を valid と偽らない
  よう `Preview` に修正し、両経路の語彙を一致させた。
- `ToggleTrigger` の draft 初期化が Manual を除外しており、
  patch 化で Manual が失われる不具合を修正（draft は enabled 集合
  として全 trigger を含み、Manual の off は guard で拒否）。

## 残課題（本スライス外）

- 実 GPUI/実 OS Gate（RCP-UX-06）— 専用 real GUI 自動レーンが
  現行にないため NotRun/Deferred。
- CLI/Workflow/Luau/Rhai/Fake AI の実 surface — sv#273 他 lease。
  本スライスは共有 action id・`TriggerReport` lowering・reason
  code の contract test まで。
- cross-gate RCP-07。
