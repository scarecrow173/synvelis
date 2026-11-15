---
id: "PLAN-034XJDnMwnWmt80Y8ExsR4"
type: "plan"
status: "approved"
title: "RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — 文書チェーン・検証計画（synvelis#120）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-f02-monitoring-overdub.md"
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/rcd-f02-monitoring-overdub.md"
    - "docs/test-specs/rcd-f02-monitoring-overdub.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-f02-monitoring-overdub.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  defers: []
  deferred-by: []
---

# RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — 文書チェーン・検証計画（synvelis#120）

## 目的

Issue #120 が必須化する ADR 合意（UX Matrix・Feedback Risk・Capture Handle Retention・Undo/Recovery・Privacy）を、`spec → design → plan → task → test-spec → adr → impl/ir + impl/exp` の canonical 文書チェーンと `evidence/sv120/` の決定・検証記録として固定する。**docs-only slice であり、実装・実 OS 受入を主張しない。**

## スコープ

### 対象（編集可）

- `docs/specs/rcd-f02-monitoring-overdub.md`
- `docs/designs/rcd-f02-monitoring-overdub-design.md`
- `docs/plans/rcd-f02-monitoring-overdub.md`（本書）
- `docs/tasks/rcd-f02-monitoring-overdub.md`
- `docs/test-specs/rcd-f02-monitoring-overdub.md`
- `docs/adr/rcd-f02-monitoring-overdub.md`
- `docs/impl/ir/rcd-f02-monitoring-overdub.md`
- `docs/impl/exp/rcd-f02-monitoring-overdub.md`
- `evidence/sv120/`（ADR 決定記録・検証記録）
- 触れた dir の管理 index（`docs/*/README.md`・`docs/impl/{ir,exp}/README.md`）の再生成のみ

### 対象外（触らない）

- 本番 crate・`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI — 実装は ADR merge 後の後続 slice
- `docs/*/README.md` の手編集 — doc-maintenance `index apply`・`sync_impl_index` のみ
- 実機・実 OS・実 Device・実 Plugin の受入 — #114 系別 Gate
- 既存 broken relation の修復（pre-existing、範囲外）

## File Map

| 成果物 | 役割 |
| --- | --- |
| `docs/specs/rcd-f02-monitoring-overdub.md` | What/Why・REQ-F02-01〜07・AC-F02-01〜10・Non-goals・Sources |
| `docs/designs/rcd-f02-monitoring-overdub-design.md` | 既存契約 inventory・UX Matrix・notice semantics・HandleGrant/retention・Undo/Recovery・Privacy・却下選択肢 |
| `docs/plans/rcd-f02-monitoring-overdub.md` | 本計画 |
| `docs/tasks/rcd-f02-monitoring-overdub.md` | task 分解・完了条件・後続実装 Owner |
| `docs/test-specs/rcd-f02-monitoring-overdub.md` | Feature/Rule/Example/Guarantee・状態語彙・fixture 契約 |
| `docs/adr/rcd-f02-monitoring-overdub.md` | **納品本体** — 5 領域の MADR 決定（D1〜D5） |
| `docs/impl/ir/rcd-f02-monitoring-overdub.md` | 実施記録（docs-only status・handoff） |
| `docs/impl/exp/rcd-f02-monitoring-overdub.md` | graph route・ID 生成・決定と選択肢の記録 |
| `evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md` | 5 領域の決定要約 + 採用/却下選択肢 |
| `evidence/sv120/rcd-f02-monitoring-overdub-verification.md` | 実行した検証の記録（コマンド・exit code・差分要約） |

## Dependencies

- 上流 spec/design: `docs/specs/rcd-f02-monitoring-overdub.md`・`docs/designs/rcd-f02-monitoring-overdub-design.md`
- baseline 契約（read-only input）: `crates/synvelis-recording/src/{monitor,gate,plan,session,journal}.rs`・`crates/synvelis-app/src/monitoring.rs`・`crates/synvelis-player/src/{monitor,monitor_engine}.rs`
- 確定済み文書: `docs/designs/rcd-03-*`（monitor/punch/latency/persistence/session-ux/vcs-diff）・`docs/designs/rcd-02-*`（durability/recovery）・`docs/{specs,designs}/recording-take-comping*`
- Issue 依存: RCD-01/03/04・`#12`・SF-02 `#105`・SV-GUI-05 `#114`。統合・受入関連: `scarecrow173/ruviform#627`/`#628`/`#629`/`#631`・synvelis `#117`/`#118`
- 後続実装 slice: ADR merge 後・直列 dispatch（`synvelis-recording` Overdub/Handle/Take Layer・`synvelis-player` Monitor Cue・`synvelis-app` Consent/Warning UX・Fixture）

## Risks

| Risk | 対応 |
| --- | --- |
| headless/design 証拠を実機 Pass と誤記 | 状態語彙を `Pass (design)`/`Pass (headless)`/`NotRun`/`Deferred`/`Unsupported` で厳密分離。POLICIES.md 準拠 |
| 既存契約の型名・variant・行番号の引用誤り | `monitor.rs`/`gate.rs`/`journal.rs`/`monitoring.rs`/`monitor.rs`(player) を実読して確認済み。行番号は参照時に再確認 |
| 既存 broken relation で graph route が blocked | exp に記録し slice 起因でないことを明記（sv#12 先例と同型）・修復しない |
| index 競合 | `index apply --dir`/`sync_impl_index` で touched dir のみ再生成・`in-sync` 確認 |
| 範囲外変更の混入 | `git status`/`git diff --stat` で占有範囲+生成 index+evidence のみであることを最終確認 |
| ADR の決定が実装不能 | Implementation Plan に想定ファイル・patterns・verification を記載し、契約は既存権威の拡張に限定 |

## Verification Matrix

| 検証 | コマンド | 種別 | 期待 |
| --- | --- | --- | --- |
| fmt | `mise run fmt-check` | docs | exit 0 |
| hook | `mise exec -- prek run --all-files` | docs | exit 0 |
| index | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan` → `index apply` | docs | 全 touched dir `in-sync` |
| impl index | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir|exp`（preview→apply） | docs | `in-sync` |
| doc audit | `node .agents/skills/doc-status/scripts/doc_status.js audit --type all --profile artifact` | docs | slice 起因の新規 broken なし（pre-existing は区別） |
| type lint | `doc_status.js lint --type <spec|design|plan|task|test-spec|adr>` | docs | 新規 chain の type 別 lint 合格 |
| impl audit | `audit_impl_record.js`・`audit_experiment_log.js` | docs | 新規 ir/exp が監査合格 |
| ADR audit | `node .agents/skills/adr-doc/scripts/audit_adr.js`・`check_code_links.js` | docs | ADR front matter/section/link 合格 |
| graph route | `route_graph.js --cwd . --json` | docs | 結果を exp に記録（blocked=pre-existing） |
| Rust | — | — | docs-only のため `NotRun`/`NotApplicable` |
| review | `review-protocol` | docs | blocker 0 |

## Downstream Handoff

- 実装 Owner（後続 slice・本 slice 対象外）: ADR D1〜D5 と design の契約に従い `synvelis-recording`/`synvelis-player`/`synvelis-app`/`synvelis-project` を変更する。Issue #120 の変更範囲（Overdub/Explicit Pre-Punch Handle/Take Layer Policy・Monitor Cue・Consent/Warning UX・Fixture）が想定範囲。
- Coordinator: 本 chain の merge 後、実装 slice を直列 dispatch。Issue #120 自体の close は Coordinator/Owner 判断（`Refs #120`、auto-close しない）。
- 実機/GUI/Plugin 受入: #114 系別 Gate。

## Rollback

- docs-only のため `git revert` で文書チェーンと evidence を戻せる。生成 index は再生成で復元可能。本番コードへの影響なし。
