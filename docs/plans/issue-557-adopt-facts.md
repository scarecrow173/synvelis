---
id: "PLAN-557AdoptFacts04"
type: "plan"
status: "approved"
title: "Issue #557 plan — adopt declared_facts 監査 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/557"
    - "docs/specs/issue-557-adopt-facts.md"
    - "docs/designs/issue-557-adopt-facts.md"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/specs/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/designs/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/plans/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/tasks/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/test-specs/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/impl/ir/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/impl/exp/issue-557-adopt-facts.md"
      - type: "file"
        path: "docs/impl/exp/issue-557-adopt-facts-exp.jsonl"
      - type: "dir"
        path: "evidence/sv557/"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-06-track-bus-adapter.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-557-adopt-facts.md"
    - "docs/designs/issue-557-adopt-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-557-adopt-facts.md"
---

# Issue #557 plan — adopt declared_facts 監査 slice

## スコープ分割

単一 slice。監査（読み取り）→ pin test 追加 →
doc/evidence 記録 → 検証 → review の直列。

## ファイルマップ

### 変更（modified）

- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
  — `cb06_strip_557_adopt_send_remove_candidate_
  materializes_teardown` を末尾に追加。test-local
  helper のみ（`adopt_op`/`fork_op`/`edit_op`/
  `send_remove_op`/`issue_agent`/`branch_grant`/
  `prepare_req`/`approve_req`/`apply_agent`/
  `complete`/`only_set`/`branch_revision`/`live_rev`/
  `live_scope_digest`/`publish`/`commit_human`/
  `boot_strip_send`/`strip_scope`/`strip_send_id`/
  等の既存 helper を再利用 — 新規 helper は追加しない
  方針で組んだ）
- `docs/test-specs/cb-06-track-bus-adapter.md` —
  監査結果の追補（adopt 経路の declared_facts 監査が
  完了し gap 無し・残存経路は `synvelis#559`）
- `docs/impl/ir/cb-06-track-bus-adapter.md` — 同上の
  実装記録追補
- `docs/impl/exp/cb-06-track-bus-adapter.md` — 同上の
  実験記録追補

### 新規（added）

- `docs/discovery|specs|designs|plans|tasks|
  test-specs|impl/ir|impl/exp/issue-557-adopt-facts.md`
  — doc chain 8 件
- `docs/impl/exp/issue-557-adopt-facts-exp.jsonl` —
  実験イベントログ
- `evidence/sv557/` — 監査証拠（audit 記録 +
  verification 記録）

### 非変更（明示）

- `crates/synvelis-gateway/src/variant.rs` — 監査対象
  だが gap が実証されなかったため変更しない
  （AC1 の「根拠記録」側で充足）
- `crates/synvelis-project/**` — Issue 指定どおり
  読み取りのみ
- `crates/synvelis-gateway/src/` の他ファイル・
  `tests/rig/mod.rs`・他テストファイル — 非接触
- `docs/**/README.md` — docs index。Coordinator が
  統合時に再生成

## タスク分解

| # | タスク | 依存 |
| --- | --- | --- |
| T1 | main 同期・`declared_facts`/`AdoptIntent`/`plan_adopt_writes`/wire capability の監査（読み取り） | — |
| T2 | adopt 受理 pin test を cb06_strip_gateway.rs に追加し対象 test を実行 | T1 |
| T3 | 残存経路（pre-553 stored payload）の評価 → `synvelis#559` 起票 | T1 |
| T4 | doc chain 8 件 + exp.jsonl + cb-06 3 ファイル追補 + `evidence/sv557/` | T2,T3 |
| T5 | 必須検証一式（check/lint/test ×2 crate + fmt-check + check-headless-deps + prek） | T4 |
| T6 | review-protocol レビュー → blocker 0 確認 → commit/push/PR | T5 |

## リスク登録

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 監査で gap を見落とす | AC1 未達 | 型定義（inner op 不在）+ wire lowering（CAS のみ）+ prepare の二層 gate の 3 点で構造的に確認。発見時は variant.rs Adopt arm を sv#553 同方針で修正（lease 内） |
| pin test が不安定 | flaky | 既存 helper のみで構成・sleep/時間依存なし。`objects == 3` は `plan_adopt_writes` の write 計画に対応する決定的値 |
| project crate の test が環境起因で失敗 | 検証未達 | 失敗時は原因切り分け（自分の変更起因でないことを確認）し証拠付きで報告 |
| docs index 未同期で CI が落ちる | 統合時の摩擦 | index は Coordinator 管轄 — PR 本文「未対応の作業」に明記 |

## ロールバック

- test 追加のみで production code 非変更のため、
  revert は test/doc/evidence の削除で完結。
- 万が一 gap が後から実証された場合は別 Issue で
  `variant.rs` Adopt arm を修正（本 slice の範囲外
  だった場合の逃げ道を残す）。

## 検証マトリクス

Issue 指定の必須コマンドを全て実行（`mise.toml` 正本）:

- `mise run check synvelis-gateway` / `lint` / `test`
- `mise run check synvelis-project` / `lint` / `test`
- `mise run fmt-check`
- `mise run check-headless-deps`
- `mise exec -- prek run --all-files`

補助証拠（canonical = mise）: cargo 直接実行は使わない
方針（mise 経由のみ）。

## 関連リンク

- `synvelis#557`（本 Issue）/ `synvelis#553` /
  `synvelis#550` / `synvelis#559` / `synvelis#283`
- `docs/discovery/issue-557-adopt-facts.md`
- `docs/specs/issue-557-adopt-facts.md`
- `docs/designs/issue-557-adopt-facts.md`
- `docs/tasks/issue-557-adopt-facts.md`
- `docs/test-specs/issue-557-adopt-facts.md`
- `POLICIES.md`
