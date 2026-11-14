---
id: "DESIGN-034X7uvDLVml1hERo5iqNc"
type: "design"
status: "approved"
title: "Media / Import / Export / Offline Render / UI / Diagnostics — Media & Render Lifecycle 設計契約 (synvelis#21)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/21"
  changes:
    added:
      - type: "file"
        path: "docs/specs/media-render.md"
      - type: "file"
        path: "docs/designs/media-render-design.md"
      - type: "file"
        path: "docs/plans/media-render.md"
      - type: "file"
        path: "docs/tasks/media-render-task.md"
      - type: "file"
        path: "docs/test-specs/media-render.md"
      - type: "file"
        path: "docs/impl/ir/media-render.md"
      - type: "file"
        path: "docs/impl/exp/media-render.md"
      - type: "file"
        path: "evidence/sv21/media-render-case-ledger.md"
      - type: "file"
        path: "evidence/sv21/media-render-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/specs/media-render.md"
  implemented-by: []
  depends-on:
    - "docs/specs/media-render.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/playback-graph-design.md"
    - "docs/designs/alib-01-asset-provider-contract-design.md"
    - "docs/designs/alib-02-streaming-verified-import-design.md"
    - "docs/designs/issue-86-verified-import-wiring-design.md"
    - "docs/designs/x-recorded-verified-source-playback-export-design.md"
    - "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/designs/l1-local-wav-verified-playback-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/media-render.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
  derived-by:
    - "docs/plans/media-render.md"
    - "docs/test-specs/media-render.md"
  verifies: []
  verified-by: []
  references:
    - "docs/specs/media-render.md"
    - "evidence/sv21/media-render-case-ledger.md"
  defers: []
  deferred-by: []
---

# Media & Render Lifecycle — 設計契約 (synvelis#21)

本書は `docs/specs/media-render.md` の設計層。**文書のみ**であり、
実装は各実装 Issue が Owner。設計合意・headless fixture・製品
受入・実機受入を区別する。

## Context

上流正本は `media-import-export-offline-render-ui-diagnostics-
architecture.md`（v0.6 Approved・14 項目合意・MR-001〜028
matrix）と `synvelis-driven-ruviform-target-architecture-v07.md`
（v0.7.1）。`playback-graph`（#18）・`project-asset-persistence`
（#17）・asset-library 系・verified-import-wiring（#86）・
recorded-verified-source（#26）・tim03/tim04（#127/#128）・
recording-take-comping（#116/#118）・workspace-model（#2）・
sv-gui-04（#114）の各確定契約を本契約の一部として扱う。

## Scope

In: MR-R-01〜19 全部、MR-AC-01〜10、MR-RCP-01、29 ケースの
写像。Out: 実装コード、GUI 実装、実 device/plugin/OS 受入、
Ruviform 変更、asset-library 文書編集（#9 系 Worker と並行）。

## Components and Responsibilities

### Owner 分離（MR-R-14 の固定表）

| 責務 | Owner | Ruviform 側の該当 capability | Synvelis 側の product policy |
|---|---|---|---|
| Container/Codec/Native PCM demux/decode | Ruviform | media-io Reader/Decoder（rf#197 系） | Media Capability 表示・Declared/Decoded/Verified 区別 |
| WAV Encode/Mux | Ruviform | io-wav staged backend（rf#191 closed） | Output Format Policy・Safe Publish・Receipt |
| Generic nonRT Driver/Job/Cancellation | Ruviform | exec offline driver（rf#198）・generic job | JobOrchestrator・phase/receipt 意味論 |
| StagedFile/Filesystem | Ruviform | StagedFile/fsync/durability | Overwrite Policy・Destination 保護 |
| Probe/Dock/Clock/Bounded Data | Ruviform | probe（rf#185）・dock（#182）・ui data registry | Diagnostics Evidence 語彙・Probe budget |
| Source/Asset/Clip/Project | Synvelis | — | SourceId/AssetId/Provenance/Retention |
| Catalog Provider/Remote Revision | Synvelis | — | Metadata-first・Pinned Revision Copy |
| Import Commit | Synvelis | — | ExpectedRevision Commit・Verified Orphan |
| RenderSpec/Render Recipe | Synvelis | — | Immutable snapshot・preflight・Degraded/Reject |
| Stem/Freeze/Bounce | Synvelis | — | 非破壊 Lineage・Project Command |
| Inspector/Signal Flow 製品意味 | Synvelis | — | Evidence level・2 地点比較規則 |

原則: generic runtime に不足があれば Synvelis で恒久的重複
実装せず Ruviform Issue へ起票（rf#253/#275/#294/#308/#309
参照）。

### 主要 Component

1. **MediaResolver**（MR-R-01/02）: SourceId→SourceVersion→
   AssetId/ContentRef→Verified Blob→Decoder/Reader→nonRT
   Prefetch→RT Frame の解決 Port。`Capability`/`Identity`/
   `Provenance` を分離し、Declared/Decoded/Verified/Inferred/
   Unavailable を型で区別。Blob 欠落は Missing、旧 Prefetch の
   新世代混入を世代 key で拒否。L1 `media_resolver`・
   `verified_media_graph` が実体の一部。
2. **AssetLibrary boundary**（MR-R-03）: Catalog 正本は
   Provider、Cached Metadata は Observation。IndexCoverage/
   IndexedAt/ProviderRevision/Offline/ResultCompleteness を
   表示。Preview/Peak/Thumbnail Cache は Evict 可能・Project
   Media と別。`alib-01/02/03/04/06/07` 設計系列が正本。
3. **ImportJob**（MR-R-04）: `Pinned Fetch → Staging →
   Size/Hash/Format Verify → Durable Materialize →
   ExpectedRevision Project Commit`。段階別 Cancel/Retry、
   Verified Orphan=Recoverable、Cancel≠即時削除、Undo≠物理
   削除。`import_streams_verifies_publishes_and_commits`・
   `replay_same_import_is_idempotent`・`cancel_*`・
   `subprocess_crash_*`・`stale_revision_rejected_then_explicit_
   rebase_converges` が実体。
4. **JobOrchestrator**（MR-R-05/11）: Import/Render/Bounce/
   Freeze/Analysis/Recovery/Preview を統合。phase machine は
   `Queued→Preflight→Prepared→Running→Draining/Finalizing→
   Validating→Publishing→Completed` + `Cancelling`/`Cancelled`/
   `Failed`/`Recoverable`/`Degraded`。Window/View 寿命から
   分離、Window Close≠Job Cancel（MR-R-11）。現行 gateway
   `job.rs`（JobKind Generation/Analysis、id distinct・
   survive disconnect）と workspace `ws_a.rs`/`workspace_
   model.rs` の `close_veto`・sv-gui-04 契約が部分実体、
   Import/Render への統一は未実装。
5. **RenderSpec/RenderSnapshot**（MR-R-06/08）: Project
   Snapshot Revision・TempoMap Revision・Asset/Source Versions・
   Plugin Instance/State Capture Evidence・Graph/Automation/
   Transport・Range Anchor・Channel/Output Format・Mode・
   Tail/Preroll/Latency を不変固定。**Live≠Saved≠Active Plan≠
   Render Source**。Preflight は欠落 Plugin/Asset・未対応
   Format・外部 Sidechain/Re-RT-only を明示 Reject/Degraded。
   `l2_06_snapshot_freeze_and_immutability`・
   `sf03_render_01_same_revision_across_consumers` が実体。
6. **OfflineRenderEngine**（MR-R-07）: 時間方向 Sequential +
   明示的独立単位のみ並列化。`run_parallel` は Chunk 独立
   前提、stateful DSP へ任意時間分割を無条件適用しない。
   Preroll/Warm-up、半開 `[start,end)`、Frame Event 二重
   評価なし、Tail Policy（Fixed/UntilSilence/User/
   Unsupported/Unknown・Max Duration 必須）。
   `l2_e2e.rs` の `l2_02_*`/`l2_03_*`/`l2_04_*` test・
   `x_e2e.rs` の `x_02_*` test・rf#198 が実体。
7. **OutputPipeline**（MR-R-09/10）: Format negotiation→
   Channel/SRC/Quantize/Dither→Encode→Drain→Mux Finalize→
   Close/Sync→Independent Readback→Verify→Atomic Publish。
   Staging/Temp へ書き検証後 Destination へ Safe Publish、
   同名は Overwrite Policy+既存 File 保護、Remote FS/異
   Volume は Capability/Recovery。`l2_05_destination_policy`・
   `l2_05_fault_injection_partial_never_completed`・
   `l2_01_master_export_reopen_pcm_match`（independent
   symphonia readback）が実体。
8. **Export vs Project-derived**（MR-R-10）: Master/Stem/
   External Export は原則非変更、Bounce/Freeze/Render-in-Place
   は Verified Derived Media を明示 Command で Commit（非破壊
   Lineage、元 Source/Plugin/Automation 保全）。Freeze 中は
   「凍結状態」UI、変更時 Unfreeze/Re-freeze。Export File は
   Project Asset ではなく明示「Import rendered result」のみ
   Commit。
9. **Window/View/Job Lifetime**（MR-R-11）: Window Close・
   Inspector/Browser Selection・Workspace 変更・View 移動が
   Record/Export/Project Job を暗黙 Cancel/変更しない。現行
   `UiRuntime` CloseRequested→Job Cancel を長時間 Job に
   適用しない設計境界。View/Inspection/Selection State と
   Project Command Target を分離。Crash/Disconnect は
   Authoritative Status を変えず View 復元→Job 再接続。
   アプリ終了後の実 Job 継続は別 Capability。
10. **UI Data Transfer**（MR-R-12）: Waveform/Spectrum/Preview
    等大容量は有界 Pull/Range、JSON Push しない。DataSource
    Key に Selection/Revision/SourceVersion/GraphGeneration。
    Render/Probe/Preview Job が UI 描画を block しない。
11. **DiagnosticsEvidence**（MR-R-13）: Configured/Prepared/
    Applied/Observed/Inferred/Unavailable + Clock/Frame/
    Revision/Provenance/Loss。Probe Off/Monitor/Capture、
    全論理 Node 能力宣言だが全 PCM 常時保存しない。Session/
    Controller は Inspector View 寿命と独立、2 Inspector で
    Capture 重複なし。比較は同 Frame/Clock/GraphGeneration+
    PDC/経路 Latency 考慮。Budget 超過は Capture Loss/
    Unavailable 可視化、録音 Fail-stop と Probe Drop 許容を
    相互適用しない。異常観測と原因特定は別、原因候補は
    Confidence 付き。`player/tests/sf04_*.rs`・`pc_c*.rs`・
    `trace_c3_05*.rs` と syn-obs-01 契約が実体。
12. **PluginPrecisionCapability**（MR-R-15）: TIM-03 契約を
    正本に Realtime/Offline の min/max frames・Transport
    Event・Automation precision を `ExactWithinVerified-
    Capability`/`FrameAccurateEvents`/`BlockBoundaryOnly`/
    `DegradedWithConsent`/`Unsupported`/`Unknown` で判定。
    Offline Exact は fail-closed（`OfflineExact`/
    `OfflineUncertified`）、consent 救済なし。`tim03_*` が実体。
13. **PluginStateReceipt**（MR-R-16）: TIM-04 契約を正本に
    `Coordinated`/`Quiescent`/`BestEffort`/`Unsupported` 品質と
    Snapshot/Asset/PluginVersion/Clock/Graph 対応証拠を Recipe
    へ固定。`Save Completed`/`Plugin State Exact`/`RT Active`
    は別 Receipt。`Strict`/Exact は BestEffort/Unsupported/
    失敗を fail-closed 拒否。`tim04_*` が実体。
14. **VerifiedMedia vs Raw vs Probe**（MR-R-17）: Recording
    Writer/Journal Verified Prefix（CRC32/durable frontier）・
    Asset SHA-256 CAS・Project Save・Recovery を別正本。Probe
    lossy capture を Lossless Raw/Offline の Source-of-truth
    にしない。Finalized 以外は絶対に再生 source にならない。
    `recording/tests/vs_c.rs`・`rcd_04_*`・`render/tests/
    x_e2e.rs::x_03_identity_revision_undo_save_reload` が実体。
15. **RCP Boundary**（MR-R-18）: 既存 Offline Render/Verified
    Output/Safe Publish=実 PCM 生成 Own、RCP-01=canonical
    recipe、RCP-02=不変 Artifact Index/GC、RCP-03=Trigger/
    Async Job。Source/PluginState/Tempo/Port/Sidechain 全
    closure 固定。Ready でも Codec/Channel/Frame/Tail/Format/
    Hash/independent readback 未充足なら Exact 昇格なし。
    BestEffort/非決定性は再生成同一性を保証しない。Job
    Completion は Save/VCS/RT Active と独立。Manual/OnSave/
    OnCommit/OnShare 任意組合せ、Trigger のみの Policy 変更で
    音響 RecipeKey 不変。OnCommit=VCS Write 未実装なら
    Unsupported、Share=RequireComplete のみ保留。

## Data and Control Flow

### Import pipeline（MR-R-02/03/04）

```
Provider Catalog (正本=Server)
  → Cached Metadata (Observation・IndexCoverage/ProviderRevision)
  → Metadata-first Page/Filter/Search (未検索≠不存在)
  → User Select → Pinned Revision Copy into Project
  → Pinned Fetch → Staging → Size/Hash/Format Verify
  → Durable Materialize → ExpectedRevision Project Commit
  → Verified Blob (CAS・SHA-256) + Provenance Receipt
  → Clip Playback (MediaResolver→nonRT Prefetch→RT Frame)
```

Cancel は段階ごと意味を持ち、Verify/Publish 失敗は
Failed/Recoverable、Commit 失敗の Verified Orphan は
Recoverable であり Completed receipt を出さない。

### Render pipeline（MR-R-06〜10）

```
Project Live/Saved/Active Plan/Render Source Snapshot (別々に記録)
  → RenderSpec (immutable: revision set + plugin state evidence
     + range anchor + channel/output + mode + tail/preroll/latency)
  → Preflight (欠落/未対応/外部 sidechain/Re-RT-only → Reject/Degraded)
  → Sequential Stateful Graph (preroll/warm-up・PDC・tail policy)
  → Master/Stem/External Export tap (monitor-only plugin 除外)
  → Output: Format/SRC/Quantize/Dither → Encode → Drain
    → Mux Finalize → Close/Sync → Independent Readback
    → Verify → Atomic Safe Publish → Receipt
```

Render 中の Project 編集は実行中 Spec を変更しない。
Export File は Project Asset ではない。

### Job/Window lifetime（MR-R-05/11/12）

```
Window/View (寿命: UI session)
  ├─ Selection/Inspection/View State (Project Command Target と分離)
  └─ DataSource Key = Selection+Revision+SourceVersion+GraphGeneration
       → bounded pull/range binary (JSON push しない)

Application/Project Session (寿命: job owner)
  └─ JobOrchestrator: Queued→…→Completed + Cancelling/Failed/Recoverable
       Window Close ≠ Job Cancel (長時間 Job に close_veto を適用しない)
       Crash/Disconnect → Authoritative Status 不変 → View 復元→再接続
```

### Diagnostics（MR-R-13）

```
Node capability 宣言 (全論理 node・PCM 常時保存しない)
  → Probe Off(default)/Monitor/Capture
  → Session/Controller (Inspector View 寿命と独立・重複 capture なし)
  → Evidence: Configured/Prepared/Applied/Observed/Inferred/Unavailable
     + Clock/Frame/Revision/Provenance/Loss
  → Budget 超過 → Capture Loss/Unavailable 可視化
  → 比較: 同 Frame/Clock/GraphGeneration + PDC/経路 Latency
  → 原因候補は Confidence 付き (異常観測≠原因特定)
```

## Media capability & resolver contract

`MediaCapability` は Container/Codec/Native PCM Layout・
Declared/Decoded/Verified/Inferred/Unavailable を分離し、
Probe/Demux/Decode/Encode/Mux/RandomSeek/Preview 可能性を
個別 Capability として持つ。`MediaResolver` は SourceId→
SourceVersion→AssetId/ContentRef→Verified Blob→Decoder/
Reader→nonRT Prefetch→RT Engine Frame の解決 Port。

- Blob 欠落は `Missing`（Decoded Frame を偽造しない）。
- Blob dedup は論理 ID/Provenance の統合を意味しない。
- Source/Asset 更新時、旧 Prefetch を新世代へ混入させない
  （世代 key で拒否）。
- Native PCM/Original File を破壊せず、変換履歴
  （Import/Proxy/Render の SRC・Channel Map・Scale・Clipping/
  Dither）を保持。
- 未知値を確定値で埋めない。`Declared`（メタデータ申告）/
  `Decoded・Verified`（decode 検証済み）/`Inferred`（推定）/
  `Unavailable`（取得不能）を型で区別。

実体: `crates/synvelis-player/src/media_resolver.rs`・
`verified_media_graph.rs`、`crates/synvelis-library` の
`local.rs`/`digest.rs`/`provider.rs`/`observation.rs`。

## Import job state machine（MR-R-04）

段階: `Pinned Fetch → Staging → Size/Hash/Format Verify →
Durable Materialize → ExpectedRevision Project Commit`。

- 各段階は独立した非 RT Job。Cancel/Retry は段階ごとに意味
  を持つ（Fetch cancel=途中破棄、Verify cancel=staging 保持で
  再開可、Commit cancel=Published で park）。
- Verify/Publish 失敗は `Failed`/`Recoverable`。Cancel≠即時
  削除（staging は回収 policy に従う）。
- Commit 失敗時の Verified Orphan は `Recoverable`（物理
  Blob は検証済み・Project 参照のみ未 commit）。
- Commit 後の Undo は Project 変更であり物理 Media 即削除
  ではない。
- 未検証 Clip を Project 正本として先行公開しない
  （Pending Ghost/Job として明示）。

実体: `crates/synvelis-player/tests/verified_import.rs`・
`import_recovery` 系・`crates/synvelis-library/tests/import.rs`・
`issue86_verified_import_wiring.rs`。

## Render contract（MR-R-06/07/08）

### RenderSpec fields（immutable）

- Project Snapshot Revision・Saved Revision・Active Plan
  Revision（別々に記録）
- TempoMap Revision・Asset/Source Versions
- Plugin Instance ID + State Capture Evidence（quality=
  Coordinated/Quiescent/BestEffort/Unsupported）
- Graph/Automation/Transport Configuration
- Range Anchor（Beat/Media/Clip-local・半開 `[start,end)`）
- Channel Layout・Output Format・Sample Rate・Bit Depth
- Render Mode（Master/Stem/Bounce/Freeze/Render-in-Place/
  External Export）
- Tail Policy（Fixed/UntilSilence{閾値,保持,最大}/User/
  Unsupported/Unknown・無限は Max Duration 必須）
- Preroll/Warm-up・PDC/Latency Policy・Sidechain closure

### Preflight

- 欠落 Plugin/Asset・未対応 Output Format・外部 Sidechain・
  Realtime-only Plugin → 明示 `Reject` または明示 Policy の
  `Degraded`。
- Source Missing の無断ゼロ埋め成功を既定禁止、明示 silence
  substitute は Receipt に記録。

### Stateful-safe execution

- Graph/Time/Event 評価意味論は Live と共用、Plugin Instance
  は独立。
- `OfflineDriver::run_parallel` の結果一致は Chunk 独立が
  前提。Delay/Reverb/Compressor/Instrument 等の状態を持つ
  DSP に任意時間 Range の分割並列を無条件適用しない。
- 初期実装は時間方向 Sequential Execution + 明示可能な独立
  単位に限った並列化。
- Stateful Graph の並列独立解析・cache 作業と stateful render
  を混同しない。

実体: `crates/synvelis-render/tests/l2_*.rs`・`x_e2e.rs`・
`sf03_render.rs`・`tim04_plugin_state.rs`・rf#198。

## Output pipeline & publish（MR-R-09）

1. **Format negotiation**: Container/Codec/Sample Rate/Bit
   Depth/Channels/Metadata/Seekability を検証。
2. **Conversion**: PCM→整数 PCM は Clipping/Level・Rounding・
   Dither/Noise Shape・Channel Map・SRC・Endianness・Metadata
   を規則化。Dither は最終量子化の単一段階のみ。
3. **Encode**: Ruviform io-wav backend（rf#191）。
4. **Drain**: Encoder の残留 frame を確定。
5. **Mux Finalize**: Header/Index を確定。
6. **Close/Sync**: File を閉じ fsync/durability。
7. **Independent Readback**: 別 Reader で Reopen して PCM/
   frame 数/hash を検証（Writer の自己申告で合格にしない）。
8. **Verify**: 期待値との照合。
9. **Atomic Safe Publish**: Staging→Destination へ rename/
   copy、同名は Overwrite Policy+既存 File 保護/Backup を
   事前確認。
10. **Receipt**: 成功時のみ `Completed` receipt。Failed
    write は `Completed` を出さない。

Remote FS・異ボリューム・部分 Finalization は Capability/
Recovery として管理。未完成出力を成功扱いしない。

実体: `crates/synvelis-render/tests/l2_e2e.rs` の
`l2_05_*`/`l2_01_master_export_reopen_pcm_match` test
（independent readback）。

## Export vs project-derived（MR-R-10）

- **Master/Stem/External Export**: 原則 Project 非変更。
  RenderSpec snapshot のみ使用、Project 参照は commit しない。
- **Bounce/Freeze/Render-in-Place**: Verified Derived Media を
  明示 Project Command で Commit。Original Source/Plugin/
  Automation/参照を保全（非破壊 Lineage）。Freeze 中の
  Plugin/Parameter UI は「凍結状態」を明示、変更時は
  Unfreeze/Re-freeze を要求。
- Verified Blob 完成と Project 参照 Commit 成功は別 receipt。
- Export File は Project Asset ではなく、明示的な「Import
  rendered result」等があるときのみ Commit。

現状: Master Export は実装済み（L2）、Stem/Bounce/Freeze/
Render-in-Place は未実装 → MR-017/MR-022 は NotRun。

## Failure taxonomy（横断）

| 失敗種別 | 分類 | 回復 | 実体 fixture |
|---|---|---|---|
| Import fetch 失敗 | Failed/Recoverable | staging 回収・再試行 | `cancel_mid_stream_*` |
| Import verify 失敗（hash/format） | Failed | staging 保持・再検証 | `sha256_*` |
| Import commit 失敗 | Failed/Recoverable | Verified Orphan 回収 | `cancel_at_commit_boundary_*` |
| Render preflight reject | Failed（Reject） | spec 修正・再実行 | `l2_e2e.rs::l2_04_unknown_tail_and_support_rejected` |
| Render stateful 並列不可 | Unsupported | sequential に限定 | `l2_e2e.rs::l2_03_stateful_inserts_block_invariant` |
| Output encode/mux/finalize 失敗 | Failed/Recoverable | staging 保護・partial 非公開 | `l2_e2e.rs::l2_05_fault_injection_partial_never_completed` |
| Publish 失敗（rename/copy/fsync） | Failed/Recoverable | destination 保護・retry | `l2_e2e.rs::l2_05_destination_policy` |
| Independent readback 不一致 | Failed | publish 済みでも receipt 不発行 | `l2_e2e.rs::l2_01_*`（成功系） |
| Plugin state BestEffort/Unsupported | Degraded/Unsupported | Strict は fail-closed | `tim04_*` |
| Plugin precision OfflineExact 不可 | Unsupported | consent 救済なし | `tim03_*` |
| Remote provider offline/stale | Stale/Unavailable | cached metadata で代替・明示 | `types.rs`/`cache.rs` |
| Window close | （job に影響なし） | job 継続・view 復元 | `close_veto` |
| Probe budget 超過 | Unavailable（loss 記録） | capture loss 可視化 | `sf04_s8_*` |

## MR-001〜028 + MR-RCP-01 design mapping

各ケースの詳細（fixture・oracle・status・証拠）は
`evidence/sv21/media-render-case-ledger.md` を正本とする。
ここでは design 上の被覆方針のみ示す。

| Case | 要求 | Design 被覆 | 実装状態 |
|---|---|---|---|
| MR-RCP-01 | snapshot pin→RCP-02 readback・Exact 昇格条件 | RenderSpec pin・independent readback・Exact cache 境界 | RCP-01/02/03 未実装・L2 readback が先行実体 |
| MR-001 | Local import→Verified Blob | ImportJob+MediaResolver | 実装済（headless） |
| MR-002 | Native PCM 変換履歴 | MediaCapability 変換履歴 | Pass(部分)：identity 保持・typed reject・SRC は Deferred |
| MR-003 | Unsupported codec typed reject | Capability 表示 | 実装済（headless） |
| MR-004 | dedup+別 ID/provenance | ContentRef+Provenance | 実装済（headless） |
| MR-005 | Remote metadata-first | AssetLibrary Catalog | 実装済（headless mock・実 server NotRun） |
| MR-006 | Stale/Offline 区別 | Observation Level | 実装済（headless・実 provider NotRun） |
| MR-007 | Preview/cache 分離 | PreviewPlan/CacheKey | 実装済（ALIB-03 headless） |
| MR-008 | pinned revision 保護 | Pinned Copy+strong pin | 実装済（headless） |
| MR-009 | import cancel/hash 回復 | ImportJob state machine | 実装済（headless・強 fixture） |
| MR-010 | commit target 再検証 | ExpectedRevision Commit | 実装済（headless） |
| MR-011 | RT prefetch underrun 診断 | MediaResolver nonRT | 実装済（headless・実 device NotRun） |
| MR-012 | Master PCM WAV export | OutputPipeline+io-wav | 実装済（headless・independent readback） |
| MR-013 | unsupported output preflight | Format negotiation | 実装済（headless・WAV PCM のみ） |
| MR-014 | range/loop/tempo 半開 | RenderSpec range anchor | 実装済（headless） |
| MR-015 | preroll/pdc/tail alignment | RenderSpec tail/preroll | Pass(部分)：master のみ・stem 未実装 |
| MR-016 | stateful sequential | OfflineRenderEngine | 実装済（headless・実 plugin NotRun） |
| MR-017 | stem/freeze/bounce | Render Mode+Lineage | **NotRun**（未実装） |
| MR-018 | final PCM/dither/SRC | Output conversion | Pass(部分)：PCM path・dither/SRC Deferred |
| MR-019 | finalize/publish failure | OutputPipeline | 実装済（headless fault injection・実 disk NotRun） |
| MR-020 | export snapshot freeze | RenderSpec immutability | 実装済（headless） |
| MR-021 | plugin missing/best-effort | Preflight+PluginStateReceipt | Pass(部分)：mock・実 plugin NotRun |
| MR-022 | bounce undo/unfreeze | Project-derived command | **NotRun**（未実装） |
| MR-023 | window close≠job stop | JobOrchestrator+Lifetime | Pass(部分)：gateway job+close_veto・統一 orchestrator 未実装 |
| MR-024 | 2 inspector/probe 非重複 | Diagnostics Session 独立 | Pass(部分)：headless・実 dock GUI NotRun |
| MR-025 | probe off/monitor/capture | Probe capability | Pass(部分)：headless・実 device NotRun |
| MR-026 | 2 probe PDC/clock 比較 | Diagnostics 比較規則 | Pass(部分)：headless・実 device NotRun |
| MR-027 | source→param→observation trace | Diagnostics Evidence | Pass(部分)：headless・実 plugin NotRun |
| MR-028 | unsafe metadata/path traversal | Library 境界検証 | Pass(部分)：headless・実 credential/SSRF NotRun |

## Risks

- **R-1 Owner 境界の曖昧さ**: Ruviform generic と Synvelis
  policy の分界が実装時に崩れると重複実装が発生。→ MR-R-14
  の表を API/Fixture/Issue で照合、generic 不足は Ruviform
  Issue へ起票。
- **R-2 Stateful 並列の誤適用**: `run_parallel` を stateful
  DSP に無条件適用すると結果が壊れる。→ MR-R-07 で時間方向
  Sequential を初期既定、並列は明示独立単位のみ。
- **R-3 Exact 昇格の過剰**: Ready render や headless Pass を
  Exact/製品受入に繰り上げる誤り。→ MR-R-18/MR-R-19 で
  Codec/Channel/Frame/Tail/Format/Hash/independent readback
  を全て要求、evidence 語彙で区別。
- **R-4 Window/Job 寿命の混同**: 現行 `UiRuntime`
  CloseRequested→Job Cancel を長時間 Job に適用すると Window
  Close で job が死ぬ。→ MR-R-11 で設計境界を固定、統一
  orchestrator は未実装として明示。
- **R-5 Probe/録音 budget の相互適用**: 録音 Fail-stop を
  Probe に、Probe Drop 許容を録音に適用すると安全性が壊れる。
  → MR-R-13 で相互適用禁止を明示。
- **R-6 asset-library 並行作業との衝突**: #9 系 Worker が
  `docs/specs/alib-*.md` 等を編集中。本 slice は参照
  のみで編集しない。

## Deferred Design Concerns

spec の Deferred Design Concerns を参照。design 層で追加する
未確定事項:

- Unified Job Orchestrator の具体 API・phase 永続化・
  Window 再接続 protocol | #23・実装 Issue | 現行 gateway
  job registry は Generation/Analysis のみ、Import/Render 未統合。
- Stem tap の具体 node 配置・sidechain closure の graph
  表現 | 実装 Issue | MR-017 が NotRun。
- Bounce/Freeze の Undo/Redo・Lineage 表現 | 実装 Issue |
  MR-022 が NotRun。
- Probe の実 device/PDC/clock 対応 | 実機 lane | MR-024〜027
  が Pass(部分)。
- Export File の「Import rendered result」UI/Command | 実装
  Issue | MR-010 の延長。

## Verification notes

- 本 slice は docs/ 設計契約 + evidence 台帳のみ。実装・
  GUI・実 device/plugin/OS 受入は対象外。
- `mise run fmt-check`・doc lint・audit・route・review-
  protocol を `evidence/sv21/media-render-verification.md`
  に記録。
- headless fixture の Pass は `evidence/sv21/media-render-
  case-ledger.md` で「Pass(部分)」として記録し、実機/GUI/
  Plugin/OS 受入に繰り上げない。
- `doc-driven-dev-graph` route が pre-existing の
  `artifact-graph`/`broken-relation` blocker を報告する場合は
  baseline として記録し、本 slice の範囲外として修復しない。
