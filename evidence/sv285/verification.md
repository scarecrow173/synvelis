# CB-08 Comparison Orchestrator headless verification (Issue #285 / synvelis#285)

OS: Windows 11（worktree `sv285-cb08-compare`）。toolchain は
`mise.toml` 管理。canonical = `mise run` 経由。cargo 直接
実行は補助証拠。

Synvelis base SHA: `16c8144`（branch
`scarecrow173/sv285-cb08-compare`）。Ruviform resolved SHA:
`Cargo.lock` git dep（checkout `ac10a2f7`）。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt` → `mise run fmt-check` | Windows 11 | mise rust | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-project` | Windows 11 | 同上 | 0 | Passed（`cargo check --locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | 同上 | 0 | Passed（clippy `-D warnings`。初回 `similar_names`/`unused_self`/`doc_markdown`/`too_many_lines`/`assert_is_empty`/`uninlined_format_args` を修正後 green） |
| `mise run test synvelis-project` | Windows 11 | 同上 | 0 | Passed（全 test target green。`cb08_compare_orchestrator` 13 件含む — `cb08_cmp_01_six_gate_scopes_pin_context_and_semantic_diff`・`cb08_cmp_01_take_usage_scope_pins_and_clean_equal`・`cb08_cmp_02_branch_edit_marks_session_and_result_stale`・`cb08_cmp_02_shared_context_edit_marks_stale_without_scope_move`・`cb08_cmp_02_tempo_and_source_moves_classify_independently`・`cb08_cmp_02_plugin_insert_marks_plugin_dimension_stale`・`cb08_cmp_cache_hit_reuse_and_content_invalidation`・`cb08_cmp_open_rejects_stale_revision_and_scope_digest`・`cb08_cmp_open_rejects_unknown_and_cross_set_endpoints`・`cb08_cmp_open_is_read_only_and_session_is_derived`・`cb08_cmp_signal_provider_gates_plugin_state_and_quality`・`cb08_cmp_signal_unaligned_policy_and_stale_flag`・`cb08_cmp_adopted_branch_compares_against_candidate`） |
| `mise run test-rt` | Windows 11 | 同上 | 0 | Passed（`cargo test --workspace --locked rt_` — workspace RT contract 全 target green、src の RT 経路未変更だが契約確認として実行） |
| `mise run check-headless-deps` | Windows 11 | node | 0 | Passed（`no GUI framework deps in 12 headless crates`） |
| `mise exec -- prek run --all-files` | Windows 11 | mise | 0 | Passed（trim/eof/yaml/toml/line-ending/BOM/private-key/actionlint 全 Passed） |
| `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan` → `index apply` | Windows 11 | node | 0 | Passed（`docs/impl/ir`・`docs/test-specs` を再生成 → 再 plan で全 10 dir in-sync） |
| `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` | Windows 11 | node | 0 | IN-SYNC（`docs/impl/ir/README.md` に cb-08 追加。再 sync で IN-SYNC） |
| `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind exp` | Windows 11 | node | 0 | IN-SYNC（exp index は `.jsonl` log のみ管理 — `.md` 実験メモは cb-01..11 先例同様 index 対象外） |
| `node .agents/skills/impl-doc/scripts/audit_impl_record.js --dir docs/impl/ir` | Windows 11 | node | 0 | Passed（cb-08 file に finding なし。`automation-modulation`/`autx-*`/`vcs-*` 既存 file の section 欠落 warning は本 slice 範囲外の既存事項 — cb-11 と同じ扱い） |
| `node .agents/skills/impl-doc/scripts/audit_experiment_log.js` | Windows 11 | node | 0 | `docs/impl/exp/issue-147-sccache.jsonl` の invalid-event-shape error は main 既存の legacy file 由来で本 slice 範囲外（cb-11 と同じ扱い）。新規 `.md` exp メモは audit 対象外 |

## 補助証拠

- `cargo test --locked -p synvelis-project`・`cargo test -p
  synvelis-project --test cb08_compare_orchestrator` の直接実行は
  反復中の補助証拠。canonical な記録は上表の `mise run` 行。
- `mise run check`/`lint`/`test` は package 引数必須の
  `mise.toml` task 経由で実行。

## Expected vs Observed（新規 test）

| test | expected | observed | result |
| --- | --- | --- | --- |
| `cb08_cmp_01_six_gate_scopes_*` | 6 scope 全て pin+context+semantic diff・edit で `Different`・raw kind 非出現 | 全 scope `Different`・`Current` freshness・raw kind なし | Pass |
| `cb08_cmp_01_take_usage_*` | TakeUsage pin・unedited `Equal`・edit で `Different` | `Equal`→`Different`・scoped kind のみ | Pass |
| `cb08_cmp_02_branch_edit_*` | candidate edit で `Stale{BranchRevision/ScopeContent/BranchPayload/ProjectRevision Right}`・shared 系非該当・aux stale flag | 期待次元のみ列挙・semantic は pin-scoped で Available | Pass |
| `cb08_cmp_02_shared_context_*` | scope 外 edit で `SharedContext`+`ProjectRevision` のみ | `SharedContext` のみ（named 次元非発火） | Pass |
| `cb08_cmp_02_tempo_and_source_*` | tempo→`Tempo`、asset→`Source`、`SharedContext` 非発火 | 各次元独立に分類 | Pass |
| `cb08_cmp_02_plugin_insert_*` | processor 挿入で `Plugin`+`SharedContext` | `PluginInstance` 同期により `Plugin` 発火 | Pass |
| `cb08_cmp_cache_*` | Miss→Hit→別 session Hit→edit で Miss+別 key | `CacheStatus`/`key` が期待通り | Pass |
| `cb08_cmp_open_rejects_*` | `StaleRevision`/`StaleScope`/`UnknownBranch`/`ScopeMismatch` fail-closed | 全 typed error | Pass |
| `cb08_cmp_open_is_read_only_*` | open/evaluate が live revision・`Project`・branch content 不変 | bitwise 不変 | Pass |
| `cb08_cmp_signal_provider_*` | Mock→`origin:Mock` flow・plugin project→`PluginStateUnknown`・NaN→`EvidenceQuality` | 全 gate 発火 | Pass |
| `cb08_cmp_signal_unaligned_*` | `Unaligned`→`AlignmentPolicy`・stale で aux `stale` flag・semantic 温存 | 期待通り | Pass |
| `cb08_cmp_adopted_*` | Live endpoint pin→self-compare で empty changes・`Current` | `Current`・changes 空 | Pass |

## Headless green の範囲（正直な境界）

- 本記録の Pass は **headless orchestration 契約**
  （pin/CAS・context 固定・layer 分離・typed staleness・
  derived cache・provider seam gate・read-only）のみを
  証明する。
- 実 signal render・PCM delta・試聴/知覚受入・plugin GUI・
  AudioLens consumer・RT publish・実機 device は
  `external-gates.md` で NotRun/Deferred/Blocked —
  headless green をそれらへ換算しない。
