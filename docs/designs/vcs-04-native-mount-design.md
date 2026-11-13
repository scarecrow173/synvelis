---
id: "DESIGN-VCS04NativeMount001"
type: "design"
status: "approved"
title: "VCS-04 History/Visual Diff — native mount/dock 配線 design"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "SPEC-VCS04NativeMount001"
    - "DESIGN-VCS04Hist0ryV1ewDsn01"
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-04-native-mount.md"
  implemented-by: []
  depends-on:
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-04-native-mount.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 History/Visual Diff — native mount/dock 配線 design

## Overview

`SPEC-VCS04NativeMount001` を実装する設計。headless slice（PR #252）
で完成した VCS History projection/op 面を、native mount/dock 配線へ
接続する。変更は `crates/synvelis-app`（screens/ui_ops/app/shell/
synvelis-native bin）と `tests/` に限定し、workspace の書き込み側・
project/library は触らない。

## 決定

### D1. dock-domain 解決は registry fallback で一般化する

現状 `dock_view_info` は静的表（`view-1`/`view-2`）のみを知る。
`syn.vcs.visual{open}` が mint する `view-{n}` は workspace
registry descriptor が kind/title を持つ。`ui_ops.rs` に
`dock_domain_view(id)` を追加し、静的表 → registry descriptor の
順で解決する。

- registry-adopted view の `DockViewInfo` は
  `control_targets: None`（VCS History は control surface を持たない）。
- `view-{n}` 形で静的表にも registry にも無い id は `Err` で
  fail closed。legacy「UI-only attach」path へ落とさない
  （半端状態の禁止）。

却下: 全 op に個別の registry 照会を散らす案 — dock metadata の
責務は 1 箇所に集約する（DRY、正しい境界）。

### D2. `syn.view.attach` は `dock_domain` を返し、shell が学習する

動的 view が dock-domain かどうかは attach 時点でしか確定しない
（attach 後に registry entry が消える経路は close 経由のみ）。
`ViewOp::Attach` result に `dock_domain: bool` を載せ、
`NativeShell` は `HashSet<String>`（dock-domain ids）を維持する。

- attach で `dock_domain=true` なら set に追加。
- detach/close/destroy は set を見て pool vs legacy close replay
  を決める。set からの除去は close 成功時のみ。
- これにより `WindowRequest` targets・`Detached` bookkeeping・
  `window_destroyed` の teardown が動的 view でも静的 view と
  同規則で動く。

却下: shell が registry を直接読んで都度判定する案 — shell は
workspace を持たず driver-neutral であるべき（層の逆転）。

### D3. `VcsScreenState` publication は post-op tail に置く

`main_screen_cell` と同型の hand-out cell
（`Rc<RefCell<BTreeMap<ViewInstanceId, VcsScreenState>>>`）を
`SynvelisApp` に追加し、`publish_project_length`（全 op の
post tail）から `publish_vcs_screens` を呼ぶ。

- 各 registered `ViewType::VcsHistory` を `vcs_history_view` で
  投影し直す。close 済み id は次回 publish で落ちる。
- project 変更（Live endpoint を stale にする commit 等）でも
  同じ経路で追従する — publish 点を 1 箇所に集約することで
  「op 後に古い投影が残る」経路を作らない。

### D4. mount は session-owned epoch で行う

`screens.rs` の `mount_main_view`/`mount_secondary_view` は
composition root の固定 id を使う。VCS view は採用済み
`ViewInstanceId` であり、fresh `AttachEpochAllocator::new()` を
mount 毎に作ると stale command namespace が host 間で衝突し得る。
`mount_screen_view` ヘルパーを抽出し、`Rc<RefCell<
AttachEpochAllocator>>` を共有する `mount_vcs_history_view` を
追加。呼び出し側が epoch を渡せない形にして、allocator 経由を
強制する（RF387 fresh-host 契約）。

### D5. native bin は env-gated で open→publish→mount する

`synvelis-native` に `SYNVELIS_VCS_HISTORY`（truthy）gate を追加。
有効時は起動時に `syn.vcs.visual{open}` を実 op 面経由で発行し、
`vcs_screen_cell` の state を `mount_vcs_history_view` で mount、
`create_view_window` に `ViewMount{host,theme:None}` を渡す。
run loop は main screen と同じく publish map の差分で
`update_view` する（`PendingMount` を spec-kind enum 化し
main/secondary/vcs を区別）。

### D6. gpui-test scenario は `GpuiGuiTest` + `SemanticGuiTest`

`scenario_gpui.rs` の composition pattern に倣い、
`ViewHost::new(view, allocator.mint())` → `host.mount(spec)` →
`driver.create_view_window(WindowSpec::default(), ViewMount{..})`
→ `gui.settle()`。`semantic()` port で component resolve /
select / invoke 拒否（read-only）/ detach / 2 window remount を
検証する。`Unsupported`（実 OS GUI/IME/pixel/plugin dock）は
`ScenarioVerdict::Unsupported` として記録し Pass にしない。

## ファイルマップ

- `crates/synvelis-app/src/screens.rs` — `VcsScreenState`、
  `history_screen_spec` の title String 化、`mount_screen_view`
  抽出、`mount_vcs_history_view` 追加。
- `crates/synvelis-app/src/ui_ops.rs` — `dock_domain_view`、
  `dock_view_info` の registry fallback、`ViewOp::Attach` result
  `dock_domain`、`syn.vcs.visual{open,close}` の ID mint/解放。
- `crates/synvelis-app/src/app.rs` — `vcs_screen_cell`、
  `publish_vcs_screens`、post-op tail 接続。
- `crates/synvelis-app/src/shell.rs` — dock-domain id set、
  detach/close/destroy bookkeeping の分岐。
- `crates/synvelis-app/src/bin/synvelis-native.rs` —
  `SYNVELIS_VCS_HISTORY` gate、VCS open/mount/update。
- `crates/synvelis-app/src/lib.rs` — export 調整。
- `crates/synvelis-app/tests/common/{mod,vcs}.rs` — 共有 fake
  provider/backend fixture（headless + gpui 両 lane で再利用）。
- `crates/synvelis-app/tests/vcs04_history.rs` — fixture 共有化
  （9 test 維持）。
- `crates/synvelis-app/tests/vcs04_native_mount.rs` — 新規 7 test。
- `crates/synvelis-app/tests/vcs04_mount_gpui.rs` — 新規 3 test。

## 検証境界

- `mise run check/lint/test synvelis-app`（gpui-test feature 含む
  全 lane）・`check-app-native`/`lint-app-native`。
- `mise run check/lint/test synvelis-workspace`（接続調整がある
  場合のみ — 本 slice では原則変更なし）。
- 実 OS GUI/2Window/IME/pixel/plugin dock は `Unsupported`/
  `NotRun`（#114）、VCS Domain Oracle 同一 Case 判定は #97。
