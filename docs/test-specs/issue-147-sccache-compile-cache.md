---
id: "TSPEC-034RIJsxayw9vKD3Ez9Wcy"
type: "test-spec"
status: "approved"
title: "Issue #147 sccache 導入検証仕様"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-147-sccache-compile-cache.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-147-sccache-compile-cache.md"
    - "docs/plans/issue-147-sccache-compile-cache.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-147-sccache-compile-cache.md"
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #147 sccache 導入検証仕様

## Purpose

Issue #147 が要求する「CI 高速化のためのコンパイルキャッシュ」を、
検証品質・証跡・required contexts を犠牲にしない形で導入するという
不変条件を pin する。cache 実装の形が将来変わっても(backend 切替・
lane 展開)、ここに挙げる保証は維持されなければならない。

## Feature

sccache コンテントアドレス型コンパイルキャッシュの `verify` /
`gw10-verify` lane(各 3 OS)への導入 — GHA cache backend・kill
switch・fail-open・hit 統計の観測を備え、残りの検証契約は不変。

## Rules

- Rule 1: sccache step 群は `verify`・`gw10-verify` のみに存在し、
  `rt-verify`・`msrv-verify`・`ui-verify`・`dep-guard`・`evidence`
  には `RUSTC_WRAPPER`・sccache 参照がない。
- Rule 2: 全 sccache step は `vars.CI_SCCACHE_DISABLE != '1'` で条件
  付けられ、repo variable のみで全 skip となる(kill switch)。
- Rule 3: `SCCACHE_GHA_VERSION` は `synvelis-v1-<RUNNER_OS>` で一貫。
  MSRV 用設計は `synvelis-msrv-v1-<OS>` の別 namespace(本変更では
  msrv lane 非導入)。
- Rule 4: fail-open。cache write/read error・credential 欠如・stats
  失敗で compile・test が止まらない(`sccache --show-stats || true`、
  credential 欠如は warning のみ)。
- Rule 5: 検証ロジックは mise タスク委譲のまま。workflow に cargo
  検証コマンドを直書きしない。
- Rule 6: 10 required contexts の job 名・`evidence` needs・
  not-run-is-not-pass 判定・各 lane の証跡 step / cleanup `always()`
  は変更しない。
- Rule 7: `rt-verify` の strict-prefix non-empty guard、GW-10
  harness の gate 独立検証、dep-guard の単一 SHA/branch-follow
  guard、MSRV lane の rust@1.89.0 実行はいずれもロジック非変更。
- Rule 8: 実測は cold run・warm run の `sccache --show-stats`
  (requests/executed/hits/misses/write errors)と job wall time で
  行い、悪化する lane/OS では展開しない。

## Examples

- cold run(初回 push): `Verify sccache environment` で
  `RUSTC_WRAPPER=sccache`・`SCCACHE_GHA_ENABLED=true`・
  `SCCACHE_GHA_VERSION=synvelis-v1-Windows` が表示され、compile は
  実行される(write error が出ても job は継続)。
- warm run(続く push): `--show-stats` で `hits > 0`。hit 分の
  compile 時間が job wall time に反映される。
- repo variable `CI_SCCACHE_DISABLE=1` 設定下の run: 全 sccache
  step が `skipped` となり、plain rustc で green(kill switch)。
- gw10-verify の warm run で verify が書いた workspace dep entry に
  hit する(cross-lane hit、同一 OS namespace)。

## Guarantees

- sccache は rustc wrapper であり、cargo の成否判定・test 結果・
  guard・証跡には関与しない — hit しても check/clippy/test は全て
  実行される。
- MSRV 1.89.0 の成果物を dev toolchain lane が誤用しない(compiler
  hash で entry 分離 + msrv lane 非導入)。
- RT strict-prefix non-empty 選択、GW-10 各 Gateway/Job/MCP/
  Persistence/Authorization の独立検証、MSRV/RT 0 件検出の維持、
  headless/実 OS 証拠境界、dep-guard、evidence 集約、10 required
  contexts はいずれも不変。
- cache miss・破損・backend 障害で正規ビルドへ fallback する。

## Non-goals

- `cargo check`/`clippy`/`test` の step 統合や job 再構成による
  重複削減(証跡粒度維持のため対象外。重複は sccache が吸収する)。
- GHAC rate limit 根治(S3 backend 等。別検討)。
- self-hosted runner・toolchain 更新・cargo profile 変更。

## Risk

- これらの保証を落とすと、cache hit による誤成果物の再利用・
  required context 弱化・RT/GW-10/MSRV の 0 件 Pass 化といった
  CI の信頼性回帰が発生する。fail-open を外すと cache backend
  障害で全 lane が fail する。
