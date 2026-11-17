---
id: "SPEC-sv517cD4eF6gH8iJ0kL1mN3"
type: "spec"
status: "approved"
title: "Issue #517 spec — render/branch pipeline 本番配線の受入仕様"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/517"
    - "docs/discovery/issue-517-render-branch-wiring.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/adr/issue-517-render-branch-wiring.md"
    - "docs/designs/issue-517-render-branch-wiring.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-517-render-branch-wiring.md"
  derived-by:
    - "docs/adr/issue-517-render-branch-wiring.md"
    - "docs/designs/issue-517-render-branch-wiring.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-517-render-branch-wiring.md"
  references:
    - "evidence/sv517/"
  defers: []
  deferred-by: []
---

# Issue #517 spec — render/branch pipeline 本番配線

## 目的

Issue #517 の 4 slice（永続 BranchContentStore・本番
RuntimePublish・`synvelis.render.*` gateway 登録・VCS-RESTORE-10
fork-copy）を本番配線として実装し、既存の認可・
prepare→proposal→approval→commit・CAS・provenance・last-good・
fail-closed・VCS restore 契約を保存する。

## 受入条件

- AC-1: file/store-backed 永続 `BranchContentStore` が
  `VariantPorts::standard()` に配線され、process restart 後も
  candidate payload が参照できることがテストで pin される。
- AC-2: 本番 `RuntimePublish` が配線され、publish 失敗が
  Last-good/Pending として型付きで返り成功と誤表示されないことが
  テストで pin される（NoopRuntimePublish のみ残さない）。
- AC-3: `synvelis.render.*` が prepare→proposal→approval→apply→
  CommitGate pipeline に capability 登録され、`synvelis.branch.*`
  と同一 Target/Grant・冪等・fail-closed parity がテストで検証
  される。
- AC-4: VCS-RESTORE-10 で restore-as-copy が payload を fork 先
  store へ複製するか `CandidateUnavailable` を正直 verdict とする
  契約が確定し、ledger 行が更新される。
- AC-5: `mise run test synvelis-gateway` / `synvelis-project` /
  `synvelis-workspace`（変更 crate の正規検証）が pass し、
  cb10_variant_actions・vcs 系・rcp 系既存 suite に回帰がない。
- AC-6: headless/fixture lane を Pass 判定とし、実
  CLI/MCP/GUI/render surface E2E・実 RT 計測は別 gate
  （#267/#288/#296/#114）として NotRun 記録する。

## 非目的

- `synvelis.render.*` の UI/Policy surface 実装（sv#295）。
- 実 render pipeline 実走・実 RT 計測（sv#267/#288/#296/#114）。
- VCS Writer・3-way merge（sv#231 系）。
- SessionRoutingEngine への RCP selection 配線本体（sv#514 —
  `crates/synvelis-app/` は読み取りのみで契約照合）。
- `crates/synvelis-render/` 側の実装変更（sv#518 領域）。
- ruviform/audio-lens リポジトリの変更。
- docs index（`docs/**/README.md`）の編集・再生成。

## 制約

- `synvelis-gateway` → `synvelis-render` 依存は追加しない —
  render 実行は `VariantPorts` 式 host 注入 port
  （`RenderDispatch`）で閉じる。
- render ops は project state を変更しない（Target::None、
  candidate = pinned base 不変）。
- render dispatch 不在・Unavailable/Failed は型付きの determined
  rejection — render success を捏造しない。
- `Refs #517` のみ使用（`Closes`/`Fixes`/`Resolves` 禁止）。
