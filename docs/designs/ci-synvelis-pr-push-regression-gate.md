---
id: "DESIGN-034RoWWK4Xx4NaZSoBYZSX"
type: "design"
status: "approved"
title: "Synvelis 独立 CI — Lane 構成・証跡・依存アクセス設計"
created: "2026-11-03"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source: []
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/blob/main/.github/workflows/ci.yml"
    - "https://github.com/scarecrow173/ruviform/blob/main/.github/workflows/consumer-verify.yml"
  defers: []
  deferred-by: []
---

# Synvelis 独立 CI — Lane 構成・証跡・依存アクセス設計

## Context

Synvelis には `.github/workflows/` がなく自動 CI が存在しない。
`mise.toml` に fmt/check/lint/test/build/gw10/test-msrv の検証タスクが
揃っており、ruviform 側には実績のある参照実装(自前 `ci.yml` の lane
構成と `consumer-verify.yml` の access mode 解決・証跡形式)がある。

Cargo workspace は 8 member で ruviform への git 依存は
`Cargo.toml` で既定ブランチ追従(rev/branch 未指定)、`Cargo.lock` で
単一 resolved SHA に収束する規約(AGENTS.md)。現在 ruviform / synvelis
とも public であり、依存取得は匿名 HTTPS で完結する。private 復帰時の
credential 経路は ruviform `consumer-verify` と同じ secret 契約
(`CROSS_REPO_DEPLOY_KEY`、ruviform への read-only deploy key)を
再利用する。

依存方向と責務: 本 CI は Synvelis 自身の PR/Push Gate。ruviform 候補
revision の両 consumer 検証は ruviform #294 が Owner であり、lane を
複製しない。両者は証跡(commit SHA + ruviform resolved SHA + OS)で
相互リンクする。

## Scope

### In Scope

- `.github/workflows/ci.yml` 単一ファイルによる lane 定義
  (ruviform ci.yml と同じ単一 workflow 構成)。
- `.github/actions/ruviform-dep-access/` composite action による
  依存 access mode 解決の集約(各 lane で重複記述しない)。
- `mise.toml` への `test-rt` タスク追加と `actionlint` tool +
  `lint-actions` タスク追加。
- Job Summary への証跡テーブル出力と `evidence` 集約 job。

### Out of Scope

- RF-GT-01〜05 共有 GUI harness lane(#310〜#314 未完了部分は接続点
  の文書化に留める)、SolidJS/TS frontend lane(frontend workspace
  未導入)、実 Device/実 Window/Native GPU opt-in lane(#114 等)。
- branch protection / required check の GitHub 側設定(repo 設定は
  コード管理外)。
- self-hosted runner、Artifact DB、artifact 長期保存設計。

## Components and Boundaries

- `.github/workflows/ci.yml`: trigger・permissions・concurrency・
  job 定義のみを持つ。検証ロジックは `mise run <task>` へ委譲し、
  workflow 内に Rust コマンドを直書きしない(dep access・証跡・
  ガード系の薄いシェルは除く)。
- `.github/actions/ruviform-dep-access/action.yml`: access mode を
  `deploy_key | anonymous` に解決する composite action。`secrets`
  context は composite 内で参照できないため、呼び出し側が
  `inputs.deploy-key` に `secrets.CROSS_REPO_DEPLOY_KEY` を渡す。
  deploy_key mode では SSH key を `$RUNNER_TEMP` に chmod 600 で
  配置、github.com host key を GitHub 公表 fingerprint 3 種と照合
  pin し、`url.insteadOf` rewrite + `GIT_SSH_COMMAND` を設定する。
  どちらの経路も取れない場合は fail(依存取得不能で緑にしない)。
- `verify` job(matrix OS): `prek run --all-files` → `mise run
  fmt-check` → `mise run check` → `mise run lint` → `mise run test`
  → `git diff --exit-code Cargo.lock`(非 locked コマンドによる
  lockfile 汚染の検出)。
- `rt-verify` job(ubuntu): `cargo test --locked rt_ -- --list` で
  `(^|::)rt_[^ ]+: test$` の非空を確認してから `mise run test-rt`。
  0 件選択を PASS にしないガードは ruviform rt-verify と同型。
- `gw10-verify` job(matrix OS): `mise run gw10`(clean → 9 harness
  suite + mcp-poc transport → strict report)。GW-10 系コードに OS
  依存分岐がないため 3 OS で実行し、OS 差異の実測証跡を得る。
- `msrv-verify` job(ubuntu): `mise install rust@1.89.0` →
  `mise run test-msrv`。`rust-version = "1.89"` 宣言との一致検証。
- `dep-guard` job(ubuntu): dep access → `cargo fetch --locked`
  (manifest↔lockfile 整合) → ruviform git 依存の manifest 表記
  ガード(rev/branch 混在の禁止) → `Cargo.lock` ruviform source
  単一 SHA ガード → resolved SHA を job output `ruviform_sha` に出力。
- `evidence` job: `needs` 全 lane・`if: always()`。lane 結果・
  OS・commit SHA・ruviform resolved SHA・toolchain を証跡テーブルに
  集約し、未導入 lane(RF-GT/frontend/実機/Visual/AudioLens)を
  NotRun 行として固定記録。全 lane not-run/証跡ゼロは fail、1 lane
  でも failure/cancelled なら fail。

## Data and Control Flow

1. `pull_request`(main 向け)/`push`(main)/`workflow_dispatch` が
   workflow を起動。concurrency group は workflow+ref 単位で、
   main 以外は in-progress run を cancel。
2. 各 lane は独立して並列実行(verify/rt/gw10/msrv/dep-guard)。
   各 lane の冒頭で `ruviform-dep-access` が mode を解決し、
   `CARGO_NET_GIT_FETCH_WITH_CLI=true` 経由の cargo git fetch が
   その経路を使う。
3. lane 内の各 step 結果は `GITHUB_STEP_SUMMARY` に lane 別証跡行
   として記録(run_id・commit・OS・toolchain・ruviform SHA)。
4. `evidence` が全 lane の `result`/`outputs` を集約し、証跡ゼロ
   run を fail にする単一 gate として機能する(branch protection
   の required check 候補)。

## Non-Functional Constraints

- `permissions: contents: read` のみ。GITHUB_TOKEN 既定のまま。
- 秘密情報を echo/ログしない。deploy key の書き込み先は
  `$RUNNER_TEMP`・chmod 600・`always()` cleanup(unset insteadOf +
  keyfile 削除)を各 lane に置く。
- host key pin の fingerprint 定数は GitHub 公式公表値を使用
  (rotation 時は workflow 定数の更新が必要 — 失敗時の error
  message にその旨を出す)。
- cache は `~/.cargo/registry` / `~/.cargo/git/db` を
  `runner.os`-`Cargo.lock` hash で。`CARGO_INCREMENTAL=0`。
- `GIT_LFS_SKIP_SMUDGE=1` を cargo fetch 系 job env に付与
  (依存 checkout の LFS smudge 回避、ruviform consumer-verify
  synvelis lane と同じ対策)。
- 各 job に `timeout-minutes` を設定(verify/gw10: 40、rt/dep-guard:
  20、msrv: 60、evidence: 10)。
- bash step は GitHub Hosted Windows runner の Git Bash で動作する
  構文のみ使用。

## Risks and Trade-offs

- macOS lane の実挙動はローカルで検証不可 — 初回 run で platform
  差異が出た場合は platform finding として扱い、fail-fast: false
  で他 OS の証跡を確保する。
- gw10 を 3 OS matrix にすると runner コストが増えるが、Gateway の
  transport/recovery 経路は OS 形の差異を持ち得るため、実測価値を
  優先。重い場合は ubuntu 固定への縮退を後続で判断できる。
- `test-rt` の `rt_` フィルタは部分文字列一致で他テストも拾う
  (ruviform 同様) — 厳密性は非空ガードが担保し、絞り込みすぎて
  0 件化するより保守的。
- composite action への secret 受け渡しは `inputs` 経由の公式
  パターン。key はログに出さず、file path のみ step 出力に出す。
- `lfs: true` checkout + `mise install git-lfs` は現在 LFS オブジェクト
  0 件でも将来のメディア fixture 追加に備えた前倒し(コスト微小)。

## Deferred Design Concerns

- RF-GT-01 Browser Playwright / RF-GT-02 IPC conformance lane | ruviform #310/#311 harness 未完了 | 共有 harness 公開時に lane 追加 | GUI 契約回帰が PR Gate に乗らない
- RF-GT-03/04 Secure Test Host / OS Driver・SV-GUI-05(#114)opt-in lane | 実機 runner 前提 | #312/#313/#114 完了時 | 実 OS/Window 差異の未検出
- SolidJS+TS build/typecheck/component lane + 本番 frontend Playwright | frontend workspace 未導入 | SV-GUI-01〜(#110〜)実装時 | frontend 回帰なし
- Visual Golden(#314)| ruviform 側基盤未確定 | #314 完了時 | 視覚回帰なし
- AudioLens consumer 回帰接続 | ruviform #294/audio-lens #142 側の責務 | 横断 Gate 整備時 | なし(証跡形式互換は本設計で確保)
- Feature 依存 lane(Recording 実 Device/Plugin Binding/PPQN-MIDI/Automation)| 各 Feature 実装待ち(Issue #83 初期/後続区分) | 各 Feature 実装開始時 | 現状 headless 契約層のみのカバー

## Verification Notes

- [ ] `mise exec -- actionlint` で workflow 構文検証が PASS する。
- [ ] `prek run --all-files` の `check-yaml` が workflow を受理する。
- [ ] 単一 SHA ガードのシェルロジックが現在の Cargo.lock(21 source
      行・単一 SHA)で PASS し、rev/branch 注入した manifest で
      fail することを確認する(ローカル試験)。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `gw10` が
      ローカル(Windows)で PASS する。
- [ ] `rt_` 非空ガードの grep パターンが現在のテスト一覧で 2 件
      (rt_control_eval_and_lower_alloc_free・rt_path_is_alloc_and_drop_free)
      を数える。
