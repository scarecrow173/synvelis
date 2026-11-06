---
id: "TESTSPEC-SVGUI02"
type: "test-spec"
status: "approved"
title: "SV-GUI-02 契約検証仕様"
created: "2026-11-04"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
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
    - "docs/designs/sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md"
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
    - "docs/plans/sv-gui-02-view-model-binding-ipc.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-02 契約検証仕様

## Feature: syn.* UI op 面 (headless)

- Rule: `syn.view.attach` は ControlsUi/VizUi 双方へ attach し、generation
  を返す。detach は in-flight gesture の contractual cancel のみ行い、
  project/audio/render はキャンセルしない。
- Rule: `syn.control.dispatch` は受理 ack のみ返し、receipt は
  outbox `syn.control.receipt` notify で届く。
- Example: attach→dispatch(begin/preview/commit)→pump→outbox drain で
  accepted→applied の receipt 列と `syn.control.projection` が観測
  できる。2 view 同 target では両 view へ projection が届く。
- Example: 未存在 target は `unsupported`/`rejected` receipt。read-only
  target は read_only=true の projection で偽装しない。
- Example: `syn.viz.query` は maxBytes/buckets を強制し、超過は
  `over-budget`。stale source revision は stale status。

## Feature: frontend binding (vitest + happy-dom)

- Rule: BindingPort 上の CommandBinding + bindControlElement で
  rvf-knob/rvf-fader の gesture→intent→receipt→projection→element
  反映が upstream 契約どおり動く(fork なし)。
- Rule: delayed/reordered/duplicate receipt は確定を偽装せず、
  timeout/unsupported は該当 phase へ落ちる。
- Example: mount/unmount/remount/dock-transfer で subscription と
  listener が二重化せず detach で cleanup される(計数検証)。
- Example: view reload で `syn.view.attach` が再発行され
  subscription が rehydrate される。
- Rule: remount/reattach 後の `syn.control.dispatch` の
  `ctx.generation` は attach reply の backend authoritative epoch
  と一致する。backend live epoch と不一致の intent は receipt
  なく drop される(backend stale guard 契約)。fake host による
  echo 検証ではなく独立 epoch ledger で照合する。
- Example: 同 viewId の fresh-VM remount(attach reply=2)で
  pre-attach/後発 binding の双方が backend epoch を wire に刻み、
  pending attach 破棄・逆順旧応答でも epoch が巻き戻らない。

## Feature: 共有 fixture (補助証拠)

- Rule: `ui/guitest` の scenario fixture を BindingPort 実装へ replay
  し、記録済み応答ではなく独立 `ContractModel` oracle の期待値で
  Pass/Fail を判定する。

## 未検証として明示するもの

- 実 OS WebView GUI(macOS/Linux を含む)、実 plugin/実 DSP、実 multi
  window — RF-GT-01 実ブラウザ lane は環境依存のため実行可否を報告。
