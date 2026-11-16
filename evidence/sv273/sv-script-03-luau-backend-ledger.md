# SV-SCRIPT-03 第 2 slice — 実 Luau backend 結合 evidence ledger

Date: 2026-11-16。Slice: `synvelis-script` に言語中立の
backend port（`backend` module）と
`ScriptWorkflowSpec::from_script_value` を追加し、新規
crate `synvelis-script-luau`（mlua 0.12.2 Luau feature、
coordinator 承認済み）で実 VM adapter を実装。
`synvelis-automation` の `FileWorkflowResolver` は注入された
`Arc<dyn ScriptBackend>` + `ScriptBuildContext` 経由で
`.luau` file を評価し、file byte digest を importer 側で
再検証してから `WorkflowScriptDefinition` → import
document → `WorkflowDefinition` へ lower する。
`synvelis-cli` の `workflow run --file x.luau` は実 VM で
評価し typed definition/import 結果を envelope で返す。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/273
（OPEN・`Refs` のみ・Close しない）。
Branch: `feat/273-luau-backend`（worktree `sv273-luau-backend`）。
PR: https://github.com/scarecrow173/synvelis/pull/505

第 1 slice（YAML Workflow DSL・`WorkflowScriptBuilder`・
importer 契約層）の evidence は
`evidence/sv273/sv-script-03-workflow-plan-ledger.md` を参照。

## Coordinator 境界決定（記録）

- `mlua` 依存は `crates/synvelis-script-luau` のみに許可。
  他 crate は `synvelis-script` の port trait のみを消費し、
  mlua/Luau 固有型を参照しない。
- Option B: `.luau` は evaluate → `ScriptWorkflowSpec` →
  import document 手前までを配線し、surface/registry 不在は
  typed fail-closed で正直に報告。envelope は definition/
  import 結果を返し、plan compile は synvelis#262 へ defer。
  `host.rs`/`Booted`・canonical `ScriptSurfaceSpec` は本
  slice のリース外。
- lease 外変更 4 件は Coordinator が事後承認
  （本来は ask 必須）: `synvelis-automation/src/error.rs`・
  `synvelis-automation/tests/common/mod.rs`・
  `synvelis-cli/tests/cli_e2e.rs`・
  `synvelis-script/tests/backend.rs`（新規 test file —
  Spec は src `#[cfg(test)]` 希望だったが容認）。

## 計測環境（第 2 slice）

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `feat/273-luau-workflow-backend`（worktree `sv273-luau-backend`） |
| OS / CPU / FS | Windows 11 / NTFS |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target |
| VM backend | mlua 0.12.2 `luau` feature（interim adapter — RF-SCRIPT 正式 contract は ruviform#559 で再検証） |

## 実行した正規検証（第 2 slice）

ローカル検証は変更範囲（touched crates）の検証であり、
統合検証は merge 後の対象 SHA に対する CI 証拠をもって
行う（本 ledger の Pass はローカル実測値）。

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-script` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-script-luau` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-automation` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-cli` | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-script` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-script-luau` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-cli` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-script` | Pass（97 test: action 22・analyzer 7・backend 12・binding 13・golden 10・luau 9・model 17・workflow 7。doc-test 0） |
| `mise run test synvelis-script-luau` | Pass（29 test、実 mlua Luau VM。doc-test 0） |
| `mise run test synvelis-automation` | Pass（184 test: action_recipe 9・compiler 43・coordinator 17・decode 11・determinism 12・importer 26・live_adapters 15・macros 17・mixed_source_run 6・script_analyzer 12・yaml_subset 15・lib 1。doc-test 0） |
| `mise run test synvelis-cli` | Pass（24 test: lib 5・cli_e2e 19。doc-test 0） |
| `mise run check synvelis-gateway` | Pass（exit 0、dependent-crate 非回帰） |
| `mise run check synvelis-workspace` | Pass（exit 0、dependent-crate 非回帰） |

## 追加検証の要点（第 2 slice）

- `synvelis-script/tests/backend.rs`（12 test）: registry の
  fail-closed 解決（未知 language/backend・contract version
  不一致・ambiguous candidate）・`ScriptValue` codec・
  `ScriptWorkflowSpec::from_script_value` の shape 検証・
  session open/evaluate/close lifecycle。
- `synvelis-script-luau/tests/backend.rs`（29 test、実 VM）:
  scalar/record 変換・integer/float 保存・workflow artifact
  生成・source digest mismatch・parse/runtime error・
  session 間 global/state 分離・budget/cancel・artifact
  contract 違反・非対応値（function/userdata 等）の typed
  reject・sandbox global strip（`require`/`loadstring`/
  `print`/`debug` が nil）・共有 subtable DAG の lift node
  予算・evaluate single-shot・`pcall` が wall-clock budget
  を呑めないこと・zero/overflow budget の typed reject。
- `synvelis-automation/tests/importer.rs`（25 test、前 slice
  16 → 25）: fake backend の fail-closed 行列（backend 無し・
  digest drift・eval failure・artifact 欠落・kind/schema
  違反・inline 成功・`.rhai` unsupported）に加え、実 VM
  backend で `.luau` file が YAML 同値 spec と同一の
  `WorkflowDefinition`・同一 plan digest を生成すること、
  Luau/Rhai/YAML 多段 inline chain が compile されることを
  検証。
- `synvelis-cli/tests/cli_e2e.rs`（19 test）:
  `workflow_luau_file_evaluates_to_typed_definition` — 有効な
  `.luau` workflow file が実 VM 評価で typed envelope を返す。
  分類行列の exit code 期待は維持。

## NotRun / Deferred / Unsupported 台帳（第 2 slice）

| 項目 | 判定 | 理由・後続検証 |
| --- | --- | --- |
| `.luau` file の resolver 解決（backend 注入時） | Pass | 実 VM backend 経由で `WorkflowDefinition`/plan digest が YAML 同値と一致 |
| `.luau` file の resolver 解決（backend 未注入） | Unsupported→Pass | `UnsupportedSource` の typed reject を維持（fail-closed） |
| `.rhai` file の resolver 解決 | Unsupported | backend 注入済みでも `UnsupportedSource` — Rhai 正式対応は対象外 |
| CLI `.luau` → `ExecutablePlan` compile | Deferred | registry/surface 入力不在（Option B）。synvelis#262/#274 領域 |
| Rhai 実 backend | Deferred | ruviform#559 gate |
| `synvelis-script-luau` の RF-SCRIPT 正式 contract 整合 | Deferred | interim adapter — ruviform#559 で再検証 |
| macOS/Linux での mlua vendored build（`luau0-src` C build） | NotRun | 検証環境は Windows 11 のみ。vendored C build の cross-platform 確認は merge 後 CI 証拠で担保 |
| `mise run gw10`・`mise run test-rt`・`*-msrv` | NotRun | 正規レーン外 |
| 実 OS・実 GPUI・実 provider lane | NotRun | headless 契約層のみ |

## Fail-closed verdict 台帳（第 2 slice）

| Case | verdict/型 | 判定 |
| --- | --- | --- |
| backend 未登録 language | `ScriptError`/`SessionError` typed | Pass |
| source digest mismatch（session open） | `SessionError` typed | Pass |
| file byte digest drift（importer 再検証） | `ImportError::SourceDrift` | Pass |
| artifact 欠落 | `AutomationError::ScriptBackend(Contract)` | Pass |
| artifact kind/schema_version 違反 | `AutomationError::ScriptBackend(Contract)` | Pass |
| Luau parse/runtime error | `ScriptError` typed kind | Pass |
| budget/cancel 超過 | `ScriptError` typed kind（interrupt hook） | Pass |
| 表現不能な戻り値（function/userdata 等） | `ScriptError` typed kind | Pass |
| `.rhai` file（backend 注入済み） | `ImportError::UnsupportedSource` | Pass |
| session 間 state/global 漏洩 | 独立 `mlua::Lua` VM で構造的に排除 | Pass |
| authority object の VM 内露出 | 型に存在しない（script は declarative spec のみ返す） | Pass |

## Review cycle 記録（2026-11-16）

review-protocol に従い cooperative（criteria）/ adversarial
（defect hunt）の 2 reviewer を並行 dispatch し、candidate を
検証した。blocker（merge 阻止級の未解決指摘）は 0 件。
CONFIRMED として修正したもの:

| 指摘 | severity | 対応 |
| --- | --- | --- |
| mlua `configure_luau` が `require`/`loadstring`（filesystem code exec・digest pin 外）・`print`（stdout 汚染）・`debug` を無条件 install — script source が digest pin を逸脱可能 | P1 | session 返却前に 4 global を strip（`STRIPPED_GLOBALS`）。実 VM test で nil を検証 |
| `lift_table` が共有 subtable DAG で 2^n path を列挙 — Rust 側処理のため全 VM budget を迂回 | P1 | `MAX_LIFT_NODES`（1M）node budget を lift に追加。DAG test で ContractViolation を検証 |
| `ImportError::SourceDrift` Display が script 制御の非 ASCII `pinned` を byte-slice → char-boundary panic | P1 | char-safe truncate に修正。非 ASCII digest の Display test を追加 |
| `Instant::now() + Duration` が巨大 budget で panic | P2 | `checked_add` → `UnsupportedBudget` typed reject |
| `evaluate_luau_spec` が builder 未経由で spec 構造未検証 — CLI が不正 spec を valid と報告し得た | P2 | `from_script_value` に `validate_script_id` + `check_calls`（builder と同一規則）を集約 |
| `max_memory_bytes: Some(0)` が mlua の 0=unlimited 仕様で fail-open | P3 | open 時に `UnsupportedBudget` で reject |
| entry chunk が bytecode mode を受理し得た | P3 | `ChunkMode::Text` を明示 |
| `Lua::new()` の内部 expect panic・`"undigested"` digest fallback | P3 | `new_with` Result + `catch_unwind` → typed `SessionError`、digest 失敗は typed `ScriptError` |
| `evaluate` が複数回呼べ stale `hit` で誤分類 | P3 | single-shot guard（2 回目は ContractViolation）+ 成功経路でも interrupt flag を再検査 |
| CLI error が canonical 絶対 path を露出・stale unsupported message | P3 | user 提示 path を使用・message 更新 |

検証して棄却/非該当としたもの:

- Reviewer A の「`pcall` が interrupt を呑み false success/無限 hang」:
  Luau の safepoint は CALL/JUMPBACK/FORNLOOP/RETURN に存在し、
  interrupt error は最も近い `pcall` へ unwind するが、catch 後の
  次 safepoint で再 throw されるため `return` に到達不能。
  `pcall_cannot_swallow_wall_clock_budget` test が実測で
  `BudgetExceeded` を確認（Reviewer B の safepoint 解析が正しい）。
  成功経路の `hit` 再検査は防御として残置。
- `synvelis-automation` の新規 public enum に `#[non_exhaustive]`
  なし: crate 既存規約（`AutomationError`/`ImportError` 等全て
  非付与）に一致。port 層（`synvelis-script/backend.rs`）は
  全 enum に付与済み。crate 全体の policy 是正は別件。
- `ExecutionBudget::default()` の無制限: port 契約上 consumer
  責務。CLI は 30s/256MiB を設定済み。#262/#274 配線時に
  最小 ceiling を検討。

## 未対応の作業（残件）

- `.luau` → `ExecutablePlan` の CLI 結合・canonical
  `ScriptSurfaceSpec`・`host.rs`/`Booted` accessor:
  synvelis#262/#274 領域（本 slice リース外・Option B で
  defer 済み）。
- Rhai 実 backend・RF-SCRIPT 正式 backend contract との
  整合: ruviform#559 gate（synvelis 側は
  `scarecrow173/ruviform#692` で製品 port 追跡）。
- Coordinator/Plan/Job 統合・製品 E2E: synvelis#265/#267。
- macOS/Linux での mlua vendored build 検証: merge 後 CI 証拠。
- `docs/*/README.md` index 再生成: Coordinator が統合時に実施
  （本 slice では index に触れない）。
