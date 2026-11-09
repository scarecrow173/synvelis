---
id: "IMPL-034UGB5wH67MKn7FnNHs1i"
type: "impl"
status: "completed"
title: "Issue #230 check-ruviform-rev REV/lock 不整合 — stale vendor 診断と復旧導線"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/230"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/issue-230-check-ruviform-rev-rev-lock-stale-vendor.md"
    modified:
      - type: "file"
        path: "tools/check-ruviform-rev.mjs"
      - type: "file"
        path: ".worktreeinclude"
      - type: "file"
        path: "AGENTS.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
    - "docs/designs/sv28-ruviform-pin-cc7d5296.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "Cargo.lock"
    - ".worktreeinclude"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Issue #230 check-ruviform-rev REV/lock 不整合 — stale vendor 診断と復旧導線

## Summary

main で `mise run check-ruviform-rev` が `ui vendor REV != lockfile
resolved` で pre-existing 失敗する報告（synvelis#107 コメント、
Issue #230）を調査した。`Cargo.lock` 側の ruviform git 依存は全 26
source が単一 SHA `0f5b5737` に整合しており lockfile・check ロジックに
欠陥はない。失敗の実体は gitignore された生成物 `ui/vendor/ruviform`
が旧 rev `20a24a66` のまま残っていた stale vendor であり、check の
fail-closed は正しい検出だった。復旧は `mise run sync-ui-deps` の
再実行で、以後の再発防止として診断メッセージ・worktree コピー注記・
AGENTS.md の運用規約に復旧手順を明記した。

## Context

- Issue: https://github.com/scarecrow173/synvelis/issues/230
- 発生経路: `ui/vendor/` は gitignore された生成物だが `.worktreeinclude`
  で Orca worktree へそのままコピーされる。コピーは生成元 workspace が
  最後に `sync-ui-deps` した時点の rev を保持するため、pin 更新
  （`Cargo.lock` の ruviform 解決 rev 変更）やブランチ切替の後に作られた
  worktree では stale vendor が混入し、`check-ruviform-rev` が
  REV/lock 不一致で fail する。
- 今回の実測: worktree の `ui/vendor/ruviform/REV` は
  `20a24a66ee90…`（2026-11-08 同期）で、synvelis の `Cargo.lock` 履歴に
  一度も現れない rev（当時の ruviform main HEAD を一時的に解決した
  workspace 状態由来と推定）。lock 側は `3f3cae1`（→`cc7d5296`）と
  `c5929c4`（→`0f5b5737`）の pin 更新で正当に進んでおり、vendor 側が
  追随していなかった。

## Implementation

### Changes

- `tools/check-ruviform-rev.mjs`: vendor REV 不一致の fail メッセージに
  復旧手順 `mise run sync-ui-deps` を追記（REV 欠落時の既存メッセージと
  同じ導線）。
- `.worktreeinclude`: `ui/vendor/` コピーが pin 更新・ブランチ切替後に
  stale になりうる旨と復旧手順をコメントで明記。
- `AGENTS.md`: Rust ワークスペース規約に、ruviform 解決 rev が変わる
  変更の取り込み後は `sync-ui-deps` で vendor を再同期する規約と、
  REV 不一致 fail の意味を追記。
- `docs/impl/ir/README.md`: 本記録を索引へ追加。

### Decisions During Implementation

- check ロジックと `Cargo.lock` は変更しない。check は実在する不整合を
  正しく fail-closed で検出しており、lockfile の ruviform 解決は全
  source 単一 SHA で正常。修正は診断・運用導線の整備に限定した。
- `sync-ui-deps` を `check-ui`/`test-ui`/`build-ui` の `depends` に
  追加する案は採らなかった。sync は `~/.cargo/git/checkouts` の
  cargo fetch 済み checkout を要求し、UI lane を Rust 依存取得へ
  結合させると fetch 前環境で lane が壊れるため。
- ruviform pin の通常更新（#28 の所掌）は対象外。lock が指す
  `0f5b5737` を更に新しい ruviform main HEAD へ進める判断は行わない。

## Related Experiments

### Adopted

<!-- Record adopted experiment logs and why they were adopted. -->

### Rejected

<!-- Record rejected experiment logs and why they were rejected. -->

## Validation

- `mise run check-ruviform-rev`: 修正前は `ui vendor REV
  20a24a66… != lockfile resolved 0f5b5737…` で exit 1 を再現。
  `mise run sync-ui-deps` で vendor を `0f5b5737` へ再生成後、
  exit 0（26 lockfile sources、REV match）を確認。
- `Cargo.lock` の ruviform source は全件 `0f5b5737` で単一 SHA 整合
  （`git log -S` で `20a24a66` が lock 履歴に存在しないことも確認）。
- `mise run fmt-check` / `check` / `lint` / `test`（`cargo test
  --workspace --locked`）/ `check-headless-deps`: すべて exit 0。
- REV を一時的に旧値へ戻して fail メッセージを実測し、復旧手順
  （`mise run sync-ui-deps`）付きで exit 1 になることを確認後、
  正しい REV へ復元して再度 pass を確認。

## Risks

- vendor は生成物のため本修正で stale 状態そのものは解消しない。
  pin 更新のたびに既存 checkout/worktree で同じ fail が再発しうるが、
  今回の診断メッセージ・AGENTS.md・`.worktreeinclude` 注記により
  復旧手順へ辿れる状態にはなった。
- `.worktreeinclude` のコピー自体を rev 照合付きにする等の構造的な
  再発防止は Orca worktree 機構側の管轄であり、本リポジトリの変更
  範囲外。

## Follow-ups

- ruviform pin の通常更新運用は synvelis#28 が Owner。pin 更新フロー
  自体の見直しが必要になった場合は #28 側で扱う。
