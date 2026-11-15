---
id: "SPEC-034XHD7VlFH8w8tiP7ebRb"
type: "spec"
status: "approved"
title: "PC-C4 webview composition — synvelis.rs への routing engine attach"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-drain-composition.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-webview-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-webview-composition.md"
  references:
    - "docs/designs/pc-c4-drain-composition-design.md"
    - "docs/test-specs/pc-c4-drain-composition.md"
  defers:
    - "実 plugin factory（ruviform#253 gate）・実 GUI（synvelis#114）・wire-schema L1 結合検証（synvelis#101 残件・後続 slice）"
    - "synvelis-native bin の共有 helper への呼出側移行（native 経路の回帰リスクを避けるため本 slice 対象外）"
    - "project.new/open 後の engine 再 attach（synvelis#106 の責務）"
  deferred-by: []
---

# PC-C4 webview composition — synvelis.rs への routing engine attach

## 背景

synvelis#101（長寿 tracker）の PC-C4 drain slice
（`docs/specs/pc-c4-drain-composition.md`）は production
composition root（`compose_routing_engine`）を定義し、
`synvelis-native` bin が `SYNVELIS_PROJECT_DIR` gate 付きで呼ぶ
ところまでを実装した。同 spec の非目標は「`synvelis`（webview）
bin への composition — 同様に後続」と明示しており、本 spec は
その deferred 項目を引き取る。

現状 `crates/synvelis-app/src/bin/synvelis.rs`（webview product
bin、`webview` feature）は `compose_routing_engine`/
`attach_routing_engine` を一度も呼ばないため engine stub のまま
起動する。`syn.control.*` op surface（`service.rs` で無条件登録
済み、sv#108 が構築中の UI が consume する）は `engine:false`・
`generation`/`surface`/`frame_base` が null の stub 応答を返し、
製品の webview 経路では control/drain/observe の実経路が到達
不能である。

Issue は本 slice merge 後も Open 継続（実 plugin factory・実
GUI・wire-schema L1・consumer 照合が残件）。PR は `Refs #101`。

## 目的

1. webview product bin が native bin と同じ条件
   （`SYNVELIS_PROJECT_DIR` 存在時・empty plugin registry・
   `<dir>/media` media root・48 kHz/stereo/256 Realtime geometry）
   で routing engine を attach し、`syn.control.*` op が非 stub
   の typed evidence を返すようにする。
2. 共有 helper の切出しは `composition.rs` への最小変更に留め、
   `synvelis-native.rs` のコード経路は変更しない（webview 側
   のみ新 helper を利用 — native 側の回帰リスクを避ける）。
3. gate・attach・op 応答の非 stub 化を headless fixture
   （`tests/pc_c4_webview_composition.rs`）で pin する。

## スコープ

- `crates/synvelis-app`: `composition.rs` への product routing
  helper 追加、`lib.rs` re-export、`bin/synvelis.rs` への呼出
  配線、fixture テスト。
- stale status 簿記: merge 済み slice の task 文書 2 件
  （`pc-c4-02-task1-plugin-instance-binding`、
  `pc-c4-plugin-binding-task`）と IR
  `pc-c4-observe-engine-wiring` の status を慣例どおり更新。
- doc-chain: `docs/{specs,designs,plans,tasks,test-specs,
  impl/ir,impl/exp}/pc-c4-webview-composition.md`。
- `evidence/sv101/` への検証記録。

## 非目標（本 slice の範囲外）

- `synvelis-native.rs` の変更・共有 helper への呼出側移行
  （read-only 参照のみ — webview 側だけが新 helper を使う最小
  構成。native 側の private `compose_product_routing`/
  `product_engine_config` は残り、統合移行は後続 slice）。
- 実 plugin factory（ruviform#253 gate）・実 VST3/AU/CLAP・
  OS backend・実 GUI（synvelis#114）・wire-schema L1 結合検証・
  AudioLens/Ruviform consumer 照合 — synvelis#101 残件。
- `project.new`/`project.open` 後の engine 再 attach
  （synvelis#106 の責務 — recreate は従来どおり detach）。
- `crates/synvelis-recording/`（sv#115）・`synvelis-project/`
  （sv#457）・`synvelis-player/`（sv#127）・`ui/`（sv#108）の
  変更 — 他 Worker のリース。
- `service.rs`・`Cargo.toml`・`mise.toml`・`.github/`・
  `POLICIES.md`・`AGENTS.md` の変更。

## 要件

- R1: `synvelis.rs` が `SYNVELIS_PROJECT_DIR` を process boundary
  で読み、`compose_product_routing` 相当を native と同じ条件で
  実行する。未設定時は engine 未 attach（意図された degrade）
  のままにする。
- R2: 共有 helper は `composition.rs` に置き、env 読み取りは
  library に持ち込まない（既存の「library never touches
  `std::env`」契約を維持）。
- R3: plugin registry は空のまま — 実 factory は ruviform#253
  gate であり、plugin-bound processor は typed
  `PluginUnavailable` で fail し、捏造しない。
- R4: fixture が gate 閉（`None` → stub）・gate 開
  （`Some(dir)` → attach → publish → 非 stub 応答）・
  recreate detach を検証する。

## 受入条件

- [ ] `synvelis.rs`（webview bin）で `SYNVELIS_PROJECT_DIR`
  存在時に routing engine が attach され、`syn.control.*` op が
  `engine:true`・publish 後の generation/surface/drain の実
  evidence を返す。
- [ ] `SYNVELIS_PROJECT_DIR` 未設定時は engine stub
  （`engine:false`/null evidence）のまま — 意図された degrade
  であり fail ではない。
- [ ] `composition.rs` の変更は shared helper の追加に留まり、
  `synvelis-native.rs` の呼出経路は無変更。
- [ ] 新規 `tests/pc_c4_webview_composition.rs` が gate・attach・
  非 stub 応答・recreate detach を fixture 検証する。
- [ ] task 文書 2 件の stale status を merge 済み事実に更新し、
  IR `pc-c4-observe-engine-wiring` の status を慣例確認のうえ
  更新する。
- [ ] canonical 検証 lane（`fmt-check`/`check`/`lint`/`test`
  synvelis-app・`check-app-webview`/`lint-app-webview`/
  `test-app-webview`/`prek run --all-files`）が exit 0。
- [ ] 実 plugin factory・実 GUI・wire-schema L1 等の外部 gate は
  NotRun/Deferred として PR 本文と evidence に正直記載し、
  `Refs #101`（`Closes` 禁止 — tracker は auto-close しない）。

## Deferred Design Concerns

- 実 plugin factory registry — ruviform#253（実 OS/plugin
  backend gate）解消後の後続 slice。
- `synvelis-native.rs` の共有 helper 移行 — native 経路の
  回帰リスクを避けるため本 slice では webview 側のみ新 helper
  を利用。重複する private helper の統合は後続。
- project 切替時の engine 再 attach — synvelis#106 の責務。
- 実 GUI/実 Window での webview boot 検証 — synvelis#114 系の
  GUI-L2 gate。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/101
- 前提 spec: `docs/specs/pc-c4-drain-composition.md`（本 slice
  が引き取る deferred 項目の親）、
  `docs/specs/pc-c4-04-app-composition.md`（composition root）、
  `docs/specs/pc-c4-06-control-op-surface.md`（op surface）。
- mirror 対象 precedent: `synvelis-native.rs` の
  `compose_product_routing`/`product_engine_config` と
  `docs/impl/ir/pc-c4-drain-composition.md` の deferred 記録。
