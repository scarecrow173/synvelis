---
id: "EXP-AUTX07ThemeParity"
type: "experiment"
status: "completed"
title: "AUTX-07 theme status tokens + #274 parity check 実験ログ（synvelis#266 SV266-NR-02/03 slice）"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/266"
    - "https://github.com/scarecrow173/synvelis/issues/274"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv266_parity.rs"
      - type: "file"
        path: "docs/impl/ir/autx-07-theme-parity.md"
      - type: "file"
        path: "docs/impl/exp/autx-07-theme-parity.md"
      - type: "file"
        path: "docs/test-specs/autx-07-theme-parity.md"
      - type: "file"
        path: "evidence/sv266/theme-parity.md"
    modified:
      - type: "file"
        path: "assets/themes/synvelis-dark.dtcg.json"
      - type: "file"
        path: "assets/themes/synvelis-light.dtcg.json"
      - type: "file"
        path: "crates/synvelis-theme/tests/theme_contract.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/autx-07-theme-parity.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/autx-07-gui-views.md"
    - "docs/impl/ir/sv-script-04-script-editor.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-07-theme-parity.md"
    - "docs/test-specs/autx-07-gui-views.md"
    - "docs/test-specs/sv-script-04-script-editor.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/autx-07-theme-parity.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/267"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/267"
    - "https://github.com/scarecrow173/synvelis/issues/274"
    - "https://github.com/scarecrow173/synvelis/issues/264"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "status-* 色値は mode 文書（dark/light）のみ — base/density 文書の文書層規約を維持"
      - "StatusTone::ALL 走査で token 数非依存の resolve 検証"
      - "parity fixture は registry_snapshot.json を ScriptApiModel と snapshot.capabilities の両側 source として共有"
      - "analyzer status 非依存は ScriptEditorDocument の全 variant 遷移 + doc.api() 不変 assert で固定"
    rejected:
      - "synvelis-base.dtcg.json への status-* 配置（color group は mode 文書所有の既存 convention）"
      - "synvelis_gateway::fixture::fixture_descriptors を parity の主 source にする案（snapshot が正本、gateway fixture は補助）"
      - "tone 数 17 の literal assert（ALL 走査が正本 — SV-SCRIPT-04 で 11→17 に増えた前例あり）"
---

# AUTX-07 theme status tokens + #274 parity check experiment log

Issue: https://github.com/scarecrow173/synvelis/issues/266

## 採用した設計判断

### 1. `status-*` 色値は mode 文書のみに配置

`assets/themes/synvelis-dark.dtcg.json` と
`synvelis-light.dtcg.json` の `color` group に `StatusTone::ALL` 全
17 path の `$value` を追加した。`synvelis-base.dtcg.json` と density
文書には `color` group 自体が存在せず、`warn`/`err`/`ok` が mode 毎に
別値を持つ既存 convention に従った
（`docs/designs/sv177-product-theme-workspace-preset-design.md`:
base = mode/density 非依存 token）。dark は `ui/src/styles.css` の
観測値系 ramp と親和する寒色〜暖色の配置、light は同 hue の
高コントラスト版とし、`cancel-unconfirmed`/`indeterminate` は
warn/err とは別 hue で「警告でもエラーでもない」意味を色相で分離
した（status.rs の module doc 契約 — `ok`/`warn`/`err` ramp の
再利用禁止を遵守）。色値は provisional であり、token path
vocabulary が stable contract。

### 2. `StatusTone::ALL` 走査で件数非依存の resolve 検証

`theme_contract.rs` の新規 test
`shipped_assets_resolve_every_status_tone_for_every_mode_density_pair`
は `StatusTone::ALL` を走査し、`color.{token_path()}` を
dark/light × compact/comfortable の 4 組合せで `theme.color(...)`
resolve する。density ペア内の値一致（density は status ramp を
再着色しない）と mode 間 ramp 差異（dark ≠ light）を assert する。
17 という件数を literal で assert しない — SV-SCRIPT-04 で
11→17 path に増えた前例があり、将来の tone 追加に自動追随する。

### 3. parity fixture は単一 `RegistrySnapshot` を両側 source に共有

`sv266_parity.rs` は `crates/synvelis-script/tests/fixtures/
registry_snapshot.json`（9 capability + schemas、全 availability
state 網羅）を `RegistrySnapshot` に復元し、
`ScriptApiModel::build(&snapshot, &surface)`（script 側）と
`snapshot.capabilities` を `resolve_capability` の入力（palette 側）
に使う。surface spec は available×6・planned・disabled・retired・
version drift（`@9.9`）・未登録 id・hidden の 12 entry で構成し、
「同一 descriptor source から同じ verdict」の不変条件を全分岐で
照合する。`entry.descriptor.is_some()` ⟺ `resolve_capability` Ok
の同値性を全 public row で assert し、解決時は descriptor の
完全一致・`capability_label()` 一致、未解決時は fail-closed verdict
（`unsupported-version`/`unknown-capability`/`unresolved-schema`/
`invalid-name`）のみ許容する。

### 4. analyzer status 非依存は document lifecycle 全 variant で固定

`ScriptAnalyzerStatus` は `project_action_row`/`project_script_api`
の入力型に現れない構造的事実を、`ScriptEditorDocument` を
`Idle→Ready→Dirty→Stale→Unavailable→Unsupported→Rejected` と
遷移させて `doc.api()` の verdict rows が全段階で不変であること、
かつ palette 側 `project_action_row` の `executable`/`blocked` が
grant `Missing` のまま不変であることを assert して固定した。
`invocable` verdict（登録・schema 健全性の分類）と
`executable=false`（grant 未充足）が同一 capability で共存する
ことを明示し、「Analyzer PASS は Gateway 認可 PASS を意味しない」
契約を headless で証明した。

## 試して棄却したもの

- **`synvelis-base.dtcg.json` への `status-*` 配置**: base 文書は
  mode/density 非依存 token の層であり `color` group を持たない。
  status 色は mode で意味のある値（dark=暗背景向け明色、light=明背景
  向け濃色）を取るため mode 文書所有が正しい。
- **`fixture_descriptors` を parity の主 source にする案**: spec が
  固定する source は `RegistrySnapshot`（digest pin 済み discovery
  source）であり、gateway fixture は descriptor 内容の補助照合に
  留めた。両者を主 source にすると「どちらが正本か」が曖昧になる。
- **tone 数 17 の literal assert**: `StatusTone::ALL` 走査が正本。
  件数固定は vocabulary 追加のたびに test 修正を強いる brittle
  assert になる（spec・Coordinator 指針どおり）。

## 検証証跡

- `mise run test synvelis-theme`: 15 passed（新規
  `shipped_assets_resolve_every_status_tone_for_every_mode_density_pair`
  含む — 17 tone × 4 組合せ resolve、density 非再着色、mode 間差異）。
- `cargo test -p synvelis-workspace --locked --test sv266_parity`
  （補助証拠、canonical = mise）: 6 passed — identity parity・
  fail-closed・availability・executable 行列・schema/target verbatim・
  analyzer 非依存の全 invariant。
- `mise run fmt-check` / `check`・`lint`・`test`（synvelis-theme・
  synvelis-workspace）/ `test-rt` / `prek run --all-files`: 全て exit 0。
- 実 OS/GPUI lane（SV266-NR-01）は #114/#267 別 Gate — **NotRun**
  （`evidence/sv266/theme-parity.md`）。
