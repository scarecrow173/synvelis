---
id: "DESIGN-034RIJlv2BuYKx5KG2VyDg"
type: "design"
status: "approved"
title: "Issue #147 sccache キャッシュ分離・段階展開設計"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/147"
    - "docs/specs/issue-147-sccache-compile-cache.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-147-sccache-compile-cache.md"
  implemented-by:
    - "docs/plans/issue-147-sccache-compile-cache.md"
  depends-on:
    - "docs/specs/issue-147-sccache-compile-cache.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ci-synvelis-pr-push-regression-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-147-sccache-compile-cache.md"
    - "docs/discovery/issue-147-sccache-eval.md"
  derived-by:
    - "docs/plans/issue-147-sccache-compile-cache.md"
    - "docs/test-specs/issue-147-sccache-compile-cache.md"
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "Cargo.toml"
  defers: []
  deferred-by: []
---

# Issue #147 sccache キャッシュ分離・段階展開設計

## Context

synvelis CI(Issue #83 設計)は `verify`/`gw10-verify`(各 3 OS)・
`rt-verify`・`msrv-verify`・`ui-verify`(3 OS)・`dep-guard`・`evidence`
の lane を持ち、検証は mise タスクへ委譲する。critical path は
`verify (windows-latest)`(198-314s)で、支配的コストは Rust コンパイル。
ruviform #329/PR #342 が sccache GHA backend の実証済み手順(47-62%
hit・10-30% 削減・fail-open・kill switch)を確立しており、本設計は同型
step 群を synvelis の lane 構成・toolchain 分離条件へ適用する。

## Scope

### In Scope

- `verify`・`gw10-verify` job への sccache step 群追加(3 OS 各 leg)。
- namespace 設計・kill switch・fail-open・統計観測。
- `rt-verify`/`msrv-verify`/`ui-verify` への段階展開設計の記録。

### Out of Scope

- 上記以外の job(`rt-verify`/`msrv-verify`/`ui-verify`/`dep-guard`/
  `evidence`)への変更。required contexts・mise タスク・profile・
  feature 集合の変更。外部 cache backend。

## Components and Boundaries

- sccache step 群(各 job 内): orchestration のみ所有。検証ロジックは
  従来どおり mise task が持ち、workflow 側は cache セットアップと統計
  観測の薄いシェルのみ追加する(既存の責務境界を踏襲)。
- `Set up sccache`(`mozilla-actions/sccache-action`
  `fc920bf0ec8de6ee65d409111f7ec508035751ba` = v0.0.11、sccache
  v0.19.0): runner への sccache 導入のみ。
- `Export sccache GHA backend env`(`actions/github-script`
  `3a2844b7e9c422d3c10d287c895573f7108da1b3` = v9.0.0): GHA cache
  service の credential(`ACTIONS_RUNTIME_TOKEN`/`ACTIONS_RESULTS_URL`)
  は JS action には届くが `run` step には届かないため、`GITHUB_ENV`
  経由で再 export し、`SCCACHE_GHA_ENABLED`/`SCCACHE_GHA_VERSION`/
  `RUSTC_WRAPPER=sccache` を後続 step へ供給する。
- `Verify sccache environment`(bash): sccache バイナリ存在・version・
  各 env の有無を fail-fast で表示(token 欠如時は warning のみで
  local disk fallback — fail-open を維持)。
- `sccache statistics`(bash): `sccache --show-stats || true` を検証
  step 後に実行し hit/miss/write error を log 証跡として残す
  (非致命的・失敗しても job 結果に影響しない)。
- kill switch: 上記 4 step はすべて `vars.CI_SCCACHE_DISABLE != '1'` を
  条件とし、repo variable 1 件の設定で全 skip(plain rustc 経路へ
  即時復帰)を実現する。workflow YAML の変更を要しない。

## Data and Control Flow

`verify` / `gw10-verify` job(OS ごと):

1. checkout → mise-action → `mise install --locked rust ...`
   → dep-access 解決 → cargo registry/git cache restore(従来どおり)。
2. **sccache step 群**(kill switch 条件付き): set up → env export →
   env verify。
3. 従来の検証 step 群(verify: prek/lint-actions/fmt-check/check/lint/
   test/lockdiff。gw10: `mise run gw10`)を `RUSTC_WRAPPER=sccache`
   配下で実行 — rustc 呼出しのみ sccache 経由となり、cargo の
   fingerprint ・test 実行・証跡は不変。
4. `sccache --show-stats`(非致命的)→ `always()` evidence 記録 →
   `always()` credential cleanup(従来どおり)。

cache keyspace:

- backend: GitHub Actions cache service。entry key は
  `sccache/x/y/z/<hash>` 形式で `SCCACHE_GHA_VERSION` が namespace
  prefix になる。
- `SCCACHE_GHA_VERSION=synvelis-v1-<RUNNER_OS>`: OS 間で entry を
  混ぜず、ruviform repo の `ruviform-v1-<RUNNER_OS>` とも衝突しない。
  verify と gw10-verify は同一 OS namespace を共有 — rustc 単位の
  workspace dep 大部分が共通するため cross-lane hit が発生する
  (実測対象)。
- sccache entry の hash 入力: compiler バイナリ hash + 全引数
  (profile/features/target)+ 前処理済みソース。したがって
  dev profile と将来の ci profile、既定 feature と webview feature、
  Rust 1.99.0 と 1.89.0、Ruviform resolved SHA の違いはすべて entry
  衝突しない — 同一 namespace 内で誤共有は構造的に発生しない。

## 段階展開・cache 分離設計(記録)

| lane | 本変更 | stage 2 設計 | namespace | 理由 |
| --- | --- | --- | --- | --- |
| verify (3 OS) | 導入 | — | `synvelis-v1-<OS>` | critical path・最大 compile 単位 |
| gw10-verify (3 OS) | 導入 | — | `synvelis-v1-<OS>`(verify 共有) | dev profile の rustc 単位が verify と大部分共通。cross-lane hit を本変更で実測 |
| rt-verify (ubuntu) | 非導入 | 同一 namespace `synvelis-v1-Ubuntu` 共有 | `synvelis-v1-Ubuntu` | dev profile かつ同一 feature 集合の test binary を verify と共有できる。列挙(`-- --list`)と本試験は同一 target dir で cargo fingerprint が重複を除去済み — sccache の効果は run 間再利用のみ。測定単位を分けるため延期 |
| msrv-verify (ubuntu, rust@1.89.0) | 非導入 | **別 namespace** | `synvelis-msrv-v1-<OS>` | compiler hash で誤共有は構造的にないが、toolchain 更新時の独立 invalidation と lane 個別無効化のため分離。toolchain pin 変更時は version bump で新 namespace |
| ui-verify (3 OS) | 非導入(NotApplicable) | cargo build/test を含む lane 化した場合のみ再評価 | (未割当) | `cargo fetch --locked` は download のみ、check/test/build は node/tsc/vitest — rustc 呼出しが存在せず sccache の効果がない |
| dep-guard / evidence | 対象外 | — | — | cargo fetch と lockfile grep、集約シェルのみ。compile なし |

RT テスト列挙の分離計測: `cargo test --workspace --locked rt_ -- --list`
(rt-guard step)と `mise run test-rt`(rt step)は同一 target dir・同一
compile 単位であり、2 回目の呼出しは cargo fingerprint で再コンパイル
しない。本変更では step 時間を run log から分離観測するのみで、job
構造・guard ロジックは変更しない(strict-prefix non-empty 選択の維持を
変更なしで担保)。

## Non-Functional Constraints

- fail-open: cache backend の write/read error・credential 欠如・
  rate limit で compile が止まらない。sccache は error 時に direct
  rustc へ fallback する設計であり、統計 step も `|| true`。
- fail-closed 検証の維持: sccache は rustc wrapper であり、cargo の
  成否・test 結果・証跡・guard の判定には関与しない。hit しても
  check/clippy/test はすべて実行される。
- 秘密情報: `ACTIONS_RUNTIME_TOKEN` は GHA cache service の ephemeral
  token であり log に値を出さない(env 名の有無のみ表示)。deploy key
  cleanup は従来どおり `always()`。
- 再現性: sccache-action は SHA pin。`mise install --locked`・
  `cargo --locked`・Cargo.lock commit 規約は不変。
- 悪化時の非展開: warm run で job 時間が改善しない OS/lane は sccache
  step を除去する(実測記録を `docs/impl/exp/` に残したうえで判断)。

## Risks and Trade-offs

- cold run で GHAC write rate limit に抵触し hit が出にくい(ruviform
  実績 36-100% write error)。6 job が同時書込する本変更でも同型の
  contention が予想されるが、fail-open で CI green は維持され、warm
  run の hit で効果を評価する。
- step 追加による冷時オーバーヘッド(sccache install + env export +
  stats で数秒〜十数秒)。warm hit で回収できるかを Before/After で
  判定し、回収できない lane は除去する。
- sccache entry は compiler+args+ソースの hash で正しく分離されるが、
  `RUSTC_WRAPPER` が乗らない経路(cargo 以外からの rustc 直接呼出し)
  では cache されない — 本 repo の検証はすべて cargo/mise 経由のため
  実害なし。
- 既存 cargo registry/git cache とは対象が別(registry index vs
  compile 成果物)で併存。actions/cache の quota を sccache entry が
  消費する点は受け入れる(10GB quota、eviction は LRU)。

## Deferred Design Concerns

- `rt-verify` 導入: stage 2。verify との namespace 共有で cross-lane
  hit を狙う設計は確定済み。実測条件が整い次第展開。
- `msrv-verify` 導入: stage 2。`synvelis-msrv-v1-<OS>` 別 namespace。
- GHAC write rate limit 対策: `SCCACHE_GHA_CACHE_WRITE=false` 併用・
  S3 backend 等は warm hit 実績が不十分な場合の次候補。

## Verification Notes

- [ ] `mise run lint-actions`(actionlint)が ci.yml 変更後も pass。
- [ ] `mise exec -- prek run --all-files` が pass。
- [ ] PR cold run で sccache step が起動し env が揃い、`--show-stats`
      で requests/executed が記録される。
- [ ] warm run で hit が発生し、write/read error があっても全 lane
      green(fail-open)。
- [ ] rt-verify/msrv-verify の step 群に `RUSTC_WRAPPER` が注入
      されていないこと(sccache step 非追加を diff で確認)。
- [ ] 10 required contexts の名称・needs・evidence 判定が不変。
