# issue-262-run-file-real-vm — `run --file` 実 VM/実 authority 実行配線 検証・Deferred 台帳 (synvelis#262)

Issue #262「[impl/cli][AUTX-03] synvelis-cli 独立 Headless 実行
ファイル」の **run --file 実実行 slice** の実測記録。
`synvelis-cli workflow run --file`（`.yaml`/`.yml`/`.luau`）と
`action run --file`（`.luau`）を、分類+deferred 実装から実
headless 実行へ置き換えた:

`source capture/evaluation → typed definition/recipe → plan
compile → WorkflowCoordinator + GatewayStepLane/GatewayJobLane +
MemoryRunJournal → versioned envelope/exit code`

単一 Gateway・単一 CLI adapter・単一 commit boundary・
single-source registry capture を composition root
（`boot_project`）で共有し、live lanes・script surface・
registry compile が同一権威を見る。Issue #262 は OPEN 継続
（auto-close しない・AC checkbox 未更新 — 本台帳が tracking
権威）。

## 環境・対象 Revision・実行コマンド

| 項目 | 値 |
|---|---|
| 対象 Repository | scarecrow173/synvelis |
| 対象 Revision | branch `feat/262-run-file-real-vm`、base `ce9aba94`（`origin/main` HEAD = sv273 `.luau` backend merge #505 + index 再生成 #506、前提 `b78d419` を含む）。検証は commit 前 working tree で実施 |
| OS / Arch | Windows / x86_64（Git Bash 経由） |
| Rust toolchain | rustc/cargo（環境導入済み。CI pin は `mise.ci.toml` 1.99.0） |
| 実行日 | 2026-11-17 |
| 作業 worktree | `G:\orca\workspaces\synvelis\sv262-run-file` |
| binary artifact | `synvelis-cli`（`mise run build-cli` = `cargo build --locked -p synvelis-cli`） |

### 実行した正規検証コマンド（実測）

| コマンド | exit | 結果 |
|---|---|---|
| `mise run fmt-check` | 0 | `cargo fmt --all -- --check` 成功（`cargo fmt --all` で整形後） |
| `mise run build-cli` | 0 | `cargo build --locked -p synvelis-cli` 成功（22.34s、Native/WebView/GUI feature なし） |
| `mise run test synvelis-cli` | 0 | `cargo test --locked -p synvelis-cli`。lib unit 5 + `cli_e2e` **26 tests 全 Pass**（計 31 passed / 0 failed） |
| `mise run test synvelis-automation` | 0 | `cargo test --locked -p synvelis-automation` 全 suite Pass（surface.rs unit 6 件を含む） |
| `mise run test synvelis-gateway` | 0 | `cargo test --locked -p synvelis-gateway` 全 suite Pass（headless seam 非回帰） |

補足: `cargo check --locked -p synvelis-cli`（cargo 直接実行）は
実装中の反復で使用 — 補助証拠であり canonical は上記 mise
タスク。lint/check の mise 実測は本 ledger 末尾の「追補」節を
参照。

## Case 台帳（実バイナリ E2E — `cli_e2e.rs` 26 tests）

| Case | 結果 | 内容 |
|---|---|---|
| `workflow_run_yaml_executes_to_completion` | Pass | `.yaml` emit-only workflow → exit 0 `completed`、`executed:true`/`state:"completed"`/`outputs.out=42`/`run_id`/`plan_digest`(64hex)/`kind:"workflow-dsl"` |
| `workflow_luau_file_executes_to_completion` | Pass | `.luau` spec（`call="emit"`）→ 実 VM 評価→plan→coordinator→ exit 0 `completed`、`evaluated:true`/`executed:true`/outputs 反映 |
| `workflow_run_inputs_inline_and_file` | Pass | `--inputs '{"count":7}'`→outputs.out=7、`--inputs-file`→outputs.out=9、宣言 input 不足→exit 2 `cli.invalid-input`、両 flag 同時→exit 2 `cli.usage` |
| `workflow_run_query_step_surfaces_typed_denial` | Pass | `synvelis.context.query@1.0` query step → live lane→Gateway→empty policy `denied` → run `Failed`、exit 3、`error.code="denied"`（gateway envelope verbatim） |
| `action_run_luau_reaches_gateway_and_surfaces_denial` | Pass | `.luau` ActionRecipe（`contextQuery` member）→ 実 VM→`ActionRecipe::build`→wire→`denied`/exit 3 |
| `action_run_luau_dry_run_reports_recipe_without_executing` | Pass | `--dry-run` → exit 0、`evaluated:true`/`executed:false`/`recipe_kind:"single"`/`members:1`/`recipe_digest` |
| `workflow_run_atomic_macro_rejects_typed` | Pass | `kind: atomic-macro` step（`mac: test.track-lane@1.0`）→ `MacroRegistry::empty()` で compile 時 `UndefinedMacro` → exit 2 `cli.invalid-input` typed reject（macros.rs `WORKFLOW_WITH_MACRO` 相当 fixture、AC9 fail-closed golden） |
| `workflow_run_job_step_rejects_typed` | Pass | `kind: async-submit` step（`capability: synvelis.job.render@1.0`、JobEngine/cap 不在）→ plan compile が capability ref を解決できず typed reject → exit 2 `cli.invalid-input`（compile reject 側を実挙動で固定、AC9 fail-closed golden） |
| `workflow_dry_run_validates_dsl_without_executing` | Pass | `.yaml --dry-run` → exit 0、`executed:false`/`dry_run:true`/`definition_valid:true`/plan_digest |
| `workflow_dry_run_rejects_invalid_dsl` | Pass | `schema_version: 2` → exit 2 `cli.invalid-input` |
| `file_classification_matrix` | Pass | 14 case: `.luau`/`.yaml` 内容不正→2、`.rhai`→5（PoC）、`.lua`→5、未知拡張子/noext→2、`.yaml` under action→2 |
| `no_script_command_or_flag` | Pass | `script run`/`--script` → exit 2 `cli.usage` |
| `dry_run_leaves_project_untouched` | Pass | 既存 dry-run 非変更の非回帰 |
| その他既存 12 tests（help/version/list/describe/invoke/prepare/project info/render/usage/interactive） | Pass | 無変更経路の非回帰 |

## Unit test 追加（`surface.rs` `#[cfg(test)]`）

| test | 内容 |
|---|---|
| `names_derive_deterministically` | `synvelis.context.query`→`contextQuery` 等の固定規則 |
| `surface_maps_every_capability_with_exact_pin` | snapshot→entry の pin/visibility 写像 |
| `name_collision_fails_closed` | 異 id 同名導出 → `DuplicateApiName` |
| `same_id_multiple_versions_still_collide` | 同 id 別 version も fail-closed（script 名 shadow 防止） |
| `invalid_identifier_names_are_emitted_for_model_verdicts` | 不正 identifier は emit→model verdict 委譲 |
| `surface_digest_is_stable` | 決定性 |

## AC 対応（本 slice 範囲 — Issue AC checkbox は未更新）

| Issue AC（抜粋） | 本 slice での状態 |
|---|---|
| action run --file (.luau) / workflow run --file (.yaml/.yml/.luau) が実バイナリで正しい ScriptKind/Version に対応 | **部分消化** — 実実行・envelope/exit は検証済み。ScriptKind/Version の versioned schema 対応は file metadata pin（version=1）で固定、registry version pin は capability ref で保持 |
| `.rhai` PoC・Lua 5.4 非サポート・script run/--script なし・未知/不一致 fail-closed | **消化** — 分類行列 e2e で検証。macro/job-step の compile-time typed reject も e2e golden 化（AC9） |
| 単一 Gateway/Plan の JSON/Exit/Project 結果一致 | **部分消化** — 単一 Gateway/adapter/commit boundary で実証。Project Create/Change/Save/Reload の独立 Expected/Observed case は未整備 |
| `--dry-run` 非変更 | **消化** — readonly boot + `executed:false`、既存 non-mutation test 非回帰 |
| JSON/NDJSON/exit 0,2〜10 golden | **部分消化** — 本 slice 範囲の envelope/exit（0/2/3/5/6/7/9/10 経路のうち実測分）を golden 化。全 code 網羅は別 gate |
| 製品 E2E・3OS 実機・実 2-process lock・durable journal | **NotRun/Deferred** — #267・Issue 対象外 |

## NotRun / Deferred（正直記録）

| 項目 | 状態 | 理由・所有 gate |
|---|---|---|
| 実 job step 実行（await-job/async-submit） | NotRun | JobEngine/`synvelis.job.*` cap 不在 — `workflow_run_job_step_rejects_typed` で compile-time typed reject（exit 2）を golden 固定済み。実 job run は後続 slice |
| named macro 実解決 | NotRun | `MacroRegistry::empty()` で compile 時 typed reject — `workflow_run_atomic_macro_rejects_typed` で `UndefinedMacro`→exit 2 を golden 固定済み。macro 定義 load 経路は AUTX-04 後続 |
| approval→apply 継続実行 | Deferred | auto-approve 禁止 — Approval parked を終端として正直報告。in-memory proposal store は cross-invocation resume を約束しない（envelope 明記） |
| durable run journal / Run 状態正本 | Deferred | Issue 対象外 |
| `workflow status|cancel|validate|preview` verb | Deferred | Issue 対象外（AUTX-05/06 後続） |
| 製品 E2E・3OS 実機受入 | NotRun | #267 gate — 本環境は Windows のみ |
| macOS/Linux build・実機 | NotRun | 同上（post-merge CI / #267） |
| 実 2-process lock・OS credential/session | Deferred | 既存 ledger Deferred 継続 |
| `mise run gw10`/`test-rt`/`*-msrv`/`*-full` lane | NotRun | Daily Full CI 委譲・任意非回帰 lane |
| GUI editor/analyzer 結合 | Deferred | #274 — canonical `ScriptSurfaceSpec` の consume 側 |

## 追補（検証の最終結果 — 全て実測）

| コマンド | exit | 結果 |
|---|---|---|
| `mise run lint synvelis-cli` | 0 | clippy `-D warnings` 成功（初回 10 error — `result_large_err`/`trivially_copy_pass_by_ref`/`doc_markdown`/`map_unwrap_or`/`missing_panics_doc`/`manual_assert_eq` を修正後 green） |
| `mise run lint synvelis-automation` | 0 | clippy 成功 |
| `mise run lint synvelis-gateway` | 0 | clippy 成功 |
| `mise run check synvelis-cli` | 0 | `cargo check --all-targets` 成功 |
| `mise run check synvelis-automation` | 0 | 同上 |
| `mise run check synvelis-gateway` | 0 | 同上 |
| `mise run check synvelis-workspace` | 0 | 同上（`tests/autx08_e2e.rs` 呼出は `--all-targets` compile で担保・変更不要だった） |
| `mise run check synvelis-script` | 0 | 依存元非回帰 check 成功 |
| `mise run check-headless-deps` | 0 | `synvelis-cli` を含む 12 headless crates に GUI framework 依存なし |
| `mise exec -- prek run --all-files` | 0 | trim/eof/yaml/toml/line-ending/BOM/private-key/actionlint 全 Pass |
| `mise run test synvelis-cli`（clippy 修正後の再実行） | 0 | 29 passed / 0 failed（非回帰確認） |
| `mise run fmt-check`（AC9 golden 追加後） | 0 | `cargo fmt --all -- --check` 成功 |
| `cargo test --locked -p synvelis-cli`（AC9 golden 追加後） | 0 | lib unit 5 + `cli_e2e` **26 tests 全 Pass**（計 31 passed / 0 failed）。新規 2 件: `workflow_run_atomic_macro_rejects_typed`・`workflow_run_job_step_rejects_typed` とも実測 exit 2 `cli.invalid-input` |
| `main.rs` HELP 文言 | — | `action run --file`/`workflow run --file` の「execution deferred (synvelis#272/#273)」記述を実配線済みの内容へ更新（`.rhai` PoC-only 注記と `--inputs`/`--inputs-file` を明記） |
