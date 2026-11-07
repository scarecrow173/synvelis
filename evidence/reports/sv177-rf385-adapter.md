# SV177/RF385 — ruviform-ui-theme 一時接続・adapter 実装証跡

- Run: run_e9e66b833704 / Task: task_c92452f08541 / Dispatch: ctx_8bd8f9948354
- Repo: scarecrow173/synvelis / Branch: feature/native-product-theme-workspace-presets
- 前段 evidence: `evidence/reports/sv177-theme-preset.md`（seam Unsupported 時点）
- OS/toolchain: Windows / rust 1.99.0 (workspace pin)

## 依存 candidate 一時接続（SHA・方法・解除条件）

- **接続先**: `origin/feature/gui-driver-neutral-design-contract`
  - 指示された candidate head: `d3b211cc09c01d74395dfab3354c895b345fc744`
    （main `355b7a1` merge 済み）
  - 接続時の実解決 SHA: **`d26d95f83fa822ed4f01e0aefab3cfcb818f7806`**
    （`d3b211cc` + evidence-docs `660034f2` + 独立 Validator P1 修復
    `d26d95f` — 非ASCII hex color の char-boundary slice panic 解消。
    `d3b211cc` は `d26d95f` の ancestor 確認済み。初回接続時は
    `660034f2` だったが candidate head 前進に追随して再解決）
  - Cargo.lock: 全 24 ruviform sources が `git+...?branch=feature%2Fgui-driver-neutral-design-contract#d26d95f` に統一
    （SV84 merge で追加された `ruviform-io-file` も同一 branch key で 1 件増）。
- **方法**: workspace `[workspace.dependencies]` の**全 ruviform git
  依存**に `branch = "feature/gui-driver-neutral-design-contract"` を
  付与（恒久 `rev` pin は追加しない）。全 dep 同時切替は
  `tools/check-ruviform-rev.mjs` の「全 ruviform dep = 単一 resolved
  SHA」fail-closed 不変条件を維持するため（ui-theme のみ別 rev を
  混在させると divergent SHAs で gate fail）。`ruviform-ui-theme`
  を workspace.dependencies に新規追加し `synvelis-theme` が消費。
- **解除条件**: RF385 candidate が ruviform default branch へ merge
  されたら全 `branch` key を除去して default 追従へ戻し
  （`cargo update -p ruviform-*` で lock 再生成）。branch 削除・
  巻き戻し前に解除しないと参照が切れる — merge 確認が先。

## 実施内容（Rev.2）

- `crates/synvelis-theme`:
  - `adapter.rs` 新設 — `resolve_documents(&[&ThemeDocument], mode)`
    が唯一の上流接続点。`TokenDocument::from_json`（上流 strict
    parser）→ `ThemeResolver::resolve` → `ResolvedTheme`。
  - `resolve.rs`（`ResolvedThemeSink`/`UnresolvedThemeResolver`
    stub）撤去 — seam は実装へ置換。
  - `document.rs` — `ThemeDocument { meta, tree, source }` 追加。
    原文 JSON（両 extension block 保持）を `source` に保持
    （`$extensions` を除く `TokenTree` からは上流 profile block が
    再構成不能のため）。
  - `validate.rs` — envelope-only へ縮退（schema/非空 field/`layer`
    tag）。`$type` 集合・value shape・alias 構文・**単文書循環拒否**
    を全撤去 — 同拒否が後段 override で合法な集合を誤拒否する
    確定 P1 の根因。`load_document`/`load_dir` は `ThemeDocument`。
  - `catalog.rs` — `ThemeDocument` を index。`resolve(&sel)` で
    adapter へ委譲。`error.rs` — `ThemeError::Resolve(ResolveError)`
    追加（`From<TokenError>` 経由で document 系も統一）。
- `assets/themes/` 5 文書: `dev.ruviform.dtcg-profile`
  （`profile="rvf-dtcg-profile.v1"`・`layer="product"`・一意 id・
  mode 文書は `modes` 宣言）を `synvelis.productTheme` と併記。
  `dimension`/`duration` を typed `{value,unit}` 形へ移行
  （CSS 文字列 `"22px"`/`"100ms"` は上流 `InvalidValue` 不適合）。
- `Cargo.toml` / `Cargo.lock`: 上記一時接続。
- `ui/vendor/ruviform/REV`: lock 追従で `d26d95f` へ同期
  （local artifact、旧 `8fb665f` / `660034f2` は stale）。
- docs: design（module 表・文書形式・data flow・deferred・risks）、
  test-spec（rules/examples）、IR（Rev.2）を実装へ整合。

## 委譲境界の確認（P1 修正の検証）

- `delegated-{type,value,ref,cycle}.dtcg.json`（token 系 problem +
  RF profile 不在）は SV envelope では load を通過 — consumer は
  token 意味論を再検証しない。
- adapter 経路では上流が拒否: `delegated-cycle` →
  `ThemeError::Resolve(ResolveError::Document(TokenError::MissingProfile))`
  （contract test `adapter_surfaces_upstream_document_errors`）。
- alias cycle は上流の **merge 後集合** でのみ検出されるため、
  下位文書の alias 環が上位 override で消える集合は合法 —
  SV 単文書事前拒否の誤拒否は構造的に解消（SV は存在しない
  判定を行わない）。

## 実行済み検証（実測・Rev.2）

| 検証 | コマンド | 結果 |
| --- | --- | --- |
| fmt | `mise run fmt-check` | Pass |
| check | `mise run check` | Pass（workspace 全体、ruviform `660034f2`→`d26d95f` 再解決後も Pass） |
| lint | `mise run lint`（clippy -D warnings） | Pass（warning 0） |
| workspace tests | `mise run test`（--locked） | Pass（86 suites・0 fail） |
| theme tests | `cargo test -p synvelis-theme` | Pass（10/10 — 4 mode×density resolve・両 envelope・委譲境界） |
| headless gate | `mise run check-headless-deps` | Pass（9 crates・GUI deps なし） |
| rev gate | `mise run check-ruviform-rev` | Pass（`d26d95f`・24 sources・REV match） |

## 未検証 / NotRun

- Native 受入（実 GPUI window・実 plugin editor・実 OS window 配置・
  pixel/golden）: **NotRun**。RF390/RF313/RF314 + SV114 gate 待ち。
- `ResolvedTheme` → GPUI/driver mapper 接続: RF386 mapper 未着地。
- light theme 値: provisional（Native Design Review 待ち）。

## 残 Gate / Follow-up

- **Refs**: 本証跡は `git+...#d26d95f`・branch tracking・解除条件を
  記録済み — 参照実体は ruviform repo `feature/gui-driver-neutral-design-contract`
  head の drift に追随（`660034f2`→`d26d95f` の前進を取り込んだ実績どおり、
  再 `cargo update` で lock が動き得る点に注意）。
- RF385 main 着地時: 全 `branch` key 除去 → default 追従復帰 →
  `cargo update` → `check-ruviform-rev` 再検証。
