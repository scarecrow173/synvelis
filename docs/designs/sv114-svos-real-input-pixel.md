---
id: "DESIGN-SV114SVOSREALINPUT0001"
type: "design"
status: "approved"
title: "SV-OS-01 real input/pixel vertical slice 設計 (synvelis#114)"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv114-svos-real-input-pixel.md"
  implemented-by: []
  depends-on:
    - "docs/discovery/sv114-svos-real-input-pixel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv114-svos-real-input-pixel.md"
    - "crates/synvelis-app/tests/sv_os_real.rs"
    - "crates/synvelis-app/tests/common/sv_os_catalog.rs"
    - "crates/synvelis-app/tests/sv_os_headless.rs"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
---

# SV-OS-01 real input/pixel vertical slice 設計

## 構成

```text
tests/sv_os_real.rs (`real-os-scenario`)
├── RealOsGuiTest::run                     … interactive preflight + real windows
├── run_entries(sv_os_entries)             … 既存 semantic catalogを変更せず実行
├── augment_sv_os_01_input_pixel           … real-only SV-OS-01 augmentation
│   ├── resolve project.save target
│   ├── os_input_preflight(Pointer)
│   ├── focus request/observation
│   ├── os_pointer_move_to / os_mouse_down / os_mouse_up
│   ├── drain Command intents
│   ├── post Applied receipts through view wire
│   ├── settle後 component observation
│   └── capture_frame + last_capture_meta
└── focus/geometry/environment rows        … 既存 real lane evidence

tests/common/sv_os_catalog.rs
├── SV-OS-01 service oracle += syn.project.save
└── CAPABILITY_CLASSIFICATIONS             … SV-OS-01 input/pixelのみ Required

tests/sv_os_headless.rs
└── capability classification assertion    … lane昇格しない境界を固定
```

catalog entry の required capabilities は `SemanticAction` のままにする。これにより Fake/GpuiSimulated の実行可能性と verdict を変えず、real-only requirement は classification/evidence/docsで表す。input/pixel augmentation は `sv_os_real.rs` に閉じ込め、`tests/common/scenario.rs`・production wiring・ruviform APIには触れない。

## 手順

1. semantic catalogを先に実行する。SV-OS-01 の通常の semantic Command/Receipt/view-stateと独立 service oracleを従来どおり記録する。
2. 同じ real window/main view上で `project.save` targetを再解決する。
3. pointer scopeの preflight、focus request/observation、target中心への move、down、upを順に行う。move/downが injected でない場合は後続 opを実行せず、最初の non-injected outcomeを evidence に残す。
4. settle後に intentを drainし、`project` targetの `save` action 1件だけを期待する。無関係 intentが混ざった場合や0件の場合は Passにしない。
5. drainした command id に対応する Applied receiptを harnessから view wireへ postし、receipt件数と `Applied` status、settle後 phaseを記録する。
6. `capture_frame` を実行し、captured時は backend/width/height/scale factor/pixel countを記録する。`pixel_count == 0` は Fail、`Unsupported`/`NotRun` はそれぞれ同じ verdictに写像する。
7. semantic verdict、input verdict、capture verdictの悪い方を SV-OS-01 row verdictとし、detailと evidence tagを更新する。

## Verdict 写像

| observed outcome | verdict |
| --- | --- |
| 3 opsとも `Injected`、save action一致、Applied receipt accepted、settled phase Applied、pixel_count > 0 | Pass |
| input opが `EnvironmentBlocked`/`NotRun` | NotRun |
| input opが `Unsupported` | Unsupported |
| captureが `Unsupported` | Unsupported |
| captureが `NotRun` | NotRun |
| injection済みだが command/receipt/phase/pixel が欠ける | Fail |
| scenario contract error（resolve/observe/post/capture API error） | Fail |

`EnvironmentBlocked` は evidence の detailに残し、Passへ書き換えない。OS automation injection を human physical inputと呼ばず、receipt originを harness-posted と明記する。

## Oracle の分離

`observed.real_input_pixel.product_state_oracle` は既存 catalog が同じ SV-OS-01 rowへ入れた `observed.service_oracle` のコピーである。これは独立 mounted `NativeShell`/`SynvelisApp` fixtureによる headless OS-L1 oracleであり、real windowから production serviceへ届いた証拠ではない。この sliceでは `syn.project.save` の service oracleを追加して、save能力の製品側 truth を別列で追跡する。

## Environment honesty

real window sessionが成立した reportには `environment_kind = real-window` を記録する。interactive preflight失敗で case bodyに入らない blocked rowsは `environment_kind = deterministic-backend` とし、real window/pixel/input evidenceを主張しない。`environment_blocked` は fail-closed NotRun の理由であり、Passの理由には使わない。
