---
id: "SPEC-SV114SVOSREALINPUT00001"
type: "spec"
status: "approved"
title: "SV-OS-01 real input/pixel vertical slice spec (synvelis#114)"
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
  implemented-by: []
  depends-on:
    - "docs/discovery/sv114-svos-real-input-pixel.md"
    - "docs/specs/sv114-svos-consumer-catalog.md"
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
  verified-by:
    - "docs/test-specs/sv114-svos-real-input-pixel.md"
  references: []
  defers: []
---

# SV-OS-01 real input/pixel vertical slice spec

## 目的

synvelis#114 の Windows GUI-L2 受入のうち、SV-OS-01 に限定して、既存 real-OS catalog lane と同じ run/case id で追跡できる OS input/pixel evidence の最小縦断を実装する。本 spec は Issue #114 全体の完了条件ではなく、`Refs #114` の partial slice である。`scarecrow173/ruviform#631` は利用する runner/evidence contract の external gate として参照するだけである。

## Scope

### 実装する

- `tests/sv_os_real.rs` の real lane で、SV-OS-01 report に real-only input/pixel augmentation を追加する。
- 対象 component は `project.save` とし、OS pointer move/down/up を `RealOsGuiTest` の OS input API で注入する。
- injection outcome、focus/preflight、Command intent、Applied receipt、settle後 observation、製品 state oracle、capture metadata を同じ SV-OS-01 rowに記録する。
- `VisualGuiTest::capture_frame` を同じ対象 target に実行し、backend/width/height/scale factor/pixel count を記録する。
- capability classification では SV-OS-01 の `RealOsPointerOrKeyboard` と `VisualCapture` のみ Required とし、SV-OS-02〜05 の分類を変えない。
- SV-OS-01 service oracle に `syn.project.save` の独立製品 service 結果を追加する。

### 実装しない

- SV-OS-02〜09、IME composition、DPI/monitor matrix、native dock、PD-01〜07。
- production `crates/synvelis-app/src/**` の wiring、CI workflow、`mise.toml`、`Cargo.toml`、ruviform/audio-lens の変更。
- `docs/**/README.md` の編集・再生成。
- interactive desktop を必要とする Worker 実走。実走は後続 Coordinator gateとする。
- Fake/GpuiSimulated/headless verdict の GUI-L2 verdict への昇格。

## 受入対応

| slice 受入 | 本 spec の要求 |
| --- | --- |
| real-only augmentation が compile される | `real-os-scenario` feature で `sv_os_real` targetが compile/lint できること |
| physical input/pixel evidence 経路が明示される | SV-OS-01 rowの `observed.real_input_pixel` に ops/Command/Receipt/observation/capture/oracle が入ること |
| `Unsupported`/`NotRun`/`EnvironmentBlocked` を Passにしない | input/capture outcome を verdict へ fail-closed に写像すること |
| headless/default lanes が非回帰 | Fake/GpuiSimulated laneの entries/required capabilities/verdict を弱めないこと |
| interactive real lane を分離 | Worker は compile/headless evidenceだけ記録し、interactive commandと確認点を PR/evidenceへ残すこと |
| scope外 zero-diff | docs index、mise/Cargo manifest、production wiring、ruviform/audio-lens を変更しないこと |

## Evidence semantics

OS automation injection は human physical manual input ではない。evidence では `input_method` を明示し、injected ops が確認できた場合だけ input verdictを Pass候補にする。Applied receipt は test harnessが consumer-to-view wireへ post した receiptであり、production service receiptとは区別する。製品 state oracleは `observed.service_oracle` の独立 headless service oracleであり、real window経由 production wiring の証拠へ昇格させない。

capture verdict は `Captured` かつ `pixel_count > 0` の場合だけ Pass候補とし、`Unsupported`/`NotRun`/未知 outcomeを Passにしない。最終 verdict は semantic catalog verdictと input/capture verdictの悪い方を残し、semantic pass を実 input/pixel pass と混ぜない。
