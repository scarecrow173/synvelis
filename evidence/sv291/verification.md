# Verification — synvelis#291 RCP-02 headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust（lint 出力の
rustc 1.99.1）。

## 実行結果（実測）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0。初回は新規ファイルの整形差分で fail → `cargo fmt --all` 適用後 green） |
| `mise run check synvelis-render` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-project` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-player` | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-render` | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-render` | Pass（exit 0 — 全 suite green。`rcp02_artifact.rs` 20/20 を含む: l2_e2e 17, rcd_04_rt_comp_pcm 2, rcp01_recipe 11, rcp02_artifact 20, sf03_render 2, tim04_plugin_state 5, x_e2e 5） |
| docs index 再生成（doc-maintenance `index plan`→`index apply` + `sync_impl_index`） | 結果は PR 本文に記録 |
| `doc_status.js lint --dir docs/test-specs` | 範囲内 Pass — `rcp-02-artifact-manifest.md` finding 0 件（dir 全体の既存 baseline finding は範囲外・未修復） |
| `audit_impl_record.js --dir docs/impl/ir` | 範囲内 Pass — `rcp-02-artifact-manifest.md` finding 0 件（baseline 残存） |

## 補助証拠（canonical = mise）

- `cargo fmt --all` 適用（mise 管理 toolchain 経由）。
- `cargo test --test rcp02_artifact` を `mise exec --` 経由で
  単体実行（18/18 Pass — 補助証拠。canonical は
  `mise run test synvelis-render`）。
- lint で検出・修正した点:
  - `store.rs`: `valid_locator` に空白文字 reject を追加
    （locator を opaque commit 識別子として fail-closed に
    — 空白を含む locator は traversal/曖昧性の温床）。
  - `publish.rs`: `commit_verified` の引数過多を
    `VerifiedManifest`/`build_verified_manifest` 分離で解消
    （`#[allow]` 不使用）。
  - `rcp02_artifact.rs`: `doc_markdown`（`NotRun`/`RemoteArtifact`
    backtick）、`cast_possible_truncation`/`cast_possible_wrap`
    （fixture の `as` cast を `try_from` + `sample_i32` helper
    へ）、`assert_is_empty`（`assert_ne!`/`assert_eq!(len,0)`
    へ）、`too_many_lines` ×2（fitness variants を
    `fitness_variants`/`solo_outcome` helper へ分割、remote
    rejection を blob/manifest 系と trust/claim/receipt 系の
    2 test + `remote_bytes`/`remanifest`/`assert_share_rejected`
    helper へ分割）、`assigning_clones`（`clone_into` へ）、
    `needless_pass_by_value`（`&ShareRejection` へ）。
- test fixture で判明・修正した点:
  - `DependencyClosure` は `recipe.rs` で private import —
    test は `synvelis_render::DependencyClosure`（lib 再
    export）を使用。
  - `valid_locator` が空白を reject しなかったため
    `"with space"` locator の test が `Ok` を返した —
    store 側を fail-closed に修正（test 期待は維持）。
- review-protocol adversarial pass で判明・修正した点
  （P1 — codec class/codec 一貫性が publish のみで
  enforce されていた gap）:
  - `codec_class_consistent` を publish.rs から
    manifest.rs の共有 manifest-domain invariant へ移動。
  - `share.rs`: `remote_media_consistent` を追加し、
    receipt flag・decode receipt・top-level codec/frame/
    temporal 宣言・codec class 一貫性・port tiling・
    per-port PCM digest・chunk digest を blob 実測と全照合
    （従来は decode scalar のみ比較）。
  - `lookup.rs`: codec class 一貫性 + port tiling を
    manifest 自己申告レベルで再検査し、矛盾を `Corrupt` に
    （`Lossless`+`"mp3"` 矛盾 manifest が `EligibleExact`
    に昇格する経路を閉塞）。
  - test 追加: `rcp_store_04_semantic_contradiction_is_corrupt`、
    `rcp_store_06_remote_media_contradiction_rejected`
    （計 2 件追加で 18→20 fixture）。修正後に
    fmt-check/check/lint/test を再実行し全て green。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク
  指定により対象外。
- 実 OS/filesystem store・実 Plugin・実 codec coverage・
  実 VCS provider・実 RT — 本スライスは headless fixture +
  `InMemoryArtifactStore` のみ。headless PASS を実
  OS/storage・実 Plugin Gate の PASS に昇格しない（後続
  Gate の責務）。
- `synvelis-workspace`/Gateway projection・cross-gate
  （RCP-07）— 別 Worker の lease / 後続 Issue の範囲。
