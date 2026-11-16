---
id: "TSPEC-034XPPHPJ34MgINj3z1hDs"
type: "test-spec"
status: "approved"
title: "AUTX-08 common E2E same-wire contract test spec (synvelis#267)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/267"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx08_e2e.rs"
      - type: "file"
        path: "docs/test-specs/autx-08-common-e2e.md"
      - type: "file"
        path: "docs/impl/ir/autx-08-headless-common-e2e-slice-same-wire-3-surface-contract-test-synvelis-267.md"
      - type: "file"
        path: "evidence/sv267/case-definitions.md"
      - type: "file"
        path: "evidence/sv267/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv267/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/tests/rig/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/autx-07-mount-wiring.md"
    - "docs/test-specs/autx-02-cli-adapter.md"
    - "docs/test-specs/autx-03-headless-cli.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-07-mount-wiring.md"
    - "docs/impl/ir/autx-08-headless-common-e2e-slice-same-wire-3-surface-contract-test-synvelis-267.md"
    - "evidence/sv267/acceptance-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/autx-07-mount-wiring.md"
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/266"
    - "https://github.com/scarecrow173/synvelis/issues/267"
  defers: []
  deferred-by: []
---

# AUTX-08 common E2E same-wire contract test spec (synvelis#267)

## Purpose

`crates/synvelis-workspace/tests/autx08_e2e.rs` が固定する AUTX-08
headless common E2E slice（synvelis#267 の Leg B）の契約を記録する。
GUI automation surface・local CLI session host・MCP adapter の 3
surface が、同一の wire semantics — capability/schema/project/
args/consistency/target/idempotency・task/grant 伝播・fail-closed
authorization・typed outcome/error — で同一 `Gateway` pipeline を
通ることを、独立 compose した実 gateway stack 上で証明する。

この test spec は「surface ごとに権限経路が分岐しない」という不変条件
を保護する。surface adapter が独自の policy・project・commit 正本を
持ち始めた場合、あるいは wire 次元の一部が adapter ごとに脱落した
場合に失われる保証であり、adapter 実装の再設計をまたいで残るべき
意図を記録する。headless slice が完了・受入された時点で、本
spec の Leg B 部分は常設 regression 契約として残り、実 OS/GUI/
provider 受入は #267 の evidence lane（`evidence/sv267/`）が別途
管理する。

## Feature

AUTX-08 common E2E — same-wire 3-surface contract（GUI catalog
lowering・local CLI claim attestation・MCP tool dispatch が同一
wire semantics で同一 GatewayOutcome/ErrorEnvelope に収束すること）

## Rules

- **Rule AUTX-E2E-SAME-01** 同一の論理操作（`synvelis.operation.
  prepare` による `synvelis.midi.note.add` on `source10`）を GUI
  catalog `Submit` lowering・CLI unattended claim・MCP
  `midi.edit.prepare` の 3 経路で実行したとき、wire の semantic
  field（capability/schema/project/args/consistency/target）は
  GUI↔CLI 間で直接一致し、gateway が産出する
  `GatewayOutcome`/`ErrorEnvelope` は 3 surface で一致する。
  MCP の wire は adapter 内部（`mcp_tools::wire_for`）で生成
  され外部から観測不能なため、MCP 側の等価は outcome payload
  （`structuredContent` == gateway `result`）と error 投影の
  一致で証明し、wire field 一致は `synvelis-gateway` の
  adapter contract test が担う。surface 固有の `request_id` と、
  agent surface の `task`/`grant` 参照・idempotency 公開範囲は
  adapter 境界の正当な差異として扱い、semantic 一致の対象外と
  する。
- **Rule AUTX-E2E-FAILCLOSED** 同一 wire が失敗を産むとき
  （serve 不能な `expected-revision` — fixture では未 commit の
  revision 99 が `SnapshotExpired` を返す —、task/grant を欠く
  agent caller）、3 surface は同一の typed error を返す。MCP
  境界では `ExternalError` 投影（diagnostic 除去）として同一
  code/category/retry/message が観測できる。human caller は
  delegated 参照なしで完了できる — human/agent の authority
  境界は surface をまたいで保存される。
- **Rule AUTX-E2E-IDEM** idempotency key を運ぶ surface（GUI
  `Submit`・CLI wire）では、同一 key の replay が再実行されず
  `DuplicateRequest` で最初の operation を指名する。per-instance
  ledger だが、同一 compose recipe の stack では指名される
  operation id も一致する。
- **Rule AUTX-E2E-NOAUTORITY** test は authority ロジックを複製
  しない。各 surface の lowering/attestation が産んだものを実
  `Gateway`/`HeadlessSessionHost`/`McpHost` に通し、観測可能な
  outcome のみを比較する。fixture descriptor・fake policy・
  scripted commit で契約を代替しない。

## Examples

- **Example AUTX-E2E-SAME-01-01** GUI: catalog `Submit` on
  `synvelis.operation.prepare@1.0`（args=`{operation: midi.note.add
  on source10}`、consistency=`expected-revision:1`）→ wire JSON →
  `Gateway::handle` human caller → `Completed{result}`。
  CLI: 同 args/consistency + `task:"t1"`/`grant:"g1"` の wire →
  `HeadlessSessionHost::handle_claim(Unattended)` → 同一
  `Completed{result}`。MCP: `midi.edit.prepare`（`base_revision:1`）
  → permit-bound client session → `structuredContent` が同一
  result payload。
- **Example AUTX-E2E-FAILCLOSED-01** serve 不能な
  `expected-revision:99`（`SnapshotExpired`）の同一 wire →
  3 surface とも typed error（MCP は `isError:true` +
  `ExternalError`）。task/grant なしの agent wire → CLI/MCP とも
  `Denied`、同一 wire の GUI human → `Completed`。
- **Example AUTX-E2E-IDEM-01** `idempotency_key:"autx08-idem"` 付き
  prepare を GUI/CLI で実行→同一 key・同一 payload・新規
  `request_id` の replay（production retry と同じ形）→ 両者
  `DuplicateRequest{original}` で同一 operation id を指名。

## Guarantees

- 3 surface が同一 wire semantics を保持する限り、本 test は
  PASS し続ける。adapter の framing・session binding・tool 語彙
  （`midi.edit.prepare` vs `synvelis.operation.prepare`）が変わって
  も、wire 次元が同一に lower される限り契約は成立する。
- いずれかの surface が独自の project/policy/commit 正本や wire
  次元の脱落を持ち込んだ場合、本 test は FAIL する — それが
  検出すべき退行である。
- GUI 側の lowering は `AutomationCatalog::lower`（authority-pushed
  catalog seam、synvelis#266 SV266-NR-05）を通る — GUI が gateway
  型を直接触らない境界を維持したまま wire 等価を証明する。

## Non-goals

- 実 Windows GUI-L2 / EXT-L3 受入（実 GPUI window・実 OS
  project open/lock・実 provider・実 CLI binary の E2E）。headless
  Pass を実機 Pass に昇格しない — POLICIES.md の層分離どおり、
  実 OS 証拠は `evidence/sv267/` の Required/Deferred/NotRun
  記録と #267 受入 lane が管理する。
- `synvelis-automation` live adapter からの catalog 実供給
  （#264）・workflow coordinator の live run（#265）・macro
  preset の live 投入 — 本 slice は engine 登録 capability と
  合成 catalog document のみを使う。live lane の `step_wire`
  は独立した wire producer であり、その等価は #264 接続後の
  別検証とする。
  > **Note (2026-11-16, Leg C)**: `synvelis#267` の Leg C
  > slice（`docs/specs/autx-08-live-adapter-leg.md`）が、
  > `step_wire` の wire byte ではなく公開 `StepLane` surface
  > （`invoke`/`operation_result`）の outcome/error/
  > `DuplicateRequest` parity を headless で固定した
  > （`autx08_live_adapter_*` 4 test、`evidence/sv267/
  > live-adapter-leg.md`）。live catalog 実供給・dispatch-hop
  > consumer・job lane の等価は引き続き #264/#265 接続後の
  > 別検証であり、本 Non-goal の対象外ではない。
- `synvelis-cli` の `build_request`/`claim_for` による wire 生成
  — `synvelis-workspace` は `synvelis-cli` に依存しないため、
  本 slice の CLI leg は `HeadlessSessionHost::handle_claim` +
  rig wire で host/attestation 経路を検証する。CLI binary 自体の
  envelope は `synvelis-cli/tests/cli_e2e.rs` が担い、
  `build_request` 出力と GUI lowering の field 一致は deferred
  cross-check とする。
- GUI `lower` → `Gateway::handle` の production dispatch hop —
  `syn.autx.action` は lowered `requests` を返すが、それを
  gateway lane へ渡す consumer は #264 の mount wiring で接続
  予定。本 slice は seam 契約どおりの合成で等価を固定する。
- commit/reconcile の実 OS durable 検証 — `CommitGate` は実
  journal/store 上で動くが、multi-process・実 filesystem fault・
  実 provider を含む commit E2E は別 lane。
- 全 capability・全 tool の総当たり — 本 spec は代表 mutation
  （prepare + note.add）と失敗経路の等価を固定し、網羅行列は
  gateway 自身の contract test 群（`synvelis-gateway/tests/`）が
  担う。
- MCP `proposal.apply` の idempotency 引数・control channel の
  human authority 経路 — tool 引数面の差異は adapter contract
  （GW-08）の責務であり、本 slice は prepare 系の wire 等価を
  固定する。

## Risk

- 本保証が落ちると、GUI/CLI/MCP が「同じ操作」を別の意味で
  実行し得る — surface ごとの暗黙の project 選択・policy 迂回・
  commit truth の誤認が検出不能になる。特に agent surface の
  task/grant 伝播が脱落すると、grant scope/effect/data-access
  検査を経ない実行経路が生まれる。
- headless 等価は実 OS での同値を保証しない。実 window・実
  credential store・実 transport でのみ現れる差異（GUI-L2/
  EXT-L3）は本 spec の対象外であり、#267 の evidence 分離を
  怠ると headless Pass が実機受入の代替として誤用される危険が
  ある。
