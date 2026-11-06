---
id: "ADR-034RUwki0c6svA0PlhUBB8"
type: "adr"
status: "accepted"
title: "Issue #157 rev.3 skip placeholder 受理方式 — gate 名照合拡張 vs workflow 構造変更"
created: "2026-11-05"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "https://github.com/scarecrow173/synvelis/pull/161"
    - "https://github.com/scarecrow173/synvelis/actions/runs/41894049161"
  changes:
    added: []
    modified:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-157-selective-execution.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
  derived-by:
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "https://github.com/scarecrow173/synvelis/blob/3f4f4450131b0258a1919b213de3ad394fbbb709/tools/ci-impact.mjs"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# Issue #157 rev.3 skip placeholder 受理方式 — gate 名照合拡張 vs workflow 構造変更

## Context and Problem Statement

matrix lane の job-level `if:` skip は、GitHub が leg を展開せず
`<lane> (${{ matrix.os }})` placeholder job を 1 件記録するため、
evidence gate の unclassified 名照合と `expected` 件数の両方を壊す
(run 41894049161 で全 required leg 緑にも関わらず evidence fail)。
placeholder をどこで・どう正当化するかを決める必要がある。

## Considered Options

* **gate 側の名照合拡張 + `expected-jobs` 正確化(採用)**: placeholder
  名を prefix+`scheduled` 判定で受理/拒否し、収集カウントを impact
  report から正確に算出する。判定・カウントの authoritative 実装を
  `tools/ci-impact.mjs` 内に閉じる。
* **placeholder を生成しない workflow 構造変更**: lane `if:` をやめ
  leg/step 単位の skip に変え、matrix 収縮そのものを起こさない。
* **placeholder は受理せず `expected` 件数のみ緩和**: 名照合は現行の
  まま、retry カウントだけ skip を見込む。

## Decision Outcome

Chosen option: "gate 側の名照合拡張 + `expected-jobs` 正確化", because
placeholder は GitHub jobs API の正常な入力形態であり、正当性判定
(scheduled 裏付け)と件数計算の両方が impact report に一本化できる。
workflow 構造変更は lane skip の runner time 削減効果を失い required
context 名を再び揺らす。件数のみの緩和は placeholder を依然 violation
として fail させるため欠陥が残り、受理側を緩めるだけでは leg 欠落との
区別が付かず fail-closed が弱まる。

## Implementation Plan

* Affected paths: `tools/ci-impact.mjs`、`tools/ci-impact.test.mjs`、
  `.github/workflows/ci.yml`(evidence job の `expected` 式のみ)。
* Constraints: fail-closed 維持 — 新たに violation を免れる入力は
  「report が scheduled=false と裏付ける lane の placeholder」のみ。
  `scheduled` フラグを placeholder 正当性の正本とし、report 欠落は
  violation(over-expect 側に倒す)。

## Verification

* [x] `node --test tools/ci-impact.test.mjs` — placeholder 受理/拒否/
      unclassified・`expected-jobs` 13/14/11・run 41894049161 fixture
      再生の各ケース。
* [x] `mise run lint-actions` — workflow 変更の構文検証。
