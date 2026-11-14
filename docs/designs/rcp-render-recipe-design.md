---
id: "DESIGN-HBZzINKHriX9qpGtNZ1ZFY"
type: "design"
status: "approved"
title: "Render Checkpoint — Render Recipe・依存閉包・有効性・Branch/Execution/Policy 統合設計 (synvelis#289 RCP-00)"
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
  implements:
    - "docs/specs/render-checkpoint.md"
  implemented-by: []
  depends-on:
    - "docs/specs/render-checkpoint.md"
    - "docs/specs/media-render.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/project-model.md"
    - "docs/specs/execution-plan.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/vcs-integration.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/rcp-00-render-recipe.md"
    - "docs/designs/execution-plan-design.md"
    - "docs/designs/media-render-design.md"
    - "docs/designs/project-persistence-design.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "evidence/sv289/render-checkpoint-contract-trace.md"
  refines:
    - "docs/designs/media-render-design.md"
  refined-by: []
  derives-from:
    - "docs/specs/render-checkpoint.md"
    - "docs/adr/rcp-00-render-recipe.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv289/render-checkpoint-contract-trace.md"
  defers:
    - "docs/specs/render-checkpoint.md"
  deferred-by: []
---

# Render Checkpoint — Render Recipe・依存閉包・有効性・Branch/Execution/Policy 統合設計 (synvelis#289 RCP-00)

本書は `docs/specs/render-checkpoint.md` の設計層。**文書のみ**で
あり、実装は #290〜#297 の各 Issue が Owner。設計合意・
headless fixture・製品受入・実機受入を区別する。2026-11-11
合意の内容を `docs/adr/rcp-00-render-recipe.md` に記録する。

## Context

既存契約の分散状態:

- `media-render.md`/`media-render-design.md`: MR-RCP-01 で
  Offline Render と Render Checkpoint を分離済み。L2 ADR
  (`l2-render-slice-boundaries.md`) で sequential offline
  render・staged publish・独立 readback 検証の境界が確定。
  `synvelis-render` に `RenderSnapshot::freeze`・
  staged verify・job/receipt の実装が存在する（read-only
  参照）。
- `project-persistence.md`/`project-persistence-design.md`:
  PROJ-RCP-01 で Save durability と Render Job の別
  Receipt・late Artifact attach の原子性を規定。
- `project-model.md`: PM-R-08 で Creative Branch
  （VariantScope/BranchContentRef）を canonical 化。
- `vcs-01`/`vcs-integration.md`: immutable snapshot
  解決・VCS read-only・Creative Branch ≠ Git branch。
- `tim-04`: plugin state の StateCaptureQuality・opaque
  保存。`tim03`: RT thread の FS/alloc/block 禁止・
  OfflineExact fail-closed。
- `playback-graph.md`/`sf-03`: plan generation
  （Configured/Prepared/Published/Active/Observed）・
  Last-good・CAS-before-publish・generation-tagged
  SourceMap。
- `execution-plan.md`/`execution-plan-design.md`: RCP-00
  〜07/F01 ノードと start/integration/acceptance edge 型。

不足していたのは、これらを横断する **Render Recipe の
canonical schema・依存閉包の無効化規則・validity/
eligibility 判定・Branch/Adopt/late-attach 不変条件・
Render Policy・Plugin Missing 下の typed subgraph 代替**
を単一正本で固定することであり、本書と spec がそれを担う。

## Scope

In: spec §Non-negotiable Invariants〜§Owners and Start DAG
の全設計判断、代替案と棄却理由、子 Issue への分解指針、
既存契約との統合点。Out: 実装コード、Ruviform 変更、
AudioLens 変更、実機/実 Plugin/RT/PCM/GPUI gate、新規
DSP Compiler/Plugin Host/Media Writer。

## Architecture Overview

### Layered 構造（上位→下位）

```
┌─────────────────────────────────────────────────┐
│ Product / UX 層 (Synvelis)                        │
│  GPUI/Gateway/CLI/Workflow/Script (RCP-06 #295)   │
│  Branch/Edit/Adopt/VCS UI (RCP-05 #294)           │
│  Render Policy 設定・Job 可視化 (RCP-03 #292)     │
├─────────────────────────────────────────────────┤
│ Render Checkpoint orchestration (Synvelis)        │
│  RenderRecipe canonicalize → RecipeKey            │
│  DependencyIndex / Semantic Resolver (RCP-01 #290)│
│  Eligibility / Validity evaluator                 │
│  Render Policy trigger/scope/completeness (#292)  │
│  Artifact store 統合・CAS index・GC (#291)        │
│  Execution Intent/path resolver (#293)            │
├─────────────────────────────────────────────────┤
│ ProjectController (Synvelis, single writer)       │
│  Project/Branch/Semantic Graph/Plugin State/Source │
│  Automation/Render Boundary 宣言 — 唯一の writer   │
├─────────────────────────────────────────────────┤
│ Ruviform generic 層 (再利用・変更は別 Issue)        │
│  typed graph/port/node・DSP/plugin host           │
│  stateful offline render・verified media          │
│  RT playback・PDC・plan activation・probe         │
└─────────────────────────────────────────────────┘
```

### Data flow（render checkpoint の典型経路）

```
ProjectController (canonical)
  │ snapshot freeze (RenderSnapshot 相当)
  ▼
Immutable Snapshot ──→ Semantic Graph expansion
  │                        │
  │                        ▼
  │                  DependencyIndex (派生 cache)
  │                        │  miss/corrupt → fail-closed 再評価
  │                        ▼
  │                  RenderRecipe canonicalize → RecipeKey
  │                        │
  │                        ▼
  │                  Eligibility evaluator ──→ typed state
  │                  (Missing/Unverified/Corrupt/Stale/
  │                   ValidRecipe/VerifiedMedia/
  │                   EligibleExact/EligiblePreview/
  │                   Unsupported/Unknown)
  │                        │
  │              ┌─────────┴──────────┐
  │              ▼                    ▼
  │        eligible artifact    render job (policy 駆動)
  │        を選択/参照            │ idempotent coalesce
  │              │                ▼
  │              │          nonRT prepare/compile
  │              │          (typed subgraph cutset)
  │              │                │
  │              │                ▼
  │              │          staged render → verify
  │              │          (hash+metadata readback,
  │              │           必要に応じ PCM decode)
  │              │                │
  │              │                ▼
  │              │          atomic publish + index CAS
  │              │                │
  │              └──→ provenance (recipe→artifact
  │                    many-to-many) を記録
  ▼
Branch/Adopt/VCS: immutable snapshot 参照 + late attach
```

### Control flow（policy trigger）

```
Edit/Save/Commit/Share/Demand event
  → Render Policy 評価 (trigger × scope × completeness)
  → target immutable snapshot を pin
  → RecipeKey + purpose で idempotent job key 生成
  → duplicate は coalesce / job queue
  → 完了時: receipt を save/vcs receipt と分離記録
  → BestEffort: Incomplete mark で unblock
  → RequireComplete: 未完なら share/public を fail-closed
```

## Persistent vs Derived Objects（設計上の配置）

spec §Persistent vs Derived Boundary の設計上の帰結:

- **ProjectController 配下に永続化するもの**: Project/
  VariantSet/Branch/BranchRevision/Semantic Graph/Plugin
  State/Source/Automation/Render Boundary 宣言/Render
  Policy 宣言。全て versioned で、変更は CAS+Undo。
- **ProjectController の外に派生させるもの**: RenderRecipe
  (canonical bytes+RecipeKey)/ArtifactManifest+RenderArtifact/
  ContentHash/DependencyIndex/Eligibility 評価/Render Job/
  ExecutionSelection/PreparedPlan/RT Generation。これらは
  再生成可能で、失っても project は損なわれない。
- **境界の強制**: 派生物の生成・更新は ProjectController
  を mutate しない。唯一の例外は Adopt で、宣言 scope を
  1 CAS/1 Undo で適用する（§Branch/VCS）。

## Semantic Graph vs Execution Plan（分離の設計）

- **Semantic Graph**（Project canonical）: typed
  Node/Port/Edge、time-range 意味、branch lineage。
  DAW 語彙を持ち、環境非依存。
- **Execution Plan**（環境由来派生）: semantic graph を
  現在の plugin/capability/clock 環境で lowering したもの。
  Configured/Prepared/Published/Active/Observed の
  generation receipt を持ち、last-good を retention 管理。
- **分離の理由**: plan は環境で変わる（plugin missing で
  subgraph cut、clock epoch で再 prepare）。plan を
  project に永続化すると「環境依存の実行詳細」が
  canonical state を汚染し、branch 間で共有できない。
- **接点**: plan の入力は常に fixed immutable snapshot
  （非 RT resolver pipeline step 1）。plan 生成後の
  revision 変化は stale publish として reject
  （`playback-graph.md`/`sf-03` の generation 規則と同一）。

## Resolver and Dependency Index（責務）

### Semantic Resolver（RCP-01 #290 の中核）

- 入力: immutable snapshot + semantic graph + context
  （branch/intent/environment capability）。
- 責務: upstream closure の全依存を typed edge として
  解決し、time-range 依存を frame 区間で記述。
- 出力: canonical RenderRecipe（schema version・
  canonicalization version・domain-separated digest）。

### Dependency Index

- 派生 cache。snapshot×context 単位で「どの semantic
  object のどの time range がこの recipe/boundary に
  影響するか」を保持。
- **fail-closed**: index missing/corrupt/版不一致は
  「valid」ではなく dependency closure の全再評価を
  起動する。index は最適化であり正しさの根拠ではない。
- 編集イベント → semantic object + affected time range
  → index 引き → downstream propagation。保守的拡張
  （time warp/automation lookahead/PDC/filter memory/
  reverb tail/MIDI state chase）は有限 horizon 証明時のみ
  interval diff、他は full-range invalidation または
  `CacheUnsupported`。

## Artifact Manifest and Content-Addressed Store（設計）

- **ArtifactManifest**（immutable）: recipe 参照・snapshot
  locator・boundary/port・temporal domain・format/
  precision/channel order/time alignment/latency・
  ContentHash・verification 証拠（hash+metadata readback、
  必要に応じ PCM decode/frame proof）・provenance。
- **ContentHash**: byte 列の同一性。RecipeKey（処理要求）
  ・Eligibility（再利用可否）とは別概念（spec §0-5）。
- **CAS index**: content-addressed store への index CAS
  で publish を atomic 化。same-key-different-content・
  conflicting writers は Quarantine で旧 verified を保持。
- **recipe→artifact many-to-many**: 同一 recipe に複数
  artifact が共存可能。選択は verified+compatible+
  quality で deterministic、provenance が異なれば共存
  （"latest wins" 禁止）。
- **staging→seal→readback→atomic publish**: 既存
  `synvelis-render` の verify.rs 相当（staged output を
  独立 reopen/decode してから publish）を artifact store
  全般へ拡張する設計。検証失敗は temp を破棄し publish
  しない。

## Validity / Eligibility Evaluation（設計）

- 評価は **保存しない**。`(snapshot, context, boundary,
  intent)` の組ごとに導出時に計算する。
- 状態機械は spec §Eligibility の typed state。
  `Unknown` は「未評価/判定不能」で valid 扱い禁止。
- **Exact 6 条件**（pinned snapshot+closure 一致・
  boundary I/O・media digest+frame count+readback・
  source/plugin 品質・再現性クラス互換・permission/ACL/
  expiry）を全て満たす場合のみ `EligibleExact`。
- **再現性クラス**: `BitExactCertified` は同一環境・
  決定的 renderer の検証済みのみ。OS/CPU 差・非決定的
  plugin・BestEffort state での自動昇格なし。
- **"same sound" = Verified Artifact replay** であり、
  再 render bit 一致ではない。share は artifact replay
  を保証し、受信側 re-render の bit 一致は要求しない。

## Policy / Job Orchestration and Receipt（設計）

- **Render Policy** は宣言（trigger/scope/completeness）
  であり実行状態ではない。Project canonical に保持。
- **Render Job** は派生の実行状態。idempotency key =
  target immutable snapshot + recipe + purpose。
- **Receipt 分離**: save/vcs の sync receipt と render
  job receipt は別（PROJ-RCP-01）。render 完了で Git
  commit を遡及変更しない。
- **trigger pin**: OnSave→durable save 確認 snapshot、
  OnCommit→VCS commit 後 immutable locator（未実装は
  typed Unsupported）、OnShare→share package+completeness、
  OnDemand→明示 authorization 必須。
- **completeness**: BestEffort→Incomplete mark で unblock、
  RequireComplete→share/public 前 fail-closed。
- **失敗分類**: PluginMissing/StateCaptureUnsupported/
  SourceUnavailable/InsufficientStorage/Cancelled/
  WorkerFailure/AuthDenied/UnknownDependency → typed
  Failed/Pending。

## Branch / VCS / Snapshot / Late-Attach（設計）

- **Branch は artifact を所有せず参照する**。Branch→
  Recipe→Artifact の参照鎖は immutable snapshot locator
  を介する。
- **Adopt**: 宣言 scope のみを 1 ProjectController CAS +
  1 Undo で適用。semantic merge/blend ではない。宣言外
  差分を巻き込まない。Adopt 後の artifact 再利用は
  「新 resolved graph との等価性」で判定（revision 変化
  だけでは stale にしない）。
- **Late attach**: commit 確定後に render artifact が
  完了しても commit を変更しない。artifact は旧 snapshot
  に pin されたまま provenance で関連付け、"latest" と
  混同しない（PROJ-RCP-01 整合）。
- **Snapshot/VCS**: VCS snapshot/share package は immutable
  locator を保持し、artifact への reachability を GC root
  にする。VCS 未接続でも Branch/Adopt/render checkpoint は
  成立（Creative Branch ≠ Git branch）。
- **GC reachability**: project/branch live+history+remote
  share ref+active lease+in-flight job+retention pin。
  reachability 不明は保持側。raw source/take/verified
  recording は破壊しない。

## Plugin-Missing Execution Modes and Last-good Safety（設計）

- **Plugin Missing**: manifest 完備・opaque state・typed
  port・automation 参照が揃えば project 編集/adopt は可。
  実行/exact audition は `PluginMissing` typed state で
  fail-closed。
- **Execution Intent 別 subgraph**: boundary は全 external
  output port（multi-out・MIDI/Event/Control 含む）を要求。
  audio から非 audio 出力を捏造しない。sidechain/return/
  parallel branch は closure 検証、double-mix/dangling
  edge reject。
- **nonRT resolver pipeline**: fixed snapshot→semantic
  expansion→capability/state/ports check→eligible render
  search→typed cutset solver→Ruviform nonRT prepare/
  compile→revision 再検証→publish→RT active receipt。
- **Last-good safety**: plan 変更・reconcile 失敗時に
  project を auto rollback しない。last-good が生存なら
  保持し "current revision mismatch" を mark。旧
  generation の param/automation 誤配信を block。
  RT thread で FS/allocation/blocking/graph search を
  しない（tim03/playback-graph 整合）。
- **plan selection 順**: correctness→editability→RT
  resources→stability。plan 変更と render invalidation は
  独立。

## Alternatives Considered and Rejected

| 代替案 | 棄却理由 |
| --- | --- |
| **Synvelis 独自 DSP Compiler/Plugin Host/Media Writer を新設** | Ruviform が generic compiler/plugin host/offline render/verified media を既に所有（L2 ADR・media-render-design Owner 分離表）。重複実装は保守・精度・RT リスクを生む。Ruviform #562 PoC で不足確認時のみ別 Issue で generic capability を追加 |
| **RenderRecipe を Project schema に直接埋め込む** | recipe は snapshot+context の派生物。project に埋めると環境依存・branch 間共有・versioning が破綻。Project は semantic 宣言のみ保持 |
| **Artifact ContentHash を再利用判定（Eligibility）に使う** | hash 一致は byte 同一性のみ。context（plugin state 品質・権限・再現性クラス）が異なれば同一 bytes でも Exact 不適。三者を分離（spec §0-5） |
| **index/plan を project に永続化して valid の根拠にする** | 派生 cache/plan は環境依存・再生成可能。missing/corrupt を valid 扱いすると false-valid。fail-closed 再評価（spec §Dependency Closure） |
| **Adopt を semantic merge/blend として実装** | Issue #289 §0-9 で「宣言 scope のみ 1 CAS/1 Undo、merge/blend ではない」と明示。merge は undo 境界・scope 制御を破壊 |
| **Plugin Missing で render 全体を禁止** | 編集/adopt は plugin 非依存で可能（spec §0-11）。実行のみ typed fail-closed。project 編集禁止は過剰制約 |
| **Preview/Compare/Audition の結果を Main/Recording Writer に反映** | project-authoritative read/selection 操作であり canonical を mutate しない（spec §0-8）。preview 成功を exact 成功と表示しない |
| **OnCommit で VCS Write 未実装でも commit 相当を捏造** | typed `Unsupported` で fail-closed。未実装 capability を成功扱いしない |
| **同一 recipe の "latest" artifact を暗黙選択** | provenance が異なる artifact は共存し deterministic 選択（verified+compatible+quality）。latest wins は provenance を破壊 |
| **`PluginStateRecipe`（既存型）を RenderRecipe として流用** | 別概念（plugin state 品質 evidence+gate）。同名衝突を避け本概念は `RenderRecipe`/`RecipeKey` と命名（spec §用語の区別） |
| **Render 完了を Git commit に遡及反映** | commit は immutable。late attach は artifact を旧 snapshot に pin して provenance で関連付け、commit を変更しない（spec §Render Policy・§Branch） |

## Integration Points（既存契約との接点）

| 接点 | 既存契約/実装 | 本設計での役割 |
| --- | --- | --- |
| Snapshot freeze | `crates/synvelis-render/src/snapshot.rs` `RenderSnapshot::freeze` | immutable snapshot 入力の既存実装。recipe/closure はこの snapshot を対象に導出 |
| Output verify | `crates/synvelis-render/src/verify.rs` | staged output の独立 reopen/decode→publish の既存パターン。artifact manifest verification の基盤 |
| Render job/receipt | `crates/synvelis-render/src/job.rs`/`lib.rs` | job state machine・typed receipt の既存実装。policy trigger はこの job を駆動 |
| Preflight/prepare | `crates/synvelis-render/src/{preflight,prepare}.rs` | capability/state 検査・nonRT prepare の既存経路。Execution Intent resolver が利用 |
| Project single writer | `synvelis-project` ProjectController CAS/Undo | canonical 変更の唯一経路。Adopt はこの CAS+Undo を通る |
| VCS immutable snapshot | `vcs-01`/`vcs-integration.md` | SnapshotLocator/Identity による immutable 参照。OnCommit/late attach/GC root の基盤 |
| Plugin state 品質 | `tim-04` StateCaptureQuality | recipe processor closure の opaque state 品質入力。`PluginStateRecipe`（spec.rs）は別概念として区別 |
| Process boundary | `tim03-process-boundary.md` | RT thread の FS/alloc/block 禁止・OfflineExact fail-closed。Last-good/plan fault の境界 |
| Plan generation | `playback-graph.md`/`sf-03` | Configured→Observed receipt・generation tag・Last-good retention。Execution Plan 派生の基盤 |
| Media owner 分離 | `media-render-design.md` Owner 表・L2 ADR | Ruviform generic（compiler/host/render/media）vs Synvelis product policy の固定境界 |
| Verified blob/digest | `synvelis-library/src/{digest,cache,registry,local,git/,webdav/}` | source blob content digest・provider registry・catalog cache。external asset closure の解決基盤 |
| Gateway/CLI | `command-gateway.md` | RCP-06 (#295) が action/authorization/observability を詳細化する接点 |
| AudioLens | 独立 consumer（Ruviform #294 系） | DAW 語彙非依存の品質ゲート。render policy を採用しない |

## Child Issue Decomposition Guidance（#290〜#297）

各子 Issue は本 spec/本設計を `derives-from`/`implements`
関係で参照し、以下の責務境界を守る:

- **#290 RCP-01**: recipe schema canonicalization・full
  dependency closure・temporal invalidation・Semantic
  Resolver・Dependency Index。RCP-REC-01〜06。
- **#291 RCP-02**: immutable ArtifactManifest・verification・
  CAS index・retention/GC・remote trust。RCP-STORE-01〜06。
- **#292 RCP-03**: Render Policy trigger/scope/
  completeness・Job・Receipt。RCP-POL-01〜07。
- **#293 RCP-04**: Plugin Missing typed subgraph
  substitution・Execution Intent・nonRT Prepare/Last-good。
  RCP-EX-01〜06。
- **#294 RCP-05**: Branch/Edit/Adopt/VCS immutable snapshot
  参照・late attach。RCP-BR-01〜06。
- **#295 RCP-06**: GPUI/Gateway/CLI/Workflow/Script
  observability・authorization・Policy。RCP-UX-01〜06。
- **#296 RCP-07**: multi-repo fixtures・PCM/RT/real
  plugin/GPUI/3OS/AudioLens acceptance convergence。
  RCP-GATE-01〜12。
- **#297 RCP-F01**: owner-authorized remote renderer
  future PoC。RCP-REMOTE-01〜04。初期 Gate 非 blocking。

**重複実装禁止**: CB(#277系)/SF(#106)/VCS(#294関連)/
Offline(L2) の既存責務を子 Issue で再実装しない。
Ruviform generic capability 不足は Ruviform Issue
（#562 PoC 結果）として記録し、Synvelis 側で恒久的に
重複実装しない（AGENTS.md Ruviform 節・POLICIES.md §6）。

## Deferred Design Concerns

- **Remote renderer の具体 protocol/consent UI**: #297
  RCP-F01 の PoC 範囲。初期 Gate をブロックしない。
- **Recipe schema の version 間 migration の全 case**:
  #290 の設計 fixture で固定。未知新版は `Unsupported`
  fail-closed。
- **Multi-out/MIDI/Event artifact の codec/manifest 詳細**:
  #291/#293 で具体化。本書は「audio から非 audio を
  捏造しない」境界のみ固定。
- **GUI/CLI/Script の action 名・authorization  granular
  詳細**: #295 で `command-gateway.md` を refines。
- **実機・実 Plugin・RT・PCM・AudioLens の受入 case
  詳細**: #296 RCP-GATE-01〜12 と `test-specs/` で固定。
  本書の設計 Pass を実機 Pass に読み替えない。

## Verification（本 slice で実行したもの）

本 slice は docs/evidence のみ。実行した検証と結果は
`evidence/sv289/render-checkpoint-verification.md` に記録。
設計上の合意・docs 整合を実装・実機 Pass と読み替えない。
