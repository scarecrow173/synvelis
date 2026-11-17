# CB-11 cross-gate headless fixture verification (Issue #288 / synvelis#288)

OS: Windows 11（MINGW64_NT-10.0-26300、worktree
`sv288-cb11-expansion`）。toolchain は `mise.toml` 管理（rustc
`1.99.1`、edition 2024、CI pin `1.99.0`）。canonical =
`mise run` 経由。cargo 直接実行は補助証拠。

Synvelis base SHA: `31e680aaa41083ea95848e4c65b382fb119e079b`
（origin/main merge 済み tip）。Ruviform resolved SHA:
`84a68f25`（`Cargo.lock` git dep）。

slice 1（2026-11-15・base `48f1e1e5`・worktree
`sv288-cb11-gate`）の実行記録は git 履歴の旧版を参照 —
本 file は slice 2（2026-11-17）の実測のみを記録する。

## 実行結果（slice 2・2026-11-17）

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows 11 | mise rust 1.99.1 | 0 | Passed（`cargo fmt --all -- --check`。初回 diff 検出 → `cargo fmt --all` 適用後に再実行して Pass） |
| `mise run check synvelis-project` | Windows 11 | 同上 | 0 | Passed |
| `mise run lint synvelis-project` | Windows 11 | 同上 | 0 | Passed（clippy `-D warnings`。`similar_names`/`too_many_lines` を cb02..cb10 先例の scoped `#[allow]` で処理 — 初回 5 件検出→修正後 Pass） |
| `mise run test synvelis-project` | Windows 11 | 同上 | 0 | Passed（全 test target green。`cb11_cross_gate` 9 件 — 既存 6 + 新規 `cb11_e2e_03_*`・`cb11_e2e_04_orchestrated_compare_*`×2） |
| `mise run check synvelis-gateway` | Windows 11 | 同上 | 0 | Passed |
| `mise run lint synvelis-gateway` | Windows 11 | 同上 | 0 | Passed（clippy `-D warnings`。`used_underscore_binding`/`needless_pass_by_value`/`doc_markdown`/`too_many_lines` を修正・scoped `#[allow]` で処理 — 初回 7 件検出→修正後 Pass） |
| `mise run test synvelis-gateway` | Windows 11 | 同上 | 0 | Passed（全 test target green。新規 `cb11_ai_vcs_off` 3 件含む） |
| `mise run test-rt` | Windows 11 | 同上 | 0 | Passed（`cargo test --workspace --locked rt_` — workspace RT contract。src 未変更だが契約確認として実行） |
| `mise run check-headless-deps` | Windows 11 | node | 0 | Passed（`no GUI framework deps in 12 headless crates` — 新規 test が GUI/GPU/WebView 依存を引き込まないことを確認） |
| `mise exec -- prek run --all-files` | Windows 11 | mise | 0 | Passed（trim/eof/yaml/toml/line-ending/BOM/private-key/actionlint 全 Passed） |
| `node .agents/skills/impl-doc/scripts/audit_impl_record.js` | Windows 11 | node | 0 | Passed（exit 0。cb-11-cross-gate.md に finding なし — grep で 0 件確認。他 file の既存 error/warning は本 slice 範囲外） |
| `node .agents/skills/impl-doc/scripts/audit_experiment_log.js` | Windows 11 | node | 0 | Passed（exit 0。cb-11 関連 finding なし — `issue-147`/`issue-530` 等の legacy `.jsonl` invalid-event-shape error は main 既存で本 slice 範囲外） |

## 補助証拠

- `cargo test -p synvelis-project --test cb11_cross_gate`・
  `cargo test -p synvelis-gateway --test cb11_ai_vcs_off` の
  直接実行は反復中の補助証拠。canonical な記録は上表の
  `mise run` 行。
- docs index（`docs/**/README.md`）は編集・再生成・
  `index apply` を実行していない — Coordinator が統合時に
  `doc-maintenance` Skill で再生成（index 未同期は許容済み）。

## Expected vs Observed（slice 2 新規 fixture）

| test | expected | observed | result |
| --- | --- | --- | --- |
| `cb11_e2e_03_arrangement_composite_adopt_undo_scope_isolation` | 2 Track/4 Clip composite candidate edit が単一 adopt=1 CAS=1 undo unit、undo/redo で pre/post-adopt `Project` bitwise 復帰、scope 外 clip・他 domain・raw material 不変 | 全 assert 一致（`undo_steps==1`・`after==before+1`・`Project` equality・scope 外 clip 同一性） | Pass |
| `cb11_e2e_04_orchestrated_compare_stale_session_never_poses_as_current` | scoped edit で自 session が Right-side dims+`ProjectRevision` stale、sibling は `ProjectRevision` のみ、shared-context edit で全 session `SharedContext` stale、stale session は pin-scoped semantic evidence + aux stale flag | 全 assert 一致（`StaleDimension` 完全一致・`ComparisonFreshness` 遷移） | Pass |
| `cb11_e2e_04_orchestrated_compare_unaligned_probe_loss_provider_honesty` | `Unaligned`→`Unavailable{AlignmentPolicy}`、Mock provider→`Available`+`EvidenceOrigin::Mock`+`Aligned`、Approximate→`Approximate` 非昇格、非有限→`Unknown{EvidenceQuality}`、opaque plugin→`Unavailable{PluginStateUnknown}`、probe loss→`Partial`+`unresolved` 非空+`Unknown` | 全 assert 一致 | Pass |
| `cb11_e2e_07_grantless_agent_ops_fail_closed_ai_off` | grantless agent の prepare が `Denied`("no applicable authorization")・proposal_count 0、apply も `Denied`、human は同 proposal を apply/commit 可能 | 全 assert 一致 | Pass |
| `cb11_e2e_07_vcs_off_variant_lifecycle_and_granted_agent` | `VariantPorts::standard(None)` rig で fork→edit→adopt(ApproveToApply+receipt)→query→discard 完走、adopted=candidate・clip muted・compare `scope_digests_equal=false` | 全 assert 一致 | Pass |
| `cb11_e2e_07_arrangement_scope_fails_closed_until_gateway_wiring` | arrangement scope fork が `variant-unsupported: no domain adapter wired for arrangement scopes` で fail-closed | message 一致 | Pass（現状 pin — src gap は coverage.md「判明した gap」） |

## 既存 fixture の非回帰

- `cb11_cross_gate` 既存 6 test（`cb11_e2e_01_*`×3・
  `cb11_e2e_04_semantic_diff_*`・`cb11_x_rcp_01_*`・
  `cb11_e2e_05_*`）は意味・契約を変えず green を維持
  （`mise run test synvelis-project` で実測）。
- `cb07_arrangement.rs`・`cb08_compare_orchestrator.rs`・
  `cb10_variant_actions.rs` 等の既存 fixture は read 参照
  のみで改変なし — 各 package test で非回帰を実測。

## 未検証（NotRun/Unsupported）

- 実 OS GUI（CB-E2E-06・CB-09）: synvelis#286 未 merge・
  `scarecrow173/ruviform#631` OPEN — NotRun。
- 実 Plugin ABI / native dock: `scarecrow173/ruviform#630`
  OPEN・段階 merge 中・synvelis#176 — NotRun。
- 実 signal/clock・PCM 実測: `scarecrow173/ruviform#628`
  closed（deterministic runtime merge 済み）だが CB PCM 軸
  接続は未評価 — NotRun。
- AudioLens consumer: `scarecrow173/ruviform#294`/`#308`/
  `#309` — NotRun。
- macOS/Linux GUI-L2/EXT-L3: 使用要件未確定 — Deferred。
- 実 AI 統合・実 VCS/durable backend・通常 DAW Gate A
  実機非回帰: 外部 gate — NotRun。
- gateway `VariantPorts::standard` の Arrangement adapter
  登録: src 変更を要する follow-up（本 slice 範囲外）。
- `*-full`・`*-msrv`・3OS 横断・Feature Matrix・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務）。
- merge 後対象 SHA の ci.yml covering run: NotRun pending
  （merge 後の CI 証拠は Coordinator が確認）。
