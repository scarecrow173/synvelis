# sv565 verification — SetProcessorBinding 退役 cascade 監査 slice

対象: `chore/565-processor-binding-audit`（base `b73018bd`）
OS: Windows（worktree `G:\orca\workspaces\synvelis\sv565-processor-binding`）
Toolchain: mise 管理（`mise.toml`/`mise.lock` 正本、Rust CI pin `1.99.0`）
検証日: 2026-11-18

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run test synvelis-project` | Passed — 全 test binary ok。新規 pin 3 件（`set_binding_rebind_retires_old_instance_id`・`set_binding_same_plugin_keeps_instance_id`・`set_binding_unresolved_retires_instance_id`）を含む |
| `mise run test synvelis-gateway` | Passed — 全 test binary ok。新規 wire pin `sv565_processor_set_binding_write_detail_carries_binding_and_retires_identity` を含む |
| `mise run check synvelis-gateway` | Passed — `cargo check --locked --all-targets` |
| `mise run lint synvelis-gateway` | Passed — clippy `-D warnings`（初回 `assert_is_empty` 指摘 → `assert_eq!` に修正して再実行で Pass） |
| `mise run check synvelis-project` | Passed — `cargo check --locked --all-targets` |
| `mise run lint synvelis-project` | Passed — clippy `-D warnings` |
| `mise run fmt-check` | Passed — `cargo fmt --all -- --check`（初回差分 → `mise run fmt` 適用後 Pass） |
| `mise run check-headless-deps` | Passed — `no GUI framework deps in 12 headless crates` |
| `mise exec -- prek run --all-files` | Passed — 全 hook Passed（trim trailing whitespace・fix end of files・check yaml/toml・mixed line ending・utf-8 BOM・detect private key・actionlint） |
| `mise exec -- cargo test --locked -p synvelis-gateway --test prepare sv565` | Passed — fmt 後の再確認、1 passed |
| `mise exec -- cargo test --locked -p synvelis-project --test plugin_instance_persist set_binding` | Passed — fmt 後の再確認、3 passed |

## pin が固定する契約

- `set_binding_rebind_retires_old_instance_id`: 別 `PluginRef` への
  rebind で新 id=2 発行・旧 id=1 は全 entry から消失・
  watermark=3 前進・`resolve(旧 plugin)`→`None`・
  `resolve(新 plugin)`→`Some(2)`・`validate_references` 通過。
- `set_binding_same_plugin_keeps_instance_id`: 同一 `PluginRef`
  の再設定は冪等（id=1・watermark=2 維持）。
- `set_binding_unresolved_retires_instance_id`: `Unresolved` 化で
  entry 削除・watermark=2 後退なし・oracle 通過。
- `sv565_processor_set_binding_write_detail_carries_binding_and_
  retires_identity`: wire `synvelis.processor.set-binding` が
  `unsupported` なしで `Completed`、`Processor` `Write` effect の
  before/after detail が `binding` 差分を運び、candidate
  `plugin_instances` が新 id 発行・watermark 前進・oracle 通過。

## 未検証の項目

| 項目 | 区分 | 理由・follow-up |
| --- | --- | --- |
| GUI-L2（実ウィンドウ操作） | NotRun | POLICIES.md §5 により日常必須検証に含めない。headless domain/gateway のみの変更で UI 経路に触れないため Daily/手動 Full CI に委譲 |
| EXT-L3（外部プラグイン実機） | NotRun | 同上。`PluginRef` は fixture のみで実 plugin runtime を介さない |
| MSRV（rust-version 1.89） | NotRun | CI toolchain pin は 1.99.0。コメント+test のみの変更で edition 2024 構文の新規使用なし。必要なら CI で確認 |
| 3OS 横断（Linux/macOS） | NotRun | POLICIES.md §5 により日常必須検証に含めない。OS 依存コードに触れない |
| Full CI | NotRun | 日常検証は crate 単位の check/lint/test + prek。Full CI は Coordinator 統合時 |
| docs index 再生成 | Coordinator 作業 | `docs/**/README.md` は生成物で worker は編集・再生成しない。統合時に Coordinator が再生成 |

## review-protocol 結果

独立 reviewer 2 名（cooperative + adversarial）によるレビューを
実施。Issue Candidate 6 件のうち CONFIRMED 4 件を処理:

- Reviewer A IC-1（P3）: IR front matter が既存 test ファイルを
  `added` と誤分類 → `modified` に修正
- Reviewer A IC-2（P3）: audit.md の consumer crate 帰属誤り
  （`routing_control.rs`→player、`host_param.rs`→project）
  → audit.md・discovery・IR の参照を crate 修飾パスに修正
- Reviewer B IC-3（P3）: variant adopt が
  `write_plugin_instance` transplant を行うという機構記述の
  誤り → audit.md・discovery を `sync_processor` のみの
  live 再導出に修正
- Reviewer B IC-4（P3）: wire test の id assertion が相対値
  → fixture 決定的のため `== 2`/`watermark == 3` の厳密値に
  強化（fmt 後に再実行で Pass 確認済み）
- Reviewer B IC-1（P2）・IC-2（P3）: `SemanticPort.binding`
  の rebind stale 化と `port_capability_generation` の
  同一 revision 検出漏れ — いずれも main 既存・本 slice
  charter 範囲外のため `evidence/sv565/audit.md` 隣接発見節
  に記録し別 Issue 検討に委ねる（非ブロッキング）

残ブロッカーなし。verdict: ACCEPT WITH NON-BLOCKING NOTES
（残件は全て P2/P3 の記録事項）。

## 変更の性質

production コードの動作変更はゼロ — 契約明記コメント 3 箇所
（`prepare.rs` op_facts arm・`prepare_engine.rs` op-table
entry・`ops.rs` op 定義 doc comment）と test pin 追加のみ。
`unsupported` 追加・store cascade 変更・wire schema 変更は
監査の結果いずれも不要と判定（`evidence/sv565/audit.md` 参照）。
