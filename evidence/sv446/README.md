# AUTX-01-F01 has_executor accessor — Case 台帳 (Issue #446 headless slice)

作成: 2026-11-15 / branch: `feat/446-has-executor-accessor`
対象: `crates/synvelis-gateway/src/registry.rs`・
`crates/synvelis-gateway/tests/contract.rs`・
`crates/synvelis-workspace/src/action_catalog.rs`・
`crates/synvelis-workspace/tests/autx01_action_catalog.rs` と
対応 docs（`docs/specs|designs|plans|tasks|test-specs` の
`autx-01-f01-has-executor`、`docs/impl/ir|exp`、
`docs/test-specs/autx-01-action-catalog.md`、
`docs/impl/{ir,exp}/autx-01-action-catalog.md`）

語彙は POLICIES.md §2/§3 に従う。**headless Pass は実機 Pass
ではない**。実機未検証項目は Case ID 付きで NotRun/Deferred
として記録する。本スライスは `crates/synvelis-gateway/` と
`crates/synvelis-workspace/` のみ変更し、他 crate・CLI・
view 契約（#266 `automation_ui`）・`Cargo.lock` は非接触。

検証環境: Windows（worktree ホスト x86_64）、rustc 1.99.1
（`mise` 管理 toolchain）。canonical 検証は `mise run` レーン。

## 1. 受入条件 → Case → test → 結果

| Issue #446 受入行 | Case | 検証内容 | test / 証跡 | Expected | Observed | Result |
| --- | --- | --- | --- | --- | --- | --- |
| `CapabilityRegistry` に登録済み executor の有無を返す読み取り専用 accessor（`has_executor` 相当）が存在し、内部状態を変更しない | SV446-AC-01 | `has_executor(&CapabilityRef) -> bool` が `&self` のみで `resolve()` 経由の `handler.is_some()` を返す。未登録 id・未登録 version・handler 無し登録は区別なく `false`（total 判定）。executor 本体は非公開 | `has_executor_reports_installed_and_absent_paths`（fixture install 済みで `true`、未知 id/未知 version/`register_capability(desc, None)` で `false`、呼出後 `list()`/`describe()` 不変 = read-only） | true/false 両経路・非変更性を headless で固定 | 全 case pass | **Pass（headless）** |
| accessor の true/false 両経路を示す headless テストを追加する | SV446-AC-02 | 上記 test が true（handler 付き登録）・false（未知 id・未知 version・No-Handler 登録）の全経路を網羅 | 同上 + `no_handler_invoke_still_returns_no_executor` | 両経路 + NoExecutor authority 不変を固定 | 全 case pass | **Pass（headless）** |
| #260 の Action Catalog/Presentation が No-Handler を `Unknown` と区別する最小の配線を行い、区別を示すテストを追加する（消費側は automation 以外 → Spec で `synvelis-workspace::action_catalog` と明示済み） | SV446-AC-03 | `ExecutorState` に `Installed`/`Absent`/`Unknown` の 3 verdict。`project` に `executors: Option<&CapabilityRegistry>`、`project_gateway` が `Gateway::registry()`/`schemas()` 経由で供給。orphan/descriptor 無し entry は `Unknown` 維持 | `executor_state_distinguishes_installed_absent_unknown`・`project_without_registry_keeps_executor_unknown`・`project_gateway_reports_executor_verdicts`（実 `register_capability(desc, None)` で No-Handler 登録 → `Absent`、fixture → `Installed`、orphan → `Unknown`） | Installed/Absent/Unknown の 3 区別を実登録経路で固定 | 全 case pass | **Pass（headless）** |
| 既存 API・挙動の回帰がないことを既存テストで証明する | SV446-AC-04 | `ExecutorState` variant 追加は serde kebab-case で後方互換。`project` の `executors` は optional（`None` で従来挙動）。`project_gateway` の `&mut`→`&` は reborrow で source 互換。既存 `autx01_action_catalog` suite 全 Pass | `mise run test synvelis-gateway` / `mise run test synvelis-workspace` 全 suite 0 failed | 既存契約の非破壊 | 全 suite Pass（0 failed） | **Pass（headless）** |

## 2. Case 詳細（headless 実施分）

| Case | crate::test | 状態 | 備考 |
| --- | --- | --- | --- |
| SV446-T-01 | `synvelis-gateway::contract::has_executor_reports_installed_and_absent_paths` | Pass | true/false 全経路 + read-only 性（`list`/`describe` 不変） |
| SV446-T-02 | `synvelis-gateway::contract::no_handler_invoke_still_returns_no_executor` | Pass | `has_executor == false` の登録が `bind` 成功・`invoke` → `GatewayError::NoExecutor` — accessor は authority を変えない |
| SV446-T-03 | `synvelis-workspace::autx01_action_catalog::executor_state_distinguishes_installed_absent_unknown` | Pass | 実 `CapabilityRegistry` に handler 有/無混在登録 → `Installed`/`Absent`/orphan `Unknown` |
| SV446-T-04 | `synvelis-workspace::autx01_action_catalog::project_without_registry_keeps_executor_unknown` | Pass | `executors: None` で全 entry `Unknown`（観測不能を `Absent` で偽装しない） |
| SV446-T-05 | `synvelis-workspace::autx01_action_catalog::project_gateway_reports_executor_verdicts` | Pass | 実 `Gateway::register_capability(desc, None)` → catalog `Absent` + 同 ref `invoke` → `NoExecutor`（catalog verdict と runtime truth の一致） |
| SV446-T-06 | `synvelis-gateway` 既存 suite（contract 他） | Pass | `has_executor` 追加による回帰なし |
| SV446-T-07 | `synvelis-workspace` 既存 suite（autx01_action_catalog 15+3 test、autx07 他） | Pass | `project` シグネチャ変更・`ExecutorState` variant 追加による回帰なし |

## 3. 検証コマンド結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt` | Windows | rustc 1.99.1 (mise) | 0 | Pass（`cargo fmt --all` 適用） |
| `mise run fmt-check` | Windows | rustc 1.99.1 (mise) | 0 | Pass（`cargo fmt --all -- --check`） |
| `mise run check synvelis-gateway` | Windows | rustc 1.99.1 (mise) | 0 | Pass（`cargo check --locked --all-targets`） |
| `mise run check synvelis-workspace` | Windows | rustc 1.99.1 (mise) | 0 | Pass（同上） |
| `mise run lint synvelis-gateway` | Windows | rustc 1.99.1 (mise) | 0 | Pass（clippy、warnings denied） |
| `mise run lint synvelis-workspace` | Windows | rustc 1.99.1 (mise) | 0 | Pass（同上） |
| `mise run test synvelis-gateway` | Windows | rustc 1.99.1 (mise) | 0 | Pass（全 test binary 0 failed — 新規 2 test 含む） |
| `mise run test synvelis-workspace` | Windows | rustc 1.99.1 (mise) | 0 | Pass（全 test binary 0 failed — 新規 3 test 含む） |

## 4. NotRun / Deferred / Unsupported 対応表

| Case | 内容 | 状態 | Required/Deferred | 後続 gate / 必要ファイル |
| --- | --- | --- | --- | --- |
| SV446-NR-01 | `automation_ui::ActionRow.executable`/palette 表示への executor verdict 反映 | **Deferred** | #266 view 契約は別 Owner — 本 slice は catalog entry の `executor` field を正しく埋めるまで。`Absent` を blocked 表示に使うかは view 側の判断 | synvelis#266 系の後続。対象: `crates/synvelis-workspace/src/automation_ui`（view 契約 Owner） |
| SV446-NR-02 | 実 GPUI render・実 OS・live Gateway adapter 経由の実行 | **NotRun** | Deferred（#114/#267 別 Gate — GUI-L2） | synvelis#114/#267。`crates/synvelis-app` native lane |
| SV446-NR-03 | CLI surface 本体（list/describe コマンド・`parse_wire` 経路の実結合） | **NotRun** | Deferred（synvelis#262 のリース — `crates/synvelis-cli`） | synvelis#262 |
| SV446-NR-04 | 実 ScriptApiModel（#271）との Capability ID/Version/Schema/Availability 同一性照合 | **NotRun** | Deferred（#271 Owner） | synvelis#271 |
| SV446-NR-05 | Script Action 登録・script→catalog 投影 | **NotRun** | Deferred（synvelis#272） | synvelis#272 |
| SV446-NR-06 | `doc-driven-dev-graph` route による task graph 生成 | **NotRun** | リポジトリ全域の pre-existing artifact-graph/broken-relation findings で router が `blocked`（`focus-required`）を返却（本 slice の変更とは無関係）。route 要求の impl/ir・impl/exp・test-specs 文書は lease 指定パスに作成済み | doc artifact graph の修復は別タスク |
| SV446-NR-07 | `*-msrv` lane・`check-full`/`lint-full`/`test-full`/`build-full`/`verify-full`・Feature Matrix・3OS 横断 | **NotRun** | POLICIES.md §5 / 共通契約どおり日常の必須検証に含めない（Daily Full CI・手動 Full CI に委譲） | CI レーン |

## 5. 補足

- `has_executor -> bool` は「id 未登録」「version 未登録」
  「handler 無し登録」を区別しない total 判定 — catalog は
  登録済み ref にのみ呼ぶため実害なし。区別が必要な consumer
  は `versions()`/`describe()` の typed error を使う。
- `ActionCatalog` は build 時 snapshot — registry への後続
  登録・handler 有無の変化は rebuild まで反映されない
  （登録は append-only/immutable）。
- `Absent` は display metadata であり authorization failure
  ではない — `Gateway::invoke` の `NoExecutor`・policy 経路は
  一切不変（`no_handler_invoke_still_returns_no_executor`・
  `project_gateway_reports_executor_verdicts` で固定）。
- sv#260 で Deferred 記録された `Gateway::schemas` read
  accessor は sv#272/PR #438 で、`has_executor` は本 slice
  （sv#446）で解消 — `docs/impl/{ir,exp}/autx-01-action-catalog.md`
  の defers を更新済み。
