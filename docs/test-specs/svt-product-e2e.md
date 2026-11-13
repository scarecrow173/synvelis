---
id: "TSTS-SvtE2e05aB2cD3eF4gH5iK0"
type: "test-spec"
status: "approved"
title: "SV-GUI-05: SVT-01〜09 Headless UI Scenario テスト仕様"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/114"
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
    - "docs/specs/svt-product-e2e.md"
    - "docs/designs/svt-product-e2e-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/svt-product-e2e-task.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/svt-product-e2e.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-05 テスト仕様: SVT-01〜09 Headless UI Scenario

## Feature: SVT product State/Receipt (OS-L1 headless)

### Rule SVT-01: Project Create keeps view composition

` syn.project.new` 後も mount 済み view の attach/subscription が
生存し、gesture が新 session の transport truth に `Applied`
receipt を返す。`syn.view.list` が view を報告し続け、
`arrangement_rows` は新 project で空。

### Rule SVT-02: Selection drives Inspector focus

view-local `select` → `syn.view.select` → `syn.view.list` の
`selection`/`inspection` と Inspector screen state が clip tag に
一致。範囲外 row は `ok=false` で authoritative state 不変。

### Rule SVT-03: Fader gesture receipt→projection→display

`Applied` receipt と `Observed` projection が分離して記録され、
committed/observed value が display に残る。

### Rule SVT-04: Undo/Redo + rejected truth

`set_ppqn` の undoable edit が `project.undo`/`redo` Button で
`undone`/`redone` を返す。空 history は `false`、unknown op は
error Result で project truth 不変。

### Rule SVT-05: Transport Requested/Applied/Observed

dispatch 後・delivery 前は `Observed` でなく、delivery 後は
`Applied`/`Observed`。`syn.control.receipt` broadcast が
`target=transport`・`status=applied` を含む。

### Rule SVT-06: Viz bounded query / stale / honest empty

`syn.viz.query` は `max_bytes` を超過すると `ok=false`。
`syn.viz.source_changed` 後は pump が resync し `status=ok`・
`view_stale=false`（transient `view_stale=true` は
`synvelis-workspace/tests/sv48_viz.rs` が `VizUi` 層で保証）。
未 bind surface の meta は error。

### Rule SVT-07: Two-window ownership / close / project lifetime

2 window が同一 project session を共有し、window `Destroyed` で
secondary view の ownership が外れても project truth と main view
は不変。生存 window の gesture は `Applied` を維持。
実 OS two-window/focus/dock・Job op は `NotRun`/`Unsupported`。

### Rule SVT-08: Save/Reload independent oracle

`HEAD` → `snapshots/r<N>.json` を app の load path を介さず直接
parse し、name/track/clip 数と revision が reload 後の
`syn.project.info`・`arrangement_rows` と一致。

### Rule SVT-09: Detach/reattach stale namespace

`syn.view.detach` 後の minted command は `Rejected` receipt で
apply されず、`syn.view.attach` 後は新 generation で `Applied`
に復帰する。

## Feature: SVT view-layer (GPUI Test seam)

`svt_product_e2e_gpui.rs` が `GpuiGuiTest` で SVT-02/03/05/07
相当の driver-neutral scenario を実行し、`ScenarioReport.driver =
GpuiSimulated`・能力申告が正直であることを検証する。semantic
pass は実機・pixel pass に昇格しない。
