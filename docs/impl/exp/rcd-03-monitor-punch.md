---
id: "EXP-rcd03-monitor-punch-session"
type: "experiment"
status: "completed"
title: "RCD-03 monitor session fixture — MonitorProbe の plan 借用競合の解決"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/rcd-03-monitor-punch.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-monitor-punch-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/rcd-03-monitor-punch-task.md"
  defers: []
  deferred-by: []
---

# RCD-03 monitor session fixture — plan 借用競合の解決

## 仮説

`DeviceRig` が所有する `PreparedRecordingPlan` を `MonitorProbe`
へ `&plan` で渡せば、capture 側（`rig.drive`/`finish` が
`&mut self` 経路）と monitor 観測側（`probe.feed`）を同一
session で合成できる — ただし `MonitorProbe<'a>` の lifetime
借用が `DeviceRig` の可変借用と競合しないかは未検証。

## 方法

fixture 設計時に `DeviceRig::open` が plan を内部所有し、
`drive`/`finish` が `&mut self` を要求すること、
`MonitorProbe::new(&plan, ctx)` が plan への共有借用を保持
することを型レベルで確認した。

## 観測結果

- **競合を確認**: `rig` が plan を所有する限り `&rig.plan`
  の共有借用を `probe` が保持したまま `rig.drive(&mut)` を
  呼べず、E0502 系の borrow conflict になる。`DeviceRig`
  の plan フィールドは private で外部から切り離せない。
- **`PreparedRecordingPlan` は非 `Clone`**: plan を複製して
  別 instance を monitor 側へ渡す安易な経路は存在しない
  （gate schedule・pass map 等を含む所有権構造のため）。
- **解決を確認**: 同一 `RecordingSpec` から `preflight` を
  2 回呼び、capture 側（`DeviceRig` 内）と monitor 側
  （`MonitorProbe` が借用）の plan を別 instance として
  構築すれば、型・意味ともに競合なく合成できる。実系でも
  monitor session は録音側と同一の prepared plan を参照する
  構造であり、spec 由来の 2 instance は同一 gate schedule
  を持つ。
- **副次的発見**: `build_payload` は staging
  `capture_stream_id` が `Project.recording.streams` に登録
  済みであることを要求する。fixture は
  `CaptureStreamId::new(500)` を明示登録し
  `DeviceRig::open` の `stream_ids` と一致させることで
  commit 契約を満たす（自動採番 ID では project 未登録で
  `Manifest` error になることを観測）。

## 結論

plan の複製ではなく「同一 spec からの `preflight` 2 回呼出し」
を採用（adopted — `docs/impl/ir/rcd-03-monitor-punch.md`）。
`DeviceRig`/`MonitorProbe` の公開 API を変える案は、
headless fixture のためだけに production 境界を広げるため
棄却。gate 権威は `GateScheduler` に残り、2 plan は同一
schedule を共有するため観測の正当性は保たれる。
