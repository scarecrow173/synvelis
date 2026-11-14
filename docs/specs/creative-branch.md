---
id: "SPEC-034Xe4RFguDtxkqBKi6834"
type: "spec"
status: "draft"
title: "Creative Branch — 領域横断 Fork/Edit/Compare/Adopt 製作候補の製品契約正本 (synvelis#277 CB-00)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/277"
    - "https://github.com/scarecrow173/synvelis/issues/1"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/creative-branch-design.md"
  depends-on:
    - "docs/specs/product-value-core.md"
    - "docs/specs/project-model.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/vcs-integration.md"
    - "docs/specs/execution-plan.md"
    - "docs/specs/daw-workflow.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/command-gateway.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/musical-time.md"
    - "docs/specs/media-render.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/creative-branch-design.md"
    - "docs/adr/cb-00-creative-branch.md"
    - "docs/designs/execution-plan-design.md"
    - "docs/designs/project-model-design.md"
    - "docs/designs/vcs-integration-design.md"
    - "docs/specs/svt-product-e2e.md"
    - "docs/specs/ai-native-baseline.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/277"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/289"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "https://github.com/scarecrow173/synvelis/issues/279"
    - "https://github.com/scarecrow173/synvelis/issues/280"
    - "https://github.com/scarecrow173/synvelis/issues/281"
    - "https://github.com/scarecrow173/synvelis/issues/282"
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "https://github.com/scarecrow173/synvelis/issues/284"
    - "https://github.com/scarecrow173/synvelis/issues/285"
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "https://github.com/scarecrow173/synvelis/issues/287"
    - "https://github.com/scarecrow173/synvelis/issues/288"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/562"
  defers: []
  deferred-by: []
---

# Creative Branch — 領域横断 Fork/Edit/Compare/Adopt 製作候補の製品契約正本 (synvelis#277 CB-00)

## What Should Be Built

Creative Branch は、**共通 Origin と明示 Scope を持ち、独立編集・
比較・採用できる制作候補**である。物理的な Audio Graph 分岐だけに
限定せず、Audio Clip の非破壊加工、Take 選択/Comp 候補、MIDI
Clip/Source の編集候補、Instrument/Preset/State、FX の
Processor 範囲、Track/Bus の宣言された構成領域、Arrangement の
Track 集合×Musical Time×変更 Domain を横断対象とする
（synvelis#277「合意した製品契約」2026-11-11）。

合意済み UX は **Fork → Edit → Compare → Audition → Adopt
（＋Retain/Discard）** の単一候補採用モデルであり、Semantic
Merge・自動 Blend は初期対象外である。本 spec はこの契約を
製品要件として固定し、型・状態遷移・失敗型への具体化は
`docs/designs/creative-branch-design.md`、方式決定の根拠は
`docs/adr/cb-00-creative-branch.md` が正本となる。

## Why Now

`docs/specs/product-value-core.md` §10 が Creative Branch を
「領域横断で Fork → Edit → Compare → Adopt を提供する探索・
比較・採用体験の新しい柱」と位置付け、#277〜#288 の下流
Issue 分解を既に前提としている。制作中の「別案を試して聞き比べ、
良い方だけを正式 Project へ反映する」探索は、現行の単一 Live
Project + Undo モデルだけでは表現できず、Take/Comp（#12）や
VCS Semantic Diff（#7）が部分的に担ってきた領域を、Clip/MIDI/
Instrument/FX/Track-Bus/Arrangement へ拡張する統合契約が必要に
なった。設計方針は Issue #277 本文で 2026-11-11 に合意済みだが、
実装・実 GUI・実 Plugin 受入は未実施であり、本 spec・設計文書・
ADR の作成は Gate PASS を意味しない。

## Users and Value

- **DAW ユーザー**: ある Clip/Take/楽器/FX 範囲/トラック構成/
  アレンジの別案を破壊せずに試し、Semantic/Signal/Perceptual の
  3 層証拠で比較し、採用候補だけを正式状態へ反映できる。
  非採用候補は Retain され、後で再比較できる。
- **AI/外部 consumer（#287 Action/CLI/AI Gateway）**: 同一の
  Fork/Edit/Compare/Adopt 契約を型付き Action 経由で利用できる。
  Creative Branch は AI・外部 VCS・Git Repository を必須条件に
  しない。
- **Synvelis/Ruviform の両 Consumer**: Ruviform 側は DAW 語彙を
  持たない Generic capability（Probe 整列・比較 Context・Render
  Recipe）を提供し、Synvelis が製品意味論を所有する
  （Owner 分離は CB-R-08）。

## Scope

### In Scope

- Creative Branch の canonical 概念型: `VariantSet`・`Branch`・
  `VariantScope`・`ComparisonSession`・`SnapshotIdentity`・
  `BranchContentRef`（Issue #277 概念型節の契約化）。
- 共通操作: Fork・Edit・Compare・Audition・Adopt・Retain・
  Discard。Fork/Discard/Adopt は Versioned Project 操作として
  Undo/Save/Reload に対応する。
- 7 対象 Domain の Scope→shared/owned→Fork/Edit/Compare/Adopt/
  Discard 契約（Audio Clip・Take/Comp・MIDI Clip/Source・
  Instrument・FX Processing Range・Track/Bus・Arrangement）。
- Processing 分岐の境界契約: Stable Semantic Processor/Port 境界、
  初期範囲（Track Strip FX Chain 入口〜末尾）、外部
  Send/Sidechain/多出力の明示検証または Reject/Unsupported。
- Compare 3 層（Semantic/Signal/Perceptual）・固定 Evaluation
  Context・時間整合/失効状態・Audio Diff と Perceptual 評価の
  分離表示契約。
- Adopt = 単一 ProjectController CAS/Composite + 1 Undo、
  Preflight fingerprint 照合、Stale/Conflict 時の拒否、
  Project CAS 成立と RT Plan Publish/Activation の別 Receipt・
  Last-good/Pending/Retry 失敗行列。
- Branch-local ID/Lineage/Scope 重複/Scope 跨ぎ外部参照/Legacy
  identity 不明に対する Reject Oracle（CB-RCP-01 を先頭とする
  追跡可能な Case ID 体系）。
- Owner 分離（Synvelis/Ruviform）と Issue 分解 DAG 規則
  （CB-01 先行正本・ドメイン Adapter 並行開始・Start/
  Integration/Acceptance 区別・#288 Cross-Gate 合流）。
- 通常 DAW Gate A・VCS・AI OFF 条件下での動作、AudioLens
  Consumer 非回帰の下流検証 Issue への明記。

### Out of Scope

- Semantic Merge/3-way Merge、Cherry-pick、時間範囲の部分
  Merge、Blend/Mix を「Adopt」と呼ぶこと（別機能として将来
  検討。本契約では Adopt = 単一候補の宣言 Scope 反映のみ）。
- 全 Branch の常時 RT 駆動、非採用候補の自動削除、
  Master への複数候補の暗黙加算。
- 独自 Project Writer、独自 DSP Compiler、Ruviform への
  DAW Track/Take/Branch Domain の移転、VCS Provider への
  暗黙 Write。
- Creative Branch を理由とした既存 Project/Routing/Plugin/
  Observability 実装の無断統合、旧 WebView ADR・全 Consumer
  受入の上書き。
- 本 spec/設計/ADR の作成だけで実装・OS/実 Plugin/PCM 受入が
  終わったとすること。

## Requirements

Issue #277 本文の合意項目を製品契約として CB-R-xx に固定する。
型名・状態名は設計段階の概念名であり、正式な実装 crate/型名は
CB-01（#278）以降で既存 API と PoC を見て決定する。

### Canonical Model（CB-R-01）

- **CB-R-01-1**: Creative Branch は `VariantSet`（`id`・
  `scope`・`origin: SnapshotIdentity`・`branches`・`adopted`・
  `revision`）を単位とし、各 `Branch` は `id`・`parent`・
  `created_from: SnapshotIdentity`・`content: BranchContentRef`・
  `revision` を持つ。`VariantScope` は AudioClip /
  TakeUsage / MidiClip / InstrumentSlot / ProcessingRange /
  StripFields / Arrangement の typed variant であり、
  `ComparisonSession` は `left`/`right` の
  `BranchRevisionRef`・`pinned_context: EvaluationContext`・
  `alignment`・`monitor` を固定して保持する。
- **CB-R-01-2**: `SnapshotIdentity` は ProjectId + 固定 locator +
  bytes digest + schema/projection を束ね、**Revision 単独では
  同一性を断定しない**。既存 `snapshot::SnapshotIdentity`
  （`crates/synvelis-project/src/snapshot/types.rs`）がこの
  契約の実装正本であり、VariantSet/Branch の `origin`/
  `created_from` はそれを再利用する。
- **CB-R-01-3**: VariantSet/Branch 状態の真値は Project の
  versioned persistent domain に保持し、History/VCS 固定
  Snapshot は read-only resolver を利用する。Plugin State が
  opaque なら全体 Snapshot/Capability のまま保持し、数値
  Parameter への変換を捏造しない。
- **CB-R-01-4**: 比較用 Render/PCM/Probe/Workspace 配置は
  非正本 Derived/Session であり、Project content revision・
  Source/Plugin/Tempo/Time mapping・Engine/Graph/Clock 等の
  比較条件を固定し、変化後は Stale を明示する。

### Operations and States（CB-R-02）

- **CB-R-02-1**: 共通操作は Fork・Edit・Compare・Audition・
  Adopt・Retain・Discard。**Merge Changes/部分的
  Cherry-pick/時間範囲の部分 Merge/Blend/Mix は別機能**であり
  Adopt と呼ばない。
- **CB-R-02-2**: **Editing・Comparing・Auditioning・Adopted・
  Runtime Active は別状態**である。試聴選択で正式
  Project/Recording/Automation Write を変更しない。
- **CB-R-02-3**: Fork は対象 Scope の Origin と固定候補
  Revision を生成し、子候補の独立編集を可能にする。
  Fork/Discard/Adopt は Versioned Project 操作として
  Undo/Save/Reload に対応する。
- **CB-R-02-4**: 採用候補（Adopted に向かう編集中候補）の通常
  編集は、正式 Live 対象とその候補の内容を**同一 Controller
  操作で同期**し、二重正本を持たない。非採用候補の編集は
  永続候補のみを変更し、正式 Live 対象を暗黙変更しない。
- **CB-R-02-5**: 唯一の Writer は ProjectController である。
  非採用候補の編集も Controller 経由で永続候補だけを変更する
  （既存 `ProjectController` の `Command{expected_revision}`
  CAS・validate→scratch apply→atomic swap・1 undo unit 契約を
  拡張して用いる）。

### Domain Scope and Adopt（CB-R-03）

- **CB-R-03-1**: 7 対象それぞれについて、Fork 時に共有される
  もの・独立化されるもの・Adopt が置換する範囲が型・Owner・
  状態遷移・失敗型へ落とされていること（Issue 本文の
  Scope/Adopt 表を正本とし、具体化は design 文書が担う）。
  対象は Audio Clip・Take/Comp・MIDI Clip/Source・
  Instrument・FX Processing Range・Track/Bus・Arrangement。
- **CB-R-03-2**: Source/Raw Take は非破壊であり、Fork/Adopt/
  Compare/Audio Diff は raw を書き込まない。Recording Raw/
  Session/Source/Take Capture/Journal/Recovery の所有は
  #12 系契約が維持する。
- **CB-R-03-3**: Arrangement の Adopt は複数 Clip 等を
  **1 Composite CAS/Undo** で採用する。部分 Merge は行わない。
- **CB-R-03-4**: Overlap Scope の規則: disjoint は共存可能、
  依存関係は明示、親子包含は scope ownership を検証、同一
  フィールド重複所有の無断 Adopt は禁止。Origin の単純番号
  一致で同一性を断定しない。

### Processing Boundary（CB-R-04）

- **CB-R-04-1**: 初期の Processing 分岐は Track Strip FX Chain
  入口・Processor 間→Chain 末尾を分岐可能とし、Start/End は
  **Stable Semantic Processor/Port 境界**で識別する。ordinal
  のみで永続化しない。任意連続範囲/Instrument Audio
  Out/Bus/Return/Master はドメイン拡張として表現可能に
  しておく。
- **CB-R-04-2**: Branch は代替経路であり、Master へ複数候補を
  暗黙加算しない。Branch 内 Processor/Plugin State は独立、
  Upstream と Downstream は共有する。
- **CB-R-04-3**: 初期は分岐範囲から外への Send/Sidechain/
  多出力を明示検証し、未サポートなら Reject/Unsupported と
  する（無断配線しない）。連続 FX 範囲は拡大しない。
- **CB-R-04-4**: Graph Spec/Node ID/Port ID/Plan Generation は
  Ruviform Runtime 識別子であり、Project semantic ID へ昇格
  しない。Configured/Prepared/Published/Active/Observed を
  区別する（既存 SF-03 契約と整合）。

### Compare（CB-R-05）

- **CB-R-05-1**: Compare は **Semantic Diff**（意味的変更）・
  **Signal Diff**（同条件で実測/Render した出力差）・
  **Perceptual 評価**（試聴・Blind/Level Match）を別データと
  し、混同しない。Audio の delta は時間・Gain・Phase・
  Plugin 非決定性を証明できる範囲のみとする。
- **CB-R-05-2**: ComparisonSession は固定 Evaluation
  Context（`pinned_context`・`alignment`・`monitor`）を持ち、
  対象以外の MIDI/Instrument/FX/Tempo/Asset/Plugin/時刻条件を
  固定する。他候補との直積評価は明示依頼時のみ。
- **CB-R-05-3**: 証拠状態は **Configured/Measured/Inferred/
  Unknown** と **Aligned/Approximate/Unaligned/Unavailable**
  を区別し、Recording/Raw Source・Lossy probe cache と高精度
  offline/render を区別する。変化後は Stale を明示する。

### Adopt / Runtime（CB-R-06）

- **CB-R-06-1**: Adopt は候補 B の宣言 Scope を Live 正式状態へ
  反映する**単一 ProjectController CAS/Composite + 1
  Undo**である。Preflight で expected ProjectRevision・
  BranchRevision・対象 Scope fingerprint・安定参照/所有者/
  Asset/Plugin/Control/Automation を照合する。
- **CB-R-06-2**: Stale/Conflict 時は自動 Rebase/Merge せず
  拒否し、再比較を促す。採用前の非採用候補は自動削除せず
  Retain 可能。Raw Source/VCS checkout/Index/外部 Asset を
  破壊しない。
- **CB-R-06-3**: Project CAS 成立と RT Plan Publish/Activation
  は**非原子的な別段階**であり、別 Receipt を持つ。Commit 後の
  Runtime 失敗は Last-good を維持し、Pending/Blocked/Retry を
  表示し、勝手に Project を巻き戻さない。Live の正式状態を
  変える Adopt と一時 Monitor 選択を混同しない。
- **CB-R-06-4**: Missing Plugin でも opaque State と Semantic
  Port/Automation 参照が保全できるなら編集/Adopt 可。Exact
  audition が不可能なら Stale/Pending/Unavailable を表示する
  （CB-RCP-01 の音響依存契約）。

### Identity / Lineage / Oracle（CB-R-07）

- **CB-R-07-1**: Branch-local 生成物と Live の Typed Stable ID
  を分離し、Adopt 時に決定的な ID mapping/参照再結線を検証
  する。既存 ID を無条件再利用しない。旧 Schema 由来が不明
  なら Ambiguous/Unknown で止める。
- **CB-R-07-2**: Legacy v5 migration を別実装せず、#17/#86/#92
  の versioned reader（`crate::migration` の順序付き registry）
  を再利用する。
- **CB-R-07-3**: Track 録音/Automation Write 中の対象変更、
  Plugin State 取得不能、Tempo/Clock 差、旧 Schema projection
  不明、Scope 外 Routing への外部依存などは明示的に
  Reject/Degraded/Unsupported とする。
- **CB-R-07-4**: CB-RCP-01 を先頭とする追跡可能な Case ID
  体系と、二重正本禁止・無断誤参照/無断変更を Reject する
  Oracle を定義する（具体的 Case ID・失敗行列は design 文書）。

### Owner / DAG / Gates（CB-R-08）

- **CB-R-08-1**: Synvelis は Branch 意味論・Typed Scope・
  ProjectController mutation・Lineage/Adopt・MIDI/Clip/Take/
  Arrangement・Compare orchestration・UX・Gateway Action・
  GUI scenario を所有する。Ruviform は DAW 語彙を持たない
  Generic Time/Typed Graph/Plan activation/Plugin Host/Media
  Render/Probe/Analysis/Driver-neutral GUI を所有し、追加
  Generic capability は必要な Gap を再現した場合のみ新 Issue
  で定義し、既存 #308/#309 等を再実装しない。
- **CB-R-08-2**: CB-01（#278）型/Schema を先行正本とし、
  Clip/Take・MIDI・Instrument/FX・Track/Bus・Arrangement の
  ドメイン Adapter（#279〜#284）は CB-01 の最低限 Typed
  Contract で並行開始可能。各ドメインの既存実装 Issue 全
  Close を開始 Blocker にしない。Compare semantics/分析契約・
  Difference Explorer UI/Action/CLI fixture（#285〜#287）は
  モック/Fake から並行可能。実信号/Plugin/OS 受入のみ関連
  Owner の Integration Gate で合流し、#288 Cross-Gate が
  最終検証を担う。
- **CB-R-08-3**: **Start / Integration / Acceptance を区別**
  し、最終検証では Project Snapshot/Undo/Save・RT Graph
  active・PCM/Audio Diff・実 OS GPUI・実 Plugin・
  Ruviform×AudioLens Consumer を別々に記録する。正式な
  実装 crate/型名・数値 SLO・Capability 不足時の処置は
  既存 API と PoC で決定し、設計合意を実測 PASS にしない。
- **CB-R-08-4**: 通常 DAW Gate A の実装完了待ちにせず、AI・
  外部 VCS・Git Repository を必須条件にしない。AudioLens の
  f64/長尺/Native 精度と Synvelis RT f32/GUI は独立 Gate で
  確認し、既存 Daily Full/手動実 OS を利用し per-PR full CI
  を追加しない。これらの非回帰条件は各下流検証 Issue に
  明記する。
- **CB-R-08-5**: 旧 WebView/SolidJS を新たな正式 UI 技術契約
  にしない。Ruviform Driver-neutral UI + 標準 GPUI と
  Synvelis Native App を使用する。

## Acceptance Criteria

Issue #277 から転記。本 spec の受入は「製品契約としての合意・
文書化」であり、実装・実 OS/実 Plugin/PCM 受入ではない。

- [ ] CB-AC-01（CB-RCP-01）: Fork→Edit→Adopt→Undo/Reload の
  Branch/Artifact 参照が音響依存で決まり、Missing Plugin で
  Edit 可能/Exact 不可を区別し、Project CAS/RT Activation
  別 Receipt を維持する。
- [ ] CB-AC-02: 上記 7 対象の Scope→shared/owned→Fork/Edit/
  Compare/Adopt/Discard 表を、具体的な型・Owner・状態遷移・
  失敗型に落とした ADR/設計文書をレビュー可能な状態で作成
  する。
- [ ] CB-AC-03: 採用候補の編集・非採用候補の編集・Undo/Redo・
  Save/Reload について二重正本が生じない状態遷移表と
  Fixture を定義する。
- [ ] CB-AC-04: Branch-local ID/Lineage/Scope 重複/Scope 跨ぎ
  外部参照/Legacy identity 不明の例を作り、無断誤参照や
  無断変更を Reject する Oracle を定義する。
- [ ] CB-AC-05: Compare 3 層、固定 Evaluation Context、時間
  整合・失効状態、Audio Diff と Perceptual 評価を混同しない
  表示契約を定義する。
- [ ] CB-AC-06: Adopt 単一 CAS/1 Undo と Runtime Publish/Active
  の別 Receipt・Last-good/Pending の失敗行列を定義する。
- [ ] CB-AC-07: 既存 #10/#12/#17/#7/#104/#105/#106/#121/#122/
  #128/#114、Ruviform #308/#309/#294 の Owner 重複・DAG 循環
  なしを確認し、関連作業 Issue へ参照を張る。
- [ ] CB-AC-08: 通常 DAW Gate A/VCS/AI OFF、AudioLens Consumer
  非回帰を各下流検証 Issue に明記する。

## Deferred Design Concerns

- VariantSet/Branch/VariantScope/ComparisonSession の正式な
  Rust 型・schema version・migration ladder — CB-01（#278）
  が正本（再評価契機: CB-01 の schema 設計着手）。
- 各ドメイン Adapter の具体的 op 集合・typed error・fixture —
  CB-02〜CB-07（#279〜#284）が正本。
- Compare orchestration の cache key・budget・probe/render
  pipeline — CB-08（#285）が正本。Ruviform 側 Generic
  capability の Gap は ruviform#562 PoC で判定。
- Difference Explorer UI・Action/CLI/AI Gateway の操作面 —
  CB-09（#286）・CB-10（#287）が正本。
- 実信号/実 Plugin/実 OS/実機 PCM 受入 — CB-11（#288）
  Cross-Gate と #114/#176 系の独立 Gate が正本。
- Semantic Merge/Cherry-pick/部分 Merge/Blend/Mix — 初期対象外。
  Adopt 契約が安定した後の別 Issue で再評価。

## Sources and Relations

- Source: [synvelis#277](https://github.com/scarecrow173/synvelis/issues/277)
  （CB-00 設計合意の正本。2026-11-11 付け合意済み製品契約・
  概念型・ドメイン別 Scope/Adopt 表・Processing 分岐境界・
  Edit/Fork/Compare/Adopt 状態競合契約・Evaluation/Resource
  契約・Owner/独立性・Issue 分解 DAG 規則・RCP/2026-11-11 合同
  契約を転記・契約化した）。上位の製品価値根拠は
  [synvelis#1](https://github.com/scarecrow173/synvelis/issues/1)
  と `docs/specs/product-value-core.md` §10。
- depends-on: Project Model（#10 系 `project-model.md` の
  PM-R-08〜R-13: VariantScope/BranchContentRef・stable ID・
  単一 Controller CAS・live revision 単調性・Save/Reload
  契約）、Persistence（#17 系 `project-persistence.md` の
  Immutable Snapshot/Safe Save/Recovery・versioned migration）、
  Recording/Take/Comp（#12 系 `recording-take-comping.md` の
  Raw Source 非破壊・TakeGroup/Comp 参照非所有契約）、
  VCS（#7 系 `vcs-integration.md` の read-only resolver・
  Semantic Diff・Apply/Restore CAS と Creative Branch の
  責務分離）、Execution Plan（#23 系 `execution-plan.md` の
  Start/Integration/Acceptance・OS 検証レベル・判定語彙）、
  SF-03（#106 系 `sf-03-map-semantic-runtime-contract.md` の
  Configured/Prepared/Published/Active/Observed・last-good
  世代契約）、Playback Graph（#18 系 `playback-graph.md` の
  Plan Activation/Last-good）、TIM-04（#128 系
  `tim-04-plugin-state-snapshot-consistency.md` の Plugin
  State 品質 Receipt）、MD-A TakeGroup/Comp（
  `mda-takegroup-comp-effective-projection-placement-contract.md`）、
  VCS-01/02（`vcs-01-immutable-snapshot-resolver.md` の
  SnapshotIdentity/`vcs-02-semantic-diff-changeset.md` の
  ChangeSet/Ambiguity）、Musical Time（#19 系
  `musical-time.md` の PPQN/Tick/TempoMap）、Media/Render
  （#21 系 `media-render.md` の Offline Render/Job）、
  Command Gateway（#45 系 `command-gateway.md` の型付き
  Action 契約）、DAW Workflow（#8 系 `daw-workflow.md` の
  Gate A 代表 Task）。
- related: `docs/designs/creative-branch-design.md`（統合設計
  正本）、`docs/adr/cb-00-creative-branch.md`（方式決定）、
  `docs/designs/execution-plan-design.md`（SV-CB DAG node/
  edge の写像先）、`docs/designs/project-model-design.md`
  §Branch/Scope/Lineage、`docs/designs/vcs-integration-design.md`
  §Creative Branch 責務分離、`docs/specs/svt-product-e2e.md`
  （SVT headless scenario への将来接続）、
  `docs/specs/ai-native-baseline.md`（AI OFF/Grant 境界）。
- references: 下流実装 Issue #278（CB-01）〜#288（CB-11）、
  関連既存 Issue #10/#12/#17/#7/#104/#105/#106/#121/#122/
  #128/#114/#175/#176/#23、Ruviform #308/#309/#294/#562、
  RCP 系 #289〜#297（Render Checkpoint との Branch/Artifact
  共有契約は #294 が接続）。
