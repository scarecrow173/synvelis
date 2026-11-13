---
id: "DESIGN-TiKdIX8rj4YqmP3Pmb58JR"
type: "design"
status: "approved"
title: "Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 — 既存 verify leg 拡張設計"
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
        path: "docs/designs/ci-os-l1-native-gpui.md"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ci-os-l1-native-gpui.md"
  implemented-by:
    - "docs/plans/ci-os-l1-native-gpui.md"
  depends-on:
    - "docs/specs/ci-os-l1-native-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-166-full-ci-ci-summary.md"
    - "docs/designs/issue-151-ui-ci-lane.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ci-os-l1-native-gpui.md"
  derived-by:
    - "docs/plans/ci-os-l1-native-gpui.md"
    - "docs/test-specs/ci-os-l1-native-gpui.md"
    - "docs/tasks/ci-os-l1-native-gpui.md"
  verifies: []
  verified-by:
    - "docs/test-specs/ci-os-l1-native-gpui.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-summary.mjs"
    - "tools/ci-summary.test.mjs"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
---

# Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 — 既存 verify leg 拡張設計

## 設計判断: 新規 job ではなく既存 verify leg への step 追加

候補は (a) `verify`/`verify-macos` への step 追加、(b) 専用
`native-verify` job(matrix + macos 専用 job)の新設だった。

(a) を採用する根拠:

- **leg universe の不変性**:`tools/ci-summary.mjs` の
  REQUIRED_LEGS/MACOS_LEGS/GATE_JOBS は ci.yml の job 集合に対し
  total であり、job 追加は gate・`expected-jobs` 件数・
  `evidence`/`ci-summary` の `needs`・契約テストの同時更新を
  要する。step 追加なら leg は不変で、step 失敗は leg 失敗と
  して既存 fail-closed gate がそのまま拒否する。
- **runner 重複の回避**:新規 job は OS ごとに checkout/mise/
  dep-access/cargo cache/sccache の setup を再払いする。Issue の
  「不要な CI 重複を増やさない」方針に対し、既存 leg の warm な
  dep cache/sccache をそのまま使う方が総 runner 時間を抑えられる。
- **証跡の一貫性**:verify leg は既に「その OS の workspace 検証」
  を担う lane であり、非既定 Feature の check/clippy/build/test
  は同じ責務の延長。lane evidence 表に step outcome を追加すれば
  OS 別の step 単位の成否が Job Summary に残る。

## 変更点

### ci.yml — `verify`(ubuntu-latest + windows-latest)と `verify-macos`

`test-full` と `Assert Cargo.lock unchanged` の間に以下を挿入
(両 job で同一、macOS は apt step を持たない):

1. `Install Linux GPUI native dependencies`(`if: runner.os ==
   'Linux'`、verify leg のみ):`libxkbcommon-dev` と
   `libxkbcommon-x11-dev` を apt install。
2. `mise run check-app-native` / `lint-app-native` /
   `build-app-native`(native feature の check・Clippy・
   `synvelis-native` bin Build/Link)。
3. `mise run check-app-gpui` / `lint-app-gpui` /
   `test-app-gpui`(gpui-test feature の check・Clippy・test。
   gpui-test は native を内包するが bin link は native のみ)。
4. `Require non-empty gpui-test scenario selection` guard:
   `cargo test -p synvelis-app --locked --features gpui-test
   --test scenario_gpui -- --list` の `: test$` 行を数え、
   0 なら fail。`rt_` guard と同型で、feature 未指定時に
   `tests/scenario_gpui.rs` が `#![cfg(feature = "gpui-test")]`
   で空になり 0 件 pass する silent pass を防ぐ。
5. lane evidence 表へ新規 step outcome 7 行を追加。

`timeout-minutes` を 40 → 60 に引き上げ(msrv-verify と同じ上限)。
gpui 依存木(blade/cosmic-text/wayland/x11/xkbcommon 等)の
compile が check/clippy/build/test で追加され、cold sccache の
初回 run が 40m を超え得るため。sccache 温機後は大幅に短縮
される見込みであり、実測は Full CI run で記録する。

### Linux GPUI SDK 依存の根拠

gpui 0.2.2 → ruviform-ui-gpui(`gpui`/`gpui-test` feature)の
Linux link 依存を依存 crate 単位で監査した結果:

- `xkbcommon` crate のみ `#[link(name = "xkbcommon")]` /
  `#[link(name = "xkbcommon-x11")]` を持ち、libxkbcommon 系の
  -dev package が link に必要。
- `yeslogic-fontconfig-sys` は `source-fontconfig-dlopen` 経由で
  dlopen( fontconfig の -dev 不要)。
- `ash`(Vulkan)は default `loaded` feature で dlopen
  (libvulkan -dev 不要)。`wayland-backend` は `dlopen` feature。
- `x11rb`/`as-raw-xcb-connection`/`xim`/`x11-clipboard`/
  `cosmic-text`/`calloop`/`oo7`/`ashpd` は pure Rust。
- `zed-scap`(screen-capture)は `screen-capture` feature 未使用の
  ため対象外。

ubuntu-latest image の同梱状況に依存せず、必要最小の 2 package
を明示 install する。

### 集約 gate の不変性

`evidence`(mode=pr、macOS observed のみ)と `ci-summary`
(mode=full、全 leg required)の leg universe・`needs`・
`expected-jobs` は変更しない。新規検証は既存 `verify`/
`verify-macos` leg の step 失敗として fail-closed に伝播し、
OS 別 NotRun/Skipped/Fail が Pass 扱いされることはない
(leg が success でなければ gate が violation)。

## 却下した案

- **専用 `native-verify` job 新設**:leg universe・gate・needs・
  契約テストの更新が必要で、setup 重複と runner 時間増を伴う。
  Issue の「既存レーンへ追加」「不要な重複を増やさない」方針に
  対し過剰。
- **`ui-verify` への追加**:SolidJS/npm lane であり Rust feature
  検証の責務境界が異なる。
- **`mise.toml` への複合 task 追加**:6 task を個別 step で呼ぶ
  方が Issue の検証方法にある task 名と証跡が 1:1 に対応し、
  失敗箇所の切り分けが容易。mise.toml は既存 task をそのまま
  使い変更しない。

## リスクと対策

- **Linux link 失敗**:監査上の link 依存は xkbcommon 系のみだが、
  依存更新で増え得る。apt step を verify leg の feature block
  先頭に置き、失敗時は run ログで不足 lib を特定して追補する。
- **verify timeout 超過**:60m へ引き上げ済み。超過時は leg
  timeout として fail-closed に記録され、sccache 温機や job 分割
  を再検討する。
- **macOS の gpui compile 時間**:verify-macos も 60m へ。macOS
  leg は pr mode で observed、full mode で required — 失敗は
  Full CI 失敗として正しく記録される。
