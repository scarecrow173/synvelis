---
id: "SPEC-034Yqs3yceKe9EYQaW2cxS"
type: "spec"
status: "implemented"
title: "AUTX-08 Leg C AI/live-adapter surface same-wire contract leg (synvelis#267)"
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
  implements: []
  implemented-by:
    - "docs/plans/autx-08-live-adapter-leg.md"
  depends-on:
    - "docs/test-specs/autx-08-common-e2e.md"
    - "docs/impl/ir/autx-08-headless-common-e2e-slice-same-wire-3-surface-contract-test-synvelis-267.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/autx-08-live-adapter-leg.md"
    - "docs/test-specs/autx-08-live-adapter-leg.md"
    - "evidence/sv267/live-adapter-leg.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/autx-08-live-adapter-leg.md"
  derived-by:
    - "docs/designs/autx-08-live-adapter-leg.md"
    - "docs/test-specs/autx-08-live-adapter-leg.md"
  verifies:
    - "docs/test-specs/autx-08-common-e2e.md"
  verified-by:
    - "docs/test-specs/autx-08-live-adapter-leg.md"
  references:
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/coordinator.rs"
    - "crates/synvelis-workspace/tests/autx08_e2e.rs"
    - "crates/synvelis-workspace/tests/rig/mod.rs"
    - "docs/automation/tasks.md"
  defers: []
  deferred-by: []
---

# AUTX-08 Leg C AI/live-adapter surface same-wire contract leg (synvelis#267)

## What Should Be Built

Issue #267 AUTX-08 の T-08-1（GUI/MCP/CLI/AI の同一
CapabilityRef→同一 Outcome/Error、case AUTX-E2E-SAME-01）の
AI lane headless 部を閉じる contract test。production
`GatewayStepLane`（`synvelis_automation::live`、AUTX-06 で
merge 済み）を `crates/synvelis-workspace/tests/rig/mod.rs`
の `compose()` が produce する fresh `GatewayStack` 上に
`GatewayStepLane::new` で組み立て、`StepLane::invoke(&StepCall)`
で Leg B と同一の論理操作 — `synvelis.operation.prepare@1.0`、
args は `note_add_args()` と同一 semantic（`operation` に
`synvelis.midi.note.add@1.0`・`source10` target・key 67/
velocity 90/source-native range）・`expected_revision: Some(1)`・
`project_id`・`principal: agent_principal()`・task `t1` +
grant `g1`（`issue_agent_authority` 相当を `ports.issue_task`/
`issue_grant` で発行）— を投入し、以下 4 assertion を全て
Pass させる:

1. **outcome parity**: `invoke` が返す `GatewayOutcome<Value>`
   が Leg B surface の outcome（GUI catalog lowering + human・
   CLI `handle_claim` + delegated agent・MCP
   `midi.edit.prepare` `structuredContent` decode）と同一
   payload。surface 由来メタ（request_id 等の correlation
   id）は同一性の対象外 — compose 対称の convention は
   Leg B に従う。
2. **SnapshotExpired parity**: `expected_revision: Some(99)`
   （未 commit revision）で `Err(Box<ErrorEnvelope>)` —
   `code`（`GatewayError` variant）/`category`/`retry`/
   `message` が Leg B surface の `ErrorEnvelope` と一致。
3. **fail-closed denial**: `task: None`/`grant: None` の
   delegated agent `StepCall` が CLI/MCP leg の denial と
   同一 typed `ErrorEnvelope` で fail-closed（`attest` は
   `caller_delegated` のみで `Human` principal は
   `GatewayError::Authentication` 固定 — AI leg は
   delegated agent 経路のみを検証する）。
4. **DuplicateRequest parity + `operation_result` reconcile**:
   同一 `idempotency_key` の `StepCall` 再 `invoke`（lane が
   mint する新 request_id）が `GatewayError::DuplicateRequest
   {original}` を返し Leg B replay と同一 `original`
   operation を名指し、`StepLane::operation_result(&original)`
   が recorded outcome を `OperationObservation::Settled`
   として返す — ledger 記録 truth との整合は本 leg では
   `operation_result` reconcile が担う（journal replay
   oracle は Leg B `AUTX-E2E-ORACLE-01` 責務のまま）。

test 命名は `autx08_*` prefix・case-ID（`AUTX-E2E-SAME-01`
該当部）を fn 名/doc comment に含める慣習（autx07・Leg B
と同じ）。配置は `crates/synvelis-workspace/tests/
autx08_e2e.rs` への Leg C section 追加を採用（判断理由は
`docs/impl/ir/autx-08-live-adapter-leg.md` に記録）。

## Why Now

`evidence/sv267/acceptance-ledger.md` の T-08-1 行は
「Pass (headless・3 surface) / AI lane Deferred」であり、
AUTX-08 cross-gate 受入（Issue #267）の same-wire contract
は AI surface を欠いた状態で残っている。AUTX-06 live
adapters（`GatewayStepLane`/`GatewayJobLane`）と AUTX-07
catalog lowering seam は merge 済みであり、Leg B の
3-surface 等価証明と合わせて 4 surface 目の headless
証明が可能になった。#264 mount wiring・#265 coordinator
接続を待たずに固定できる公開 `StepLane` surface 契約が
本 slice の対象。

## Users and Value

- #267 受入 lane の判定者: T-08-1 の AI lane headless 部が
  実測 Pass となり、残件が real-OS/live-catalog/dispatch-hop
  の deferred 群に限定される。
- `synvelis-automation` adapter の保守者: `step_wire` が
  private である以上、公開 `StepLane` surface
  （`invoke`/`operation_result`）が産出する
  outcome/error/`DuplicateRequest` parity が AI leg の
  回帰検出面であることが test として固定される。
- 将来の workflow/coordinator slice（#264/#265）: 本 test
  が「live lane の wire semantics は他 surface と同一」と
  いう前提を headless で保証し、上位結合時の差分切り分けを
  容易にする。

## Scope

### In Scope

- `crates/synvelis-workspace/tests/autx08_e2e.rs` への Leg C
  section 追加（4 test + `live_lane`/`live_step_call`
  helper、既存 Leg B helper の再利用 — rig・fixture・
  authority 発行の変更なし）
- `crates/synvelis-workspace/Cargo.toml` の
  `[dev-dependencies]` への `synvelis-automation` path
  dev-dep 追加（Coordinator 承認済み — acyclic・
  headless-safe・外部依存なし・`synvelis-gateway`/`synvelis-cli`
  同一先例）と `Cargo.lock` の対応差分
- `evidence/sv267/` の Leg C 記録（既存 3 file への追記 +
  `live-adapter-leg.md` 新規）
- `docs/test-specs/autx-08-common-e2e.md` への note 追記のみ
  （Non-goal「live lane の `step_wire` の等価は #264 接続後の
  別検証」に対し、本 slice が公開 `StepLane` surface の
  parity を headless で固定した旨 — front-matter・
  Non-goal 文・構成は変更しない）
- 本 slice の doc-chain（discovery/spec/design/plan/task/
  test-spec/impl-ir/impl-exp + exp.jsonl）

### Out of Scope

- `crates/synvelis-automation/src/**`・`tests/**`・
  `Cargo.toml` の変更（READ-ONLY — `step_wire` 公開化・
  `attest` の Human principal 対応・reconcile 変更は行わない。
  defect 露呈時は修正せず ledger + PR 本文へ報告）
- `GatewayJobLane` の job 観測 leg・workflow coordinator
  live run・job submit/await/cancel の same-wire parity
  （#265 接続後の別検証）
- live-registry→`syn.autx.catalog` producer（#264 接続後）・
  `AutomationCatalog::lower` → gateway dispatch-hop の
  production consumer 接続（#264 mount wiring）
- `synvelis-cli` `build_request`/`claim_for` 出力と
  `lower`/`step_wire` wire の field cross-check
  （`synvelis-workspace` は `synvelis-cli` に依存しない）
- MCP `proposal.apply` idempotency 引数 surface・control
  channel human authority 経路（GW-08 adapter contract）
- real-OS GUI-L2/EXT-L3（実 GPUI window・実 OS 入力・実
  provider・実 CLI binary・実 credential store/transport・
  multi-process durable commit）— #114・`scarecrow173/
  ruviform#628`〜`scarecrow173/ruviform#631` 系 Gate
- `AUTX-ACT/AUTH/MAC/WF/CLI/GUI-*`・`SCRIPT-*` 個別 case・
  T-08-2 journal/policy oracle 新設・docs index
  （`docs/**/README.md`）の編集・再生成
- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 lane
  （Daily Full CI 委譲）

## Requirements

- production `GatewayStepLane` を fake ではなく実 adapter と
  して `rig::compose()` stack 上に組み立て、`StepLane::
  invoke`/`operation_result` のみを観測面とすること
  （`step_wire` は private — wire byte assert はしない）。
- Leg B と同一論理操作（prepare + `midi.note.add` on
  `source10`・`expected_revision: Some(1)`・`t1`/`g1`・
  agent principal）を投入し、4 assertion 全てを Pass
  させること。
- headless Pass を real-OS/EXT-L3 Pass に昇格しないこと
  （POLICIES.md §2-3 — evidence/ledger/PR で NotRun/
  Deferred を明示）。
- dev-dep 追加は承認済みの 1 件のみ。`Cargo.lock` 差分は
  再生成してコミットに含める。
- 実行していない検証を Pass と記録しないこと。

## Acceptance Criteria

- [x] `cargo test -p synvelis-workspace --locked --test
  autx08_e2e` で新規 4 test を含む 8/8 が green（実測 —
  `evidence/sv267/live-adapter-leg.md`・`verification.md`）。
- [x] `mise run fmt-check` / `check synvelis-workspace` /
  `lint synvelis-workspace` / `test synvelis-workspace` /
  `check-headless-deps` / `test-rt` / `prek run --all-files`
  が全て exit 0（実測値は `evidence/sv267/verification.md`）。
- [x] `evidence/sv267/acceptance-ledger.md` T-08-1 行の
  AI lane を実測に基づき `Pass (headless)` へ更新し、
  `case-definitions.md` に live-adapter lane 行を追記。
- [x] `docs/test-specs/autx-08-common-e2e.md` に note 追記
  のみ実施（front-matter・Non-goal 文は不変）。
- [ ] merge 後の main SHA に対する必須 CI 証拠・real-OS
  GUI-L2/EXT-L3 受入・live-catalog producer・dispatch-hop
  consumer は Coordinator/後続 slice の責務（本 slice では
  NotRun/Deferred のまま）。

## Deferred Design Concerns

- `GatewayJobLane` の `observe`/`request_cancel` parity
  （job submit→await→cancel の live workflow 経路）|
  #265 workflow coordinator 接続後に job を含む
  `StepCall`/`JobCall` 経路が production 結線される |
  coordinator live run slice | job lane の
  `CancelRequested` 非昇格など honest-cancel semantics を
  same-wire で固定する検証が残る |
- `step_wire` の wire field と `synvelis-cli`
  `build_request`/`claim_for` 出力の cross-check |
  `synvelis-workspace` は `synvelis-cli` に依存せず、
  両 producer の wire shape は別 crate |
  （`synvelis-gateway`/`synvelis-cli` 側 test または
  共有 wire-builder 抽出が必要） | #264 mount wiring・
  CLI binary E2E lane | producer 間の field drift を
  検出する cross-check が残る |
- `AutomationCatalog::lower` → `Gateway::handle` の
  production dispatch-hop consumer | #264 mount wiring で
  `syn.autx.action` の lowered `requests` を gateway lane
  へ渡す接続が未実装 | #264 slice | seam 契約どおりの
  合成経路のみが検証済みで、production 結線の等価は
  接続後に再検証が必要 |

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/267>
- `docs/automation/tasks.md` §AUTX-08（T-08-1〜T-08-3）
- `docs/test-specs/autx-08-common-e2e.md`（Leg A 契約 freeze・
  Rule AUTX-E2E-SAME-01/FAILCLOSED/IDEM/NOAUTHORITY）
- `docs/impl/ir/autx-08-headless-common-e2e-slice-same-wire-3-
  surface-contract-test-synvelis-267.md`（Leg B IR）
- `docs/impl/ir/autx-06-live-adapters.md`（live adapter 本体）
- `docs/discovery/autx-08-live-adapter-leg.md`（調査記録）
- `docs/designs/autx-08-live-adapter-leg.md`（設計）
- `docs/test-specs/autx-08-live-adapter-leg.md`（検証仕様）
- `evidence/sv267/`（acceptance-ledger・case-definitions・
  verification・live-adapter-leg）
