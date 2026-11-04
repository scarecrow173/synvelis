---
id: "TSPEC-034RoWXtHq8FkSjQ9NWwqn"
type: "test-spec"
status: "approved"
title: "Synvelis 独立 CI ワークフロー検証仕様"
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
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Synvelis 独立 CI ワークフロー検証仕様

## Purpose

Synvelis 独立 CI(`.github/workflows/ci.yml` + dep-access composite
action + mise 追加タスク)が、Issue #83 の fail-closed 性質
(実行不能は緑にしない・lockfile/依存 SHA 整合・証跡記録)を満たす
ことを固定する。GUI harness・frontend・実機 lane が実装された時点で
本 spec を拡張し、CI が撤廃・再設計された場合に退役する。

## Feature

Synvelis PR/Push 独立 CI は、PR・main push・手動 dispatch で起動し、
workspace 検証・RT 契約・GW-10 harness・MSRV・依存整合を lane 別に
実行し、結果を証跡として集約する。

## Rules

- R1: trigger は `pull_request`(main 向け)・`push`(main)・
  `workflow_dispatch` のみ。permissions は `contents: read`。
- R2: 依存取得は匿名 HTTPS が既定。`CROSS_REPO_DEPLOY_KEY` が設定
  された場合のみ SSH deploy key 経路を使い、key は `$RUNNER_TEMP`
  に chmod 600・host key fingerprint pin・`always()` cleanup。
  秘密値をログに出さない。どちらの経路も無い場合は lane は fail。
- R3: `cargo` 実行は `--locked` を強制する経路(dep-guard の
  `cargo fetch --locked`・test/build 系 mise タスク)で、実行後の
  `Cargo.lock` 差分があれば fail。
- R4: `Cargo.lock` の ruviform git source は単一 resolved SHA に
  収束すること。0 件・複数 SHA は fail。manifest 側の ruviform 依存に
  `rev`/`branch` 指定があれば fail(既定ブランチ追従規約)。
- R5: `rt_` 契約テストは厳密接頭辞マッチ `(^|::)rt_[^ ]+: test$` の
  非空を事前確認してから実行する。0 件は fail。
- R6: 証跡は lane・OS・commit SHA・ruviform resolved SHA・toolchain・
  結果を含み、未導入 lane は NotRun 行として明示する。全 lane が
  not-run の run・1 つでも failure/cancelled を含む run は fail。
- R7: 既存 mise タスク(fmt-check/check/lint/test/build/gw10/
  test-msrv)の意味・成功条件を変更しない。追加は `test-rt`・
  `lint-actions`・`actionlint` tool のみ。
- R8: workflow と action の YAML は actionlint と prek `check-yaml`
  が受理する構文であること。

## Examples

- E1: public repo・secret 未設定の PR run で、dep-access step が
  `mode=anonymous` を出し、verify lane が `mise run test` まで完走
  する。
- E2: `Cargo.toml` の ruviform 依存に `rev = "…"` を注入した場合、
  dep-guard の manifest ガードが fail する(ローカル試験で確認)。
- E3: `Cargo.lock` の ruviform source が 2 種類の SHA を持つ場合、
  単一 SHA ガードが fail する(ローカル試験で確認)。
- E4: `cargo test --locked rt_ -- --list` が現在 2 件の厳密
  `rt_` テストを返し、非空ガードを通過する。
- E5: secret 未設定かつ匿名到達不能の場合、dep-access が
  `::error::` を出して lane を fail にする(構造レビュー)。
- E6: evidence job が lane 結果テーブルと NotRun 行(RF-GT-01/02、
  RF-GT-03/04+SV-GUI-05、frontend、Visual、AudioLens)を Job
  Summary に出力する。

## Guarantees

- PR 作成者は lane 名・OS・step で失敗箇所を特定でき、run の Job
  Summary から commit/依存 SHA/toolchain をトレースできる。
- lockfile 不整合・依存 revision 分裂・RT テスト 0 件・全 lane
  未実行のいずれも緑にならない。
- private 復帰時も secret 設定のみで同一 workflow が復旧する
  (コード変更不要)。

## Non-goals

- GitHub runner 上の実挙動(macOS/Windows 実測、所要時間、runner
  キャッシュ効率)は本 spec の検証対象外 — 初回 run で観測する。
- RF-GT-01〜05・SolidJS frontend・実機 lane の内部仕様は
  対象外(接続点の NotRun 記録のみ)。
- branch protection/ruleset 等の repo 設定は検証対象外。

## Risk

- これらの保証が外れると、未検証コードが main に入り、依存 SHA
  分裂や lockfile 汚染が consumer-verify 側の証跡と食い違い、
  再現不能な失敗として横断 Gate を汚す。
