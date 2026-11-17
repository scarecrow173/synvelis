---
id: "TSPEC-MYFHRUK4femA1AglhsravS"
type: "test-spec"
status: "approved"
title: "Issue #516 test spec — 製品 port 交換の検証保証"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "docs/specs/issue-516-mlua-adapter.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-516-mlua-adapter.md"
    - "docs/designs/issue-516-mlua-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-516-mlua-adapter.md"
    - "docs/plans/issue-516-mlua-adapter.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-516-mlua-adapter.md"
    - "docs/designs/issue-516-mlua-adapter.md"
  verified-by: []
  references:
    - "docs/adr/issue-516-mlua-adapter.md"
    - "evidence/sv516/binding-fit.md"
  defers: []
  deferred-by: []
---

# Issue #516 test spec

## 検証対象

`crates/synvelis-script-luau/tests/backend.rs`（real-VM — mock
なし、製品 `ruviform_script_luau::LuauBackend` 経由で実 Luau
実行）+ consumer regression（`synvelis-automation` 全 test・
`synvelis-cli` check）。

## Rule

### R1 descriptor / 言語・digest fail-closed

- `luau_descriptor_is_compatible_with_contract` — id/language/
  version/capabilities が interim と同一値を広告。
- `open_session_rejects_language_mismatch` —
  `SessionError::LanguageMismatch`。
- `open_session_rejects_source_digest_drift` —
  `SessionError::SourceDigestMismatch`。
- `open_session_rejects_unknown_digest_tag` — `sha256:v1:` 等の
  製品側 tag を synvelis 側が拒否（consumer は `sha256:` のみ）。

### R2 budget fail-closed

- `unsupported_instruction_budget_is_rejected` —
  `UnsupportedBudget{Instruction}`。
- `unsupported_host_call_budget_is_rejected` — 製品は vacuous
  受理するが adapter が `UnsupportedBudget{HostCall}` で拒否
  （port 契約「要求次元は enforce 必須」を保持）。
- `zero_memory_budget_is_rejected` — 製品 `validate_budget` →
  `UnsupportedBudget{Memory}`。
- `overflowing_wall_clock_budget_is_rejected` — 同様に拒否。
- `pcall_does_not_swallow_wall_clock_budget` — 製品 interrupt
  deadline が pcall を貫通。
- `budget_exceeded_reports_dimension` — 単一 armed 次元は
  `Some(kind)`、複数併用は `None`（`attribute_budget`）。

### R3 evaluate / artifact / provenance

- `evaluate_runs_source_and_returns_record` — record payload で
  `ScriptResult` 返却・artifact digest は synvelis canonical
  （`digest_of(to_json(payload))`）。
- `evaluate_twice_is_contract_violation` — 2 回目 `ContractViolation`。
- `evaluate_after_close_is_backend_error` — close 後 `Backend`。
- `non_record_artifact_payload_is_internal` — 製品契約の Record
  必須が `Internal` で先に効く（interim の scalar 受理からの
  分類差・記録済み）。
- `artifact_provenance_tracks_synvelis_source` — source_digest
  `sha256:` 形式・source_revision・produced_by.impl_version は
  製品 backend 値。

### R4 sandbox / isolation

- `sandbox_strips_escape_globals` — `require`/`loadstring`/
  `print`/`debug` 剥奪（製品 VM の globals 剥奪を検証）。
- `sessions_are_isolated` — session 毎 fresh VM・状態非共有。
- `shared_subtable_node_limit_applies` — lift nodes 上限。
- `cancel_marks_session_cancelled` — CancelToken 経由で
  `Cancelled`。

### R5 consumer regression

- `mise run test synvelis-automation` — 全 test green（`.luau`
  resolver/eval の real-VM 経路含む）。
- `mise run check-cli` + `mise run lint-cli` — lease 外 consumer
  が無変更でコンパイル・lint 通過。
- `node tools/check-headless-deps.mjs` — headless 境界維持
  （mlua は ruviform-script-luau 経由のみ）。

## 記録済みの分類差（test 更新根拠）

| ケース | interim | adapter+製品 | test 対応 |
| --- | --- | --- | --- |
| non-record artifact payload | 受理（scalar 可） | `Internal`（Record 必須） | record 形へ更新・bare scalar は Internal を検証 |
| sparse/mixed table lift | `ContractViolation` | `Internal` | 分類差を記録・consumer 非依存 |
| 非有限数（record 内包） | `ValueOutOfRange` | `ValueOutOfRange` | adapter 変換で維持 |
| BudgetExceeded kind | `BudgetKind` | `Option<BudgetKind>` | `Some`/`None` パターンへ更新 |

## NotRun

- 実 Luau.Analysis native FFI bridge 走行（analyzer）— sv#274
  gate。本 slice は runtime port のみ。
- macOS/Linux 実機受入 — sv#267 gate。Windows `x86_64` のみ実測。
