---
id: "IMPL-wudSTaSGqUbfYIh4qzyqGq"
type: "impl"
status: "completed"
title: "Target Architecture 設計レビュー 実装記録 — docs chain 7 文書 + evidence/sv22 台帳"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/tasks/target-architecture-task.md"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/target-architecture.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "dir"
        path: "docs/specs"
      - type: "dir"
        path: "docs/designs"
      - type: "dir"
        path: "docs/plans"
      - type: "dir"
        path: "docs/tasks"
      - type: "dir"
        path: "docs/test-specs"
      - type: "dir"
        path: "docs/impl/ir"
      - type: "dir"
        path: "docs/impl/exp"
  implements:
    - "docs/tasks/target-architecture-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/target-architecture-task.md"
    - "docs/designs/target-architecture-design.md"
    - "docs/plans/target-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/target-architecture.md"
    - "docs/specs/target-architecture.md"
    - "docs/test-specs/target-architecture.md"
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/target-architecture-task.md"
  derived-by: []
  verifies:
    - "docs/test-specs/target-architecture.md"
  verified-by: []
  references:
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/target-architecture.md"
    rejected: []
---

# Target Architecture 設計レビュー — Implementation Record

## 実装した内容

synvelis#22「Target Architecture — TA-001〜012 設計レビュー・
Production Dependency DAG・GUI 採用方針の検証台帳」に対し、
承認済み v0.7.1 を現行コード・依存 DAG・参照 Issue と突合する
**設計レビューの docs chain と証拠台帳**を作成した。
コード・manifest・lockfile・CI は一切変更していない
（docs/evidence-only slice）。

### 作成ファイル

| ファイル | 内容 |
| --- | --- |
| `docs/specs/target-architecture.md` | 要件 R-TA-01〜14・Case SV22-C01〜14・Evidence 分類ルール・Out of Scope |
| `docs/designs/target-architecture-design.md` | Owner 境界写像・DAG/型漏出/GUI 証拠評価・決定 D1〜D7・Fixture 対応表・Owner/Case 照合 |
| `docs/plans/target-architecture.md` | 実施ステップ・リスク対処・検証マトリクス |
| `docs/tasks/target-architecture-task.md` | T-01〜T-10 タスク・完了条件 |
| `docs/test-specs/target-architecture.md` | Rule SV22-F-TA-01〜14（oracle・fixture・証拠分類） |
| `docs/impl/ir/target-architecture.md` | 本記録 |
| `docs/impl/exp/target-architecture.md` | 実験記録 |
| `evidence/sv22/case-ledger.md` | Environment・実行検証・C01〜C14 ケース・分類集約 |
| `evidence/sv22/dependency-dag.md` | 再現コマンド・辺一覧・dev/optional/feature/target 分類・TA-005 注目辺・GUI driver 辺 |

### 主なレビュー結論

- **TA-001/005（DAG）**: Synvelis resolve + Ruviform
  （fd1d4d5・42 member）とも normal-dep cycle = 0。
  plugin-host→audio は dev-dep のみで境界を満たす。
- **TA-002（型漏出）**: Ruviform 公開 API に Synvelis
  DAW 型の侵入なし。`synvelis_dark` theme fixture は
  P2 観測として分離。
- **TA-003〜TA-011**: 各契約・fixture が resolved rev で
  実在することを pin。実走は NotRun/先行参照に分離し、
  検証不能経路を Pass にしない。
- **TA-012**: Audio/Video/Plugin/CLI/UI/Synvelis/AudioLens
  を別責務・別 Suite・別 NotRun 根拠で記録。AudioLens は
  Unsupported→#219/#294 Deferred。
- **GUI（C13）**: #175（accepted）と #384（proposed）が
  方向一致。旧 WebView ADR は superseded で歴史保持。
  GPUI 型非露出を feature 分離 + check-headless-deps で確認。
- **Native Dock（C14）**: #388/#389/#176/#262 に Deferred
  接続。Floating/Generic/Mock/simulated を最終 PASS にしない。

## 実行した検証（実測）

| コマンド | exit | 結果 |
| --- | --- | --- |
| `cargo metadata --format-version 1 --locked --offline` + node DFS | 0 | normal-dep cycle = 0（Synvelis resolve 全体 + Ruviform 42 member） |
| `mise run fmt-check` | 0 | フォーマット差分なし |
| `mise run check-ruviform-rev` | 0 | resolved fd1d4d5…（27 sources、REV match） |
| `mise run check-headless-deps` | 0 | no GUI framework deps in 10 headless crates |

`check-ruviform-rev` は初回 stale vendor（`ui/vendor/ruviform`
が旧 rev `20a24a66…`）を検出して fail したため、
`mise run sync-ui-deps` で `fd1d4d5` へ再同期してから
再実行し Pass を確認した（synvelis#230 の既知 stale vendor
事象。`ui/vendor/ruviform` は gitignore された生成物で
コミット対象外）。

## 未実行の検証（NotRun/Deferred）

- `mise run check`/`test`/`test-rt`/`lint`（package 引数必須・
  docs slice で crate 変更なし）→ case-ledger に根拠記録。
- ruviform fixture 実走（別 repo・build 対象外）→ 先行
  slice/CI の SHA 付き記録を参照。
- 実 OS Device/実 Plugin/実 Native Dock/実 GUI（EXT-L3/
  GUI-L2）→ #176/#262/#388/#389/#294 に Deferred。
- AudioLens 側証拠 → 別 repo・Unsupported→#219/#294 Deferred。
- merge 後対象 SHA の CI 証拠 → Coordinator が確認。

## 発見した問題（範囲外・記録のみ）

- **pre-existing broken relations**: `doc-driven-dev-graph`
  の route が `artifact-graph` + 多数の `broken-relation`
  で blocked（無関係な既存 docs が原因）。本 slice 範囲外
  として修復せず記録のみ。
- **v0.7.1 crate 数ドリフト**: v0.7.1 は「現行 28 crate」
  と記述するが fd1d4d5 は 42 member（M1〜M7 で増加）。
  責務分担自体は整合。v0.7.1 更新は Ruviform #193 の
  migration snapshot 管理。
- **`synvelis_dark` theme fixture**: `ruviform-ui-theme`
  generated.rs に consumer 名 module。DAW 型ではないが
  consumer 名を Ruviform ソースに持つ点を P2 観測として
  Ruviform Owner へ記録。
- **plugin-gui-embedding ADR**: wry `build_as_child` の
  旧経路が accepted のまま。#389 で再設計予定 — Ruviform
  Owner が supersession を管理。

## 変更しなかったもの（scope 確認）

- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/本番 crate:
  変更なし（`git status` で docs/evidence/index のみ）。
- Ruviform/AudioLens repo: 読み取り参照のみ・変更なし。
- sv#20/#21 docs: 参照のみ・編集/作成なし。
- `docs/*/README.md`: `doc-maintenance` の `index apply` で
  再生成（手編集なし）。
