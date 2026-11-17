---
id: "ADR-sv517jK2lM3nO4pQ5rS6tU7"
type: "adr"
status: "accepted"
title: "Issue #517 — render/branch pipeline 本番配線の方式決定"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/designs/issue-517-render-branch-wiring.md"
    - "docs/specs/issue-517-render-branch-wiring.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/variant-sets-vcs-restore.md"
    - "docs/adr/rcp-00-render-recipe.md"
    - "docs/adr/l2-render-slice-boundaries.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-517-render-branch-wiring.md"
  derived-by:
    - "docs/impl/ir/issue-517-render-branch-wiring.md"
  verifies: []
  verified-by: []
  references:
    - "docs/designs/variant-sets-vcs-restore.md"
  defers: []
  deferred-by: []
---

# Issue #517 ADR — render/branch pipeline 本番配線

## Context

`VariantPorts::standard()` が in-memory store + Noop runtime で、
candidate payload は process restart で失われ、runtime publish は
常に Noop。`synvelis.render.*` は gateway に未登録。VCS-RESTORE-10
は payload bytes を fork しないため復元 project の candidate 操作が
`CandidateUnavailable` になりうる。

## Decision

### ADR-1: file-backed DirectoryBranchContentStore

`<root>/<sha256-hex>` の content-addressed file store を
`synvelis-project/src/variant/store.rs` に実装し、
`VariantPorts::standard(Some(root))` で `root/branch-content` に
配線。StagedFile + FileAndDirectorySync + CreateNew で torn write
を排除し、既存 file は readback digest 検証で fail-closed。
GC なし（orphan は無害、CAS 参照のみ authoritative）。

却下: bundle store への payload 埋め込み — `BranchContentRef` の
referenced-not-embedded 設計に反する。

### ADR-2: JournaledRuntimePublish — typed Pending のみ

request を digest 名の JSON record として durable journal し、
常に typed `Pending` を返す。`Published` は返さない —
`synvelis-project` 内に activation を確認できる runtime がなく、
project commit と runtime activation は別証跡（RCP-BR-04
Last-good/Pending）だから。journal 書込失敗のみ typed `Failed`。
engine/session 到達は sv#514 の領域で、本 slice は project 側の
正直な journal まで。

### ADR-3: synvelis.render.* — host 注入 RenderDispatch port

gateway→synvelis-render 依存を避け、`VariantPorts` 式の
`RenderDispatch` port を `GateDeps.render` に注入。9 capability
は `synvelis.branch.*` と同じ三層（descriptor/prepare_engine/
commit gate）に登録し、Target::None で project state を変更
しない。dispatch は CommitGate fence 内で revision fence 付き、
Dispatched/Pending は committed provenance、Unavailable/Failed/
port 不在は determined rejection — success を捏造しない。
CLI/MCP は `UnavailableRenderDispatch` を注入（headless の正直
な wiring）。

### ADR-4: VCS-RESTORE-10 — fork-copy + honest unavailable

`RestorePorts.branch_content` read port（`BranchContentStore`
blanket impl）を追加し、snapshot の全 `Captured` digest を
source→staging `branch-content/<hex>` へ digest 検証付きで複製。
port 不在・confirmed-absent は receipt `unavailable` に記録
（`CandidateUnavailable` verdict）。transport 失敗・digest
mismatch は restore 失敗 — absent への矯正禁止。

## Consequences

- `VariantPorts::standard()` のシグネチャ変更に伴い全呼出点を更新
  （durable project root を渡す本番経路、`None` の discovery/
  fixture 経路）。
- `GateDeps` に `render` field 追加 — 全 literal 更新。
- restore receipt に `branch_content` report 追加
  （`#[serde(default)]` で後方互換）。
- 実 render backend 接続・UI/Policy surface・実 E2E/RT 計測は
  後続 Issue（sv#295/sv#518/sv#267/sv#288/sv#296/sv#114）。
