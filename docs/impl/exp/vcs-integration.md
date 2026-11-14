---
id: "EXP-VCSIntegration001"
title: "VCS Integration 統合契約 docs-only slice 判断ログ (synvelis#7)"
created: "2026-11-14"
updated: "2026-11-14"
---

# VCS Integration 統合契約 docs-only slice — experiment log

## E1. 文書チェーンの生成方法

`new_spec.js`/`new_design.js`/`new_plan.js`/`new_task.js`/
`new_test_spec.js`/`new_impl_record.js` の `--name`/`--dir`
オプションで指定ファイル名どおりに生成した。ID は script 採番の
opaque ID（`SPEC-034X9CW66GeytQXJHrogRB` 等）をそのまま使用し、
手動での ID 付与は行わなかった。`new_experiment_log.js` が生成
する `.jsonl` event log は task 指定パス（`.md`）と形式が異なる
ため採用せず、`vcs-04-native-mount.md` 等の markdown experiment
log 先例に倣って `docs/impl/exp/vcs-integration.md` を手書きした。

## E2. status 方針

spec/design/test-spec は `approved`、plan は `in-progress`、
task は `in-progress`→完了時 `done`、impl-ir は `in-progress`→
`completed` とした。worker 契約によりユーザー承認局面は
review-protocol のレビュー（blocker なし）を承認として扱うため、
plan-doc の design gate（approved design 必須）は既存 approved
design 群で充足される。自分で作った design を `approved` にした
点は review-protocol での事後検証対象とした。

## E3. verdict 転記方針

`evidence/sv7/vcs-integration-case-ledger.md` の verdict は
`evidence/reports/vcs-06-fixture-matrix-ledger.md` の実測を
転記のみとし、集計・再評価・昇格を行わなかった。`Pass*` は
persisted/headless 層のみの Pass、`NotRun`/`Unsupported`/
`Blocked` は理由付きで維持。Issue #7 の 14 受入条件 → Case
family の写像表を ledger 冒頭に置き、spec 受入条件の各行が
どの Case/証拠/設計記録に対応するかを一意に追跡可能にした。

## E4. 8 view 採否の根拠

Issue #7 §6 の HTML mockup は chat 添付で repo 未収録（Issue が
「GitHub リポジトリへの HTML 本体の追加は別作業」と明記）。
pixel レビューは実行不能なため、採否判定は Issue 本文の画面
構成と既存 slice spec の確定済み view mode（SideBySide/
ChangeFocus 採用、Overlay/Evolution Map/Rendered Audio Diff
延期、`plugin_instances` diff は #324 系）を根拠に契約レベルで
記録した。Clip Detail は「条件付き採用」（独立 view mount は
後続 slice）、Plugin State は「未採用（現時点）」、Audio は
「延期」とした。これは実機 UI レビューの代替ではなく、未実施を
Deferred として明示する記録である。

## E5. つまずき・注意点

- `new_impl_record.js`/`new_experiment_log.js` に `--name` がなく、
  生成ファイル名が指定パスと一致しない。ir は `mv` で
  `vcs-integration.md` に改名（ID は front matter 内で不変）。
  exp は `.jsonl` ではなく `.md` が要求されたため手書きに切替。
- doc graph の route は focus なしで `blocked`（`artifact-graph`・
  `focus-required`・大量の pre-existing broken relations:
  legacy `ISSUE-*`/`SPEC-*` token が現行 graph で解決不能）。
  これは本スライス以前からの repo 全体の状態であり、本スライス
  起因の新規 broken relation のみを修正対象とする方針とした。
- `doc-maintenance index apply` は preview-first（`index plan` で
  planHash 確認 → `--expect` 付き apply）。手編集はしない。
