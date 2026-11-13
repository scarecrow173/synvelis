---
id: "PLAN-mr20261114mediaRenderA"
type: "plan"
status: "approved"
title: "Media / Import / Export / Offline Render / UI / Diagnostics — Media & Render Lifecycle 実装計画 (synvelis#21)"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/media-render-design.md"
  derived-by:
    - "docs/tasks/media-render-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/media-render.md"
  references: []
  defers: []
  deferred-by: []
---

# Media & Render Lifecycle — 実装計画 (synvelis#21)

本計画は `docs/specs/media-render.md` + `docs/designs/
media-render-design.md` の実装分解。**本 slice は docs/ 設計
契約 + evidence 台帳のみ**であり、実装は #23 配下の各実装
Issue が Owner。本 plan は「何をどの Issue/Fixture で満たす
か」の対応表と、未実装領域の後続 Issue への委譲を固定する。

## Goal

Issue #21 の 29 受入ケース（MR-RCP-01・MR-001〜028）と
MR-AC-01〜10 を、既存 Synvelis/Ruviform fixture・Issue・
Receipt へ写像し、Owner 境界と evidence 語彙を固定する。
設計完了を実装完了・実機受入とみなさない。

## Scope

### In Scope（本 slice）

- 7 文書鎖（spec/design/plan/task/test-spec/impl-ir/impl-exp）
  + `evidence/sv21/`（case ledger + verification）
- MR-001〜028・MR-RCP-01 の fixture/Issue/Owner/status 写像
- Ruviform generic / Synvelis DAW policy の Owner 境界表
- #127（TIM-03）・#128（TIM-04）・#116/#86（VS-C/ALIB-02）・
  RCP-00（#289）〜RCP-F01（#297）の横断写像
- index 再生成・docs 検証・review-protocol・PR

### Out of Scope

- 実装コード（`crates/`・`Cargo.toml`・`Cargo.lock`・`mise.toml`・
  CI・Ruviform）
- GUI 実装・実 device/plugin・実 OS 受入
- asset-library 文書編集（#9 系 Worker と並行・参照のみ）
- 実 PCM・実 Plugin・実機 PASS の宣言

## Upstream

- `docs/specs/media-render.md`（MR-R-01〜19・MR-AC-01〜10）
- `docs/designs/media-render-design.md`（component・state
  machine・failure taxonomy・MR mapping・Owner 表）
- `docs/designs/media-import-export-offline-render-ui-diagnostics-
  architecture.md`（v0.6 Approved baseline）
- `docs/designs/synvelis-driven-ruviform-target-architecture-v07.md`
- 確定済み縦断契約: l1/l2/x/issue-86/alib/vs-c/rcd-02/
  recording-take-comping/tim03/tim-02/tim-04/playback-graph/
  project-persistence/workspace-model/sv-gui-04/sf-04/syn-obs-01/
  syn-rf305

## File Map

| File | 種別 | 状態 |
|---|---|---|
| `docs/specs/media-render.md` | spec | 本 slice で作成 |
| `docs/designs/media-render-design.md` | design | 本 slice で作成 |
| `docs/plans/media-render.md` | plan | 本 slice で作成 |
| `docs/tasks/media-render-task.md` | task | 本 slice で作成 |
| `docs/test-specs/media-render.md` | test-spec | 本 slice で作成 |
| `docs/impl/ir/media-render.md` | impl record | 本 slice で作成 |
| `docs/impl/exp/media-render.md` | experiment record | 本 slice で作成 |
| `evidence/sv21/media-render-case-ledger.md` | evidence | 本 slice で作成 |
| `evidence/sv21/media-render-verification.md` | evidence | 本 slice で作成 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | index | 再生成のみ |

## Implementation Streams（Owner 別）

本 slice は実装を行わない。以下は「どの実装 Issue がどの MR
ケースを満たすか」の対応表。

### Stream A: Synvelis Media/Import/Project（MR-001〜010）

| MR | Owner Issue | 現状 | 後続 |
|---|---|---|---|
| MR-001〜004 | #24（L1・closed）+ #17/#86 | headless Pass(部分) | 実機・実 codec 拡張 |
| MR-005〜010 | #9/#85〜#91（asset-library） | headless Pass(部分)・実 server NotRun | 実 provider・#9 系 |

### Stream B: Synvelis Offline Render/Export（MR-011〜022）

| MR | Owner Issue | 現状 | 後続 |
|---|---|---|---|
| MR-011〜016 | #25（L2・closed）+ #20 | headless Pass(部分) | 実 plugin・stem |
| MR-017 | 未割当（#23 配下） | **NotRun** | stem/bounce/freeze Issue |
| MR-018 | #25+#20 | Pass(部分)：PCM path・dither/SRC Deferred | SRC/dither 実装 |
| MR-019〜020 | #25（closed） | headless Pass(部分) | 実 disk/FS・実機 |
| MR-021 | #127/#128 | Pass(部分)：mock | 実 plugin matrix |
| MR-022 | 未割当（#23 配下） | **NotRun** | bounce/freeze Issue |

### Stream C: Synvelis Job/UI/Diagnostics（MR-023〜028）

| MR | Owner Issue | 現状 | 後続 |
|---|---|---|---|
| MR-023 | #114/#23 | Pass(部分)：gateway job+close_veto | unified orchestrator |
| MR-024〜027 | #107 系・#114 | Pass(部分)：headless | 実 dock GUI・実 device |
| MR-028 | #9/#85〜#91 | Pass(部分)：headless | 実 credential/SSRF |

### Stream D: RCP Render Checkpoint（MR-RCP-01）

| Case | Owner Issue | 現状 | 後続 |
|---|---|---|---|
| MR-RCP-01 | synvelis #290〜#297（RCP-01〜F01）+ ruviform #289 | **NotRun**（RCP 未実装） | RCP 系 Issue |

### Stream E: Ruviform generic capability

| 責務 | Ruviform Issue | 状態 |
|---|---|---|
| WAV Encode/Mux | rf#191 | closed |
| Reader/Decoder | rf#197 | closed |
| Sequential Offline Driver | rf#198 | closed |
| Probe/Dock/Clock/UI data | rf#181/#182/#184/#185/#186/#187/#188/#189/#190/#196/#282/#308/#309 | closed |
| Generic job/cancellation・bounded data | rf#253/#275/#294 | open |

## Dependency Graph

```
spec (media-render.md)
  └─ design (media-render-design.md)
       └─ plan (media-render.md) ── task (media-render-task.md)
            └─ test-spec (media-render.md) ── impl-ir/exp ── evidence/sv21/
```

実装依存（本 slice 外）:

```
#24/#25/#26 (closed: L1/L2/X headless)
  → #9/#85〜#91 (asset-library: remote/preview)
  → #127/#128 (tim03/tim04: plugin precision/state)
  → #289〜#297 (RCP: render checkpoint)
  → #23 (execution plan: stem/bounce/freeze/orchestrator)
  → 実機 lane (GUI-L2/EXT-L3: 実 device/plugin/OS)
```

## Risk Register

| Risk | 影響 | 対策 |
|---|---|---|
| Owner 境界崩壊（Ruviform/Synvelis 重複） | 重複実装・保守コスト | MR-R-14 表を API/Fixture/Issue で照合、generic 不足は Ruviform Issue |
| Stateful 並列誤適用 | render 結果破壊 | MR-R-07 sequential 既定・並列は明示独立単位 |
| Exact 昇格過剰 | 偽の完了主張 | MR-R-18/19 全条件要求・evidence 語彙 |
| Window/Job 寿命混同 | Window close で job 死亡 | MR-R-11 設計境界・orchestrator 未実装を明示 |
| Probe/録音 budget 相互適用 | 安全性破壊 | MR-R-13 相互適用禁止 |
| asset-library 並行衝突 | merge conflict | 参照のみ・編集しない |

## Verification Plan

本 slice の検証（docs/ 設計契約のみ）:

1. `mise run fmt-check` — Markdown/構成ファイル整形
2. `node .agents/skills/doc-status/scripts/doc_status.js lint
   --type {spec,design,plan,task,test-spec} --json`
3. `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
   index apply`（触った dir の index 再生成）
4. `node .agents/skills/impl-doc/scripts/audit_impl_record.js
   --json`・`audit_experiment_log.js --json`
5. `node .agents/skills/doc-status/scripts/audit_docs.js
   --type all --json`
6. `node .agents/skills/doc-driven-dev-graph/scripts/
   route_graph.js --focus media-render --json`
7. `review-protocol` — 設計契約のレビュー（blocker なしを承認）

実装検証は対象外（実装 Issue が Owner）。headless fixture の
Pass は evidence ledger で「Pass(部分)」として記録し、実機/
GUI/Plugin/OS 受入に繰り上げない。

## Rollback

- 本 slice は docs/ + evidence/ のみ。revert で完全に戻る。
- index 再生成は生成物のため、revert 時に再生成で整合。
- 実装への影響なし（コード変更なし）。

## Task Breakdown

| Task | 内容 | 状態 |
|---|---|---|
| T-1 | spec 作成（MR-R-01〜19・MR-AC-01〜10） | 本 slice |
| T-2 | design 作成（component・state machine・failure・MR mapping・Owner 表） | 本 slice |
| T-3 | plan 作成（file map・stream・dependency・risk・verification） | 本 slice |
| T-4 | task 作成（done-when・verification） | 本 slice |
| T-5 | test-spec 作成（MR-T-R・case map・example） | 本 slice |
| T-6 | impl-ir/exp 作成 | 本 slice |
| T-7 | evidence/sv21/ case ledger + verification | 本 slice |
| T-8 | index 再生成・docs 検証・review-protocol | 本 slice |
| T-9 | commit・push・PR・worker_done | 本 slice |

後続（本 slice 外・実装 Issue）:

- T-10 stem/bounce/freeze/render-in-place（#23 配下・MR-017/022）
- T-11 unified job orchestrator（#114/#23・MR-023）
- T-12 RCP-01/02/03（#290〜#297・MR-RCP-01）
- T-13 実 plugin/OS/GUI 受入（実機 lane・MR-021/024〜027）
