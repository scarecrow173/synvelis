---
id: "SPEC-g52wQZrVCdTt4O37271f7f"
type: "spec"
status: "approved"
title: "Render Checkpoint — Render Recipe・依存閉包・有効性・Branch/Execution/Policy 統合仕様正本 (synvelis#289)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/289"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/rcp-render-recipe-design.md"
  depends-on:
    - "docs/specs/media-render.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/project-model.md"
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/vcs-integration.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/command-gateway.md"
    - "docs/specs/musical-time.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "evidence/sv289/render-checkpoint-contract-trace.md"
  refines:
    - "docs/specs/media-render.md"
    - "docs/specs/project-persistence.md"
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/289"
  derived-by:
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
  verifies: []
  verified-by: []
  references:
    - "evidence/sv289/render-checkpoint-contract-trace.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/290"
    - "https://github.com/scarecrow173/synvelis/issues/291"
    - "https://github.com/scarecrow173/synvelis/issues/292"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/295"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/synvelis/issues/297"
  deferred-by: []
---

# Render Checkpoint — Render Recipe・依存閉包・有効性・Branch/Execution/Policy 統合仕様正本 (synvelis#289)

## What Should Be Built

Issue #289 RCP-00: Creative Branch/Adopt、Plugin Missing、Port/Edge
Typed Graph、Render Checkpoint、Render Policy、VCS 共有を単一の
整合契約へ統合する **実装前仕様正本**。本書は Issue #289
§0〜§9 の合意内容（2026-11-11 合意）を canonical spec として
codify し、後続の子 Issue #290〜#297（RCP-01〜07・RCP-F01）が
参照する共通契約を固定する。

本 slice は **docs/ 設計契約 + evidence 台帳のみ**。Issue 作成・
本仕様の確定は実装完了・実 Plugin/RT/OS Gate PASS を意味しない。
実装・統合・受入は子 Issue 側の別 Gate が担う。

## Why Now

- 既存契約は部分的に揃っている: `media-render.md` の
  MR-RCP-01（Offline Render と Render Checkpoint の分離）、
  `project-persistence.md` の PROJ-RCP-01（Save durability と
  Render Job の別 Receipt）、`execution-plan.md` の RCP-00〜07/
  F01 ノードと DAG edge 型。しかし「Render Recipe の canonical
  schema」「依存閉包の無効化規則」「Exact/Preview/Unknown/
  Corrupt の判定 Oracle」「Branch/Adopt/late attach の不変条件」
  「Render Policy trigger と非原子性」「Plugin Missing 下の
  typed subgraph 代替」を単一の正本で固定する文書がなかった。
- Creative Branch（PM-R-08）・VCS immutable snapshot
  （VCS-01）・plugin state 品質（TIM-04）・execution plan
  世代（SF-03/PG）・offline render verified publish（L2 ADR・
  `synvelis-render`）は別々の契約として存在し、Render
  Checkpoint がそれらを横断する際の語彙衝突（特に
  `PluginStateRecipe` という既存の別概念）と false-valid
  リスクを解消する必要があった。
- #290〜#296 は既存契約＋Fake Adapter/typed contract で並行
  着手可能とする依存方針を、正本の DAG として明文化する
  必要があった（Issue #289 §9）。

## Users and Value

- 実装者（#290〜#297 の各 Issue Owner）: recipe schema・
  依存閉包・validity/eligibility・policy・branch 参照の
  単一正本を参照し、Issue ごとの解釈ずれと重複実装を防ぐ。
- レビュアー/受入者: RCP-SPEC‑01〜08 の受入条件を本書の
  対応セクションへ写像し、docs-level の整合を検証できる。
- 将来の consumer（AudioLens・remote renderer PoC）: DAW 語彙に
  依存しない再利用可能な recipe/artifact/policy 概念の境界を
  保つ。

## Scope

In:

- Canonical `RenderRecipe` v1 schema・canonicalization・
  `RecipeKey` digest 規則（§Render Recipe）。
- Full dependency closure（sidechain/send/return/MIDI/
  automation/tempo/plugin state/external asset/time-range）と
  temporal invalidation、Exclude List、false-valid 防止。
- Exact/Preview/Unknown/Corrupt 状態機械、再現性クラス、
  readback/hash oracle。
- Branch→Recipe→Artifact 参照、Adopt 1 CAS/1 Undo、late
  attach、Snapshot/VCS retention・GC roots。
- Plugin Missing/Edit/Adopt、Execution Intent 別 typed
  subgraph cut/substitution、Prepared plan・Last-good・
  clock/capability fault。
- Render Policy trigger/scope/completeness/job/receipt と
  Save/Commit/Share の非原子性。
- Artifact publish/retention/GC/sharing の安全性規則。
- Owner/DAG（#290〜#297）・Start/Integration/Acceptance
  edge 型・Ruviform/AudioLens 境界。
- 受入条件 RCP-SPEC‑01〜08 の写像と evidence schema。

Out（本 Issue では実装しない・設計上の Pass 宣言もしない）:

- 新規 DSP Compiler・独自 Plugin Host・RT Runtime・Git
  Provider・AudioLens コード・Raw Recording Writer。
- Ruviform 側の generic capability 追加（#562 PoC で不足が
  確認された場合のみ別 Issue で起票）。
- 実 Plugin/RT/OS/PCM/GPUI/AudioLens の実機受入（#296
  RCP-GATE・別 lane）。
- Remote renderer の本実装（#297 RCP-F01 は将来 PoC で
  初期 Gate をブロックしない）。
- `crates/**` のコード変更全般（本 slice は docs/evidence のみ）。

## Non-negotiable Invariants（Issue #289 §0）

以下は本契約の非交渉不変条件。後続の全ての子 Issue・実装・
レビューはこれに従う。違反は設計欠陥として扱う。

1. **Project が canonical source**: versioned Project /
   VariantSet / Branch / Semantic Graph / Plugin State /
   Source / Automation / Render Boundary 宣言の正本は
   Project のみ。派生物（recipe・artifact・index・plan）は
   Project を変更しない。
2. **ProjectController が唯一の writer**: Project canonical
   state への全変更は ProjectController の単一経路
   （CAS + Undo）を通る。Render/Preview/Adopt/GC が
   controller を迂回して project を mutate してはならない。
3. **外部 Git/VCS は optional**: Creative Branch は Git
   branch と同一概念ではない。VCS 未接続・provider 未実装
   でも Branch/Adopt/render checkpoint は成立する。
4. **RenderArtifact は immutable な verified derived
   media であり project authority ではない**: artifact が
   失われても project は損なわれない。artifact は常に
   「特定 snapshot × recipe × context の派生物」として
   provenance を持つ。
5. **RecipeKey・ContentHash・Eligibility は別概念**:
   - `RenderRecipe`/`RecipeKey` = 処理要求の同一性
     （何をどう処理するか）。
   - `RenderArtifact`/`ContentHash` = バイト列の同一性
     （何が生成されたか）。
   - `Eligibility` = 指定 context での再利用可否
     （この artifact をこの用途に使ってよいか）。
   三者を混同・同一視してはならない。RecipeKey 一致は
   bit-identical PCM の証明ではなく、ContentHash 一致は
   再利用適格の証明ではない。
6. **Execution Plan は環境由来の派生物**: plan は実行環境
   （plugin 有無・clock・capability）から導出され、
   Project/Branch に RT NodeId・PlanId・Plugin Runtime
   Instance を永続化してはならない。
7. **receipt 状態の分離**: Configured/Prepared/Published/
   Active/Observed（plan 側 receipt）と Render Job 状態
   （render 側 receipt）は別の状態機械であり、相互に
   代替・統合しない。
8. **Preview/Compare/Audition は project-authoritative な
   read/selection 操作**: Main・Recording Writer を
   暗黙に mutate しない。preview 成功を exact 成功と
   表示しない。
9. **Adopt は宣言 scope のみ・1 ProjectController CAS・
   1 Undo**: semantic merge や blend ではない。宣言外の
   差分を巻き込まない。
10. **missing/stale/unverified render を silent・bypass・
    old・"Exact" として提示しない**: 表示は typed state
    （§Eligibility）に従い、fail-closed。
11. **Plugin missing は編集禁止を意味しない**: plugin が
    欠落していても project 編集・branch 操作・adopt は
    可能。ただし実行・exact audition は別判定。
12. **AudioLens は DAW 語彙から独立**: consumer 品質
    ゲートとして独立性を保ち、Synvelis の render policy
    をそのまま採用しない。

## Persistent vs Derived Boundary and Identifiers（Issue #289 §1）

### Persistent（Project canonical / versioned）

| 概念 | 識別子 | 備考 |
| --- | --- | --- |
| Render Boundary 宣言 | `RenderBoundaryId` | semantic boundary/port 集合・scope/output port・temporal domain を宣言。PM-R-08 の VariantScope/BranchContentRef と整合 |
| Semantic Node/Port/Edge | semantic ID | stable。Track/Clip/Take/MIDI/Instrument/FX Range/Bus/Master/Arrangement を横断する scope を stable semantic ID で参照 |
| Branch / BranchRevision | BranchRevisionRef | lineage を持ち、artifact を所有せず参照する |
| Plugin identity/state | plugin format/vendor/UID/version/binary identity + opaque state | TIM-04 の StateCaptureQuality と整合 |
| Source/Automation/Transport 宣言 | 既存 project model ID | versioned project の一部 |
| Render Policy 参照 | policy ID/版 | trigger/scope/completeness の宣言。実行状態ではない |

### Derived（再生成可能・非 canonical）

| 概念 | 識別子/実体 | 備考 |
| --- | --- | --- |
| Render Recipe | `RenderRecipe` schema + `RecipeKey` digest | 処理要求の canonical 表現。Project に recipe を埋め込まず、snapshot+context から導出 |
| Artifact | `RenderArtifactId` + `ArtifactManifest` + `ContentHash` | immutable verified derived media。content-addressed |
| Dependency Index | dependency index（snapshot×context 単位） | 派生 cache。missing/corrupt は fail-closed で再評価 |
| Validity/Eligibility | 評価結果（typed state） | 保存せず context ごとに再評価 |
| Render Job | job ID + receipt | 実行状態。persistent project state ではない |
| Execution Selection / Prepared Plan | plan generation + receipt | 環境由来。last-good は retention 管理 |
| RT Generation | generation tag | plan activation の世代証拠 |

### 識別子規則

- Semantic ID は安定で、表示名・順序・一時的 UI 状態に
  依存しない。
- Branch は artifact を「所有」せず「参照」する。artifact
  の GC 到達可能性は project/branch live・history・VCS
  snapshot/share package・pinned comparison・running job/
  lease を root とする（§Artifact Publish/Retention）。
- `RenderBoundaryId` は semantic graph 上の境界宣言であり、
  実行時の node/port 実体ではない。

## Canonical Render Recipe v1（Issue #289 §2）

`RenderRecipe` は「この境界のこの出力を、この文脈で、
この処理要求で render する」という要求の canonical 表現。
`RecipeKey` はその決定的 digest。

### 用語の区別（必須）

- 本書の `RenderRecipe`/`RecipeKey` は **新規概念**。
- `crates/synvelis-render/src/spec.rs` の `PluginStateRecipe`
  は **別物**: plugin state の品質 evidence + gate を表す
  既存型であり、本仕様の recipe ではない。同名衝突を避ける
  ため、実装時は本概念を `RenderRecipe`/`RecipeKey` と呼び、
  `PluginStateRecipe` を流用・改名・包含しない。

### Header（canonicalization 識別）

| フィールド | 内容 |
| --- | --- |
| `recipe_schema_version` | recipe schema の版。migration 規則は §Recipe Migration |
| `canonicalization_version` | canonical byte 表現の規則版 |
| `hash_algorithm` + domain separator | digest 方式と recipe 固有の domain 分離文字列 |
| `audio_semantics_version` | 音声意味論の版（sample 表現・tempo 写像等の規則版） |

### Target（対象）

- semantic boundary/port 集合（`RenderBoundaryId` と
  output port 集合）
- project snapshot locator + digest + schema projection
- `BranchRevisionRef` + 解決済み shared context digest
  （branch が参照する shared upstream の解決結果）

### Upstream closure（入力閉包）

以下を全て列挙し、各々に安定参照（ID/digest）を持たせる:

- source blob（content digest + verified 状態）
- clip/trim/loop/stretch/pitch/placement
- MIDI notes/CC/PPQN/tick
- automation/modulation
- tempo/meter/transport・musical↔frame 写像
- routing/send/return/sidechain/fan-in
- external input identity
- gain/PDC（latency/preroll/tail/valid extent）

### Processor closure（処理閉包）

- plugin format/vendor/UID/version/binary identity
- opaque state byte digest + `StateCaptureQuality` +
  snapshot generation（TIM-04 整合）
- external preset/sample/IR/content 依存
- host processing compatibility/capability profile
- DSP implementation/renderer version
- precision/backend/oversample/interpolation/dither/
  seeds/denormal policy

### Temporal（時間域）

- sample rate/format/channels
- `[start_frame, end_frame)` half-open 区間
- clock epoch/transport/seek/loop/tempo discontinuity
- preroll/warmup/tail
- latency/PDC/valid output extent
- algorithmic dependency horizon（有限/無限の区別）

### RecipeKey 規則

- 決定的 canonical bytes から domain-separated
  cryptographic digest を計算する。
- Exclude List（digest 対象外）: UI 表示名・作成時刻・
  artifact 配置場所・trigger 設定・無関係な view state。
- `RecipeKey` 一致は bit-identical PCM の証明ではない。
  再現性は §Reproducibility Classes と readback oracle で
  別途判定する。

### Recipe Migration

- `recipe_schema_version`/`canonicalization_version`/
  `audio_semantics_version` のいずれかが変わると RecipeKey
  は変わり得る。migration は「旧 recipe で生成された
  artifact の eligibility を新 recipe で再評価する」形で
  行い、旧 artifact を破壊しない。
- 未知の新版 recipe を旧実装が受け取った場合は
  `Unsupported`（fail-closed）とし、推測 decode しない。
- 相互互換 migration の具体 case は RCP-01（#290）の
  設計 fixture で固定する（RCP-SPEC‑01）。

## Dependency Closure and Partial Invalidation（Issue #289 §3）

### 依存グラフ

- typed semantic graph（port/edge）と time-range 依存
  グラフの二層。semantic edge は型付き、temporal edge は
  frame 区間で重み付けされる。
- 全入力を Source/MIDI/Automation/PluginState/
  ExternalAsset/Transport/Sidechain/Send/Return/Bus/
  Master へ trace する。到達不能・未型付けの依存は
  `UnknownDependency` として fail-closed。
- cycle は既存契約どおり zero-delay 規則で reject
  （`playback-graph.md`/`sf-03` 整合）。

### 編集→無効化の伝播

- 編集イベントは semantic object + 影響 time range を特定し、
  下流へ伝播する。
- 保守的拡張が必要なケース: time warping・automation
  interpolation・lookahead・PDC・filter memory・reverb
  tail・MIDI held note/CC64/state chase。これらは有限
  horizon が証明できる場合のみ interval diff、それ以外は
  conservative full-range invalidation または
  `CacheUnsupported`。
- 空の変更（no-op edit）は既存 artifact を無効化しない。
- stale 判定は artifact を変更せず、target snapshot ×
  context × boundary ごとに再評価する。
- Dependency Index は派生 cache。index missing/corrupt は
  fail-closed で dependency closure を再評価し、
  「index が無いから valid」扱いをしない（false-valid
  防止）。
- shared upstream の変更はそれに依存する全 branch へ伝播
  する。branch 内部の編集は他 branch/live を変更しない。
- Adopt 後の再利用判定は「ProjectRevision が変わったか」
  ではなく「新しく解決された graph との等価性」で行う
  （§Branch/Adopt）。

### Exclude List（無効化対象外）

- UI 表示名・view state・trigger 設定・artifact 配置・
  作成時刻など、処理結果に影響しない要素は recipe/依存
  閉包の digest 対象外（§RecipeKey Exclude List と同一
  集合）。
- 「見た目が同じだから valid」「名前が同じだから同一」
  のような非意味的一致を valid 根拠にしない。

## Eligibility and Trustworthiness（Issue #289 §4）

### 状態機械（artifact eligibility）

以下は **永続 Job 状態とは別** の評価 state。未評価は
`Unknown` であり、使用不可（fail-closed）。

```
Missing → Unverified → Corrupt → Stale → ValidRecipe
                                    → VerifiedMedia
                                    → EligibleExact
                                    → EligiblePreview
                                    → Unsupported
                                    → Unknown（未評価/判定不能）
```

- `Missing`: artifact が存在しない/到達不能。
- `Unverified`: 存在するが digest/manifest/readback 検証が
  未了。
- `Corrupt`: 検証失敗（hash 不一致・decode 不能・manifest
  矛盾）。
- `Stale`: recipe/context/snapshot のいずれかが変化し
  再検証が必要。
- `ValidRecipe`: recipe が canonical に解決可能。
- `VerifiedMedia`: manifest + content digest + 必要に応じ
  PCM decode/frame proof の readback 検証済み。
- `EligibleExact`: Exact 用途に必要な全条件を満たす。
- `EligiblePreview`: Preview 用途の緩和条件を満たす
  （exact ではないことを明示）。
- `Unsupported`: capability/形式が非対応（技術的理由を伴う）。
- `Unknown`: 未評価・判定材料不足。**valid 扱い禁止**。

### Exact 再利用の 6 条件（全て必須）

1. pinned snapshot + full dependency closure 一致
2. boundary I/O・port/layout・time/PDC/tail 一致
3. media digest + frame count + container/codec/
   format-independent readback 検証
4. source/plugin state 品質 + external asset 解決可能性が
   要求水準を満たす
5. renderer/plugin/platform/backend の再現性クラスが
   用途と互換
6. permission/ACL/expiry が有効

### Reproducibility Classes

| Class | 意味 | 昇格条件 |
| --- | --- | --- |
| `BitExactCertified` | bit 一致が証明済み | 同一環境・決定的 renderer での検証済み artifact |
| `VerifiedWithinTolerance` | 許容誤差内で検証済み | oracle が tolerance を定義し pass |
| `ReplayOfVerifiedArtifact` | 検証済み artifact の replay | 共有先での再 render ではなく artifact 再生 |
| `PreviewOnly` | preview 用途のみ | exact へ自動昇格しない |
| `Unknown` | 未分類 | valid 扱い禁止 |

- 異なる OS/CPU・非決定的 plugin・BestEffort state では
  自動的に `BitExactCertified` へ昇格しない。
- 「same sound」の共有保証は **Verified Artifact replay
  guarantee** であり、再 render の bit 一致ではない。

### Plugin Missing / 判定 Oracle

- Plugin Missing でも manifest 完備・opaque state 保存・
  typed port・automation 参照が揃えば、project 編集と
  adopt は可能。実行・exact audition は別判定
  （§Execution Intent）。
- 未対応 plugin state・未知 port を推測して valid 扱い
  しない。
- Hash collision/Version 変化時の判定 oracle: digest
  algorithm/domain/version のいずれかが変われば別
  RecipeKey。collision 疑いは `Corrupt`/`Unknown` で
  fail-closed、silent 上書きしない。
- 同一 recipe に複数 artifact がある場合、verified +
  compatible + quality で deterministic に選択する。
  provenance が異なれば共存させ、暗黙の "latest wins"
  をしない。

## Subgraph Substitution / Execution Intent（Issue #289 §5）

### Non-RT resolver pipeline（順序固定）

1. fixed immutable snapshot を入力にする
2. semantic graph expansion
3. plugin capability/state/ports check
4. eligible render search（§Eligibility）
5. typed subgraph cutset solver
6. Ruviform nonRT prepare/compile
7. revision 再検証
8. publish
9. RT active receipt

### Render Boundary と出力

- boundary は必要な全 external output port を含む
  （multi-out 含む）。MIDI/Event/Control 出力は対応する
  artifact 型を要求し、audio から非 audio 出力を捏造
  しない。
- sidechain external input・return・parallel branch は
  dependency closure を検証し、double-mix/dangling edge
  を reject する。

### Execution Intent

| Intent | 用途 | 等価性 |
| --- | --- | --- |
| `MainPlayback` | 通常再生 | project-authoritative |
| `EditingPreview` | 編集中 preview | 非等価を明示 |
| `BranchAudition` | branch 試聴 | 非等価を明示 |
| `OfflineRender` | offline render job | recipe 駆動 |
| `Export` | export | recipe 駆動 |

- preview 系 intent は結果に「非等価」を明示し、exact
  と混同しない。

### Plan selection / fault

- plan 選択順: correctness → editability → RT resources →
  stability。
- 連続編集の debounce/coalesce は許容するが、revision/
  plugin capability/clock epoch 変化時の stale publish は
  reject する。
- plan 変更と render invalidation は独立（plan が変わっても
  artifact eligibility は recipe/context で別判定）。
- 高速 param 更新は recompile 不要でも、下流 checkpoint
  validity を再評価する。
- Plugin missing→available・新 artifact 到着時は auto
  re-selection するが、実 render 生成は policy に従う。
- project CAS 後の reconcile 失敗: project の auto rollback
  はしない。last-good が生きていれば保持し「current
  revision mismatch」を mark、旧 generation の param/
  automation 誤配信を block する。RT thread で FS/
  allocation/blocking/graph search を行わない
  （`tim03-process-boundary.md`/`playback-graph.md` 整合）。

## Render Policy / Save / Commit / Share（Issue #289 §6）

### Trigger

- `Manual` は常に有効。
- `OnSave`/`OnCommit`/`OnShare`/`OnDemand` は非排他的に
  併用可能。
- target scope: `PlaybackCompatibility` /
  `CollaborativeEditing` / `ExplicitBoundaries` /
  `MixSnapshot`。
- quality/render 設定の変更は recipe を変えるが、trigger
  設定自体は recipe に含めない（Exclude List）。

### Trigger 別の pin 規則

- `OnSave`: durable project save が確認された snapshot に
  pin する。save 未完了の live state を対象にしない。
- `OnCommit`: VCS commit 後の immutable locator/manifest
  に関連付ける。VCS Write 未実装なら typed `Unsupported`。
- `OnShare`: share package と required artifact
  completeness を評価する。
- `OnDemand`: user approval または明示的な設定済み
  authorization が必要。承認なしに CPU/GPU/network を
  消費しない。

### Completeness / Job / Receipt

- `BestEffort`: save/commit を unblock し、Incomplete を
  mark する。
- `RequireComplete`: share/public 前に fail-closed
  （artifact 未揃いを成功扱いしない）。
- 安全な render 完了のために Git commit を遡及変更しない。
- auto render job の idempotency key =
  target immutable snapshot + recipe + purpose。重複要求は
  coalesce する。
- 失敗分類: `PluginMissing`/`StateCaptureUnsupported`/
  `SourceUnavailable`/`InsufficientStorage`/`Cancelled`/
  `WorkerFailure`/`AuthDenied`/`UnknownDependency` →
  Reason-typed `Failed`/`Pending`。
- render receipt は sync save/version control の結果と
  **別 receipt**（PROJ-RCP-01 整合）。
- artifact は編集が進んでも旧 snapshot に pin され、
  「latest」として混同しない。

## Artifact Publish / Retention / GC / Sharing（Issue #289 §7）

### Publish 手順（順序固定）

1. staging write
2. seal/finalize
3. hash + metadata readback
4. 必要に応じ独立 PCM decode/frame proof
5. atomic publish/index CAS

- crash/disk full/partial chunk/checksum mismatch/
  same-key-different-content/conflicting writers は
  `Fail`/`Quarantine`。旧 verified artifact を保持する。
- store は content-addressed + immutable manifest。
  recipe→artifact は many-to-many provenance。
- PCM/codec precision・channel order・time alignment・
  format/latency を表示可能にする。
- lossy probe/cache を exact lossless renderer output と
  して再利用しない。

### Retention / GC

- GC roots: project/branch live + history + remote share
  ref + active lease + in-flight job + retention pin。
- mark/sweep、grace/quarantine period を設ける。
- reachability 不明時は安全側（保持）に倒す。
- remote unsynced/ACL 変更/offline peer を考慮する。
- raw source/take/verified recording を決して破壊しない。

### Sharing / Remote trust

- remote manifest を盲信しない: size/bounds/hash/
  container format/codec/metadata/ACL/principal/consent を
  検証する。
- unavailable artifact は `Missing`/`Unavailable`。
- plugin binary/license material/secrets を共有しない。
- remote render request は snapshot/permission/grant/pin/
  receipt を pin する。owner の plugin を使うことへの
  consent は別途必要。

## Test Plan / Failure Matrix（Issue #289 §8）

受入条件 RCP-SPEC‑01〜08 に対応する設計 fixture/case。
実装・実機 gate は子 Issue 側。

| Case 群 | 内容 | 対応 AC |
| --- | --- | --- |
| canonicalization | 同一 semantic 入力→同一 RecipeKey、UI 名/時刻変更→RecipeKey 不変、version 変更→migration 経路 | RCP-SPEC‑01 |
| partial invalidation | clip edit→該当区間のみ stale、tempo 変更→全域 conservative、index corrupt→fail-closed 再評価、no-op→valid 維持、false-valid=0 | RCP-SPEC‑02 |
| audio/trust | Exact 6 条件の各欠落→非 Exact、Corrupt→表示 typed、Preview→非等価明示、collision→fail-closed | RCP-SPEC‑03 |
| branch | Branch→Recipe→Artifact 参照、Adopt=1 CAS/1 Undo、late attach で commit 不変、GC root 保護 | RCP-SPEC‑04 |
| plan/RT | plugin missing→typed substitution、intent 別 boundary、Last-good+revision mismatch、clock/capability fault 行列 | RCP-SPEC‑05 |
| policy | trigger 別 pin、RequireComplete fail-closed、非原子 receipt、idempotent coalesce | RCP-SPEC‑06 |
| owner/DAG | #290〜#297 の owner/型/DAG、重複実装禁止、AudioLens 独立、実機 gate 分離 | RCP-SPEC‑07/08 |

## Owners and Start DAG（Issue #289 §9）

### Owner 分離

| 領域 | Owner | 備考 |
| --- | --- | --- |
| recipe semantics・DAW dependency closure・policy・project/branch/VCS/GUI・artifact storage 統合・execution intent/path resolver・receiver capability・share UX | Synvelis | 本書の canonical 契約 |
| DAW 非依存 typed graph・generic port/node dependency API（不足時のみ）・DSP/plugin host・stateful offline render・verified media/RT playback・PDC/plan activation/probe | Ruviform | 新 crate/public API は #562 PoC で不足確認後のみ |
| consumer 品質ゲート | AudioLens | 独立。f64/long-media/CLI/GUI 精度・性能を保護。DAW render policy を採用しない |

### 子 Issue DAG（#290〜#297）

- `SV-RCP-00(#289)` → `start` → `SV-RCP-01(#290)` 〜
  `SV-RCP-06(#295)`：既存契約＋Fake Adapter/typed
  contract で **並行着手可能**。
- `integration` edge: `RF#562`・`RF#196`・`RF#198`・
  `SV-SF-03(#106)`・`SV-TIM-04(#128)`・`SV-CB-00(#277)`・
  `SV-MEDIA(#285)` → `SV-RCP-01..06`。
- `acceptance` edge: `SV-RCP-01..06` → `SV-RCP-07(#296)`
  （multi-repo fixtures・PCM/RT/real plugin/GPUI/3OS/
  AudioLens acceptance convergence）。
- `SV-RCP-F01(#297)`: owner-authorized remote renderer
  将来 PoC。初期 Gate をブロックしない。
- edge は forward-only・acyclic・定義済み node のみ。
  `acceptance` edge の to は Gate node のみで、Gate node
  から `start` edge を出さない（`execution-plan.md`/
  `execution-plan-design.md` の edge 型と同一規則）。

### 子 Issue 対応表

| Issue | Lane | AC 接頭辞 | 主担当領域 |
| --- | --- | --- | --- |
| #290 RCP-01 | canonical recipe・full dependency closure・temporal invalidation/Semantic Resolver | RCP-REC-01〜06 | recipe/closure/index |
| #291 RCP-02 | immutable media manifest・verification・CAS index・retention/GC・remote trust | RCP-STORE-01〜06 | artifact store |
| #292 RCP-03 | Manual/OnSave/OnCommit/OnShare/OnDemand・Job・Completeness | RCP-POL-01〜07 | policy/job |
| #293 RCP-04 | missing-plugin typed subgraph substitution・Execution Intent・nonRT Prepare/Last-good | RCP-EX-01〜06 | execution intent |
| #294 RCP-05 | Branch/Edit/Adopt/VCS immutable snapshot 参照・late attach | RCP-BR-01〜06 | branch/vcs |
| #295 RCP-06 | GPUI/Gateway/CLI/Workflow/Script observability・authorization・Policy | RCP-UX-01〜06 | UX/observability |
| #296 RCP-07 | multi-repo fixtures・PCM/RT/real plugin/GPUI/3OS/AudioLens 受入 gate | RCP-GATE-01〜12 | acceptance |
| #297 RCP-F01 | owner-authorized remote renderer future PoC | RCP-REMOTE-01〜04 | future（非 blocking） |

## Acceptance Criteria（RCP-SPEC‑01〜08 写像）

| AC | 内容 | 本書の対応セクション | 状態 |
| --- | --- | --- | --- |
| RCP-SPEC‑01 | Canonical Recipe フィールド・Hash/Context 版・Exclude List・Unknown 対照表と相互互換 Migration を設計 fixture で固定 | §Canonical Render Recipe・§Dependency Closure | 設計契約確定・fixture は #290 |
| RCP-SPEC‑02 | Sidechain/Send/Return/MIDI/Automation/Tempo/Plugin State/External Asset/Time-range 依存 closure と無効化の正負例、false-valid=0 | §Dependency Closure | 同上 |
| RCP-SPEC‑03 | Exact/Preview/Unknown/Corrupt 状態機械・再現性クラス・Audio readback・Hash collision/Version 変化 oracle | §Eligibility | 同上 |
| RCP-SPEC‑04 | Branch→Recipe→Artifact・Adopt 1 CAS/Undo・late attach・Snapshot/VCS 保存・GC Roots の状態遷移と安全性反例 | §Branch/Adopt・§Artifact Publish | 同上 |
| RCP-SPEC‑05 | Plugin Missing/Edit/Adopt・Execution Intent 別 Subgraph cut・Plan activation/Last-good/Clock/Capability 故障行列を型と fixture へ | §Subgraph Substitution | 同上 |
| RCP-SPEC‑06 | Render Policy trigger/Scope/Completeness/Job Receipt と Save/Commit/Share 非原子性を検証条件へ | §Render Policy | 同上 |
| RCP-SPEC‑07 | Owner/DAG/PR 触るファイル/AudioLens 互換/実 OS・実 Plugin 未実施 Gate を子 Issue ごとに分離、重複実装禁止 | §Owners and Start DAG | 本書で固定 |
| RCP-SPEC‑08 | #290〜#297 の Owner/型・Start/Integration/Acceptance DAG を照合、Ruviform DAW 混入・重複 DSP Compiler/Plugin Host/Media Writer なし | §Owners and Start DAG・§Non-negotiable Invariants | 本書で固定 |

**重要**: 上表の「設計契約確定」は docs-level の整合を
意味し、実装・fixture 実走・実機 gate の Pass ではない。
各 AC の実装検証は対応する子 Issue の test-spec/fixture/
evidence が担う。

## Evidence Status Vocabulary

本契約および `evidence/sv289/` で使用する状態語彙
（`POLICIES.md` §3 準拠）:

- 要求状態: `Required` / `Deferred` / `NotApplicable`
- 実行結果: `Pass` / `Fail` / `NotRun` / `Unsupported`
- 本 slice（docs-only）では実機・実装系は原則
  `NotRun`/`Unsupported`/`Deferred` とし、docs 検証のみ
  `Pass` と記録する。設計上の合意を実装 Pass に読み替え
  ない。

## Related Contracts and Existing Anchors

- `docs/specs/media-render.md` MR-R-18/MR-RCP-01:
  Offline Render と Render Checkpoint の分離。
- `docs/specs/project-persistence.md` PROJ-RCP-01:
  Save durability と Render Job の別 Receipt、late
  Artifact attach の原子性。
- `docs/specs/execution-plan.md` / `docs/designs/
  execution-plan-design.md`: RCP-00〜07/F01 ノード、
  start/integration/acceptance edge 型。
- `docs/specs/project-model.md` PM-R-08/09/10/11:
  Creative Branch・VariantScope・BranchContentRef。
- `docs/specs/vcs-01-immutable-snapshot-resolver.md` /
  `vcs-integration.md`: immutable snapshot 解決・
  VCS read-only・Creative Branch ≠ Git branch。
- `docs/specs/tim-04-plugin-state-snapshot-consistency.md`:
  StateCaptureQuality・plugin state opaque 保存。
- `docs/specs/tim03-process-boundary.md`: RT thread での
  FS/alloc/block 禁止・OfflineExact fail-closed。
- `docs/specs/playback-graph.md` / `sf-03-map-semantic-
  runtime-contract.md`: plan generation receipt・
  Last-good・CAS-before-publish・generation-tagged
  SourceMap。
- `docs/specs/command-gateway.md`: Gateway/CLI/Script の
  action/authorization 面（RCP-06 が詳細化）。
- `crates/synvelis-render/src/{snapshot,verify,job,
  preflight,prepare,spec}.rs`: 既存 L2 offline render +
  verified publish（read-only 参照。`PluginStateRecipe`
  は別概念）。
- `crates/synvelis-library/src/{digest,cache,registry,
  local,git/,webdav/}`: verified blob/digest・catalog
  cache・provider registry（read-only 参照）。
- `docs/adr/l2-render-slice-boundaries.md`: sequential
  offline render・staged publish・独立 readback 検証の
  既存境界決定。
