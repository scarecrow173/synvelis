---
id: "PLAN-uGKWo3QVS9uwBCoBMRyVGT"
type: "plan"
status: "approved"
title: "Timeline Editing / Selection / Tool System — 計画（synvelis#11）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/timeline-editing-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/timeline-editing-design.md"
    - "docs/specs/timeline-editing.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/timeline-editing-task.md"
    - "docs/test-specs/timeline-editing.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/timeline-editing.md"
  references: []
  defers: []
  deferred-by: []
---

# Timeline Editing / Selection / Tool System — 計画

## スコープ

**docs-only slice**。synvelis#11 の共通 UX 仕様・設計・受入 Fixture・
設計文書チェーン 7 文書と `evidence/sv11/` 受入台帳を作成する。
Rust crate・UI・CI・Cargo/mise 設定の変更は **行わない**。実装は
下流 Issue（#112/#114・#10/#12 系実装 Issue・#9 import 系）が担当
する。

### File Map

| ファイル | 内容 | 状態 |
| --- | --- | --- |
| `docs/specs/timeline-editing.md` | What/Why/AC(TE-01〜14)/Non-goals/Sources | 作成 |
| `docs/designs/timeline-editing-design.md` | Owner 境界・既存実装対応付け・Selection/Tool/Snap/Gesture/Cross-View 契約・F-UX-01〜08・Decision Log(D-01〜12)・Scenario/Oracle・検証戦略 | 作成 |
| `docs/plans/timeline-editing.md` | 本計画 | 作成 |
| `docs/tasks/timeline-editing-task.md` | タスク分解・完了条件 | 作成 |
| `docs/test-specs/timeline-editing.md` | Feature/Rule/Example/Guarantee・Fixture 契約 | 作成 |
| `docs/impl/ir/timeline-editing.md` | 実装記録（docs slice のみ） | 作成 |
| `docs/impl/exp/timeline-editing.md` | 実験記録（router 状態・ID 生成・設計判断） | 作成 |
| `evidence/sv11/coverage.md` | TE-01〜14 × 既存被覆の受入台帳 | 作成 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | 生成 index（doc-maintenance で再生成） | 再生成 |

### スコープ外（触らない）

- `crates/**`、`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI — 実装
  Owner の領域。本 slice は docs のみ。
- Ruviform GPUI Driver・共通 Pointer/HitTest/Viewport/Gesture —
  独自再実装しない。DAW 固有編集命令を Ruviform へ持ち込まない。
- Pointer/Selection の暗黙 Project/Graph Commit 化。
- `docs/*/README.md` の手編集 — index は `doc-maintenance` の
  `index apply` のみ。
- Issue #11 のクローズ・PR マージ — Coordinator の責務。

## 依存関係

### 上流（消費する既存契約）

- ED-A #38 / ED-B #65: Selection/Tool/Snap/Gesture/Undo/Linked
  Context・media domain 編集の headless 実装契約。
- WS-A #36 / workspace-model #2: View/Selection/LinkGroup/Command
  routing・寿命独立・入力文脈別 Undo lane。
- MD-A #42: ClipPlacement/ClipPlayback 時間契約・TakeGroup/Comp
  投影・`EditorEditingTarget`。
- MIDI Source #52: Linked Source 共有・明示 Detach・note 編集 op。
- Context Inspector #5 / VCS-01〜06 / `synvelis-library` import:
  Explore 非破壊・read-only Snapshot・Remote Asset 検証契約。
- #19/#20: 可変 PPQN Musical Time・Transport Span。

### 下流（本設計が供給する先）

- #112/#114: 製品 GUI 実装・実機受入の契約・Fixture・Case ID。
- #10/#12 系実装 Issue: Project Model・Take/Comp が継承する共通
  編集 UX の境界。
- #9 系 import: Timeline への Remote Asset 取込の検証境界。

## リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| headless 実装済みを製品 Pass と誤記 | POLICIES.md 違反・虚偽受入 | coverage.md で `Pass (headless)`/`Partial`/`NotRun`/`Deferred`/`Unsupported` を分離。design で #112/#114 Gate を明示 |
| ED-A/ED-B の PoC 契約を製品契約と混同 | 未確定数値の固定化 | design D-11 で数値を PoC 後決定と明記。契約は優先順/分離/固定/拒否のみ |
| #10/#12 未確定領域を先取り設計 | 他 Worker との矛盾・範囲侵犯 | #10/#12 は「継承境界のみ」定義し、entity/UI 詳細は deferred に記録 |
| Ruviform primitive の再実装誘発 | 責務境界違反 | spec/design で Ruviform 消費・DAW 意味論のみ Synvelis と明記 |
| 既存 broken relation の巻き込み修復 | 範囲外変更 | router blocked を exp に記録し、無関係な relation は修復しない（sv5/sv6 先例と同じ） |

## 検証マトリクス

| 検証 | コマンド | 対象 | 期待 |
| --- | --- | --- | --- |
| Markdown/構成の軽量チェック | `mise run fmt-check` | docs のみ（crate 変更なし） | exit 0 |
| index 整合 | `doc_maintenance.js index plan` → `index apply --expect <hash>` → 再 `index plan` | 触れた 7 dir | `in-sync` |
| doc-status 監査 | `doc-status`/doc_maintenance の確認手順 | docs 全体 | 新規 doc の front matter/関係が valid（既存 broken relation は範囲外として記録） |
| review-protocol | 仕様整合 + 反例/敵対的レビュー | spec→design→test-spec→coverage | Confirmed P0/P1 = 0 |
| Rust build/test | **対象外**（crate 変更なし） | — | NotRun/NotApplicable として記録 |

## Downstream Handoff

- 実装 slice は `docs/designs/timeline-editing-design.md` の
  Selection/Tool/Snap/Gesture/Cross-View 契約・F-UX Fixture・
  Decision Log と `docs/test-specs/timeline-editing.md` の Fixture
  契約を消費する。
- `evidence/sv11/coverage.md` が TE-01〜14 ごとの既存被覆と残
  Gate を示し、#112/#114 が GUI/実機受入の対象を特定する。

## Rollback

- docs-only のため、問題があれば当該ファイルを revert するだけで
  復旧可能。生成 index は `doc-maintenance` で再生成して戻す。
- 実装・永続データ・schema への影響はない。
