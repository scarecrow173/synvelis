---
id: "TSPEC-m5UJFsYVE3wcIp2bbhjQ6C"
type: "test-spec"
status: "approved"
title: "Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 検証仕様"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/377"
    - "docs/specs/ci-os-l1-native-gpui.md"
  changes:
    added:
      - type: "file"
        path: "docs/test-specs/ci-os-l1-native-gpui.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/ci-os-l1-native-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-166-ci-summary-gate.md"
    - "docs/test-specs/issue-151-ui-ci-lane.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/ci-os-l1-native-gpui.md"
    - "docs/plans/ci-os-l1-native-gpui.md"
  derived-by: []
  verifies:
    - "docs/specs/ci-os-l1-native-gpui.md"
    - "docs/designs/ci-os-l1-native-gpui.md"
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-summary.mjs"
    - "crates/synvelis-app/tests/scenario_gpui.rs"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
---

# Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 検証仕様

## Feature: 3OS OS-L1 feature lane

`ci.yml` の `verify`(ubuntu-latest/windows-latest)と
`verify-macos` の各 leg が、`native`/`gpui-test` feature の
OS-L1 検証を実走し、既存の fail-closed 集約で証拠化すること。

### Rule 1: native feature の check/lint/link

- 各 leg で `mise run check-app-native`(=`cargo check -p
  synvelis-app --features native --all-targets`)と
  `lint-app-native`(clippy `-D warnings`)が exit 0 で実走する。
- `mise run build-app-native`(=`cargo build -p synvelis-app
  --features native --bin synvelis-native --locked`)が exit 0
  で実走し、3 OS での bin link を証明する。

### Rule 2: gpui-test feature の check/lint/test

- 各 leg で `mise run check-app-gpui`/`lint-app-gpui`/
  `test-app-gpui` が exit 0 で実走する。
- `test-app-gpui` は `crates/synvelis-app/tests/scenario_gpui.rs`
  を含む GPUI-simulated headless scenario を実行する。実窓・実
  OS 入力・pixel・実機 Plugin は対象外(GUI-L2/EXT-L3、synvelis
  #114 へ委譲)。

### Rule 3: 0 件/未選択 silent pass の防止

- `Require non-empty gpui-test scenario selection` guard が
  `cargo test -p synvelis-app --locked --features gpui-test
  --test scenario_gpui -- --list` の出力から `: test$` 行を数え、
  0 件なら leg を fail させる。feature 未指定で
  `#![cfg(feature = "gpui-test")]` により test が空になる
  silent pass を防ぐ(rt_ guard と同型)。

### Rule 4: 既存 lane・集約の維持

- ruviform resolved SHA guard、UI/SolidJS、GW-10、RT、MSRV、
  dep-guard の job・step・証拠を変更しない。
- job/leg を追加しないため `tools/ci-summary.mjs` の
  REQUIRED_LEGS と `expected-jobs` は不変。新規 step の失敗は
  leg 失敗として `evidence`/`ci-summary` が fail-closed に拒否
  する。OS 別 NotRun/Skipped/Fail を Pass 扱いしない。

### Rule 5: Linux GPUI SDK 依存

- `verify` leg の Linux runner で apt install
  `libxkbcommon-dev libxkbcommon-x11-dev` が feature step より
  先に実行される。runner image 同梱状況への暗黙依存を避ける。

### Example: 実走証拠

- Windows ローカル:`mise run check-app-native`/
  `lint-app-native`/`build-app-native`/`check-app-gpui`/
  `lint-app-gpui`/`test-app-gpui` が全て exit 0、
  `test-app-gpui` は scenario 14 件 pass。
- guard の `-- --list` は scenario_gpui の 5 件を列挙
  (5 > 0)。
- Full CI:対象 SHA の手動 `ci.yml` dispatch で 3 leg の新規
  step 成功と run URL、leg 所要時間を PR に記録する。結果が
  未取得の項目は `NotRun` と明記し、推測で Pass にしない。
