# SV177/RF385 — ruviform-ui-theme 一時接続・adapter 実装証跡

- Run: run_e9e66b833704 / Task: task_c92452f08541 / Dispatch: ctx_8bd8f9948354
- Repo: scarecrow173/synvelis / Branch: feature/native-product-theme-workspace-presets
- 前段 evidence: `evidence/reports/sv177-theme-preset.md`（seam Unsupported 時点）
- OS/toolchain: Windows / rust 1.99.0 (workspace pin)

## 依存接続（SHA・方法・解除実績）

- **最終状態（解除済み）**: RF385 candidate は ruviform main へ PR #397
  （squash `df45d10cd86918d10fa81dc80a3f5f5f2564d5ae`、Validator P1 修復
  込み）で正式着地。**全 `branch` key を除去して default 追従へ復帰済み**
  — Cargo.lock の全 24 ruviform sources は `git+...#5c0f16b`（branch
  参照ゼロ、RF394 統合後の main head・`df45d10` を包含）、恒久
  `rev` pin なし。
- **接続先（履歴）**: `origin/feature/gui-driver-neutral-design-contract`
  - 指示された candidate head: `d3b211cc09c01d74395dfab3354c895b345fc744`
    （main `355b7a1` merge 済み）
  - 接続時の実解決 SHA: `660034f2` → candidate head 前進に追随し
    `d26d95f8`（独立 Validator P1 修復 — 非ASCII hex color の
    char-boundary slice panic 解消）まで branch tracking していた。
  - squash merge のため `d26d95f` は `df45d10` の git ancestor では
    ないが、内容（ui-theme crate + P1 修復）は main に統合済み。
- **方法（当時）**: workspace `[workspace.dependencies]` の**全 ruviform
  git 依存**に `branch = "feature/gui-driver-neutral-design-contract"` を
  付与（恒久 `rev` pin は追加しなかった）。全 dep 同時切替は
  `tools/check-ruviform-rev.mjs` の「全 ruviform dep = 単一 resolved
  SHA」fail-closed 不変条件を維持するため。`ruviform-ui-theme`
  を workspace.dependencies に新規追加し `synvelis-theme` が消費。
- **解除実績**: candidate main 着地を coordinator 通知（msg_af921dd26988）
  で確認 → ruviform `main` head `df45d10` を `git ls-remote` で検証 →
  全 `branch` key 除去 → `cargo generate-lockfile` → 24 sources 全て
  `df45d10` → `ui/vendor/ruviform/REV` 同期 → `check-ruviform-rev` pass。
  その後 main 前進（RF394・msg_3248a1320960）に追随して最終解決は
  `5c0f16b574c0eaea3856318992d32dcddd869523` へ更新・REV 再同期。

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
- `ui/vendor/ruviform/REV`: lock 追従で `5c0f16b` へ同期
  （local artifact、旧 `8fb665f` / `660034f2` / `d26d95f` /
  `df45d10` は stale）。
- docs: design（module 表・文書形式・data flow・deferred・risks）、
  test-spec（rules/examples）、IR（Rev.2）を実装へ整合。

## 委譲境界の確認（P1 修正の検証）

- `delegated-{type,value,ref,cycle}.dtcg.json`（token 系 problem を
  含む両 envelope 文書）は SV envelope では load を通過 —
  consumer は token 意味論を再検証しない。
- adapter 経路では上流が**各名前の** rejection を返す
  （`delegated_fixtures_surface_their_named_upstream_rejections`）:
  `delegated-type` → `Document(UnknownTokenType)`・
  `delegated-value`/`delegated-ref` → `Document(InvalidValue)`・
  `delegated-cycle` → `AliasCycle`。RF profile 不在文書は
  `Document(MissingProfile)`（`adapter_surfaces_upstream_document_errors`）。
- alias cycle は上流の **merge 後集合** でのみ検出されるため、
  下位文書の alias 環が上位 override で消える集合は合法 —
  `overridden_cycle_members_make_the_merged_set_legal` が
  P1 の回帰を直接固定（cycle doc + override doc → resolve 成功）。
  SV 単文書事前拒否の誤拒否は構造的に解消。
- Reviewer B findings 反映: `catalog.resolve` は declared
  vocabulary 外 mode/density を `ThemeError::Selection` で拒否
  （silent 部分層解決の抑止）、両 envelope coherence
  （layer 一致・modes 包含・mode+density 併記）は `Contract`
  problem で fail-closed（upstream 内部の再検証ではなく
  SV 所有の整合契約）。`load_dir` は文書の `Schema`/`Validation`
  error を構造のまま伝搬（file path を detail に注記）— 従来の
  `Io` 文字列化による structured problem 喪失を Validator A3 指摘で解消。

## 実行済み検証（実測・Rev.2）

| 検証 | コマンド | 結果 |
| --- | --- | --- |
| fmt | `mise run fmt-check` | Pass |
| check | `mise run check` | Pass（workspace 全体、ruviform `5c0f16b`（main）上） |
| lint | `mise run lint`（clippy -D warnings） | Pass（warning 0） |
| workspace tests | `mise run test`（--locked） | Pass（86+ suites・947 tests・0 fail） |
| theme tests | `cargo test -p synvelis-theme` | Pass（14/14 — 4 mode×density resolve・両 envelope・委譲境界・selection/coherence guard・P1 override 回帰） |
| headless gate | `mise run check-headless-deps` | Pass（9 crates・GUI deps なし） |
| rev gate | `mise run check-ruviform-rev` | Pass（`5c0f16b`・24 sources・default 追従・REV match） |

## 未検証 / NotRun

- Native 受入（実 GPUI window・実 plugin editor・実 OS window 配置・
  pixel/golden）: **NotRun**。RF390/RF313/RF314 + SV114 gate 待ち。
- `ResolvedTheme` → GPUI/driver mapper 接続: RF386 mapper 未着地。
- light theme 値: provisional（Native Design Review 待ち）。

## 残 Gate / Follow-up

- **Refs**: 解除完了 — default 追従へ復帰済みで temporary connection
  は存在しない。`Cargo.lock` 実解決 `5c0f16b` を記録。
- ruviform main の前進は通常の `cargo update` 運用で追随する
  （本証跡記録の SHA は接続・解除時点の実測値）。
