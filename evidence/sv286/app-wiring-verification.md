# sv#286 verification evidence — CB-09 Difference Explorer app wiring

Slice: `cb-09-diff-explorer`（headless view-model、別 evidence
`verification.md`）を `synvelis-app` へ配線。`VariantDiffEngine`
facade（CB-08 `VariantPorts`/`simulate`/`execute` を opaque に
所有）経由で compare/adopt-preview/adopt を typed intent
dispatch し、`syn.variant-diff.visual`/`view`/`action` op・
detached view lifecycle・`ViewSpec` mount・per-view session・
runtime publish observation を実装した。

## Environment

| Field | Value |
| --- | --- |
| Synvelis commit | `31e680aa`（origin/main base）+ this change set |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1（mise managed; CI pin 1.99.0 in mise.ci.toml） |
| Cargo.lock Ruviform SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（変更なし） |

## Commands run (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt` | 0 | pass — `cargo fmt --all` で整形適用 |
| `mise run fmt-check` | 0 | pass |
| `mise run check synvelis-workspace` | 0 | pass（`cargo check --locked --all-targets -p synvelis-workspace`） |
| `mise run check synvelis-app` | 0 | pass（`cargo check --locked --all-targets -p synvelis-app`） |
| `mise run lint synvelis-workspace` | 0 | pass（`cargo clippy --locked --all-targets -p synvelis-workspace -- -D warnings`。needless_pass_by_value/map_unwrap_or/unused_self/assert_is_empty/len_zero 等を修正して pass） |
| `mise run lint synvelis-app` | 0 | pass（同上。`op_variant_diff_action` の too_many_lines を `variant_diff_run_intent`/`_compare`/`_adopt` への分割で解消、collapsible_if を let-chain 化、test file の needless_borrows_for_generic_args/too_many_lines を修正） |
| `mise run lint-app-gpui` | 0 | pass（`cargo clippy -p synvelis-app --all-targets --features gpui-test -- -D warnings`） |
| `mise run test synvelis-workspace` | 0 | pass — 全 test binary `test result: ok`。`cb09_diff_explorer` 40/40（facade test 7 件: `cb09_engine_compare_projects_cb08_layers`/`_resolves_arrangement_adapter`/`_maps_signal_provider_provenance`/`cb09_engine_adopt_preview_and_commit_keep_cas`/`cb09_engine_adopt_rejects_typed`/`cb09_engine_compare_rejects_stale_and_unknown_pins`/`cb09_engine_authority_and_context_reflect_live_state` を含む） |
| `mise run test synvelis-app` | 0 | pass — 全 test binary（50 binary）`test result: ok`。`cb09_variant_diff` 11/11（lifecycle/kind gate・target seed・view-local authority 非接触・compare pin-scope・stale/implicit reject・preview 非 commit・approval 必須・CAS commit・stale CAS reject・ViewSpec mount smoke・project 切替での variant-diff state クリア） |
| `mise run check-headless-deps` | 0 | pass — "no GUI framework deps in 12 headless crates" |
| `mise run check-app-gpui` | 0 | pass（`cargo check -p synvelis-app --features gpui-test --all-targets`） |

## Auxiliary evidence (canonical = mise, cargo direct)

`cargo clippy --locked --all-targets -p synvelis-workspace -p
synvelis-app -- -D warnings` — lint 修正の反復確認として
direct cargo も使用（正式記録は上記 mise 経路）。

## Doc index 再生成

- `node .agents/skills/impl-doc/scripts/sync_impl_index.js
  --kind ir --apply` → `docs/impl/ir/README.md` に
  `issue-286-diff-explorer-app-wiring.md` 追加
- `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index apply --dir docs/test-specs` →
  `docs/test-specs/README.md` 更新
- `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index apply --dir docs/tasks` →
  `docs/tasks/README.md` 更新

## NotRun

- `*-msrv` 系 — Daily Full CI 委譲。
- `mise run lint-full`/`test-full`/`build-full`/`verify-full` —
  タスク固有の必須検証に含まれず未実行（変更 crate の
  check/lint/test は全て実行済み）。
- `mise run check-app-native`/`lint-app-native`/
  `build-app-native`/`run-app-native` — 実 GPUI window lane。
  本 slice の検証は `gpui-test` feature lane（GPUI-simulated
  scenario）までであり、実 Window/実 OS 入力は #288 系 gate。
- 実 plugin state inspect・PCM/RT signal 測定・real
  perceptual evaluation — provider seam の provenance 付き
  evidence 投影まで。実測定は #288 系 gate。
- `test-rt`/`test-recording-real-input`/`test-recording-real-fs`/
  `test-project-real-fs` 等の real-device lane — 本 slice と
  無関係・hardware 依存のため未実行。

## Known limitation（修復しない・正直記録）

- `syn.variant-diff.view` の mount smoke は ViewSpec parse +
  `mount_variant_diff_view` の component mount 確認まで。
  実 GPUI window での描画・focus・keyboard は未検証。
- exact audition capability は plugin/render probe 未 bind の
  ため `Unavailable` を維持（捏造しない設計）。
- adopt receipt の `/runtime/state` decode 失敗時は publish
  observation を更新しない — 観測欠落は `NotRequested` の
  まま正直に扱う。
