---
id: "IMPL-034XKoNx7I2brmiVo8RNSa"
type: "impl"
status: "completed"
title: "AUTX-07 theme status tokens + #274 parity check（SV266-NR-02/03 slice、synvelis#266）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
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
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/test-specs/autx-07-theme-parity.md"
  implemented-by: []
  depends-on:
    - "docs/test-specs/autx-07-gui-views.md"
    - "docs/test-specs/sv-script-04-script-editor.md"
    - "docs/test-specs/sv177-theme-preset-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-07-gui-views.md"
    - "docs/test-specs/sv-script-04-script-editor.md"
    - "docs/test-specs/sv177-theme-preset-contract.md"
    - "docs/impl/ir/autx-07-gui-views.md"
    - "docs/impl/ir/sv-script-04-script-editor.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
    - "docs/designs/command-gateway-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-07-theme-parity.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/267"
  defers:
    - "実 GPUI/実 OS 入力・Pixel・IME/DPI/2Window/WindowClose の実機受入（synvelis#114/#267 別 Gate — SV266-NR-01 NotRun）"
    - "synvelis-automation live adapter 接続（SV266-NR-04・#264）・composition mount 配線（SV266-NR-05）・gpui-test scenario（SV266-NR-06）"
    - "Script Editor surface 本体の実装変更（synvelis#274 Owner — 本 slice は表示投影の parity 照合のみ）"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-07-theme-parity.md"
    rejected: []
---

# AUTX-07 theme status tokens + #274 parity check（SV266-NR-02/03 slice）

## Summary

Issue #266 tracker の残件 Deferred Case 2 件を統合する小粒 slice。
(a) `assets/themes/synvelis-{dark,light}.dtcg.json` の `color` group に
`synvelis_theme::status::StatusTone::ALL` 全 path の `status-*` token
値を追加し、`theme_contract.rs` に mode×density 全 4 組合せの resolve
検証を拡張する（SV266-NR-03）。(b) `synvelis-workspace` に
`tests/sv266_parity.rs` を新設し、Action Palette 側
（`project_action_row`/`resolve_capability`）と Script Editor 側
（`project_script_api`/`ScriptApiModel`）が同一
`RegistrySnapshot`/`CapabilityDescriptor` source から同じ
capability/schema/availability verdict を示すことを契約レベルで照合
する（SV266-NR-02 の部分充足 — src は sv#112 lease のため不変、
tests-only）。

## Context

前 slice（AUTX-07 headless GUI views、PR #427）で確定した
`StatusTone` vocabulary（17 tone、`status.rs:83-126`）と
`evidence/sv266/README.md` §3 の Case 台帳に基づく。SV-SCRIPT-04
（synvelis#274）merge 済みの `script_editor.rs`・`synvelis-script`
model が parity 照合の script 側 source。受入条件・検証方法は
`docs/test-specs/autx-07-theme-parity.md` が正本。

## Implementation

### Changes

- `assets/themes/synvelis-dark.dtcg.json` /
  `synvelis-light.dtcg.json`: `color` group に `status-*` 17 token の
  `$value` を追加（mode 文書のみ — base/density 文書の文書層規約を
  維持）。envelope・`$extensions`・`id`・`modes` 構造は不変。
- `crates/synvelis-theme/tests/theme_contract.rs`:
  `shipped_assets_*` に `StatusTone::ALL` 走査の resolve 検証を追加
  （4 組合せ・density 非再着色・mode 間 ramp 差異）。
- `crates/synvelis-workspace/tests/sv266_parity.rs`（新規）:
  `registry_snapshot.json` fixture + surface spec で構築した
  `ScriptApiModel` と `snapshot.capabilities` を共有 source とする
  parity 照合 test 群。

### Decisions During Implementation

- **`status-*` は mode 文書（dark/light）のみに配置**: `color` group は
  mode 文書所有の既存 convention（`warn`/`err`/`ok` が mode 毎に別値、
  base/density 文書に `color` group 無し —
  `docs/designs/sv177-product-theme-workspace-preset-design.md` の
  文書層規約）。base への配置は検討したが、status 色は mode で意味の
  ある値を取るため mode 文書が正しいと判断。
- **色値は `StatusTone::ALL` 順・17 件**: dark は既存 ramp
  （`warn #f0a868`/`err #e86a6a`/`ok #6ac88a`）と親和する配置、
  light は同 hue の高コントラスト版。`cancel-unconfirmed`/
  `indeterminate` は warn/err と別 hue で「警告でもエラーでもない」
  意味を色相分離（`status.rs` module doc の ramp 再利用禁止契約を
  遵守）。値は provisional — provenance 行に明記。
- **`StatusTone::ALL` 走査・件数非固定**: spec 指針どおり `ALL`
  走査とし、17 という literal assert は置かない（SV-SCRIPT-04 で
  11→17 に増えた前例）。
- **parity の source は `registry_snapshot.json` に固定**:
  `ScriptApiModel::build(&snapshot, &surface)` と
  `resolve_capability(&snapshot.capabilities, ..)` が同一
  `CapabilityDescriptor` 列を共有する構成。`fixture_descriptors`
  との突合は補助に留めた。
- **analyzer 非依存は document lifecycle で固定**:
  `ScriptAnalyzerStatus` は両 projection の入力型に現れない構造的
  事実を、`ScriptEditorDocument` の全 variant 遷移
  （Idle→Ready→Dirty→Stale→Unavailable→Unsupported→Rejected）で
  `doc.api()` 不変 + palette `executable`/`blocked` 不変を assert
  して証明。`invocable` verdict と grant 未充足 `executable=false`
  の共存を明示。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-07-theme-parity.md` — mode 文書配置・
  `ALL` 走査・snapshot 共有 source・analyzer 非依存の固定方法を
  採用。base 配置・gateway fixture 主 source・件数 literal assert
  の 3 案を棄却として記録。

### Rejected

- `synvelis-base.dtcg.json` への `status-*` 配置（color group は
  mode 文書所有）。
- `synvelis_gateway::fixture::fixture_descriptors` を parity の
  主 source にする案（`RegistrySnapshot` が正本）。
- tone 数 17 の literal assert（`ALL` 走査が正本）。

## Validation

Windows 11（worktree `G:\orca\workspaces\synvelis\sv266-theme-parity`、
rustc/cargo 1.99.1 — `mise.toml`/`mise.lock` 管理）。
全て exit 0・Passed:

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-theme` | Passed |
| `mise run lint synvelis-theme` | Passed（`--all-targets -D warnings` 系、warning 0） |
| `mise run test synvelis-theme` | Passed — 15 tests（新規 status resolve test 含む） |
| `mise run check synvelis-workspace` | Passed |
| `mise run lint synvelis-workspace` | Passed（warning 0） |
| `mise run test synvelis-workspace` | Passed — 全 test binary ok（`sv266_parity` 6 tests 含む） |
| `mise run test-rt` | Passed（workspace `rt_` filter、exit 0） |
| `mise exec -- prek run --all-files` | Passed（全 hook Passed/Skipped） |
| `cargo test -p synvelis-workspace --locked --test sv266_parity` | Passed — 6 tests（補助証拠、canonical = mise） |

parity test は 1 発で全 6 test Pass — src 側契約 bug の露呈無し。
詳細な case 毎の照合結果は `evidence/sv266/theme-parity.md`。

## Risks

- parity test が `automation_ui.rs`/`script_editor.rs` の契約 bug を
  露呈した場合、src は sv#112 lease のため修正せず Fail/未対応として
  記録する（本 slice の禁止事項）。→ 実測では露呈せず。
- `status-*` の色値は暫定（provisional）— native design review で
  調整され得る。token path vocabulary は不変。
- headless Pass は実機 Pass ではない — SV266-NR-01（実 GPUI
  OS/pixel/IME/DPI）は #114/#267 lane で **NotRun**。

## Follow-ups

- SV266-NR-01（実 GPUI lane）・NR-04/05/06 は別 slice/別 Gate
  （`evidence/sv266/README.md` §3 と `evidence/sv266/theme-parity.md`）。
- `status-*` 色値の native design review（provisional 値の確定）。
