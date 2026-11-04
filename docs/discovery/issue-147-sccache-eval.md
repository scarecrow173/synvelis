---
id: "DISC-034RIJkpAqpAIZlwbFLKgU"
type: "discovery"
status: "resolved"
title: "Issue #147 sccache コンパイルキャッシュ先行評価"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/147"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/issue-147-sccache-compile-cache.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ci-synvelis-pr-push-regression-gate.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-147-sccache-compile-cache.md"
    - "docs/designs/issue-147-sccache-compile-cache.md"
    - "docs/plans/issue-147-sccache-compile-cache.md"
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "Cargo.toml"
  defers: []
  deferred-by: []
---

# Issue #147 sccache コンパイルキャッシュ先行評価

## Summary

Synvelis 独立 CI の critical path は `verify (windows-latest)`(直近 main
run で 198-314s)で、Rust コンパイルが支配的。ruviform #329/PR #342 で
sccache(GHA cache backend)が verify lane 47-62% hit・job 10-30% 削減・
fail-open・kill switch を実証済み。同パターンを synvelis の OS / toolchain
/ feature / profile / Ruviform SHA 条件で検証する(confirmed: 導入評価
妥当、ただし実測で悪化する lane は無条件展開しない)。

## Exploration Goal

Issue #147 (CI-OPT-01) の実装範囲 1-6 に必要な前提を確定する:
sccache 導入対象 lane の選定、cache namespace 分離設計(MSRV toolchain・
feature・profile・Ruviform revision の誤共有防止)、RT テスト列挙
コンパイルと本試験の分離計測方法、保護対象(10 required contexts・RT
strict selection・GW-10 各ゲート・dep-guard・evidence)の非回帰条件。

## Key Issues and Assumptions

- `verify` job は `check`/`lint`/`test` の 3 cargo 呼出しを dev profile で
  逐次実行する(confirmed: mise.toml L35-49)。`check`/`clippy` は metadata
  系、`test` は codegen を含むため fingerprint 差で別 entry になるが、
  sccache は rustc 呼出し単位で cache するため両系統を吸収できる。
- `rt-verify` は `cargo test --workspace --locked rt_ -- --list`(列挙用に
  全 test binary をコンパイル)→ `mise run test-rt`(同 binary を実行)の
  2 段構成。同一 job 内では cargo fingerprint で再コンパイルは発生しない
  (confirmed: 同一 target dir・同一 feature set)。sccache の効果は
  run 間の再利用に限られる。
- `gw10-verify` は `cargo run -p synvelis-gateway --bin gw10_report` +
  `cargo test -p synvelis-gateway --test gw10_*` + `cargo test -p mcp-poc
  --test gw10_transport` で、verify と同一 dev profile の rustc 単位が
  大部分を共有する(confirmed: mise.toml L59-66)。同一 OS namespace
  共有で cross-lane hit が期待できる。
- `msrv-verify` は rust@1.89.0、他 lane は rust 1.99.0。sccache の hash
  入力にコンパイラ本体が含まれるため同一 namespace でも誤共有しないが、
  独立無効化・独立 invalidation のため `synvelis-msrv-v1-<OS>` 別
  namespace を設計として記録(本 PR では未導入)。
- `ui-verify` は `cargo fetch --locked`(download のみ)+ `sync-ui-deps` +
  npm 系 task で rustc 呼出しを含まない。sccache は無効(confirmed:
  ci.yml L414-438)。NotApplicable として記録。
- dep-access は anonymous https / deploy key の 2 経路で、sccache とは
  無関係(cargo git fetch は cache 対象外。`CARGO_NET_GIT_FETCH_WITH_CLI`
  も不変)。
- sccache の hash 入力は compiler バイナリ + 全引数 + 前処理済みソース。
  profile/features/target OS が異なれば entry が分離されるため、同一
  namespace 内で feature/profile 混在しても誤った成果物を返さない
  (ruviform #329 §3 でも ui-verify `webview` feature と verify 既定
  feature の分離を実測確認済み)。
- GHAC backend の cold run write rate limit は ruviform で観測済み
  (36-100% write error)だが fail-open で CI green を維持する。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| sccache + GHA cache backend(ruviform 同型) | 実証済み手順・kill switch・fail-open・追加 infra 不要 | cold run write rate limit で初回 hit が出にくい | yes |
| actions/cache で `target/` 全体を保存 | 実装が単純 | target dir は数百 MB〜GB で cache 保存/復元コストが大きく、fingerprint 変化で stale 化。check/clippy/test で部分更新も難しい | no |
| Swatinem/rust-cache | 導入 1 step | profile ごとの部分 hit 制御が粗く、MSRV/dev toolchain 分離の明示設計がしにくい。ruviform 実績との一貫性に欠ける | no |
| sccache + S3 等外部 backend | rate limit 回避 | 認証情報管理が新設要。GHAC 実績が先 | no(将来候補) |
| 何もしない | 変更なし | Windows verify の critical path が残る | no |

## Tentative Conclusions and Hypotheses

- confirmed: `verify`(3 OS)へ ruviform 同型の sccache step 群を導入。
  namespace は `synvelis-v1-<RUNNER_OS>`。kill switch は repo variable
  `CI_SCCACHE_DISABLE=1`。
- hypothesis(実測で判定): `gw10-verify`(3 OS)へも同時導入し、verify と
  namespace 共有による cross-lane hit を計測する。悪化が確認された
  lane/OS では除去する(不採用も正当な成果)。
- confirmed: `rt-verify`/`msrv-verify` は本 PR で sccache を導入せず、
  namespace 分離設計のみ記録する(段階展開 stage 2)。
- confirmed: `ui-verify` は rustc 非呼出しのため sccache 対象外
  (NotApplicable)。
- confirmed: RT strict-prefix non-empty guard・GW-10 各 gate 独立検証・
  MSRV/RT の 0 件検出・dep-guard・evidence・10 required contexts の
  構成・名称・fail-closed 性は一切変更しない。
- hypothesis: Windows verify lane で最も効果が出る(ruviform で
  -29.3% の実績、synvelis でも windows が最長 leg)。

## Open Questions

- なし(採否は PR 上の cold/warm 実測で判定する運用条件であり、
  事前回答を要する設計上の未確定事項は残っていない)。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/issue-147-sccache-compile-cache.md`
- [ ] adr-doc needed: なし(キャッシュ戦略は既存 CI 設計
  `docs/designs/ci-synvelis-pr-push-regression-gate.md` の運用最適化
  範囲。新規 architecture decision ではなく実験記録 `docs/impl/exp/`
  で追跡)

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/147
- 先行評価: ruviform https://github.com/scarecrow173/ruviform/issues/329 /
  https://github.com/scarecrow173/ruviform/pull/342 /
  `F:/repositry/audio-products/ruviform/docs/impl/exp/issue-329-sccache-compile-cache.md`
- Baseline 契約: ruviform https://github.com/scarecrow173/ruviform/issues/327
  (CI-OPT-00: OS/toolchain/profile/Cargo.lock/Ruviform SHA と Cold/Warm
  比較条件の記録義務)
- 上流 CI 設計: `docs/designs/ci-synvelis-pr-push-regression-gate.md`
  (Issue #83)

## Evidence

- 2026-11-04 Before 計測(main 直近 run、job wall time 秒):
  - run 41810580452 (main 4d21d2f): verify u/w/m = 139/239/194,
    rt = 92, gw10 u/w/m = 47/101/56, msrv = 104, dep-guard = 22
  - run 41809620685 (main 48030c5): verify = 95/314/194, rt = 95,
    gw10 = 46/76/53, msrv = 144
  - run 41790537327 (main 4c536b0): verify = 112/228/192, rt = 85,
    gw10 = 43/106/36, msrv = 134
  - run 41816354694 (main 16e10c6、ui-verify 導入後):
    verify = 144(fail)/198/143, rt = 91, gw10 = 35/79/63,
    msrv = 200, ui-verify u/w/m = 36/88/54
  - 傾向: verify windows が常に critical path(198-314s)。gw10 は
    35-106s で verify より軽いが 3 OS 分の runner 時間を消費する。
- run 41816354694 の verify(ubuntu)失敗は `mcp-poc --test
  gw10_transport::gw10f_delegated_apply_commits_and_survives_restart_real`
  の `Connection reset by peer`(real transport フレーク)。sccache とは
  無関係の既存 flake であり、PR 上で再発した場合は rerun で対処し、
  スコープ外失敗として記録する。
- ruviform #329 実測(2026-11-04): verify lane hit 47-62%、job -10〜-30%。
  cold run GHAC write error 36-100% → warm run では verify 系 0。
  kill switch `CI_SCCACHE_DISABLE=1` repo variable で 4 step skip を実証。
  sccache-action pin `fc920bf0ec8de6ee65d409111f7ec508035751ba` = v0.0.11、
  sccache v0.19.0。
- actions/cache による `~/.cargo/registry|git` cache は全 lane で既存。
  sccache とは対象が別(registry/git index vs コンパイル成果物)で併存可。
