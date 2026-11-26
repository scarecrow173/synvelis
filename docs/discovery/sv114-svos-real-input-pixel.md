---
id: "DISC-SV114SVOSREALINPUT000001"
type: "discovery"
status: "completed"
title: "SV-OS-01 real input/pixel vertical slice 調査記録 (synvelis#114)"
created: "2026-11-26"
updated: "2026-11-26"
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
  implemented-by:
    - "docs/specs/sv114-svos-real-input-pixel.md"
  depends-on:
    - "docs/specs/sv114-svos-consumer-catalog.md"
    - "evidence/sv114/sv-os-case-definitions.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv114-svos-real-input-pixel.md"
    - "docs/test-specs/sv114-svos-real-input-pixel.md"
    - "evidence/sv114/sv-os-real-input-pixel-verification.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-app/tests/sv_os_real.rs"
    - "crates/synvelis-app/tests/common/sv_os_catalog.rs"
    - "crates/synvelis-app/tests/sv_os_headless.rs"
  defers: []
---

# SV-OS-01 real input/pixel vertical slice 調査記録

## 調査範囲

synvelis#114 SV-GUI-05 のうち、SV-OS-01 に限定した real OS input/pixel の最小縦断を調査した。既存 `tests/sv_os_real.rs` は `RealOsGuiTest` で製品 Main/Secondary screen を実 window に mount し、catalog の semantic Command/Receipt/view-state と focus/geometry を記録するが、各行に real OS input/pixel/IME/native dock evidence を主張しない旨を残していた。本調査では、この real lane を弱めずに SV-OS-01 だけ physical-input 相当の OS automation injection と pixel capture を同じ run/case rowへ追加できるかを確認した。

参照した外部契約は pin 済み ruviform revision の `RealOsGuiTest`/`os_input`/`VisualGuiTest` API と、AudioLens consumer の real scenario 先例である。`scarecrow173/ruviform#631` は runner/evidence contract の external gate として参照するだけであり、本 slice の owner ではない。ruviform/audio-lens 両リポジトリは read-only reference であり、API や実装を変更しない。

## 確認した前提

- `RealOsGuiTest` は `os_input_preflight`、`os_pointer_move_to`、`os_mouse_down`、`os_mouse_up`、`capture_frame`、`last_capture_meta`、focus/geometry/window events を公開している。
- OS input outcome は `Injected`/`Unsupported`/`EnvironmentBlocked`/`NotRun` を区別できる。`Unsupported`・`NotRun`・`EnvironmentBlocked` を Pass にしてはいけない。
- AudioLens の先例は、injection が human physical input ではなく OS automation injection であること、blocked/unsupported を正直に verdict へ写像すること、capture metadata（backend/width/height/scale/pixel count）を記録することを示している。
- 既存 Synvelis catalog の SV-OS-01 は `project.save`/`reload`/`undo`/`redo` の semantic action と独立した mounted `NativeShell`/`SynvelisApp` service oracle を持つ。`project.save` は同じ case/run id に input/pixel augmentation をぶら下げる最小対象として適している。
- `tests/common/scenario.rs` と `tests/common/mod.rs` は本 slice の編集範囲外である。したがって augmentation は `sv_os_real.rs` 内の real-only hook とし、catalog body を Fake/GpuiSimulated lane に増殖させない。

## 設計判断に効いた制約

1. Fake/GpuiSimulated pass を GUI-L2 pass に昇格させない。headless laneの semantic verdict と capability projection は変えない。
2. real OS input/pixel capability は SV-OS-01 のみ Required に分類し、SV-OS-02〜05 は従来どおり Deferred に残す。`NativePluginDock` は 01〜05 で NotApplicable のままにする。
3. Worker は interactive desktop で実 window を開く real lane を実行しない。compile/static/headless evidence と、interactive gate の exact command・確認点を残す。
4. OS automation injection を physical manual input と呼ばない。evidence には injection method を明記する。
5. harness が view wire に post する Applied receipt は production service receipt ではない。製品 state oracle は既存の独立 headless service oracleを同じ rowに保持し、real-window経由 production wiring の完了を主張しない。

## 残る制約・ギャップ

- 本 slice が通すのは SV-OS-01 の `project.save` component に対する pointer move/down/up と pixel capture の最小縦断だけである。Project Create/Open/Close 全量、IME、DPI/monitor matrix、native dock、PD-01〜07、SV-OS-02〜09 は対象外である。
- Worker 環境で interactive real lane は NotRun のままである。実走では SV-OS-01 row が input ops、Command intent、Applied receipt、settled observation、pixel metadata、product state oracleを同一 runで示すことを Coordinator gate が確認する。
- `capture_frame` が `Unsupported`/`NotRun` を返す backend では Pass にせず、それぞれ Unsupported/NotRun に留める。
