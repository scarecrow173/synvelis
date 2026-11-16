---
id: "PLAN-034YqsfzCOeZQHDLHpvBy9"
type: "plan"
status: "completed"
title: "AUTX-08 Leg C live-adapter leg — 実装 plan"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/267"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx08_e2e.rs"
      - type: "file"
        path: "evidence/sv267/live-adapter-leg.md"
      - type: "file"
        path: "docs/impl/exp/autx-08-live-adapter-leg-exp.jsonl"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/test-specs/autx-08-common-e2e.md"
      - type: "file"
        path: "evidence/sv267/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv267/case-definitions.md"
      - type: "file"
        path: "evidence/sv267/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/autx-08-live-adapter-leg.md"
  implemented-by: []
  depends-on:
    - "docs/designs/autx-08-live-adapter-leg.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/autx-08-live-adapter-leg.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/autx-08-live-adapter-leg.md"
    - "docs/designs/autx-08-live-adapter-leg.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-08-live-adapter-leg.md"
  references:
    - "crates/synvelis-workspace/tests/autx08_e2e.rs"
    - "crates/synvelis-workspace/tests/rig/mod.rs"
    - "crates/synvelis-automation/src/live.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# AUTX-08 Leg C live-adapter leg — 実装 plan Implementation Plan

## Goal

production `GatewayStepLane` を `rig::compose()` stack 上に
組み立て、Leg B と同一論理操作の `StepCall` に対する
outcome/error/`DuplicateRequest`/`operation_result` parity
4 assertion を `autx08_e2e.rs` の Leg C section として
固定し、Issue #267 T-08-1 の AI lane headless 部を閉じる。

## Scope

単一の実装ストリーム。変更は `autx08_e2e.rs` への section
追加 + 承認済み dev-dep 1 件 + evidence/doc-chain であり、
同一の検証 lane（`synvelis-workspace` package test +
headless deps + rt contract）で一括確認できるため
1 plan とする。

## Upstream Documents

- `docs/specs/autx-08-live-adapter-leg.md`

## Design Inputs

- `docs/designs/autx-08-live-adapter-leg.md`
- `docs/test-specs/autx-08-common-e2e.md`（Rule
  AUTX-E2E-SAME-01/FAILCLOSED/IDEM/NOAUTHORITY と
  adapter-internal wire の parity convention）
- `docs/discovery/autx-08-live-adapter-leg.md`

## File Map

- `crates/synvelis-workspace/tests/autx08_e2e.rs` — Leg C
  section 追加（`live_lane`/`live_step_call` helper +
  `autx08_live_adapter_*` 4 test、module doc 更新、
  import 追加）
- `crates/synvelis-workspace/Cargo.toml` —
  `[dev-dependencies]` に `synvelis-automation` path 追加
- `Cargo.lock` — `synvelis-workspace` deps 欄の差分
- `evidence/sv267/{acceptance-ledger,case-definitions,
  verification}.md` — Leg C 記録追記
- `evidence/sv267/live-adapter-leg.md` — Leg C 専用
  evidence（新規）
- `docs/test-specs/autx-08-common-e2e.md` — note 追記のみ
- `docs/{discovery,specs,designs,plans,tasks,test-specs,
  impl/ir,impl/exp}/autx-08-live-adapter-leg.md` +
  `docs/impl/exp/autx-08-live-adapter-leg-exp.jsonl` —
  本タスクの doc-chain（新規）

## Implementation Sequence

- [x] `rig::compose()`/`GatewayStack` の field と
  `CommitPort` impl、`autx06_live_adapters.rs`/
  `live_adapters.rs` の lane composition 先例、
  `StepCall`/`StepLane`/`OperationObservation` の公開
  surface を確認する
- [x] `crates/synvelis-workspace/Cargo.toml`
  `[dev-dependencies]` に `synvelis-automation` path
  dev-dep を追加し、`cargo check` で `Cargo.lock` 差分を
  確定する
- [x] `autx08_e2e.rs` に Leg C section を追加:
  `live_lane`/`live_step_call` helper と 4 test
  （outcome parity・SnapshotExpired parity・fail-closed
  denial・DuplicateRequest + `operation_result` reconcile）
- [x] `cargo test -p synvelis-workspace --locked --test
  autx08_e2e` で 8/8 pass を確認（新規 4 + Leg B 非回帰）
- [x] `mise run fmt-check` / `check synvelis-workspace` /
  `lint synvelis-workspace` / `test synvelis-workspace` /
  `check-headless-deps` / `test-rt` /
  `prek run --all-files`（実行結果は
  `evidence/sv267/verification.md` と PR 本文に記録）
- [x] `evidence/sv267/` 追記 + `live-adapter-leg.md` 新規、
  `docs/test-specs/autx-08-common-e2e.md` note 追記、
  doc-chain・IR/exp・exp.jsonl を作成し commit・push・
  PR 化する

## Dependencies and Risks

- dependency: AUTX-06 live adapters（`GatewayStepLane`/
  `GatewayJobLane`）・AUTX-07 catalog lowering seam・
  Leg B slice（`compose()`/`GatewayStack`/`autx08_e2e.rs`
  helper）は全て merge 済み（base `0b57299` で確認）。
- risk: `DuplicateRequest.original` の operation id が
  Leg B CLI leg と一致しない → per-instance counter は
  compose recipe 同一なら揃う（Leg B 実績）。live lane の
  `invoke` は gateway call を正確に 1 回産むため counter
  系列は一致する設計。乖離時は fixture ではなく adapter
  の wire 産出差異として報告する。
- risk: `expected_revision` の wire 写像が Leg B の
  `consistency` field と異なる semantic を持つ →
  `step_wire` は `expected_revision` を CAS
  `expected-revision` へ写す（adapter contract）。
  `SnapshotExpired` parity が実測で確認済み。
- risk: dev-dep が想定外の依存を引き込む →
  `synvelis-automation` の依存は serde/serde_json/sha2 +
  workspace 内 path のみで、`check-headless-deps` が
  HEADLESS_CRATES 登録済み。実測で green。

## Task Breakdown

- `docs/tasks/autx-08-live-adapter-leg.md` に全手順を
  集約した単一 task とする（test 追加 + dev-dep +
  evidence/doc-chain で細分化の利益がない）。

## Verification

- [x] `cargo test -p synvelis-workspace --locked --test
  autx08_e2e` 8/8 pass（新規 4 test 全て green）
- [x] `mise run fmt-check` / `check synvelis-workspace` /
  `lint synvelis-workspace` / `test synvelis-workspace`
- [x] `mise run check-headless-deps`（dev-dep 追加後も
  GUI/GPU/WebView crate を引き込まない）
- [x] `mise run test-rt`（workspace `rt_` contract 非回帰）
- [x] `mise exec -- prek run --all-files`
- [ ] merge 後 main SHA の必須 CI 証拠・real-OS
  GUI-L2/EXT-L3・live-catalog producer・dispatch-hop
  consumer・`GatewayJobLane` job leg・CLI binary
  cross-check: Coordinator/後続 slice（NotRun/Deferred）

## Open Questions / Gaps

- `GatewayJobLane` の job 観測 parity と honest-cancel
  semantics は #265 coordinator 接続後の別検証。
- `step_wire` ↔ `synvelis-cli build_request` ↔ `lower`
  の 3-producer wire field cross-check は #264 mount
  wiring + CLI binary E2E lane。
- live-registry → `syn.autx.catalog` producer・
  `lower` → gateway dispatch-hop consumer は #264。

## Self-Review

- [x] spec の要求がすべて task に対応している
- [x] placeholder が残っていない
- [x] file path、relation、status、用語が整合している

## Implementation Handoff

- Recommended mode: inline execution（Worker dispatch
  による単一 slice）
- Independent work streams: none
- User approval required before dispatch: yes
  （review-protocol のレビューで blocker 0 を確認済み
  として扱う）
- If approved: discover available implementation and
  delegation capabilities in the current environment,
  then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the
  same dependency graph and verification matrix.

## Review Handoff

- レビュー観点: production `GatewayStepLane` を fake
  なしで使用していること、`step_wire` が private で
  ある以上公開 `StepLane` surface parity が正当な
  証明面であること、headless Pass を実機 Pass に
  昇格していないこと、dev-dep が承認済み 1 件のみで
  あること、実行していない検証を Pass と記録して
  いないこと。
- upstream: spec / design / test-spec を参照。
