---
id: "DESIGN-034XHDrzEI83tAEfivcKq7"
type: "design"
status: "approved"
title: "PC-C4 webview composition design — synvelis.rs への routing engine attach"
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
    - "docs/specs/pc-c4-webview-composition.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-webview-composition.md"
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-webview-composition.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-webview-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-webview-composition.md"
  references:
    - "docs/designs/pc-c4-drain-composition-design.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
  defers: []
  deferred-by: []
---

# PC-C4 webview composition design — synvelis.rs への routing engine attach

## Context

`synvelis-native` bin は drain slice で `compose_product_routing`
（private）+ `product_engine_config`（private）を持ち、
`SYNVELIS_PROJECT_DIR` gate・empty plugin registry・
`<dir>/media` media root・48 kHz/stereo/256 Realtime geometry で
`compose_routing_engine` を呼ぶ。`synvelis`（webview）bin は
同じ `SynvelisApp`/`AppService`/`UiRuntime` 経路を持つが
composition 呼出を持たず、engine stub のまま `syn.control.*`
の `engine:false`/null evidence を返していた。

共有 helper の置き場は `composition.rs`（production composition
root の既存の家）。env 読み取りは library に持ち込めない
（「the library never touches `std::env` itself」の既存契約 —
bin が process boundary で `var_os` を読み、`Option<PathBuf>`
として渡す）。

## Scope

### In Scope

- `composition.rs` への `compose_product_routing(app,
  project_dir: Option<PathBuf>)` と private
  `product_engine_config(media_root)` の追加。
- `lib.rs` の re-export 追加。
- `bin/synvelis.rs` への env gate + 呼出配線（`attach_logging`
  直後・`NativeShell::new_with_data` 前）。
- `tests/pc_c4_webview_composition.rs`（新規 fixture）。

### Out of Scope

- `synvelis-native.rs` の変更 — private helper は残り、webview
  側のみ新 helper を使う（native 回帰リスク回避の coordinator
  確定事項）。
- 実 plugin factory・project 切替再 attach・実 GUI 検証。

## Components and Boundaries

- `composition.rs::compose_product_routing` — `None` → no-op
  （gate 閉、engine stub 維持）。`Some(dir)` →
  `compose_routing_engine(app, product_engine_config(dir.join
  ("media")), vec![])`。戻り値なし（engine handle は app/session
  が所有、caller は op surface で観測）。
- `composition.rs::product_engine_config`（private）— native
  bin の同名 private fn と同一 geometry: 48 kHz/stereo/256
  block・`PrefetchConfig{256,8,2,48k}`・`BuildBudget{4096,
  16384,512}`・`BuildMode::Realtime`・`RuntimeInstanceId(1)`・
  `RuntimeClockEpochRef(1)`・`engine_config_generation: 1`。
- `bin/synvelis.rs` — `std::env::var_os("SYNVELIS_PROJECT_DIR")
  .map(PathBuf::from)` を読み `compose_product_routing(&mut
  app, …)` を呼ぶ。gate の env var 名知識は process boundary
  に留まる。

## Data and Control Flow

1. bin: logging install → backend probe → `SynvelisApp::new` →
   `attach_logging` → **`compose_product_routing`**（本 slice
   の追加点）→ asset check → `NativeShell::new_with_data` →
   `run_multi`。
2. helper: `Some(dir)` → `SessionEngineConfig{media_root:
   dir/media, …}` → `compose_routing_engine` → fresh
   `PluginHostProcessors::product(vec![])`（empty registry）→
   `SessionRoutingEngine` → `attach_routing_engine`（session
   `EnginePort` + `routing_engine` field）→
   `set_control_report_drain`。
3. `syn.control.status` は `routing_engine.is_some()` を
   `engine` に映し、publish 後は `control_generation()`/
   `control_report()`/`last_control_drain` の実 evidence を返す。
4. `ui_pump`（16 ms tick・op tail）が `drain_control_reports`・
   `drain_control_observations` を駆動 — drain slice の配線が
   webview 経路でもそのまま機能する。

## Non-Functional Constraints

- headless build 非破壊: helper は feature 非依存の lib コード、
  bin 呼出は `webview` feature の `synvelis.rs` のみ。
  `native` feature 側は無変更で `check-app-native` 非回帰。
- RT 非侵入: composition は非 RT 経路（起動時 1 回）。engine の
  `run`/`observe_block` 駆動は caller（ui_pump/tick）が供給する
  既存契約のまま。
- 捏造禁止: empty registry のため plugin-bound target の bind
  は typed reject、`frame_base` は null のまま — stub evidence
  を「attach 済み」に見せかけない。

## Risks and Trade-offs

- **native との helper 重複**: `product_engine_config` の body
  が native private fn と重複する。native 呼出移行は本 slice
  範囲外（明示的 deferred）— 重複は一時的で、統合は後続
  slice が native を新 helper へ切替える際に解消する。
- **untitled session への attach**: webview bin は boot project
  を open しない（native の `open_boot_project` 相当は本 slice
  対象外）ため、`SYNVELIS_PROJECT_DIR` 設定時も engine は
  untitled session に attach される。media root は config で
  あり clip 解決時にのみ使われる — 実害なし。project.open 後の
  再 attach は #106 の deferred 責務。
- **gate を `Option<PathBuf>` で表現**: env var 名の解釈を bin
  に残し、gate 意味（`None` → stub）を library の testable な
  契約にする。env 読みを lib に入れる代替案は「library never
  touches std::env」契約違反のため却下（exp EXP-1）。

## Deferred Design Concerns

- `synvelis-native.rs` の `compose_product_routing`/
  `product_engine_config` private 実装を共有 helper へ移行 —
  後続 slice（native 側回帰 lane を伴う変更として）。
- project recreate 後の engine 再 attach — synvelis#106。
- 実 plugin factory registry — ruviform#253。

## Verification Notes

- [x] `tests/pc_c4_webview_composition.rs` が gate 閉/開・
  attach・publish・非 stub `syn.control.status`・drain・
  recreate detach を fixture 検証する。
- [x] `check-app-webview`/`lint-app-webview`/`test-app-webview`
  で `webview` feature の bin compile・lint・test を検証する。
- [x] `synvelis-native.rs` 無変更を `git diff` で確認する。
