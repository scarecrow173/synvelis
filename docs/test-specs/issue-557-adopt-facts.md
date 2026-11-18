---
id: "TSPEC-557AdoptFacts06"
type: "test-spec"
status: "approved"
title: "Issue #557 test-spec — adopt declared_facts 監査の検証仕様"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/557"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-557-adopt-facts.md"
    - "docs/designs/issue-557-adopt-facts.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-557-adopt-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/tasks/issue-557-adopt-facts.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-557-adopt-facts.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/issue-557-adopt-facts.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #557 test-spec — adopt declared_facts 監査の検証仕様

## Feature

variant adopt 経路の `declared_facts` 伝播監査
（synvelis#557）の検証。

## Rules

### Rule 1 — adopt 受理 pin（新規 test）

`cb06_strip_557_adopt_send_remove_candidate_
materializes_teardown`（`crates/synvelis-gateway/
tests/cb06_strip_gateway.rs`）が以下を保証する:

- fork → candidate `send.remove` edit（modulation
  entities を持たない send — post-sv#553 で合法な
  唯一の wire 経路）が candidate 側で commit され、
  live は adopt まで send を保持する。
- adopt（agent + human approval の cb10 規約経路）が
  `GatewayOutcome::Accepted` で commit され、
  provenance が `branch == candidate`・
  `outcome == "applied"`・`objects == 3` を記録する。
- adopt 後の live から send config・`Send` kind の
  `RoutingConnection` edge・send-owned `SendLevel`
  `ParameterTarget` が除去され、variant set の
  adopted branch が candidate に更新され旧 adopted
  branch が freeze される。

意図: adopt が「candidate の scope diff を実 diff
経由で物質化し post-simulation `derive_effects` +
`authorize_effects` の認可を通る」契約を固定する。
`objects == 3` は `plan_adopt_writes` の `Removed`
write 計画数を pin する。

### Rule 2 — Edit 経路非回帰（既存 pin の再実測）

以下の既存 test が `mise run test synvelis-gateway`
で引き続き Pass する:

- `cb06_strip_553_variant_edit_send_remove_
  unobservable_cascade_denied`（synvelis#553 —
  candidate edit 時点の unverifiable cascade deny）
- `cb06_strip_550_send_remove_unobservable_cascade_
  denied`（top-level prepare の同一 deny）
- `cb06_strip_gateway` の他全 test（9 件 — 新規 1 件
  を含む）

### Rule 3 — 監査根拠の記録（文書・evidence）

以下が repo 内に存在し、AC1 の「gap が無いことの
根拠」を構成する:

- `docs/discovery/issue-557-adopt-facts.md` §1–4
- `docs/impl/ir/issue-557-adopt-facts.md` 監査節
- `evidence/sv557/`（audit + verification 記録）
- `synvelis#559`（残存経路の hardening Issue —
  stored candidate payload の観測不能削除物質化）

## Examples

- positive: Rule 1 の test が Pass — adopt が
  teardown を物質化し provenance `objects == 3`。
- negative（deny 側・条件付き AC2）: gap が実証
  された場合のみ、不可視削除入り candidate の adopt
  が `PreparationRejected` で拒絶される test を
  追加する。本監査では gap 無しのため不実施
  （残存経路は `synvelis#559`）。

## 非対象

- 実 PCM/実 plugin/実 GUI lane（POLICIES.md §5 —
  headless のみ）。
- `crates/synvelis-project/**` の test 追加 —
  domain 側の adopt fail-closed は既存 test
  （`cb_take_03_recording_active_adopt_rejects_typed`
  等）で実測済み。
- stored pre-553 payload の再現 test — 現行 wire で
  生成不能かつ payload 捏造が必要なため
  `synvelis#559` に委譲。

## 関連リンク

- `synvelis#557`（本 Issue）/ `synvelis#553` /
  `synvelis#550` / `synvelis#559` / `synvelis#283`
- `docs/specs/issue-557-adopt-facts.md`
- `docs/designs/issue-557-adopt-facts.md`
- `docs/test-specs/cb-06-track-bus-adapter.md`
- `docs/tasks/issue-557-adopt-facts.md`
- `docs/impl/ir/issue-557-adopt-facts.md`
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
- `POLICIES.md`
