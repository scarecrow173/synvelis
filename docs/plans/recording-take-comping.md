---
id: "PLAN-xOv3jxNuAQWUfmfAuMGI8P"
type: "plan"
status: "approved"
title: "Recording / Take / Comping — Session・Take・Comp・Raw Writer・Monitor・Recovery・Source Native Frame 文書チェーン・Owner Matrix 計画（synvelis#12）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/recording-take-comping-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/recording-take-comping.md"
    - "docs/designs/recording-take-comping-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/recording-take-comping-task.md"
    - "docs/test-specs/recording-take-comping.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/recording-take-comping-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/recording-take-comping.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/118"
---

# Recording / Take / Comping — 文書チェーン・Owner Matrix 計画（synvelis#12）

## 目的

Issue #12 の基本アーキテクチャ合意を、`spec → design → plan → task → test-spec → impl/ir + impl/exp` の canonical 文書チェーンと、69 受入条件の Owner Matrix（`evidence/sv12/`）として固定する。**docs-only slice であり、実装・実 OS 受入を主張しない。**

## スコープ

### 対象（編集可）

- `docs/specs/recording-take-comping.md`
- `docs/designs/recording-take-comping-design.md`
- `docs/plans/recording-take-comping.md`（本書）
- `docs/tasks/recording-take-comping-task.md`
- `docs/test-specs/recording-take-comping.md`
- `docs/impl/ir/recording-take-comping.md`
- `docs/impl/exp/recording-take-comping.md`
- `evidence/sv12/`（Owner Matrix・coverage ledger）
- 触れた dir の管理 index（`docs/*/README.md`）の再生成のみ

### 対象外（触らない）

- 本番 crate・`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI — 実装は #115〜#118 が Owner
- Ruviform Generic Device Input/File/Clock — DAW 固有仕様を持ち込まない
- `docs/*/README.md` の手編集 — doc-maintenance `index apply` のみ
- 実機・実 OS・実 Device・実 Plugin の受入 — #114・EXT-L3 の別 Gate
- Creative Branch Take/Comp 候補 — #278/#279/#288

## File Map

| 成果物 | 役割 |
| --- | --- |
| `docs/specs/recording-take-comping.md` | What/Why・69 AC・Non-goals・Sources |
| `docs/designs/recording-take-comping-design.md` | Owner 境界・C1〜C7 契約・状態モデル・Scenario/Oracle・Decision Log |
| `docs/plans/recording-take-comping.md` | 本計画 |
| `docs/tasks/recording-take-comping-task.md` | task 分解・完了条件・実装 Owner・index/evidence タスク |
| `docs/test-specs/recording-take-comping.md` | Feature/Rule/Example/Guarantee・fixture 契約 |
| `docs/impl/ir/recording-take-comping.md` | 実施記録（docs-only status・handoff） |
| `docs/impl/exp/recording-take-comping.md` | graph route・ID 生成・判断記録 |
| `evidence/sv12/owner-matrix.md` | 69 AC × Owner Issue × fixture × OS Gate × 状態 |

## Dependencies

- 上流 spec: VS-A〜VS-D（#13〜#16 closed）・RCD-01〜RCD-04（#115〜#118）・MD-A #42・MD-B #50・PC-C1 #98・TIM-02 #126・X slice・workspace/ED-A/TE #11。
- 先行 slice 前提: sv#117（Monitor/Punch headless fixture・PR#387）統合済みで参照可能。
- 実装 Owner への handoff: #115（device capture）・#116（writer/journal/recovery）・#117（monitor/punch/latency）・#118（comp/midi/recovery E2E）。

## Risks

| Risk | 対応 |
| --- | --- |
| headless 成功を実機 Pass と誤記 | owner-matrix で `Pass (headless)`/`NotRun`/`Deferred`/`Unsupported` を厳密分離。POLICIES.md 準拠 |
| 69 AC の Owner 対応付け漏れ | AC-01〜69 を spec・matrix・test-spec で同一 ID に揃え、集計表で件数照合 |
| 既存 broken relation で graph route が blocked | exp に記録し、slice 起因でないことを明記（sv#5/#6/#11 先例と同型） |
| index 競合 | `index apply --dir` で touched dir のみ再生成・`in-sync` 確認 |
| 範囲外変更の混入 | `git status`/`git diff --stat` で占有範囲+生成 index のみであることを最終確認 |

## Verification Matrix

| 検証 | コマンド | 種別 | 期待 |
| --- | --- | --- | --- |
| fmt | `mise run fmt-check` | docs | exit 0 |
| index | `node scripts/doc_maintenance.js index plan/apply --dir <touched>` | docs | 全 touched dir `in-sync` |
| impl index | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir|exp` | docs | preview→apply・`in-sync` |
| doc audit | `doc-status` audit（doc-maintenance/impl-doc script） | docs | 新規 broken relation なし |
| graph route | `route_graph.js --cwd . --json` | docs | 結果を exp に記録 |
| Rust | — | — | docs-only のため `NotRun`/`NotApplicable` |
| review | `review-protocol` | docs | blocker 0 |

## Downstream Handoff

- 実装 Owner: `evidence/sv12/owner-matrix.md` の各 AC 行から、自分の Issue・fixture・OS Gate を参照して実装・検証する。
- Coordinator: 本 chain の merge 後、#115〜#118 の実装 slice が本契約を消費する。Issue #12 自体の close は Coordinator 判断（設計合意の記録であり実装完了ではない）。
- 後続 PoC: 詳細 schema・数値は #115〜#118 の fixture で確定。

## Rollback

- docs-only のため `git revert` で文書チェーンと evidence を戻せる。生成 index は再生成で復元可能。本番コードへの影響なし。
