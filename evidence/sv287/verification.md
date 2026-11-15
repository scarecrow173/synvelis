# CB-10 gateway typed contract slice verification (Issue #287 / synvelis#287)

OS: Windows 11（worktree `sv287-cb10-gateway-actions`）。toolchain は
`mise.toml` 管理。canonical = `mise run` / `mise exec` 経由。cargo
直接実行は補助証拠。

Synvelis base SHA: `f445563b0c0c01fdf8ca62283c2b71e762da031b`
（branch `scarecrow173/sv287-cb10-gateway-actions`、先行 commit
`b726162` = resource kinds）。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows 11 | mise rust | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise exec -- cargo check --workspace --all-targets` | Windows 11 | mise rust | 0 | Passed（workspace 全 target。clippy 指摘修正後に再実行） |
| `mise run lint synvelis-gateway` | Windows 11 | mise rust | 0 | Passed（clippy `-D warnings`。初回 29 件指摘 → `result_large_err` module allow・`too_many_lines`/`too_many_arguments`/`must_use`/`needless_return`/`single_match_else`/`unused_self`/`redundant_closure`/`needless_pass_by_value` を codebase 先例に倣い解消） |
| `mise run lint synvelis-project` | Windows 11 | mise rust | 0 | Passed |
| `mise run lint synvelis-cli` | Windows 11 | mise rust | 0 | Passed |
| `mise run lint mcp-poc` | Windows 11 | mise rust | 0 | Passed |
| `mise run lint synvelis-workspace` | Windows 11 | mise rust | 0 | Passed |
| `mise exec -- cargo test -p synvelis-gateway` | Windows 11 | mise rust | 0 | Passed（全 suite green。新規 `cb10_variant_actions` 14 件含む — 下表） |
| `mise run test synvelis-project` | Windows 11 | mise rust | 0 | Passed（FAILED/error 0 件。engine.rs は derive 追加のみ） |
| `mise run test synvelis-workspace` | Windows 11 | mise rust | 0 | Passed |
| `mise run test synvelis-cli` | Windows 11 | mise rust | 0 | Passed |
| `mise run test mcp-poc` | Windows 11 | mise rust | 0 | Passed |

## CB-10 contract cases（`cb10_variant_actions`、全 14 Pass）

| test | expected | observed | result |
| --- | --- | --- | --- |
| `fork_prepares_without_mutating_then_commits_with_provenance` | prepare は live 不変・`PreparedMutation::Variant` 保持・simulated candidate に set+2 branch、commit で provenance journal・replay settled | 全 assert 一致 | Pass |
| `fork_fails_closed_without_variant_ports` | ports 無し prepare は拒否 | `variant ports` を含む error | Pass |
| `fork_requires_the_branch_capability_in_the_grant` | capability floor 分離 | mute のみ grant 拒否・fork 付与で prepare 成功 | Pass |
| `edit_candidate_branch_commits_payload_not_live` | candidate edit は payload のみ・live clip 不変・compare digest 不一致 | 全 assert 一致 | Pass |
| `edit_rejects_nested_variant_and_batch_ops` | nested variant/batch 拒否 | "ordinary project operations only" | Pass |
| `edit_with_stale_branch_revision_fails_closed` | simulation CAS 拒否 | `stale-branch-revision` | Pass |
| `stale_base_variant_proposal_cannot_apply_after_live_moves` | apply SEC-11 fence | `RevisionConflict` | Pass |
| `adopt_without_approval_fails_closed` | receipt 無し adopt は `unapproved` reject・adopted 不変 | 全 assert 一致 | Pass |
| `adopt_with_consumed_receipt_commits` | receipt 注入で adopt 成功・payload live 化・provenance runtime block | 全 assert 一致 | Pass |
| `discard_removes_the_branch` | branch 除去・adopted 温存 | 全 assert 一致 | Pass |
| `queries_list_describe_and_compare_on_the_pin` | read-only・scope_kind filter・digest equal・store 再検証 verified | 全 assert 一致 | Pass |
| `describe_and_compare_require_the_set_target` | target 必須 | `TargetRequired` | Pass |
| `variant_ops_cannot_be_batch_members` | batch member 拒否 | "batch" error | Pass |
| `commit_rechecks_the_expected_revision_for_variants` | fence 内 project rev CAS | `RevisionConflict` | Pass |

## 補助証拠

- `cargo test -p synvelis-gateway`（直接実行）は反復中の補助。
  canonical は上表の `mise run`/`mise exec` 行。

## 未検証（NotRun/Unsupported — issue #287 残件）

- 実 CLI binary E2E（`synvelis` CLI からの `synvelis.branch.*`
  呼び出し）。
- MCP stdio E2E（`mcp-poc` の stdio transport 越し呼び出し）。
- workflow-engine（`synvelis-automation`/`synvelis-script` 経由の
  実行）。
- GUI シナリオ（desktop app からの branch 操作）。
- RuntimePublish 本番統合（`NoopRuntimePublish` のみ配線）。
- plugin/PCM adapter の外部 provider 検証。
- cross-gate 受入（CB-11 ledger との結合判定）。
- `VariantPorts::standard()` は `InMemoryBranchContentStore` を
  使う — 永続 store 配線は follow-up。
