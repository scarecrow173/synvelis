---
id: "IMPL-034T7IhBoewhoZNaZfjXP1"
type: "impl"
status: "completed"
title: "SV177 theme/preset implementation record"
created: "2026-11-06"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/177"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv177-theme-preset-impl.md"
    - "docs/plans/sv177-product-theme-workspace-presets.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/sv177-product-theme-workspace-inventory.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv177-theme-preset-contract.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV177 theme/preset implementation record

## Summary

`synvelis-theme` leaf crate（DTCG 文書契約）と
`synvelis-workspace::preset`（slot-based preset）を headless
契約として実装。5 theme asset・3 preset asset・26 contract
テスト・headless-deps gate 登録を含む。独立 Reviewer A/B →
Validator → Judge の review-protocol で確認された blocking
findings（破壊前検証欠落・session state 混入・gate 未登録等）を
全て修正済み。

**Revision 2（2026-11-07）**: RF385（`ruviform-ui-theme`・
`rvf-dtcg-profile.v1`）ruviform main 着地
（PR #397 squash `df45d10cd86918d10fa81dc80a3f5f5f2564d5ae`）に伴い、
`UnresolvedThemeResolver` seam を実 adapter
（`adapter::resolve_documents` → `TokenDocument::from_json` →
`ThemeResolver::resolve` → `ResolvedTheme`）へ置換。SV 側の
type/unit/alias 汎用 validation（P1 根因の単文書循環拒否を含む）
を撤去し、envelope-only 検証 + 原文保持 `ThemeDocument` へ縮退。
出荷 5 資産は `synvelis.productTheme` と
`dev.ruviform.dtcg-profile` を併記し typed `{value,unit}` 形へ
移行。証跡: `evidence/reports/sv177-rf385-adapter.md`。

## Context

Task `docs/tasks/sv177-theme-preset-impl.md`、plan
`docs/plans/sv177-product-theme-workspace-presets.md`、design
`docs/designs/sv177-product-theme-workspace-preset-design.md`
（approved）に従う。Rev.1 は上流契約未着地のため
`UnresolvedThemeResolver` seam で明示 Unsupported とした。
Rev.2 で RF385 candidate（`ruviform-ui-theme`・
`rvf-dtcg-profile.v1`）へ実接続し、interpreter 責務は上流へ
完全委譲（第 2 interpreter 不許可の方針は維持）。

## Implementation

### Changes

- `crates/synvelis-theme`: `error.rs`（ThemeError/ProblemKind — Rev.2
  で `Resolve(ruviform_ui_theme::ResolveError)` 追加・token 系 kind
  撤去）・`document.rs`（envelope+TokenTree parse・Rev.2 で原文保持
  `ThemeDocument` 追加）・`validate.rs`（Rev.1: known `$type`・
  value shape・alias 構文・同一文書循環 → Rev.2: envelope-only へ
  縮退 — `schema`/非空 field/`layer` tag のみ）・`catalog.rs`
  （`ThemeKey`・layered `documents_for`・`ThemePreferences`・
  Rev.2 で `resolve(sel)` 委譲追加）・`adapter.rs`（Rev.2 新設 —
  `resolve_documents` が唯一の上流接続点。旧 `resolve.rs` の
  `ResolvedThemeSink`/`UnresolvedThemeResolver` stub は撤去）。
- `assets/themes/`: base/dark/light/density-compact/comfortable の
  5 DTCG 文書。dark は既存 style 値を literal 化（alias 不使用で
  self-contained）、light は `provisional-pending-native-design-review`
  provenance 付き。Rev.2 で全文書に `dev.ruviform.dtcg-profile`
  block（`layer="product"`・一意 id・mode 文書は `modes` 宣言）を
  併記し、`dimension`/`duration` を typed `{value,unit}` 形へ移行
  （CSS 文字列 `"22px"`/`"100ms"` は上流 `InvalidValue` 不適合）。
- `Cargo.toml`（workspace）: `ruviform-ui-theme` を default 追従
  dep として追加。candidate landing 前は全 ruviform git 依存を
  `branch = "feature/gui-driver-neutral-design-contract"` 追従へ
  一時切替していたが、main 着地（`df45d10`）確認後に全 `branch`
  key を除去して default 追従へ復帰（解除条件消化済み、恒久
  `rev` pin なし — 証跡 `evidence/reports/sv177-rf385-adapter.md`）。
  `Cargo.lock` 再生成 — 24 ruviform sources 全て
  `5c0f16b574c0eaea3856318992d32dcddd869523`（RF394 統合後の
  main head、RF385 landing `df45d10` を包含。SV84 merge 由来の
  `ruviform-io-file` 含む）。
- `crates/synvelis-workspace`: `preset.rs` 新規（PRESET_SCHEMA=2
  slot 型・`PresetViewState` 部分集合・validate・from_snapshot・
  legacy import・`apply_preset`）。`ViewType::PluginDock` 語彙追加・
  `WorkspaceError::Preset`・必要 field の `pub(crate)` 化。
- `assets/workspaces/`: editing/mixing/plugin-dock の 3 preset。
- `tools/check-headless-deps.mjs`: `synvelis-theme` を HEADLESS_CRATES
  に追加（9 crates）。

### Decisions During Implementation

- `apply_preset` は `LayoutSnapshot::restore`（Result 版）を window
  close 前に実行。`restore_or_default` は検証失敗を黙って default
  化するため採用しない（Reviewer B1 修正）。
- preset state は `PresetViewState`（tool/grid/snap/inspection_mode/
  viewport）に限定。実 ViewInstanceId・EditBinding・pinned
  SemanticTarget・gesture・revision_inbox を template に持ち込ま
  ない（Reviewer B2/A6 修正）。
- catalog は merge 済み値ではなく文書を保持し `documents_for` で
  層集合を返す。merge/interp は RF385 の責務（Reviewer B4/A9 修正）。
  Rev.2 では `TokenTree` に加え**原文 JSON**（両 extension block
  保持）を `ThemeDocument.source` として保持 — 上流
  `TokenDocument::from_json` が profile block を要求するため、
  `$extensions` を除いた tree では再構成不能。
- dark theme は alias ではなく literal 値で出荷（cross-doc alias
  解決は未規格のため self-contained に）。
- Rev.2: SV 側の token 意味論検証は全撤去し envelope-only へ。
  単文書循環拒否は後段 override で合法な集合を誤拒否する P1 の
  根因であり、alias 解決は上流の merge 後集合で行われる。
  `delegated-*` fixtures（token 系 problem を持つが envelope 正当）
  が consumer load を通過することが境界の証跡。
- Rev.2: 依存 candidate への接続は `rev` pin ではなく全 dep の
  branch 追従で実施（`check-ruviform-rev` の単一 SHA 契約維持。
  SHA・方法・解除条件を証跡化）。

## Related Experiments

### Adopted

- なし（直接実装）。

### Rejected

- `restore_or_default` による fallback 経路（検証を黙殺）。
- 既存 view への preset state 上書き適用（session 意味を壊す）。

## Validation

Rev.2 実行分（2026-11-07、ruviform `5c0f16b`（main）上）:

- `mise run fmt-check`: pass。
- `mise run check`: workspace 全体 pass。
- `mise run lint`（`cargo clippy --workspace --all-targets
  -- -D warnings`）: warning 0。
- `mise run test`（`cargo test --workspace --locked`）: 86 test
  suites・0 fail。
- `node tools/check-headless-deps.mjs`: 9 headless crates、GUI
  framework deps なし。
- `mise run check-ruviform-rev`: resolved
  `5c0f16b574c0eaea3856318992d32dcddd869523`（24 lockfile
  sources・lockfile-primary・REV match）。
- `cargo test -p synvelis-theme`: 14 pass / 0 fail —
  出荷 5 資産が SV envelope + upstream parser 双方を通過し、
  dark/light × compact/comfortable 全 4 組合せで
  `catalog.resolve` が `ResolvedTheme` を返すことを実 RF
  pipeline で検証。Reviewer B findings 反映後の追加固定:
  declared vocabulary 外 mode/density の `Selection` 拒否・
  両 envelope coherence（`Contract`）・`delegated-*` 各 variant
  （`UnknownTokenType`/`InvalidValue`/`AliasCycle`）・P1 override
  回帰（cycle 消滅集合の resolve 成功）・real alias 解決。

Rev.1 実行分: `cargo test -p synvelis-theme` 11 pass・
`cargo test -p synvelis-workspace` 全 suite pass・clippy
warning 0・fmt 適用・headless-deps pass。

## Risks

- `ResolvedTheme` → GPUI/driver mapper は RF386 未着地 — adapter は
  実接続済みだが driver 写像の消費点が無い（upstream error は
  `ThemeError::Resolve` で透過）。
- preset apply は破壊前検証済みだが、mutation 開始後の host/dock
  失敗では部分的 layout 残置があり得る（view/dock 不変は維持）。
- 真の schema-1 preset ファイル移行は既知 limitation（現行
  `WorkspaceSnapshot` 形状のみ legacy import 受理）。
- `synvelis-light` は `color.bg-active`/`color.bg-input` を含まない
  （dark 12色 vs light 10色、provisional）— Native Design Review
  で確定。mapper 消費時に `None` となり得る既知差分。

## Follow-ups

- ~~RF385 candidate merge 時に `branch` key 除去~~ — 済
  （`df45d10` 着地確認後、全 `branch` key を除去して default
  追従へ復帰。以後の main 前進は `cargo update` / lock 再生成で
  追随 — 単一 source なので個別 `-p` 更新は行わない）。
- 複数文書の resolve error にはどの文書が失敗したかの id 情報が
  無い（upstream `TokenError` の path のみ）— 上流側の error
  context 強化要望は RF 側 issue として扱う。
- Native 受入（実 GPUI window/実 plugin editor/実 OS）: **NotRun**。
  RF390/RF313/RF314 + SV114 gate 後に実施。
- `ResolvedTheme` → renderer/GPUI mapper 接続は RF386 mapper
  着地後（adapter 出力は driver-neutral のまま）。
