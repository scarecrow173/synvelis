---
id: "TASK-034YqsgahiIK0CTp18Za5F"
type: "task"
status: "done"
title: "AUTX-08 Leg C live-adapter leg — T-08-1 AI lane headless closure"
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
    - "docs/plans/autx-08-live-adapter-leg.md"
  implemented-by: []
  depends-on:
    - "docs/plans/autx-08-live-adapter-leg.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-08-live-adapter-leg.md"
  references:
    - "crates/synvelis-workspace/tests/autx08_e2e.rs"
    - "crates/synvelis-automation/src/live.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# AUTX-08 Leg C live-adapter leg — T-08-1 AI lane headless closure

## Work

Issue #267 AUTX-08 T-08-1 の AI lane headless 部を閉じる。
`crates/synvelis-workspace/tests/autx08_e2e.rs` に Leg C
section を追加し、production `GatewayStepLane` を
`rig::compose()` の fresh `GatewayStack` 上に
`GatewayStepLane::new` で組み立てて、Leg B と同一論理
操作（`synvelis.operation.prepare@1.0` + `midi.note.add`
on `source10`・`expected_revision:1`・`t1`/`g1`・agent
principal）の `StepCall` に対し 4 assertion —
(i) outcome parity、(ii) `SnapshotExpired` parity、
(iii) fail-closed denial parity、(iv) `DuplicateRequest`
parity + `operation_result` reconcile — を固定する。
`crates/synvelis-workspace/Cargo.toml`
`[dev-dependencies]` に `synvelis-automation` path
dev-dep を追加（Coordinator 承認済み）し `Cargo.lock`
差分を含める。evidence/sv267 の記録更新と
`docs/test-specs/autx-08-common-e2e.md` への note 追記、
本 doc-chain を作成する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/autx-08-live-adapter-leg.md`
- `docs/specs/autx-08-live-adapter-leg.md`
- `docs/designs/autx-08-live-adapter-leg.md`
- `docs/test-specs/autx-08-live-adapter-leg.md`
- `docs/test-specs/autx-08-common-e2e.md`
- `docs/discovery/autx-08-live-adapter-leg.md`

## Dependencies

- AUTX-06 live adapters（`GatewayStepLane`/`GatewayJobLane`）
  — merge 済み（`0b57299`）
- AUTX-07 catalog lowering seam — merge 済み
- Leg B slice（`compose()`/`GatewayStack`/`autx08_e2e.rs`
  helper）— merge 済み
- `synvelis-workspace` → `synvelis-automation` dev-dep —
  Coordinator 承認済み（acyclic・headless-safe・外部依存
  なし）

## Affected Files

- `crates/synvelis-workspace/tests/autx08_e2e.rs`（Leg C
  section + helper + import + module doc）
- `crates/synvelis-workspace/Cargo.toml`
  （`[dev-dependencies]` のみ）
- `Cargo.lock`（`synvelis-workspace` deps 欄の差分）
- `evidence/sv267/{acceptance-ledger,case-definitions,
  verification}.md`（追記）・`evidence/sv267/
  live-adapter-leg.md`（新規）
- `docs/test-specs/autx-08-common-e2e.md`（note 追記のみ）
- `docs/{discovery,specs,designs,plans,tasks,test-specs,
  impl/ir,impl/exp}/autx-08-live-adapter-leg.md` +
  `docs/impl/exp/autx-08-live-adapter-leg-exp.jsonl`
  （新規 doc-chain）

## Done When

- [x] `cargo test -p synvelis-workspace --locked --test
  autx08_e2e` が新規 4 test を含む 8/8 で green
- [x] `mise run fmt-check` / `check synvelis-workspace` /
  `lint synvelis-workspace` / `test synvelis-workspace` が
  pass
- [x] `mise run check-headless-deps` が pass（dev-dep
  追加後も GUI/GPU/WebView crate を引き込まない）
- [x] `mise run test-rt` が pass
- [x] `mise exec -- prek run --all-files` が pass
- [x] `evidence/sv267/acceptance-ledger.md` T-08-1 行の
  AI lane を実測に基づき `Pass (headless)` へ更新、
  `case-definitions.md` に live-adapter lane 行を追記、
  `verification.md` に実測記録
- [x] `docs/test-specs/autx-08-common-e2e.md` に note
  追記のみ実施（front-matter・Non-goal 文不変）
- [ ] merge 後 main SHA の必須 CI 証拠・real-OS
  GUI-L2/EXT-L3・live-catalog producer・dispatch-hop
  consumer・`GatewayJobLane` job leg・CLI binary
  cross-check（Coordinator/後続 slice、NotRun/Deferred）

## Verification

- [x] `cargo test -p synvelis-workspace --locked --test
  autx08_e2e`: 8/8 pass（`autx08_live_adapter_lands_
  same_outcome`・`autx08_live_adapter_unavailable_
  revision_fails_identically`・`autx08_live_adapter_
  without_authority_fails_closed`・`autx08_live_adapter_
  duplicate_reconciles_original` + Leg B 4 test 非回帰）
- [x] `mise run fmt-check` / `check synvelis-workspace` /
  `lint synvelis-workspace` / `test synvelis-workspace` /
  `check-headless-deps` / `test-rt` /
  `prek run --all-files`（実測値は `evidence/sv267/
  verification.md`）
- [x] `audit_impl_record.js`・`audit_experiment_log.js`・
  `doc_status.js lint`（docs/impl 配下の新規 doc）
