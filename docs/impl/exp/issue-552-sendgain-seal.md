---
id: "IMPLOEXP-552SendGainSeal09"
type: "impl-exp"
status: "approved"
title: "Issue #552 impl-exp — SetSendGainTarget seal 解放の検証証拠"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/552"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-552-sendgain-seal.md"
    - "docs/test-specs/issue-552-sendgain-seal.md"
    - "docs/impl/ir/issue-552-sendgain-seal.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-552-sendgain-seal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv552/README.md"
    - "docs/test-specs/issue-552-sendgain-seal.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-552-sendgain-seal.md"
    - "docs/test-specs/issue-552-sendgain-seal.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #552 impl-exp — SetSendGainTarget seal 解放の検証証拠

## 実行検証（headless）

実施環境: Windows（worktree `sv552-sendgain-seal`）、
mise 管理 toolchain、`mise run` 経由の canonical
検証。

- `mise run check synvelis-project` — exit 0。
- `mise run lint synvelis-project` — exit 0。
- `mise run test synvelis-project` — exit 0。
  ctl_02 の rebind 3 試験と cb06_strip_fields の
  candidate positive を含む全 test binary が Pass。
- `mise run fmt-check` — exit 0。
- `mise exec -- prek run --all-files` — exit 0。

## AC 対応

- Some→Some rebind が旧 SendLevel seal を解放して
  受理される → `send_gain_rebind_releases_the_previous_seal`
  + `send_gain_rebind_via_routing_intent_releases_the_seal`
  + `cb_strip_550_sends_ops_mask_gated` で実測。
- rebind 中の半端な seal 状態が発生しない → apply が
  scratch clone 上で不可分（release → seal → post-apply
  oracle）に実行され、undo bit-for-bit 復元を
  `send_gain_rebind_releases_the_previous_seal` で実測。
- 既存 typed `Unsupported` が positive 経路に置き換わる
  → `variant/adapter.rs` の arm から除去し、
  `cb_strip_550_sends_ops_mask_gated` が positive
  candidate edit を検証。

## NotRun / Unsupported

- 実 PCM/plugin・実 GUI・RT lane: 本 slice は domain
  semantics の headless 修正 — 対象外（NotRun）。
- gateway wire positive 実測の追加
  （`cb06_strip_gateway.rs`）: `synvelis#553` 占有 —
  本 slice では未実施（NotRun — wire lowering は
  既存で domain 修正により自動的に通る）。
- send 以外の seal/rebind 系 op の横断見直し:
  Issue 対象外（別 Issue）。
