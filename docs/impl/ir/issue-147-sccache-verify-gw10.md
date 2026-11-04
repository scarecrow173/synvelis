---
id: "IMPL-034RJ4QUQGExYEXb9HnIW3"
type: "impl"
status: "completed"
title: "Issue #147 sccache verify/gw10 導入実装記録"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/147"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-147-sccache-eval.md"
      - type: "file"
        path: "docs/specs/issue-147-sccache-compile-cache.md"
      - type: "file"
        path: "docs/designs/issue-147-sccache-compile-cache.md"
      - type: "file"
        path: "docs/plans/issue-147-sccache-compile-cache.md"
      - type: "file"
        path: "docs/test-specs/issue-147-sccache-compile-cache.md"
      - type: "file"
        path: "docs/tasks/issue-147-t1-sccache-verify-gw10.md"
      - type: "file"
        path: "docs/impl/exp/issue-147-sccache-compile-cache.md"
      - type: "file"
        path: "docs/impl/exp/issue-147-sccache.jsonl"
      - type: "file"
        path: "docs/impl/ir/issue-147-sccache-verify-gw10.md"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "docs/impl/ir/syn-obs-01-host-logging-owner-obs-02-source-catalog-synvelis-app.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/discovery/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/issue-147-t1-sccache-verify-gw10.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "docs/impl/exp/issue-147-sccache-compile-cache.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-147-sccache-compile-cache.md"
    rejected: []
---

# Issue #147 sccache verify/gw10 導入実装記録

## Summary

CI の `verify`(3 OS)と `gw10-verify`(3 OS)に sccache v0.19.0
コンパイルキャッシュ(GitHub Actions cache backend・OS 別 namespace
`synvelis-v1-<RUNNER_OS>`)を導入した。kill switch
`CI_SCCACHE_DISABLE=1` を実測で検証し、cold/warm 両 run で全 13 job
green・warm hit 75–100%・cache error 0 を確認。`rt-verify`・
`msrv-verify`・`ui-verify` は分離設計のみ記録し今回不導入(段階展開)。

## Context

`docs/tasks/issue-147-t1-sccache-verify-gw10.md` →
`docs/plans/issue-147-sccache-compile-cache.md` →
`docs/designs/issue-147-sccache-compile-cache.md` →
`docs/specs/issue-147-sccache-compile-cache.md`(← discovery)のチェーンに
従って実装。Issue #147(Synvelis CI-OPT-01: Windows bottleneck を主因とする
Rust CI 高速化、sccache 採用評価)の対応。先行評価は ruviform #329/PR #342
(verify hit 47–62%・job 10–30% 削減・kill switch 検証済み)だが、結果は
転用せず Synvelis の OS/toolchain/profile/Ruviform SHA で実測した。

## Implementation

### Changes

- `.github/workflows/ci.yml`:
  - `verify` と `gw10-verify` の cargo cache restore 直後に 4 step を
    追加(全て `if: vars.CI_SCCACHE_DISABLE != '1'`):
    - `Set up sccache`: `mozilla-actions/sccache-action`
      `fc920bf0ec8de6ee65d409111f7ec508035751ba`(v0.19.0)に SHA pin。
    - `Export sccache GHA backend env`: `actions/github-script` で
      `ACTIONS_RUNTIME_TOKEN`/`ACTIONS_RESULTS_URL` を context から
      再 export し、`SCCACHE_GHA_ENABLED=true`・
      `SCCACHE_GHA_VERSION=synvelis-v1-${RUNNER_OS}`・
      `RUSTC_WRAPPER=sccache` を `GITHUB_ENV` に設定。
    - `Verify sccache environment`: version・`RUSTC_WRAPPER`・backend
      変数・credential 存在を確認(fail-closed、cache 未接続なら失敗)。
    - `sccache statistics`: `sccache --show-stats || true` で
      hit/miss/error を job log に出力(統計失敗で lane を落とさない)。
  - `verify`/`gw10-verify` 以外の job(`rt-verify`・`msrv-verify`・
    `ui-verify`・`dep-guard`・`evidence`)には step を追加していない
    (diff 確認済み)。
- 別論理 commit: `docs/impl/ir/syn-obs-01-...md` の `defers` 4 件を
  外部 GitHub issue URL 化(doc graph の blocking 解消。既存不整合の
  最小修正)。

### Decisions During Implementation

- OS 別 namespace のみで lane 共有: `SCCACHE_GHA_VERSION` に lane 名を
  含めず、同一 OS 内で verify/gw10-verify が entry を共有する設計。
  sccache の cache key(compiler hash・全引数・ソース fingerprint)が
  feature/profile/toolchain 差を分離するため、同一 OS・同一
  toolchain(1.99.0)の lane 間共有は安全で hit 率を最大化できる
  (warm の gw10 ubuntu 100% が verify 書込みを含む共有の実証)。
- MSRV lane 非導入: rust 1.89.0 と通常 1.99.0 の成果物誤共有を
  構造的に排除(compiler hash でも分離されるが、lane 自体を外して
  二重の防御にした)。rt-verify は段階展開(stage 2 候補)として
  分離方針のみ exp §5 に記録。
- gw10 windows/macos の wall 微増(+6〜12s vs Before 中央)は
  誤差範囲内・critical path 外と判断し、OS 別条件分岐の複雑さを
  入れず全 OS 統一で採用(lane 合計ほぼ中立、ubuntu -32%)。

## Related Experiments

### Adopted

- `docs/impl/exp/issue-147-sccache-compile-cache.md`: cold(run
  41823038077)/warm(run 41824169427)の Before/After 実測。
  verify warm hit 75–90%・Before 中央値比で全 OS 改善
  (windows -16s・ubuntu -14s・macos -16s)、gw10 hit 88–100%。
  判定: verify・gw10-verify ともに採用。

### Rejected

- `rt-verify`/`msrv-verify`/`ui-verify` への今回導入: 段階展開の
  分離設計に留めた(不採用ではなく deferred。exp §5/§7 に条件記録)。

## Validation

実行済みで成功した項目のみ記録(2026-11-04):

ローカル:

- `mise run lint-actions`(actionlint)— PASS(ci.yml 変更後)。
- `mise exec -- prek run --all-files` — PASS。
- doc-maintenance regenerate — 6 index 再生成、既存エントリ保持。
  残 171 findings は pre-existing の legacy front matter 系(今回非対象)。

CI(PR #155、`gh pr checks --watch` で完了確認):

- cold run 41823038077: 全 13 job PASS(verify 3OS・gw10 3OS・rt・
  msrv・ui 3OS・dep-guard・evidence)。sccache stats: verify hit
  1.35–9.05%・write errors 0–47(GHAC rate limit)でも fail-open で
  全緑。read/cache errors 0。
- warm run 41824169427: 全 13 job PASS。verify hit
  74.89–90.05%・gw10 hit 87.88–100%・errors 全 0。job wall:
  verify 112/223/177s(u/w/m)、gw10 35/101/69s — Before 4 run
  中央値対比で verify 全 OS 改善、gw10 は lane 合計ほぼ中立。
- kill switch: `CI_SCCACHE_DISABLE=1` 下で `gw10-verify
  (ubuntu-latest)` 単体 rerun — sccache 4 step 全 skip・harness
  success(変数は検証後に削除済み)。
- RT 分離計測(cold run): 列挙+strict カウント 56s vs 本試験 4s —
  non-empty RT 選択の維持と列挙コンパイル支配コストを確認。

未実行:

- cache 破損注入による read error 実証(fail-open は cold write
  error 実績 + 設計上 `|| true`/wrapper fallback で担保)。
- rt-verify/msrv-verify/ui-verify の sccache 有効化計測(今回
  スコープ外・stage 2 候補)。

## Risks

- GHAC rate limit による cold write error(最大 47)は既知挙動。
  fail-open で compile/test は完走するが、write 失敗分は次 run の
  hit に寄与しない(warm では 0 に収束)。
- 観測は cold/warm 各 1 run。runner 負荷で wall は変動するため、
  継続監視で回帰を検出する必要がある。
- `SCCACHE_GHA_VERSION` を変更しない限り古い entry が残る。
  toolchain/profile 変更時は key 自体が変わるため旧 entry は自然に
  miss 化するが、namespace バージョン(`synvelis-v1` → `v2`)で
  全破棄も可能。

## Follow-ups

- stage 2 候補: `rt-verify` への sccache 導入(列挙コンパイル 56s が
  支配コスト。`synvelis-v1-Ubuntu` で verify の test compile 単位と
  大部分を共有できる見込み。厳格 non-empty 選択の維持を条件に)。
- `ui-verify` 再評価(npm 主体で Rust compile 対象が限定的。
  ruviform でも ui lane の hit 率は低く分散大)。
- 継続監視: 複数 run で hit 率・wall の推移を確認し、悪化が続く
  OS/lane があれば `CI_SCCACHE_DISABLE=1` で即時切離し可能。
