---
id: "EXP-VCS04NativeMount001"
title: "VCS-04 native mount/dock 配線 実装メモ (synvelis#95)"
created: "2026-11-14"
updated: "2026-11-14"
---

# VCS-04 native mount/dock 配線 — experiment log

## E1. dock-domain 解決の一般化

静的 `dock_view_info` は `view-1`/`view-2` のみを知るため、
`syn.vcs.visual{open}` が mint する `view-{n}` は attach 時に
解決不能だった。registry descriptor（kind/title を保持）を
fallback として `dock_domain_view` を導入。`view-{n}` 形で
静的表にも registry にも無い id は fail closed —
「UI attached だが dock 未登録」の半端状態を防ぐ。
`syn.view.attach` result に `dock_domain: bool` を載せ、
shell は `HashSet<String>` で動的 dock-domain id を追跡し、
detach（pool）/close（除去）/destroy（pool）を静的 view と
同規則で処理する。shell が workspace registry を直接読まない
設計にしたのは、shell が driver-neutral で workspace 非依存の
ままにするため。

## E2. publish 点の集約

`main_screen_cell` と同型の hand-out cell
（`Rc<RefCell<BTreeMap<ViewInstanceId, VcsScreenState>>>`）を
`SynvelisApp` に追加し、`publish_project_length`（全 op の
post-op tail）から `publish_vcs_screens` を呼ぶ。これにより
VCS op 後と project 変更（Live endpoint を stale にする commit
等）の双方で mounted spec が追従し、「op 後に古い投影が残る」
経路を作らない。publish は登録済み view のみ再投影し、close
済み entry を落とす。

## E3. epoch の強制

`mount_main_view`/`mount_secondary_view` は固定 root id を使う
ため、採用済み `ViewInstanceId` の VCS view には使えない。
`mount_screen_view` ヘルパーを抽出し、`Rc<RefCell<
AttachEpochAllocator>>` を共有する `mount_vcs_history_view` を
追加。呼出側が epoch を渡せない形にすることで、fresh mint を
強制し stale command namespace の衝突を防ぐ（RF387 fresh-host
契約）。

## E4. gpui-test composition

`scenario_gpui.rs` の pattern に倣い、
`ViewHost::new(view, allocator.mint())` → `host.mount(spec)` →
`driver.create_view_window(WindowSpec::default(),
ViewMount{host,theme:None})` → `gui.settle()`。semantic port で
component resolve・select/invoke 拒否（read-only）・detach・
2 window remount を検証。`SYNVELIS_VCS_HISTORY` gate は native
bin の起動時 env で、test 側は `set_vcs_backend` 注入 +
`syn.vcs.visual{open}` op 経由で同等経路を辿る。

## E5. つまずき

- `tests/common/vcs.rs` 抽出時に `scripted_nodes` で 'c' node を
  重複させた — fixture グラフ（d→{c,e}, c→b, b→a, a→∅）を
  修正。
- `vcs04_native_mount.rs` の helper `action` が glob import した
  fixture の `action` と衝突 — `send_intent` に改名。
- `dispatch_op` の `.last()` が clippy `double_ended_iterator_last`
  に抵触 — `.next_back()` に修正。
- `gpui-test` lane は default features では compile されない
  （`vcs04_mount_gpui` は feature gate）。canonical 検証には
  `check-app-gpui`/`lint-app-gpui`/`test-app-gpui` が必要。
