---
id: "EXP-workspace-model-docchain"
type: "experiment"
status: "completed"
title: "Workspace Model — 文書チェーン生成と playback churn gap 補完"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/2"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/workspace-model.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-model-design.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/workspace-model-task.md"
    - "docs/test-specs/workspace-model.md"
  defers: []
  deferred-by: []
---

# Workspace Model — 文書チェーン生成と playback churn gap 補完

## 問題設定

Issue #2 は「Workspace / Docking / Selection / Command の統合モデルを
具体化・検証する」design/verification タスク。既存の WS-A PoC
（`ws_a.rs` 8 test）・ED-A・UI-C5・SV-GUI-04（`two_window.rs`/
`sv113_dock.rs`/`dock_ops.rs`/`close_veto.rs`/`shell.rs`）が大部分の
受入条件を既に固定しているため、本 slice の実作業は
(1) 文書チェーンの生成、(2) coverage gap の特定と最小補完、
(3) evidence 台帳化、に絞られた。

## 試行と結果

### E1: 既存 fixture の coverage 照合

- 方法: Issue #2 の 9 受入条件を WSM-AC-01〜09 → WSM-T-01〜20 Case
  へ分解し、既存 test 関数を 1 件ずつ照合。
- 結果: WSM-T-01〜07・09〜15 は既存 test で充足。**WSM-T-08
  （transport playing 中の dock/window churn）だけが直接の oracle を
  持たない** — `ws_a_05` は playing 未設定で churn のみを検証する。
- 判断: 「再生中」という Issue #2 の明示 qualifier を直接証明する
  test を `crates/synvelis-workspace/tests/workspace_model.rs` に
  1 件追加。他の Case に新規 fixture は不要（重複・hard-code 化を回避）。

### E2: playback churn test の初回 FAIL と修正

- 方法: `transport.play` command で `playing=true` を立て、
  detach/reattach/move/window event/close veto churn 後に
  transport・revision・graph_builds・undo・view state を検証。
- 結果: 初回 `assert_eq!(revision, rev0+1)` で FAIL（実際 +3）。
  原因: churn 前の `undo()`+`redo()` roundtrip 自体が project
  mutation として revision/graph_builds を各 +1 進める
  （`ws_a_05` が remove+undo+redo=+3 を期待するのと同じ会計）。
- 修正: 最終 assert を `rev0+3`/`builds0+3` に修正し、undo+redo+remove
  の会計をコメントで明記。test は pass。
- 教訓: revision 会計は「project mutation の回数」であり、view-local
  op（select/inspect/transport.play/dock churn）は寄与しない。
  undo/redo は project history 操作なので revision を進める。

### E3: doc-driven-dev-graph router の blocked 確認

- 方法: `route_graph.js --focus docs/specs/workspace-model.md --json`。
- 結果: `current: "probe"`・`status: "blocked"`・`edgeId: null`。
  blockers は `artifact-graph` + rcd/tim/vcs/pc/sf 等の無関係文書の
  既存 broken-relation。
- 判断: repo baseline の問題であり本 task の新規文書起因ではない。
  編集範囲外のため修復せず、evidence/sv2 台帳 §4 に外部 blocker
  として記録。文書チェーンは spec→design→plan→task→test-spec→ir/exp
  の正規順で手動整合させた。

### E4: review-protocol（spec+design）

- 方法: 独立 Reviewer A（cooperative）/B（adversarial）相当の
  分離 pass + Validator/Judge を self-review として実施
  （subagent 不使用 — task が helper worker を要求しないため）。
- 結果: CONFIRMED 3 件を修正 — (a) design Event→Receipt 例の
  `syn.project.rename` が実在しない op（`syn.project.set_ppqn` に
  修正）、(b) design の `§10` 節参照が存在しない（legacy 注記の
  実位置へ修正）、(c) spec の `refines` relation が ws-a 慣行と
  不整合（`derives-from` へ修正）。P0/P1 blocker なし。
- 判定: ACCEPT WITH NON-BLOCKING NOTES。spec/design を approved に
  遷移し plan/task/test-spec の scaffold gate を通過。

## 結論

- 採用: 既存 fixture 再利用 + WSM-T-08 のみ新規 test。文書 7 件 +
  evidence 2 件 + index 再生成。
- 棄却: 新規 workspace 実装・大規模 fixture 追加（既存契約と重複、
  contract の hard-code 化リスク）。
- 残件: 実機 gate（#114/#176/#294/#313/#314）と repo baseline の
  doc graph broken-relation は本 slice 範囲外として evidence に記録。
