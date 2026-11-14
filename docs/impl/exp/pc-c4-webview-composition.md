---
id: "EXP-034XHD9Jq3sQv8bP1tU"
type: "experiment"
status: "completed"
title: "PC-C4 webview composition — gate 形状と helper 配置の検証"
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
  implements: []
  implemented-by:
    - "docs/impl/ir/pc-c4-webview-composition-synvelis-rs-engine-attach.md"
  depends-on:
    - "docs/impl/ir/pc-c4-webview-composition-synvelis-rs-engine-attach.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/pc-c4-observe-engine-wiring.md"
    - "docs/impl/exp/pc-c4-plugin-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/pc-c4-webview-composition.md"
    - "docs/test-specs/pc-c4-webview-composition.md"
  defers: []
  deferred-by: []
metadata:
  experiments: []
---

# PC-C4 webview composition — experiment log

## EXP-1: `SYNVELIS_PROJECT_DIR` gate の表現 — env 読みの所在

### 問い

webview bin が `SYNVELIS_PROJECT_DIR` gate で engine composition
を呼ぶとき、env 読み取りをどの層に置くか。`synvelis-native.rs`
は bin 内の private `compose_product_routing` が
`std::env::var_os` を直接読むが、webview 側は共有 helper を
`composition.rs`（library）に置く設計だったため、gate の
`Option` 化をどこで行うかが分岐点だった。

### 検討した選択肢

1. **library helper が env を読む** —
   `compose_product_routing(app)` が内部で
   `std::env::var_os("SYNVELIS_PROJECT_DIR")` を読む形。呼出側は
   最も薄いが、composition.rs の既存契約
   「the library never touches `std::env` itself」（bin が
   process boundary で env を解決し `Option<PathBuf>` として
   渡す）に違反する。
2. **bin が env を読み `Option<PathBuf>` を渡す** —
   `compose_product_routing(app, project_dir:
   Option<PathBuf>)`。env var 名知識は process boundary の
   bin に留まり、gate 意味（`None` → stub 維持）は library の
   testable な契約になる。fixture が env を触らず helper を
   直接呼べる。
3. **fixture が env を set して bin 経路を spawn 検証** —
   process boundary ごと検証できるが、webview bin は実
   WebView2 backend を要求し headless fixture では起動
   できない。本 slice の fixture 契約（in-process
   headless）と合わない。

### 結果

選択肢 2 を採用。`compose_product_routing(app,
project_dir: Option<PathBuf>)` を `composition.rs` に追加し、
`bin/synvelis.rs` が `std::env::var_os("SYNVELIS_PROJECT_DIR")
.map(PathBuf::from)` を `attach_logging` 直後・
`NativeShell::new_with_data` 前に渡す。選択肢 1 は library
契約違反、選択肢 3 は headless fixture 不能のため却下。

## EXP-2: native bin の呼出側移行を本 slice で行うか

### 問い

`composition.rs` に product helper を置くなら、重複する
`synvelis-native.rs` の private `compose_product_routing`/
`product_engine_config` も同 helper へ切替えるのが DRY 的に
自然に見えるが、coordinator 確定事項は「native 側のコード経路
は変更しない（webview 側のみ新 helper を利用 — native 側の
回帰リスクを避ける）」だった。

### 検討した選択肢

1. **native も新 helper へ切替** — 重複解消できるが、
   `synvelis-native.rs` は本 slice の編集許可範囲外
   （read-only 参照のみ）であり、`native` feature lane の
   回帰検証も本 slice の必須 lane に含まれない。
2. **webview のみ新 helper を利用し native は据置** —
   `product_engine_config` の body が一時的に重複するが、
   native 経路は無変更で非回帰が保証される。統合移行は
   native 側回帰 lane を伴う後続 slice の責務。

### 結果

選択肢 2 を採用。`composition.rs` の helper は feature 非依存
の lib コードとして追加し、`synvelis.rs`（webview feature）
のみが呼ぶ。`synvelis-native.rs` は無変更 — `git diff` で
確認済み。重複は design の Deferred Design Concerns に記録し、
後続 slice で native を新 helper へ切替える際に解消する。

## EXP-3: fixture が pin する経路の粒度

### 問い

`tests/pc_c4_webview_composition.rs` が検証するのは
「webview 経路の composition」だが、bin の env 読み自体は
process boundary で fixture からは到達できない。helper を
直接呼ぶ fixture が production wiring を十分に pin するか。

### 検討した選択肢

1. **`compose_product_routing` を直接呼ぶ** — bin と同一の
   lib helper を経路として使い、`PluginHost`/
   `PluginHostProcessors` を名指さない。gate 閉（`None`）・
   gate 開（`Some(dir)` → attach → publish → 非 stub
   `syn.control.status`）・recreate detach を in-process で
   検証できる。env 読みの 1 行は compile
   （`check-app-webview`）と review で担保する。
2. **bin を subprocess spawn して env 経由で検証** —
   process boundary ごと pin できるが、webview bin は実
   backend を要求し headless では起動不能。native bin でも
   `webview` feature 非依存の headless spawn 経路は存在
   しない。

### 結果

選択肢 1 を採用。`compose_product_routing` の `Option`
契約が gate 意味を testable にしており、bin 側の env read は
`check-app-webview` の compile + code review で担保する
（test-spec の Risk 節に明記）。`SetProjectName` revision
が空 project でも generation 1 の control surface を
publish することを実測で確認した。
