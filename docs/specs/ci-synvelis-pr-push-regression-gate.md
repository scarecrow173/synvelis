---
id: "SPEC-034RoWOUkW5OezJQSLBWKd"
type: "spec"
status: "approved"
title: "Synvelis 独立 PR/Push CI — Workspace/RT/Recording/Gateway 回帰 Gate"
created: "2026-11-03"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/83"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/ruviform/issues/219"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/310"
    - "https://github.com/scarecrow173/ruviform/issues/311"
    - "https://github.com/scarecrow173/ruviform/issues/312"
    - "https://github.com/scarecrow173/ruviform/issues/313"
    - "https://github.com/scarecrow173/ruviform/issues/314"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/83"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Synvelis 独立 PR/Push CI — Workspace/RT/Recording/Gateway 回帰 Gate

## What Should Be Built

`.github/workflows/ci.yml` を新設し、Synvelis 自身の `pull_request`
(main 向け)・`main` への `push`・`workflow_dispatch` で自動実行される
独立 CI を構築する。検証の正規入口は `mise.toml` のタスクで、GitHub
Hosted Runner 上の GitHub Actions のみで構成する。

提供する lane:

- `verify`: OS matrix (ubuntu-latest / windows-latest / macos-latest) で
  `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test`(--locked) + `prek run --all-files` の設定ファイル検査。
- `rt-verify`: RT no-alloc/no-block 契約テスト(`rt_` 接頭辞)を
  非空選択ガード付きで実行(0 件を PASS にしない)。
- `gw10-verify`: `mise run gw10`(Gateway GW-10 evidence-gated harness
  全体: contract/security/concurrency/recovery + MCP transport +
  strict 集計)を OS matrix で実行。
- `msrv-verify`: `mise run test-msrv`(rust-version = 1.89 宣言の
  toolchain で `cargo test --workspace --locked`)。
- `dep-guard`: ruviform git 依存の access mode 解決(匿名 HTTPS /
  deploy key)、`cargo fetch --locked` による manifest↔lockfile
  整合、Cargo.lock 内 ruviform 単一 resolved SHA ガード。
- `evidence`: lane 結果・commit SHA・ruviform resolved SHA・OS・
  toolchain を集約した証跡テーブルを Job Summary に出力し、全 lane
  未実行や証跡ゼロの run を fail にする集約 gate。

## Why Now

Issue #83。main に `.github/workflows/` が存在せず、Synvelis 自身の
PR/Push に対する自動検証がない状態。Execution Plan #23 / #28 の継続的
回帰検証を PR 単位で fail-closed にする必要がある。ruviform 側の
cross-consumer `verify-synvelis`(#219/#294)は候補 revision 横断 Gate
であって本 CI の代替ではなく、両者は証跡(commit SHA + ruviform
resolved SHA + OS)で相互リンク可能であるべき。

## Users and Value

- Synvelis maintainer: PR マージ前に Workspace/RT/Recording/Gateway
  回帰を OS 横断で検出できる。失敗が領域別に分類され、どの lane が
  何を検証したか証跡として残る。
- ruviform maintainer: Synvelis 側 CI の証跡形式が consumer-verify
  と同じ evidence 行構造を持ち、候補 revision 検証と突き合わせ
  られる。
- 外部 contributor(public repo): 秘密情報なしで CI が完走する。

## Scope

### In Scope

- PR/main push/手動 dispatch を trigger とする GitHub Actions
  workflow(最小権限 `contents: read`、concurrency cancel、cache)。
- 上記 6 lane の定義と、mise タスクへの薄い委譲(新規 `test-rt`
  タスク + actionlint を mise tool として追加)。
- ruviform git 依存アクセス: public 期は匿名 HTTPS(秘密ゼロ)、
  private 復帰時は read-only deploy key secret
  (`CROSS_REPO_DEPLOY_KEY`、ruviform consumer-verify と同一契約)
  への切替を workflow 内で吸収。
- 証跡: `run_id`・対象 commit・`Cargo.lock` resolved SHA・
  OS/toolchain・lane 結果(Pass/Fail/NotRun/Unsupported)を Job
  Summary に記録。NotRun/0 件テストを PASS にしない強制。
- workflow 自体の構文検証(actionlint)。

### Out of Scope

- 実 OS/実 Device/実 Window/実 Plugin/Native GPU の opt-in/専用
  Runner lane(RF-GT-03/04 = ruviform #312/#313、SV-GUI-05 = #114)。
  接続点の文書化のみ。
- RF-GT-01(#310) Browser DOM/Accessibility、RF-GT-02(#311) IPC
  Fixture、Visual Golden(#314)の実行 — ruviform 側 harness 未完了。
  SolidJS+TS frontend lane — Synvelis に frontend workspace 未導入。
  いずれも NotRun として証跡に明記し、接続点を設計文書に残す。
- Recording 実 Device(#115〜118)・ControlOwner/Plugin Binding
  (#121/#122)・PPQN/MIDI 精度(#125〜128)・Automation 領域(#131〜134)
  等の Feature 依存 lane — 実装開始後に追加する(Issue #83 明示)。
- ruviform 候補 revision の両 consumer 横断 Gate(ruviform #294 が
  Owner)。本 CI はその lane を複製しない。
- AudioLens #142 回帰 — audio-lens 側/ruviform cross-consumer lane
  の責務。本 CI は証跡形式の互換のみ保つ。
- self-hosted runner・専用 Artifact DB・二重 Runner の構築。

## Requirements

- workflow は `contents: read` 最小権限。秘密情報をログへ出力せず、
  deploy key は `$RUNNER_TEMP` に chmod 600 で配置し `always()`
  cleanup で除去する。
- public 匿名 HTTPS で ruviform git 依存を取得できること。private
  復帰時は最小権限 credential(read-only deploy key)で同一 lane が
  読み取り継続できること。
- `cargo` は `--locked` 経路で lockfile 整合を fail-closed に検証
  し、ruviform 依存が単一 resolved SHA に収束することを機械的に
  保証する。manifest/lockfile の revision 分裂は fail。
- lane 結果は OS/runner/toolchain/commit/resolved SHA と共に証跡化
  し、ruviform cross-consumer 側と相互リンク可能な形式にする。
- 全 lane が skip/not-run の run は緑にしない(証跡ゼロは fail)。
- 既存 `mise run fmt-check/check/lint/test/build/gw10/test-msrv`
  の意味を変更しない。
- macOS/Windows など環境依存で未検証の範囲は NotRun/Unsupported
  として明示し、PASS に計上しない。

## Acceptance Criteria

Issue #83 の Acceptance Gates を正本とし、本 spec では次を観測可能な
基準として固定する:

- [ ] PR ・ main push ・ workflow_dispatch の 3 trigger で workflow
  が起動し、対象 branch/commit・OS・feature・依存 resolved SHA・
  失敗 lane が run 単位で追跡できる。
- [ ] 少なくとも Linux で workspace の check/test と GW-10 harness
  が実行され、fmt/lint も検査される(全ゼロ・全 skip を PASS に
  しない非空ガード付き)。
- [ ] Windows/macOS lane が実行され、環境依存で実行不能な部分は
  NotRun/Unsupported として証跡に区別される。
- [ ] `cargo fetch --locked` 系の整合検査と ruviform 単一 SHA
  ガードが fail-closed で動作する。
- [ ] public 匿名 HTTPS で ruviform 依存を解決でき、deploy key
  fallback 経路が workflow 内に実装されている(秘密情報非出力)。
- [ ] RT no-alloc 契約テストが非空選択ガード付きで実行される。
- [ ] MSRV(1.89)lane が rust-version 宣言と一致して実行される。
- [ ] evidence job が lane/OS/commit/resolved SHA/toolchain/結果を
  集約し、実行 lane ゼロの run を fail にする。
- [ ] 未導入 lane(GUI harness/frontend/実機)が NotRun として証跡に
  明記され、接続点が設計文書に残る。
- [ ] 既存 mise タスクの意味・成功条件を弱めていない。

## Deferred Design Concerns

- RF-GT-01/02/03/04 lane 接続 | ruviform #310〜#313 の harness 未完了 | ruviform 側が共通 harness を公開次第、本 workflow に lane 追加 | GUI 回帰が PR Gate で未検出のまま進む
- SolidJS+TS frontend lane | frontend workspace 未導入(#110〜#114 実装待ち) | SolidJS 本番 frontend の初回導入時 | UI 回帰を headless contract 層だけでカバーする期間が延びる
- Recording 実 Device / 実 Window / Native GPU opt-in lane | 実機アクセス可能な runner 環境が未整備 | RF-GT-03/04・SV-GUI-05(#114)完了時 | 実機依存の不具合が headless CI をすり抜ける
- Visual Golden(#314)lane | ruviform 側 Golden 基盤未確定・Native は #262 採否待ち | #314 完了時 | 視覚回帰の検出がない
- AudioLens consumer 側回帰の接続 | 本 repo の責務外(ruviform #294 / audio-lens #142) | 横断 Gate 実装時 | なし(責務は横断 Gate 側)

## Sources and Relations

- 正本: [synvelis #83](https://github.com/scarecrow173/synvelis/issues/83)
  (Acceptance Gates・GUI Testing v1.0 追補・SolidJS 追補)。
- 横断 DAG: [synvelis #23](https://github.com/scarecrow173/synvelis/issues/23)、
  [synvelis #28](https://github.com/scarecrow173/synvelis/issues/28)。
- 参照実装: ruviform `.github/workflows/ci.yml`(lane 構成・mise 経由・
  非空ガード)と `consumer-verify.yml`(access mode 解決・known_hosts
  pin・証跡テーブル・not-run ≠ pass 強制)。
- 接続点 Issue: ruviform #219/#294/#310/#311/#312/#313/#314、
  synvelis #103/#109/#110〜#114/#115〜#118、audio-lens #142。
