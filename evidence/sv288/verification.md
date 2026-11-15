# CB-11 cross-gate headless fixture verification (Issue #288 / synvelis#288)

OS: Windows 11（MINGW64_NT-10.0-26300、worktree
`sv288-cb11-gate`）。toolchain は `mise.toml` 管理（rustc
`1.99.1 (58cf8f9c3 2026-10-13)`、edition 2024、CI pin
`1.99.0`）。canonical = `mise run` 経由。cargo 直接実行は
補助証拠。

Synvelis base SHA: `48f1e1e5d807579d2af67ad97771b38eae34f6f0`
（origin/main merge 済み tip）。Ruviform resolved SHA:
`fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock`）。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows 11 | mise rust 1.99.1 | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-project` | Windows 11 | 同上 | 0 | Passed |
| `mise run lint synvelis-project` | Windows 11 | 同上 | 0 | Passed（clippy `-D warnings`。新規 test の `too_many_lines` を cb02/cb03 先例に倣い scoped `#[allow]` + doc backtick 修正済み） |
| `mise run test synvelis-project` | Windows 11 | 同上 | 0 | Passed（全 test target green。`cb11_cross_gate` 6 件含む — `cb11_e2e_01_adopt_single_cas_undo_save_reload_across_domains`・`cb11_e2e_01_take_comp_adopt_undo_save_reload_raw_immutable`・`cb11_e2e_01_fork_edit_isolation_across_five_domains`・`cb11_e2e_04_semantic_diff_reports_scoped_changes_only`・`cb11_x_rcp_01_render_pin_scope_vs_shared_context`・`cb11_e2e_05_stale_reject_is_atomic_and_domain_isolated`） |
| `mise run test-rt` | Windows 11 | 同上 | 0 | Passed（`cargo test --workspace --locked rt_` — workspace RT contract 全 target green、src 未変更だが契約確認として実行） |
| `mise run check-headless-deps` | Windows 11 | node | 0 | Passed（`no GUI framework deps in 12 headless crates` — 新規 test が GUI/GPU/WebView 依存を引き込まないことを確認） |
| `mise exec -- prek run --all-files` | Windows 11 | mise | 0 | Passed（trim/eof/yaml/toml/line-ending/BOM/private-key/actionlint 全 Passed） |
| `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan` | Windows 11 | node | 0 | Passed（初回 `docs/test-specs` changes=2 → `index apply` で `docs/test-specs/README.md` 再生成 → 再 plan で全 10 dir in-sync） |
| `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` | Windows 11 | node | 0 | Passed（`docs/impl/ir/README.md` に cb-11-cross-gate 追加。再 sync で IN-SYNC） |
| `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind exp` | Windows 11 | node | 0 | IN-SYNC（exp index は `.jsonl` log のみ管理 — `.md` 実験メモは cb-01..06 先例同様 index 対象外） |
| `node .agents/skills/impl-doc/scripts/audit_impl_record.js` | Windows 11 | node | 0 | Passed（cb-11-cross-gate.md に finding なし。vcs-* 既存 file の `defers` warning は本 slice 範囲外の既存事項） |
| `node .agents/skills/impl-doc/scripts/audit_experiment_log.js` | Windows 11 | node | 0 | Passed（exit 0。`docs/impl/exp/issue-147-sccache.jsonl` の invalid-event-shape error は main 既存の legacy file 由来で本 slice 範囲外。新規 `.md` exp メモは audit 対象外） |

## 補助証拠

- `cargo test --locked -p synvelis-project` 等の直接実行は
  反復中の補助証拠。canonical な記録は上表の `mise run` 行。
- `mise run check`/`lint`/`test` は package 引数必須の
  `mise.toml` task 経由で実行。

## Expected vs Observed（新規 fixture）

| test | expected | observed | result |
| --- | --- | --- | --- |
| `cb11_e2e_01_adopt_single_cas_undo_save_reload_across_domains` | 5 domain candidate edit が 1 adopt=1 CAS=1 undo unit、undo で pre-adopt `Project` と完全一致、save/reload で variant set・lineage・`BranchContentRef`・payload digest が verbatim | 全 assert 一致（`Project` equality・receipt・reload 後 serialize 比較） | Pass |
| `cb11_e2e_01_take_comp_adopt_undo_save_reload_raw_immutable` | Take/Comp adopt 後も raw source・asset・recording registry・take 実体 digest が不変、undo/save/reload で provenance verbatim | 全 assert 一致 | Pass |
| `cb11_e2e_01_fork_edit_isolation_across_five_domains` | candidate edit が live state・sibling candidate から隔離される | 全 assert 一致 | Pass |
| `cb11_e2e_04_semantic_diff_reports_scoped_changes_only` | scoped adoption が scoped semantic change のみ報告、raw material churn を報告しない | `compare_snapshots` の `SemanticChange` が scope 内 object のみを列挙 | Pass |
| `cb11_x_rcp_01_render_pin_scope_vs_shared_context` | scope edit で `scope_digest` のみ、shared-context edit で `shared_context_digest` のみ変化 | 両方向で digest 変化を確認 | Pass |
| `cb11_e2e_05_stale_reject_is_atomic_and_domain_isolated` | stale adopt が `VariantError::StaleScope` で fail-closed、live revision・sibling candidate 無汚染 | reject 後 `Project` snapshot 一致・revision 単調・candidate digest 不変 | Pass |

## 未検証（NotRun/Unsupported）

- 実 OS GUI（CB-E2E-06・CB-09）: `scarecrow173/ruviform#631`
  実 Window runner・synvelis#114/#176 未 merge — NotRun。
- 実 Plugin ABI / native dock: `scarecrow173/ruviform#630`・
  synvelis#176 未 merge — NotRun。
- 実 signal/clock・PCM 実測: `scarecrow173/ruviform#628`・
  #296 未 merge — NotRun（headless render-pin 解決までが
  runnable 範囲）。
- AudioLens consumer: `scarecrow173/ruviform#294`/`#308`/
  `#309` — NotRun。
- macOS/Linux GUI-L2/EXT-L3: 使用要件未確定 — Deferred。
- CB-07 Arrangement（#284）・CB-08 orchestrated compare
  （#285）・CB-10 gateway actions（#287）: 前提未 merge —
  Blocked。
- `*-full`・`*-msrv`・3OS 横断・Feature Matrix・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務）。
- merge 後対象 SHA の ci.yml covering run: NotRun pending
  （merge 後の CI 証拠は Coordinator が確認）。
