---
id: "SPEC-034RIJkUYtv28x8ZpNn8Tv"
type: "spec"
status: "approved"
title: "Issue #147 sccache コンパイルキャッシュ導入仕様"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/147"
    - "docs/discovery/issue-147-sccache-eval.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/issue-147-sccache-compile-cache.md"
    - "docs/plans/issue-147-sccache-compile-cache.md"
  depends-on:
    - "docs/discovery/issue-147-sccache-eval.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-147-sccache-eval.md"
  derived-by:
    - "docs/designs/issue-147-sccache-compile-cache.md"
    - "docs/plans/issue-147-sccache-compile-cache.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-147-sccache-compile-cache.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "Cargo.toml"
  defers: []
  deferred-by: []
---

# Issue #147 sccache コンパイルキャッシュ導入仕様

## What Should Be Built

`.github/workflows/ci.yml` の `verify`(3 OS matrix)および
`gw10-verify`(3 OS matrix)job に、sccache コンテントアドレス型
コンパイルキャッシュ(GitHub Actions cache backend)を導入する。
kill switch(repo variable `CI_SCCACHE_DISABLE=1`)と fail-open
(cache backend 障害・write error でも通常 compile へ継続)を備え、
cold/warm cache hit・job 時間を Before/After 実測して採否を判定する。
`rt-verify`/`msrv-verify`/`ui-verify` は段階展開設計の記録対象とし、
本変更では手を入れない。

## Why Now

`verify (windows-latest)` が直近 main run で 198-314s の critical path
を形成し、Rust コンパイルが支配的(discovery 実測)。ruviform #329/
PR #342 で同一パターンが verify lane 47-62% hit・job 時間 10-30% 削減・
fail-open・kill switch 動作を実証済みであり、synvelis 固有の OS /
toolchain / feature / Ruviform SHA 条件での検証が残るのみ。

## Users and Value

- synvelis 開発者: PR フィードバック時間の短縮(特に Windows leg)。
- reviewer/maintainer: 既存の検証粒度・証跡・required contexts を維持
  したまま CI コストを削減できる。
- CI 運用者: `CI_SCCACHE_DISABLE=1` で workflow 変更なく即時ロール
  バックできる。

## Scope

### In Scope

- `verify` job への sccache step 群追加:
  `mozilla-actions/sccache-action`(SHA pin)セットアップ →
  `actions/github-script` による GHA backend env export
  (`ACTIONS_RUNTIME_TOKEN`/`ACTIONS_RESULTS_URL`/`SCCACHE_GHA_ENABLED`/
  `SCCACHE_GHA_VERSION=synvelis-v1-<RUNNER_OS>`/`RUSTC_WRAPPER=sccache`)
  → 環境検証 step → 検証 step 群 → `sccache --show-stats` 統計 step。
- `gw10-verify` job への同型 sccache step 群追加(同一 OS namespace
  `synvelis-v1-<RUNNER_OS>` を verify と共有し、cross-lane hit を
  実測対象とする)。
- kill switch: 全 sccache step を `vars.CI_SCCACHE_DISABLE != '1'` で
  条件付け、repo variable 設定のみで全 skip(plain rustc 経路)にできる
  こと。
- Cold/Warm の hit rate・job wall time・cache write/read error の実測
  記録(`docs/impl/exp/`)と Before/After・Rollback 手順の記録。
- `rt-verify`/`msrv-verify`/`ui-verify` の段階展開・cache 分離設計を
  design doc に記録(実装は別 Issue)。
- `cargo test --workspace --locked rt_ -- --list` の列挙コンパイル時間
  と `mise run test-rt` 本試験時間の分離計測(step 時間の観測で実施、
  job 構造は変更しない)。

### Out of Scope

- `rt-verify`/`msrv-verify`/`ui-verify`/`dep-guard`/`evidence` job の
  変更(名称・needs・証跡構造・required contexts 構成は不変)。
- mise タスク定義・cargo profile・feature 集合の変更。
- `cargo check`/`clippy`/`test` の step 統合など検証粒度を変える再構成
  (step 証跡の分離を維持するため)。
- RT strict-prefix non-empty guard・GW-10 harness・dep-guard 等の
  保護対象ロジックの変更。
- S3 等の外部 cache backend、self-hosted runner、 toolchain 更新。

## Requirements

- sccache は cache write/read error 時も compile を継続すること
  (fail-open。`sccache --show-stats` は `|| true` で非致命的)。
- `ACTIONS_RUNTIME_TOKEN`/`ACTIONS_RESULTS_URL` は `run` step に届かない
  ため `actions/github-script` 経由で `GITHUB_ENV` へ再 export する
  こと(ruviform 同型)。
- `SCCACHE_GHA_VERSION` は `synvelis-v1-<RUNNER_OS>` とし、OS 間・
  他 repo との namespace 混線を防ぐこと。MSRV toolchain 導入時は
  `synvelis-msrv-v1-<RUNNER_OS>` 別 namespace とする設計を記録。
- profile/features/toolchain/Ruviform resolved SHA の差異は sccache
  hash 入力で分離されること(compiler バイナリ + 全引数 + 前処理済み
  ソース)。MSRV 1.89.0 成果物を 1.99.0 lane が誤用しない。
- 検証ロジックは引き続き mise タスクへ委譲し、workflow 内に cargo の
  検証コマンドを直書きしない。
- sccache 導入により CI 合計時間・job 時間が悪化した lane/OS では
  無条件展開せず、該当 step を除去する(不採用も正当な成果)。
- 既存の cargo registry/git `actions/cache` とは併存し、キー設計を
  変更しない。

## Acceptance Criteria

- [ ] PR 上で `verify`(3 OS)・`gw10-verify`(3 OS)・`rt-verify`・
      `msrv-verify`・`ui-verify`(3 OS)・`dep-guard`・`evidence` が
      従来どおり合格する。
- [ ] Cold run(初回 PR push)と Warm run(続く push または rerun)で
      `sccache --show-stats` の requests/hits/misses/write errors と
      job wall time を記録し `docs/impl/exp/` に残す。
- [ ] MSRV 1.89.0 lane は sccache 非導入のまま成功し、dev toolchain
      lane との成果物共有が発生しない(設計上・実測上とも)。
- [ ] rt-verify の strict-prefix non-empty guard が `rt_` テストを
      0 件 Pass 化せず検出を維持する(guard ロジック非変更で担保)。
- [ ] 10 required contexts の job 名・needs 構成・`evidence` の
      not-run-is-not-pass 判定が維持される。
- [ ] cache write error / backend 障害が発生しても compile・test が
      完走する(fail-open。stats で write error 数を記録)。
- [ ] Before/After と Rollback 手順(repo variable による kill switch
      および step 除去手順)が記録される。

## Deferred Design Concerns

- `rt-verify` への sccache 展開 | 同一 OS namespace を verify と共有すれば
  dev profile の test binary 群で cross-lane hit が見込めるが、段階展開の
  測定単位を分けるため stage 2 へ延期 | verify/gw10 の実測で hit 率と
  write contention が確認でき次第 | 延期のままでも現状維持で害はない。
- `msrv-verify` への sccache 展開 | rust@1.89.0 別 toolchain。compiler
  hash で誤共有しないが独立 invalidation/無効化のため
  `synvelis-msrv-v1-<RUNNER_OS>` 別 namespace 設計を design doc に記録 |
  stage 2 | 現状維持。
- `ui-verify` への sccache | rustc 呼出しを含まないため NotApplicable。
  将来 cargo build/test を含む lane 化した場合に再評価。
- GHAC write rate limit 対策(`SCCACHE_GHA_CACHE_WRITE=false` 併用・
  S3 backend 等)| cold run で write error が集中する既知挙動。効果が
  不十分な場合の次候補 | cold/warm 実測で判定 | hit 率が伸びない
  場合の改善余地として保持。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/147
- Discovery: `docs/discovery/issue-147-sccache-eval.md`
- 先行評価: ruviform #329 / PR #342 /
  `docs/impl/exp/issue-329-sccache-compile-cache.md`(ruviform repo)
- 上流 CI 設計: `docs/specs/ci-synvelis-pr-push-regression-gate.md`
  (Issue #83)、Ruleset Issue #145
- Baseline 計測契約: ruviform #327 (CI-OPT-00)
