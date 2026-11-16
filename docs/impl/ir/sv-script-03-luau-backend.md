---
id: "IMPL-034YAd9GVhddJjb5Q7ndg0"
type: "impl"
status: "completed"
title: "SV-SCRIPT-03 Luau backend — headless .luau workflow 評価 slice"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/273"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-script/src/backend.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/backend.rs"
      - type: "file"
        path: "crates/synvelis-script-luau/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-script-luau/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-script-luau/tests/backend.rs"
      - type: "file"
        path: "docs/specs/sv-script-03-luau-backend.md"
      - type: "file"
        path: "docs/designs/sv-script-03-luau-backend.md"
      - type: "file"
        path: "docs/plans/sv-script-03-luau-backend.md"
      - type: "file"
        path: "docs/tasks/sv-script-03-luau-backend.md"
      - type: "file"
        path: "docs/test-specs/sv-script-03-luau-backend.md"
      - type: "file"
        path: "docs/discovery/sv-script-03-luau-backend.md"
      - type: "file"
        path: "docs/impl/ir/sv-script-03-luau-backend.md"
      - type: "file"
        path: "docs/impl/exp/sv-script-03-luau-backend.md"
      - type: "file"
        path: "docs/impl/exp/sv-script-03-luau-backend-exp.jsonl"
      - type: "file"
        path: "evidence/sv273/sv-script-03-luau-backend-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-script/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-script/src/workflow.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/importer.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/error.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-automation/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-automation/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-automation/tests/importer.rs"
      - type: "file"
        path: "crates/synvelis-cli/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-cli/src/script.rs"
      - type: "file"
        path: "crates/synvelis-cli/tests/cli_e2e.rs"
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv-script-03-luau-backend.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
    - "docs/designs/sv-script-03-luau-backend.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-script-03-luau-backend.md"
    - "docs/test-specs/sv-script-03-luau-backend.md"
    - "docs/impl/exp/sv-script-03-luau-backend.md"
    - "docs/impl/exp/sv-script-03-luau-backend-exp.jsonl"
    - "evidence/sv273/sv-script-03-luau-backend-ledger.md"
    - "docs/test-specs/sv-script-03-workflow-plan.md"
    - "evidence/sv273/sv-script-03-workflow-plan-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-script/src/backend.rs"
    - "crates/synvelis-script-luau/src/lib.rs"
    - "crates/synvelis-automation/src/importer.rs"
    - "crates/synvelis-cli/src/script.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv-script-03-luau-backend-exp.jsonl"
    rejected: []
---

# SV-SCRIPT-03 Luau backend — headless .luau workflow 評価 slice

## Summary

Issue #273 SV-SCRIPT-03 の第 2 slice。headless pipeline に
実 Luau backend を結合した。`synvelis-script` に言語中立の
backend port（`backend` module）と
`ScriptWorkflowSpec::from_script_value` を追加し、新規 crate
`synvelis-script-luau`（mlua 0.12.2 Luau feature、coordinator
承認済み）で実 VM adapter を実装。
`synvelis-automation` の `FileWorkflowResolver` は注入された
`Arc<dyn ScriptBackend>` + `ScriptBuildContext` 経由で
`.luau` file を評価し、file byte digest を importer 側で
再検証してから `WorkflowScriptDefinition` → import
document → `WorkflowDefinition` へ lower する。
`synvelis-cli` の `workflow run --file x.luau` は実 VM で
評価し typed definition/import 結果を envelope で返すが、
registry/surface 入力が存在しない plan compile は
coordinator 承認の Option B 境界に従い #262/#274 へ defer
する（`host.rs`/`Booted`・canonical `ScriptSurfaceSpec`
は本 slice のリース外）。

## Context

- 対象 Issue: <https://github.com/scarecrow173/synvelis/issues/273>
  （OPEN・`Refs` のみ・Close しない）
- 前段 slice: `docs/impl/ir/sv-script-03-workflow-plan.md`
  （typed spec → import document → `WorkflowDefinition` の
  契約層。実 VM backend は未結合で `.luau` file は
  `UnsupportedSource` だった）
- Task: `docs/tasks/sv-script-03-luau-backend.md`
- Plan: `docs/plans/sv-script-03-luau-backend.md`
- Spec: `docs/specs/sv-script-03-luau-backend.md`
- Design: `docs/designs/sv-script-03-luau-backend.md`
- Test spec: `docs/test-specs/sv-script-03-luau-backend.md`
- Discovery: `docs/discovery/sv-script-03-luau-backend.md`
- Evidence ledger:
  `evidence/sv273/sv-script-03-luau-backend-ledger.md`

## Implementation

### Changes

- `crates/synvelis-script/src/backend.rs`（新規）:
  言語中立の backend port mirror。`ScriptSource`（language・
  source bytes・digest・limits）、`ExecutionBudget`（step/
  instruction/time/cancel flag）、`ScriptValue`（Absent/
  Null/Bool/Integer/Number/String/Bytes/Array/Record/Time/
  ResourceId — authority 表現を持たない中立値）、
  `ScriptArtifact`（kind・schema_version・payload）、
  `ScriptResult`、`ScriptError`/`SessionError`（typed
  kind + position + reason）、`ScriptSession`（evaluate/
  close/session_id）、`ScriptBackend`（language_id・
  capabilities・open_session — source digest・artifact
  kind・budget/isolation を検証）、`ScriptBackendRegistry`
  （AvailableBackends からの fail-closed 解決: 未知
  language/backend・未対応 contract version・ambiguous
  candidate を typed reject）。公開 enum は全て
  `#[non_exhaustive]`。
- `crates/synvelis-script/src/workflow.rs`:
  `ScriptWorkflowSpec::from_script_value` を追加 —
  `ArtifactKind::WorkflowDefinition` の artifact payload
  （`ScriptValue::Record`）を serde 経由で typed spec へ
  decode し、`validate_script_id` + `check_calls` で
  builder と同一の構造規則（id・digest・language/backend
  整合・bindings・duplicate id・node budget）を fail-closed
  検証。shape 不正は `WorkflowBuildError`。
- `crates/synvelis-script/src/lib.rs`: `backend` module と
  全公開型を export。
- `crates/synvelis-script/tests/backend.rs`（新規）: fake
  backend による registry fail-closed・codec・spec decode・
  session lifecycle の契約 test（12 件）。
- `crates/synvelis-script-luau/`（新規 crate）:
  - `Cargo.toml` — `mlua = { version = "0.12.2",
    default-features = false, features = ["luau"] }` のみ
    （coordinator 承認。mlua は本 crate に隔離し、他 crate
    は port trait のみを消費する）。
  - `src/lib.rs` — `LuauBackend`/`LuauSession` が
    `ScriptBackend`/`ScriptSession` を実装。open_session で
    language（Luau）・source digest（SHA-256 照合）・
    artifact kind・source/budget 上限を検証し、session 毎に
    独立した `mlua::Lua` VM を生成（global/state 漏洩なし）。
    `evaluate` は source を text-mode chunk
    （`ChunkMode::Text`、bytecode 拒否）として load・eval し、
    戻り値の Luau 値を中立 `ScriptValue` へ decode
    （integer/float 保存・table→Array/Record・
    function/userdata/thread 等の表現不能値は typed reject）。
    `evaluate` は single-shot で、成功経路でも interrupt
    flag を再検査する（`pcall` に呑まれた interrupt が
    success を装えない）。mlua `configure_luau` が無条件に
    install する `require`/`loadstring`（digest pin 外の
    filesystem code 実行）・`print`（host stdout 汚染）・
    `debug` は session 返却前に global から strip する
    （`STRIPPED_GLOBALS`）。lift は `MAX_LIFT_NODES`
    （1M node）で共有 subtable DAG の指数爆発を Rust 側で
    fail-closed に止める。interrupt hook で wall-clock/
    cancel を safepoint 強制、`set_memory_limit` で memory
    を強制（`Some(0)` は unlimited 化を防ぐため open 時に
    typed reject）。`Instant + Duration` の overflow・
    `Lua` state 生成失敗（`new_with` + `catch_unwind`）・
    artifact digest 失敗は panic/`"undigested"` ではなく
    typed `SessionError`/`ScriptError`。runtime/parse/
    budget/cancel/contract error を `ScriptError` kind へ
    写像。VM 内に authority object・capability 呼出し API
    は存在しない（script は declarative spec record のみ
    返す）。
  - `tests/backend.rs` — 実 VM test 29 件: scalar/record
    変換・int/float 保存・workflow artifact 生成・digest
    mismatch・parse/runtime error・session 分離・budget/
    cancel・artifact contract 違反・非対応値・sandbox
    global strip・DAG lift 予算・single-shot evaluate・
    pcall による budget 回避不可・zero/overflow budget。
- `crates/synvelis-automation/src/importer.rs`:
  - `ScriptBuildContext` — `AvailableBackends` +
    `ScriptApiModel` + `BindingSet` + `ExecutionBudget` を
    まとめて注入する build 入力。
  - `evaluate_luau_workflow(backend, context, text,
    source_digest_hex)` — `SessionConfig`（Luau・digest・
    budget・`ArtifactKind::WorkflowDefinition`）で session
    を open→evaluate→close し、artifact の存在・
    schema_version・kind を検査、payload を
    `ScriptWorkflowSpec` へ decode、`WorkflowScriptDefinition`
    （Luau・Luau backend requirement・digest・spec の
    id/version）を構築して `WorkflowScriptBuilder` へ渡す。
    失敗は `AutomationError::ScriptBackend`（boxed）で
    typed fail-closed。
  - `FileWorkflowResolver` に `luau_backend` +
    `ScriptBuildContext` の注入経路を追加 — `.luau` file
    は bytes の SHA-256 を取り、評価結果の
    `source_digest` と照合（drift は `SourceDrift`）。
    backend 未注入の `.luau` は従来通り
    `UnsupportedSource`。`.rhai` は backend 注入済みでも
    `UnsupportedSource`。
  - `AutomationError::ScriptBackend(Box<ScriptBackendError>)`
    — `ScriptBackendError::{Open,Evaluate,Contract}`。
    variant payload を box 化して Err サイズを 128B 閾値内に
    維持（clippy `result_large_err`）。
  - `ImportError::SourceDrift` の Display を char-safe な
    truncate に修正 — script 制御の `pinned` digest が
    非 ASCII を含んでも panic しない（review で検出した
    byte-slice char-boundary panic）。
- `crates/synvelis-automation/tests/common/mod.rs`:
  再利用可能な `FakeScriptBackend`/`FakeSession` を追加。
- `crates/synvelis-automation/tests/importer.rs`: fake
  backend による fail-closed 行列（backend 無し・digest
  drift・eval failure・artifact 欠落・kind/schema 違反・
  inline 成功・Rhai unsupported）と、実 VM backend による
  `.luau` 解決 test（YAML 同値 spec との definition/plan
  digest 一致・Luau/Rhai/YAML 多段 inline chain）を追加。
- `crates/synvelis-cli/src/script.rs`: `workflow run --file
  x.luau` を `synvelis-script-luau` の実 backend で評価し、
  typed `ScriptWorkflowSpec` + `WorkflowScriptDefinition`
  を envelope で返す経路を追加。registry/surface 入力が
  ない plan compile は unavailable を正直に扱い、envelope
  の `definition`/`import` 結果のみを返す（Option B — plan
  compile は #262/#274 へ defer）。`.yaml`/`.rhai`/`.lua`/
  未知拡張子の分類・reject は不変。
- `crates/synvelis-cli/tests/cli_e2e.rs`:
  `workflow_luau_file_evaluates_to_typed_definition` を追加
  — 有効な `.luau` workflow file が実 VM 評価で typed
  envelope を返すことを検証。分類行列の exit code 期待は
  維持。
- `Cargo.toml`/`Cargo.lock`: workspace member
  `synvelis-script-luau` 追加と mlua 依存木（mlua・
  mlua-sys・luau0-src の追加のみ）を記録。
- docs chain（`docs/{specs,designs,plans,tasks,test-specs,
  discovery,impl/ir,impl/exp}/sv-script-03-luau-backend.md`
  + `docs/impl/exp/sv-script-03-luau-backend-exp.jsonl`）と
  `evidence/sv273/sv-script-03-luau-backend-ledger.md`。

### Decisions During Implementation

- **Option B 境界（coordinator 承認）**: `.luau` は
  evaluate → `ScriptWorkflowSpec` → import document 手前まで
  を配線し、surface/registry 不在は typed fail-closed で
  正直に報告。envelope は definition/import 結果を返し、
  plan compile は synvelis#262/#274 へ defer。`host.rs`/
  `Booted`・canonical `ScriptSurfaceSpec` は本 slice の
  リース外。Option A（`ExecutablePlan` まで結合）は
  registry/surface/`host.rs` accessor が全てリース外・
  未存在のため不採用。
- **mlua 依存隔離（coordinator 承認）**: `mlua = 0.12.2`
  `luau` feature を `synvelis-script-luau` のみに許可。
  他 crate は `synvelis-script` の port trait のみを消費し、
  mlua/Luau 固有型を参照しない。
- **lease 外変更 4 件の事後承認**: Coordinator が retro-
  approve（本来は ask 必須）。`synvelis-automation/src/
  error.rs`（ScriptBackendError 追加・SourceDrift panic 修正）、
  `synvelis-automation/tests/common/mod.rs`、
  `synvelis-cli/tests/cli_e2e.rs`、
  `synvelis-script/tests/backend.rs`（新規 test file —
  Spec は src `#[cfg(test)]` 希望だったが容認）。PR 本文に
  明記。
- **review-protocol 実施（2 reviewer・blocker 0）**:
  cooperative（criteria）/ adversarial（defect hunt）を並行
  dispatch。CONFIRMED P1×3（sandbox escape・lift DAG
  爆発・SourceDrift panic）+ P2×2 + P3×5 を修正し、
  Reviewer A の pcall-budget-bypass 指摘は safepoint 解析で
  棄却（`pcall_cannot_swallow_wall_clock_budget` test で
  実測確認）。詳細は ledger「Review cycle 記録」。
- **doc 配置の修正（gate 対応）**: 当初は前段 slice の
  `docs/impl/ir/sv-script-03-workflow-plan.md` と
  `evidence/sv273/sv-script-03-workflow-plan-ledger.md`
  へ追記していたが、gate 指摘により origin/main へ revert
  し、本 slice 専用の新規 file・8 件 doc chain へ移した。

### Non-goals / Deferred

- 実 Luau/Rhai VM 実行は本 slice で Luau のみ結合済み
  （`synvelis-script-luau`）。Rhai の実 backend・
  ruviform#559 同一バイナリ PoC 結合は依然 **Deferred** —
  本 slice の adapter は interim 実装であり、RF-SCRIPT の
  正式 backend contract への整合は #559 gate（製品 port
  追跡: `scarecrow173/ruviform#692`）後に再検証する。
- `.luau` file 解決は注入された backend 経由で結合済み。
  backend 未注入時・`.rhai` file は従来通り
  `UnsupportedSource`（fail-closed）。
- `synvelis-cli` の `workflow run --file` は `.luau` の
  typed 評価まで配線済み（Option B）。registry/surface/
  plan compile の CLI 結合・canonical `ScriptSurfaceSpec`・
  `host.rs`/`Booted` accessor は synvelis#262/#274 領域
  （リース外）のまま **Deferred**。
- Run/Cancel/Resume/Indeterminate・Grant revoke は既存
  #265 Coordinator/Gateway のみ — Definition/Plan/解決物に
  VM stack・coroutine・continuation は存在しない。
- 製品 E2E・GUI/OS/provider lane・`gw10`/`test-rt`/MSRV
  lane は本 slice 対象外（NotRun）。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-script-03-luau-backend-exp.jsonl` —
  mlua adapter 方針・Option B 境界・review cycle の試行
  記録を採用。

### Rejected

- なし（実験的分岐は発生せず、Option A は設計判断として
  discovery で棄却）。

## Validation

Rules/Examples に対応する検証を実施し、全て green
（詳細: `evidence/sv273/sv-script-03-luau-backend-ledger.md`）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check synvelis-script` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run check synvelis-script-luau` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run check synvelis-automation` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run check synvelis-cli` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run lint synvelis-script` — Pass（exit 0、clippy
  warning 0）
- `mise run lint synvelis-script-luau` — Pass（exit 0、
  clippy warning 0）
- `mise run lint synvelis-automation` — Pass（exit 0、
  clippy warning 0）
- `mise run lint synvelis-cli` — Pass（exit 0、clippy
  warning 0）
- `mise run test synvelis-script` — Pass（97 test）
- `mise run test synvelis-script-luau` — Pass（29 test、
  実 mlua Luau VM）
- `mise run test synvelis-automation` — Pass（184 test）
- `mise run test synvelis-cli` — Pass（24 test: lib 5・
  cli_e2e 19）
- dependent-crate 非回帰: `mise run check
  synvelis-gateway`・`mise run check synvelis-workspace` —
  Pass（exit 0）

ローカル検証は変更範囲（touched crates）の検証であり、
統合検証は merge 後の対象 SHA に対する CI 証拠をもって
行う。macOS/Linux での mlua vendored build（`luau0-src`
C build）は NotRun — 検証環境は Windows 11 のみ。

補助証拠（canonical = mise）: `cargo test -p
synvelis-script --test workflow`・`cargo test -p
synvelis-automation --test importer` の直接実行も同一結果。

## Risks

- `synvelis-script-luau` の adapter は interim 実装で
  あり、RF-SCRIPT 正式 backend contract（ruviform#559 /
  scarecrow173/ruviform#692）との整合は同 gate で再検証が
  必要。session 分離は VM 単位で保証するが、budget は
  instruction 数カウントの近似であり hard real-time 保証
  ではない。
- mlua `configure_luau` の install 集合が更新で変わると
  strip 対象が陳腐化する — adapter test が global strip を
  固定しているが、mlua 更新時は再検証が必要。
- `AutomationError::ScriptBackend` は `Box` 化により Err
  サイズを維持しているが、今後 variant を増やす際は同じ
  制約（128B）に注意が必要。
- `ExecutionBudget::default()` は無制限 — port 契約上
  consumer 責務。CLI は 30s/256MiB を設定済み。#262/#274
  配線時に最小 ceiling を検討。

## Follow-ups

- ruviform#559（製品 port 追跡: `scarecrow173/ruviform#692`）:
  Luau/Rhai 同一バイナリ PoC 結合 — interim
  `synvelis-script-luau` adapter の正式 contract 整合・
  Rhai backend・VM state/continuation 設計はこの gate で
  再検証。
- synvelis#262/#274: CLI `workflow run --file` の
  registry/surface/plan compile 結合・canonical
  `ScriptSurfaceSpec`・`host.rs`/`Booted` accessor
  （リース外・Option B で defer 済み）。
- synvelis#265/#267: Coordinator/Plan/Job 統合・製品 E2E。
- `docs/*/README.md` index 再生成: Coordinator が統合時に
  実施（本 slice では index に触れない）。
