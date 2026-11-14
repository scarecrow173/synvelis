# Verification — synvelis#292 RCP-03 headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust。

## 実行結果（実測）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0。新規ファイルの整形差分を `cargo fmt --all` で解消後 green） |
| `mise run check synvelis-render` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-project` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-player` | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-render` | Pass（exit 0、clippy `-D warnings`。synvelis-project/recording/library/player/render を含む dependency 経路で warning 0） |
| `mise run test synvelis-render` | Pass（exit 0 — 全 suite green。`rcp03_render_policy.rs` 21/21 を含む: l2_e2e 17, rcd_04_rt_comp_pcm 2, rcp01_recipe 11, rcp02_artifact 20, rcp03_render_policy 21, sf03_render 2, tim04_plugin_state 5, x_e2e 5、計 83 件 + doc-tests 0） |
| docs index 再生成（doc-maintenance `index apply --dir`） | `docs/impl/ir/README.md`・`docs/test-specs/README.md` を再生成（`docs/impl/exp` は in-sync・書込 0）。test-spec ID 修正後に test-specs index を再適用済み |
| `doc_status.js lint --dir docs/test-specs` | 範囲内 Pass — `rcp-03-render-policy.md` finding 0 件（初回は test-spec ID 形式 `TSPEC-<22-char>` 不適合で `invalid-id-prefix` 1 件 → ID を `TSPEC-Rcp03RenderPolicyTestX` に修正し index 再生成・再 lint で 0 件。dir 全体の既存 baseline finding 145 件は範囲外・未修復） |
| `audit_impl_record.js --dir docs/impl/ir` | 範囲内 Pass — `rcp-03-render-policy.md` finding 0 件（baseline 残存） |
| `audit_impl_record.js --dir docs/impl/exp` | 範囲内 Pass — `rcp-03-render-policy.md` finding 0 件（baseline 残存） |

## 補助証拠（canonical = mise）

- `cargo fmt --all` 適用（mise 管理 toolchain 経由）。
- lint で検出・修正した点（`#[allow]` 不使用・warning 0 化）:
  - `trigger.rs`: 大きな enum variant（`SaveEvent::Completed`/
    `CommitEvent::Committed` の receipt）を `Box` 化し
    `large_enum_variant` 解消。
  - `model.rs`（render_job）: `RenderExecution::Output` を
    `Box<RenderedOutput>` 化、dead variant 除去、doc
    backtick 修正。
  - `orchestrator.rs`: `too_many_lines` を
    `resolve_recipes`/`coalesce_or_insert`/`drive_render`/
    `drive_demand`/`fail_demand`/`publish_rendered`/
    `find_reusable`/`inspect_share` 等の helper 分割で解消、
    `needless_pass_by_value` を参照化、`map_or`/
    `map_or_else`/`let-else`/`?` への idiomatic 変換、
    wildcard match を明示 variant 列挙へ、命名衝突
    （renderer/rendered）解消。
  - `rcp03_render_policy.rs`: `Ports` helper を consuming
    `build` 化、fixture cast を checked 変換へ、assertion
    を `assert_eq!`/`assert_ne!` へ、未使用 binding/import
    除去、共有 `Rc<Cell<usize>>` call counter で orchestrator
    の `&mut executor` 借用中も assert 可能に。
- test fixture で判明・修正した実装側 bug:
  - `retry()` が retryable worker failure を
    `pending_reason` に記録する経路を認識せず `false` を
    返していた → `JobFailureReason::retryable()`/
    `gate_managed()` helper を追加し、retryable pending
    のみ retry で queued に戻す。`InsufficientStorage`/
    `QuotaExceeded` は gate-managed のまま `drive()` の
    環境再評価に限定し、`retry()` が gate を bypass
    しないことを維持。
- review-protocol adversarial pass で判明・修正した点:
  - `attach_external_manifests` が
    `attach_external_manifest` の `Result` を `let _ =` で
    破棄しており、doc comment の "recorded evidence" と
    実装が乖離していた（unsafe locator・quarantined
    artifact・store error が無証拠で消失）→
    `ExternalReceipt::ExternalManifestAttachFailed`
    variant を追加し external receipt stream に記録
    （render job receipt・commit locator は不変のまま）。
  - `submit()` が job record を `Queue` で作成後
    `Resolve`→`Queue` と逆行する Transition receipt を
    出していた → record 初期 state を `Resolve` にし
    `Resolve→Queue` の順行 transition のみ記録。
  - `fail_demand` の retryable pending が `Rendering→Queue`
    を receipt なしで遷移していた → `transition()` 経由に
    修正し全 state 変更に Transition receipt を保証。
  - `terminate()` が Published/Partial/Cancelled で
    Transition receipt を出さず Failed のみ二重経路
    だった → `terminate()` が `from != to` のとき常に
    Transition receipt を発行するよう統一
    （`terminate_with_failure` の重複 transition を除去）。
- 非 blocking 観察（修正せず記録）:
  - terminal（Published/Partial/Failed/Cancelled）job への
    同一 key の重複 event は coalesce され新規 work を
    起こさない — 冪等・fail-closed 方向で receipt
    （`Coalesced`）には残る。retryable failure は
    `retry()`、永久 failure は policy 変更（新 key）で
    再試行する設計。RCP-POL-01 の「同時 event coalesce」
    の範囲内と判断。
  - trigger routing 経路（save/commit/share/demand）は
    `JobCostEstimate::default()`（全 0）を積む —
    budget/storage-quota gate は caller-declared estimate
    を評価するため、routing 層では推定を行わない
    （cost model は後続 job-gate slice の責務）。
    `submit()` direct caller は estimate を明示できる。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク
  指定により対象外。
- 実 OS/filesystem・実 VCS provider・実 plugin・実 codec
  coverage・実 async scheduler・実 RT lane・GPUI・
  cross-gate — 本スライスは headless fixture +
  `InMemoryArtifactStore` + injected `RenderExecutor` の
  み。headless PASS を実 Gate の PASS に昇格しない
  （後続 Gate の責務）。
- GUI/Headless CLI/Workflow/Script の実 Gateway 投影 —
  lease 外（sv#272・sv#279）。typed port/contract の
  coalesce 検証まで。
