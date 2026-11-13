---
id: "TSPEC-mr20261114mediaRenderC"
type: "test-spec"
status: "approved"
title: "Media / Import / Export / Offline Render / UI / Diagnostics — Media & Render Lifecycle テスト仕様 (synvelis#21)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/21"
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
    - "docs/designs/media-render-design.md"
    - "docs/specs/media-render.md"
    - "docs/test-specs/playback-graph.md"
    - "docs/test-specs/l1-e2e-local-wav-verified-playback.md"
    - "docs/test-specs/l2-master-sequential-offline-wav-export.md"
    - "docs/test-specs/x-e2e-recorded-source-playback-export.md"
    - "docs/test-specs/issue-86-verified-import-wiring.md"
    - "docs/test-specs/recording-take-comping.md"
    - "docs/test-specs/tim03-process-boundary.md"
    - "docs/test-specs/tim-04-plugin-state-snapshot.md"
    - "docs/test-specs/workspace-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/media-render-design.md"
  derived-by: []
  verifies:
    - "docs/designs/media-render-design.md"
    - "docs/specs/media-render.md"
  verified-by: []
  references:
    - "evidence/sv21/media-render-case-ledger.md"
    - "evidence/sv21/media-render-verification.md"
  defers: []
  deferred-by: []
---

# Media & Render Lifecycle — テスト仕様 (synvelis#21)

## Purpose

Issue #21 の MR-RCP-01・MR-001〜028 の 29 受入ケースについて、
「何を・どの fixture/oracle で・どの status で検証するか」を
固定する。本 slice は docs/ 設計契約 + evidence 台帳のみ。
実装・GUI・実 device/plugin/OS 受入は対象外。

## Feature

Media & Render Lifecycle の設計契約が、MR-R-01〜19 と
MR-AC-01〜10 を満たし、29 ケースが既存 fixture/Issue/Owner/
evidence へ一貫して写像されること。

## Rules

### MR-T-R-01 要件網羅

- spec の MR-R-01〜19 全てが design・plan・test-spec・
  evidence で参照される。
- MR-AC-01〜10 が acceptance criteria として spec に存在し、
  evidence ledger で被覆状況が記録される。

### MR-T-R-02 ケース写像

- MR-RCP-01・MR-001〜028 の全 29 ケースが case ledger に
  存在し、fixture・oracle・Owner・status・証拠を持つ。
- 各ケースの status は `Pass`/`Fail`/`Unsupported`/`NotRun`/
  `Pass(部分)` のいずれかで、実行した検証と一致する。

### MR-T-R-03 Owner 境界

- Ruviform generic（media-io/io-wav/exec/probe/dock/ui-data）
  と Synvelis DAW policy（project/source/asset/import/render/
  job/inspector/signal-flow）の Owner 境界が design・plan・
  evidence で一貫する。
- generic 不足は Synvelis 重複実装ではなく Ruviform Issue
  への起票として記録される。

### MR-T-R-04 Evidence Honesty

- headless fixture の Pass は `Pass(部分)` として記録され、
  実機/GUI/Plugin/OS 受入に繰り上げない。
- 未実装（stem/bounce/freeze/orchestrator/RCP）は `NotRun`。
- 設計レビュー Pass と実測 Pass を区別する。
- `Pass`/`Fail`/`Unsupported`/`NotRun` は実際に実行した
  検証のみを反映する。

### MR-T-R-05 横断契約

- #127（TIM-03）・#128（TIM-04）・#116/#86（VS-C/ALIB-02）・
  RCP-00〜F01（#289〜#297）の横断写像が case ledger に存在する。
- Verified Media≠Project Commit・independent readback・
  no fake lossless・stateful sequential・window/job 分離・
  probe/recording budget 分離が一貫して適用される。

### MR-T-R-06 文書鎖整合

- spec→design→plan→task→test-spec→impl-ir/exp→evidence の
  依存が front matter relations で一貫する。
- index が再生成され、lint/audit/route が実行・記録される。

## Case Map（概要 — 詳細は evidence/sv21/media-render-case-ledger.md）

Fixture 表記は `file::test_fn`。`l1_*`/`l2_*`/`x_*`/`sf04_*`/
`tim03_*`/`tim04_*` は共有 test file 内の test 関数名。

| Case | 要求 | Fixture/Oracle | Owner | Status |
|---|---|---|---|---|
| MR-RCP-01 | snapshot pin→RCP-02 readback・Exact 境界 | RenderSpec pin・independent readback（設計）・RCP Issue・先行実体 `l2_e2e.rs::l2_01_*` | Synvelis #290〜#297 + Ruviform #289 | NotRun |
| MR-001 | Local import→Verified Blob | `l1_e2e.rs::l1_01_*`・`verified_import.rs::sha256_*`・`import.rs::import_streams_*` | #24/#17/#86 | Pass(部分) |
| MR-002 | Native PCM 変換履歴 | `l1_e2e.rs::l1_05_*`・typed reject | #24 | Pass(部分) |
| MR-003 | Unsupported codec reject | `l1_e2e.rs::l1_04_*`・`verified_import.rs::sha256_missing_*`・`opaque_*` | #24/#86 | Pass(部分) |
| MR-004 | dedup+別 ID/provenance | `import.rs::same_bytes_*`・`same_import_id_*`・`project.rs::imported_via_*`・`foreign_provider_ref_*` | #24/#17 | Pass(部分) |
| MR-005 | Remote metadata-first | `local.rs`/`git.rs`/`webdav.rs` paging・browse・`project.rs::snapshot_catalog_*`・alib | #9/#85〜#91 | Pass(部分) |
| MR-006 | Stale/Offline 区別 | `types.rs::observations_*`・`cache.rs`・`git.rs::refresh_*`・`webdav.rs::offline_*` | #9/#85〜#91 | Pass(部分) |
| MR-007 | Preview/cache 分離 | `cache.rs::preview_range_*`・`local.rs::preview_*`・`git.rs::preview_*`・`webdav.rs::preview_*`・ALIB-03 | #9/#87 | Pass(部分) |
| MR-008 | pinned revision 保護 | `local.rs::changed_content_*`・`renamed_*`・`git.rs::force_push_*`・`pinned_commit_*`・`webdav.rs::open_pinned_*` | #9/#85〜#91 | Pass(部分) |
| MR-009 | import cancel/hash 回復 | `import.rs::cancel_*`・`subprocess_crash_*`・`import_recovery.rs`・`verified_import.rs::sha256_*` | #24/#86 | Pass(部分) |
| MR-010 | commit target 再検証 | `import.rs::stale_revision_*`・`missing_lane_*`・`wrong_track_*`・`import_recovery.rs::*`・`verified_import.rs::undo_then_*` | #24/#17 | Pass(部分) |
| MR-011 | RT prefetch underrun | `l1_e2e.rs`（underrun/EOF・`l1_03_*`） | #24 | Pass(部分) |
| MR-012 | Master PCM WAV export | `l2_e2e.rs::l2_01_*`・`x_e2e.rs::x_01_*`・io-wav・independent readback | #25 | Pass(部分) |
| MR-013 | unsupported output preflight | `l2_e2e.rs::l2_04_unknown_*`・`l2_07_*`・`tim04_plugin_state.rs::strict_export_*` | #25 | Pass(部分) |
| MR-014 | range/loop/tempo 半開 | `l2_e2e.rs::l2_02_*`・`x_e2e.rs::x_02_*` | #25/#26 | Pass(部分) |
| MR-015 | preroll/pdc/tail | `l2_e2e.rs::l2_04_*`（master のみ） | #25 | Pass(部分) |
| MR-016 | stateful sequential | `l2_e2e.rs::l2_03_*`・`sf03_render.rs::sf03_render_02_*`・rf#198 | #25 | Pass(部分) |
| MR-017 | stem/freeze/bounce | （未実装） | #23 配下 | **NotRun** |
| MR-018 | final PCM/dither/SRC | `l2_e2e.rs::l2_01_*`・`l2_07_*`・`tim04_plugin_state.rs::standard_*`・dither/SRC Deferred | #25/#20 | Pass(部分) |
| MR-019 | finalize/publish failure | `l2_e2e.rs::l2_05_*`・`store_publish.rs`・syn-rf305 | #25/#82 | Pass(部分) |
| MR-020 | export snapshot freeze | `l2_e2e.rs::l2_06_*`・`sf03_render.rs::sf03_render_01_*`・`x_e2e.rs::x_03_*` | #25 | Pass(部分) |
| MR-021 | plugin missing/best-effort | `tim04_plugin_state.rs::strict_*`・`standard_*`・`tim03_process_boundary.rs::tim03_pb_*`（mock） | #127/#128 | Pass(部分) |
| MR-022 | bounce undo/unfreeze | （未実装） | #23 配下 | **NotRun** |
| MR-023 | window close≠job stop | `job.rs::*`・`ws_a.rs`/`workspace_model.rs` `close_veto`・`l2_e2e.rs` cancel | #114/#23 | Pass(部分) |
| MR-024 | 2 inspector/probe 非重複 | `sf04_probe_acceptance.rs::sf04_probe_same_key_*`・`ws_a.rs`・`sv113_dock.rs` | #107 系/#114 | Pass(部分) |
| MR-025 | probe off/monitor/capture | `sf04_s8_probe_faults.rs::sf04_s8_*`・`sf04_s3_probe_discontinuity.rs::sf04_s3_probe_modes_*` | #107 系 | Pass(部分) |
| MR-026 | 2 probe PDC/clock 比較 | `sf04_s7_probe_compare.rs::sf04_s7_*` | #107 系 | Pass(部分) |
| MR-027 | source→param→observation trace | `sf04_s2_probe_trace_bridge.rs::sf04_s2_*`・`sf04_probe_adapter.rs::sf04_probe_*`・`pc_c6_oracle_*`・`trace_c3_05*`・`pc_c4_0*_observation*` | #107 系 | Pass(部分) |
| MR-028 | unsafe metadata/path traversal | `local.rs::traversal_*`・`symlink_*`・`project.rs::hostile_*`・`webdav.rs::url_parse_*`・`cross_origin_*`・`credentials_*`・`types.rs::opaque_*` | #9/#85〜#91 | Pass(部分) |

## Examples

### EX-1: MR-012 Master PCM WAV Export（Pass(部分)）

- **Given**: L2 fixture が local WAV source を import→
  verified blob→clip→master mix として構成済み。
- **When**: `l2_e2e.rs::l2_01_master_export_reopen_pcm_match`
  を headless で実行。
- **Then**: 実 Master PCM WAV が出力され、独立 Reader
  （symphonia）で reopen して PCM/sample frame/provenance を
  検証。Writer の自己申告で合格にしない。
- **Status**: Pass(部分) — headless・実 device/OS NotRun。

### EX-2: MR-RCP-01 Render Checkpoint（NotRun）

- **Given**: RenderSpec が snapshot pin・plugin state evidence・
  range anchor・output format を固定済み。
- **When**: RCP-01/02/03 が実装され、snapshot pin 付き
  Offline Render output を RCP-02 independent readback で
  検証。
- **Then**: lossy probe・partial artifact・unknown source・
  plugin state を Exact Cache に流用しない。Ready でも
  Codec/Channel/Frame/Tail/Format/Hash/readback 未充足なら
  Exact 昇格なし。
- **Status**: NotRun — RCP 系 Issue（#290〜#297）が未実装。

### EX-3: MR-023 Window Close vs Job（Pass(部分)）

- **Given**: gateway `job.rs` が job id distinct・disconnect
  survive を検証、workspace `ws_a.rs`/`workspace_model.rs` の
  `close_veto` が window close の job cancel 要求を headless
  で確認（sv-gui-04 契約）。
- **When**: Window Close を実行。
- **Then**: 長時間 Job（import/render/export）は暗黙 cancel
  されない設計境界が確認される。View/Selection State と
  Project Command Target は分離。
- **Status**: Pass(部分) — unified orchestrator 未実装・
  実 GUI/OS NotRun。

## Guarantees

- 全 29 ケースが case ledger に存在し、status が実行した
  検証と一致する。
- headless Pass が実機/GUI/Plugin/OS 受入に繰り上げられない。
- 未実装領域（stem/bounce/freeze/orchestrator/RCP）が
  `NotRun` として明示される。
- Ruviform generic / Synvelis DAW policy の Owner 境界が
  一貫する。
- #127/#128・#116/#86・RCP-00〜F01 の横断写像が存在する。

## Non-goals

- 実装コードの作成・変更。
- GUI・実 device・実 plugin・実 OS での受入。
- asset-library 文書の編集（#9 系 Worker と並行）。
- 実 PCM・実 Plugin・実機 PASS の宣言。
- stem/bounce/freeze/unified orchestrator/RCP の実装。

## Risk

- **R-TS-1 status の誤分類**: headless Pass を `Pass` と
  記録すると実機受入と混同される。→ `Pass(部分)` を使用し、
  実機/GUI/Plugin/OS は別 status として明示。
- **R-TS-2 fixture の陳腐化**: 参照する fixture 名が実装と
  乖離する。→ case ledger に fixture path を明記し、lint/
  audit で link を検証。
- **R-TS-3 横断契約の漏れ**: #127/#128・#116/#86・RCP の
  写像が漏れる。→ MR-T-R-05 で必須化。
