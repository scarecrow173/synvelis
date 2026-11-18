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
| Synvelis commit | `e40b15f`（origin/main `bc90f0e0` merge 後の最終 head）+ this change set |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1（mise managed; CI pin 1.99.0 in mise.ci.toml） |
| Cargo.lock Ruviform SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（変更なし） |

## Commands run (canonical = mise) — final head `e40b15f` で再検証

| Command | Exit | Result |
| --- | --- | --- |
| `mise run check-app-native` | 0 | pass — `cargo check -p synvelis-app --features native --all-targets`（GPUI driver 組立 bin 含む、2m17s） |
| `mise run lint-app-native` | 0 | pass — `cargo clippy -p synvelis-app --all-targets --features native -- -D warnings` |
| `mise run test-app-gpui` | 0 | pass — `cargo test -p synvelis-app --features gpui-test`。全 test binary `test result: ok`（0 failed）。`screens.rs` の `mount_variant_diff_view` を含む GPUI-simulated scenario lane |
| `mise run check synvelis-workspace` | 0 | pass（`cargo check --locked --all-targets -p synvelis-workspace`） |
| `mise run check synvelis-app` | 0 | pass（`cargo check --locked --all-targets -p synvelis-app`） |
| `mise run lint synvelis-workspace` | 0 | pass（`cargo clippy --locked --all-targets -p synvelis-workspace -- -D warnings`） |
| `mise run lint synvelis-app` | 0 | pass（同上 `-D warnings`） |
| `mise run lint-app-gpui` | 0 | pass（`cargo clippy -p synvelis-app --all-targets --features gpui-test -- -D warnings`） |
| `mise run test synvelis-workspace` | 0 | pass — 全 test binary `test result: ok`。`cb09_diff_explorer` 40/40（facade test 7 件: `cb09_engine_compare_projects_cb08_layers`/`_resolves_arrangement_adapter`/`_maps_signal_provider_provenance`/`cb09_engine_adopt_preview_and_commit_keep_cas`/`cb09_engine_adopt_rejects_typed`/`cb09_engine_compare_rejects_stale_and_unknown_pins`/`cb09_engine_authority_and_context_reflect_live_state` を含む） |
| `mise run test synvelis-app` | 0 | pass — 全 test binary（50 binary）`test result: ok`。`cb09_variant_diff` 11/11（lifecycle/kind gate・target seed・view-local authority 非接触・compare pin-scope・stale/implicit reject・preview 非 commit・approval 必須・CAS commit・stale CAS reject・ViewSpec mount smoke・project 切替での variant-diff state クリア） |
| `mise run check-headless-deps` | 0 | pass — "no GUI framework deps in 12 headless crates" |
| `mise run check-app-gpui` | 0 | pass（`cargo check -p synvelis-app --features gpui-test --all-targets`） |
| `mise run fmt-check` | 1 | **環境起因 fail（diff 無関係）** — `cargo fmt --all -- --check` が cargo-fmt の target 列挙段階で `os error 206`（Windows ファイル名長）を返す。本 worktree 固有の現象: clean な commit 済み tree で再現、同一 repo の sibling worktree（`sv559-adopt-unobservable` @ `848ebb73`）と main repo（`848ebb73`）では pass。全 15 workspace member に対する `cargo fmt -p <pkg> -- --check` は個別に全 pass（workspace 外 path dependency は存在しないため `--all` と同一対象を網羅）。CI（Linux）では os error 206 は発生しない。 |

## fmt-check 補足（回帰ではない根拠）

- `cargo fmt --all` の失敗は rustfmt 実行前の cargo-fmt 内部列挙で発生
  （`CARGO_LOG=debug` で 0.8s 時点の `exit_with_error` を確認）。
- 全 member 個別 check: synvelis-theme / project / library / recording /
  recording-poc / player / render / gateway / automation / script /
  script-luau / cli / mcp-poc / workspace / app — 15/15 pass。
- 本 slice の変更 file は `cargo fmt -p synvelis-workspace` /
  `-p synvelis-app` で clean を確認済み。

## Doc index（Coordinator 管理への移行）

当初 branch 側で `docs/impl/ir/README.md`・`docs/test-specs/README.md`・
`docs/tasks/README.md` に index 行を追加していたが、Spec 上 index apply
（`docs/**/README.md`）は Coordinator 管理のため、merge `e40b15f` で
branch 側の index 変更を全て破棄し `origin/main` 側の内容をそのまま
採用した（`docs/tasks/README.md` の content conflict を含む）。
index 再生成は Coordinator 側で実施される。

## NotRun

- `*-msrv` 系 — Daily Full CI 委譲。
- `mise run lint-full`/`test-full`/`build-full`/`verify-full` —
  タスク固有の必須検証に含まれず未実行（変更 crate の
  check/lint/test は全て実行済み）。
- `mise run build-app-native`/`run-app-native` — 実 GPUI window
  lane（`synvelis-native` 実窓起動）。実 Window/実 OS 入力・
  pixel 証拠は #288 系 gate の範囲であり本 slice では未実施。
  `check-app-native`/`lint-app-native`/`test-app-gpui`（GPUI
  driver 組立・clippy・GPUI-simulated test）は上記のとおり
  final head で実行済み。
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
