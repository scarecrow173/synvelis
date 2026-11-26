---
id: "DESIGN-SV114SVOSCONSUMER0000001"
type: "design"
status: "approved"
title: "SV-OS consumer catalog — catalog/headless/real lane 設計 (synvelis#114)"
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
    - "docs/specs/sv114-svos-consumer-catalog.md"
  implemented-by: []
  depends-on:
    - "docs/discovery/sv114-svos-consumer-catalog.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv114-svos-consumer-catalog.md"
    - "crates/synvelis-app/tests/common/sv_os_catalog.rs"
    - "crates/synvelis-app/tests/sv_os_headless.rs"
    - "crates/synvelis-app/tests/sv_os_real.rs"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
---

# SV-OS consumer catalog 設計

## 構成

```text
tests/common/scenario.rs
└── #[path = "sv_os_catalog.rs"] pub mod sv_os_catalog
    ├── SvOsLane<G>                … adapter + main/secondary window/view ids
    ├── sv_os_entries<G>()         … SV-OS-01〜05 ScenarioEntry table
    ├── CAPABILITY_CLASSIFICATIONS … Required/Deferred/NotApplicable 記録
    └── run_sv_os_01..05           … 通常の Rust case body

tests/sv_os_headless.rs
├── Fake lane (default)
├── GpuiSimulated lane (`gpui-test` cfg)
└── ScenarioMatrix projection test

tests/sv_os_real.rs (`real-os-scenario` required)
├── product main/secondary ViewHost → RealWindowMount
├── RealOsGuiTest::run preflight/session
├── 同じ sv_os_entries を run_entries で実行
└── focus/geometry/environment/evidence rows
```

`tests/common/mod.rs` は lease 外なので編集しない。catalog は `scenario.rs` の
`#[path]` module として公開し、lane test から `common::scenario::sv_os_catalog`で参照する。

## Catalog 契約

`sv_os_entries<G>()` は 5 件の `ScenarioEntry<SvOsLane<G>>` を返す。各 entry は
frozen case id (`SV-OS-01`〜`SV-OS-05`)、scenario id `synvelis.sv-os.consumer`、
required capabilities、spec extras、run function を declaration として持つ。
`run_entries`/`run_matrix` の契約を消費し、独自 runner・JSON DSL・第二の verdictauthority は追加しない。

## Case body と oracle

case body は既存 driver-neutral helper を再利用する。

- SV-OS-01: `project.save/reload/undo/redo` button が `project` target の actionを mint し、Applied receipt が view wire に着弾する。
- SV-OS-02: arrangement row select が view-local `select` action/selectionを生じ、inspector table は read-only reject を保つ。
- SV-OS-03: fader Begin/Preview/Commit、master-strip target、receipt+projection、undo/redo action を照合する。
- SV-OS-04: transport position gesture、play action、save/reload actionを照合する。
- SV-OS-05: secondary window の seek/play、tab cycle、stale generation pin reject、2 window ownershipを照合する。

これらに加えて、各 case は独立した mounted `NativeShell`/`SynvelisApp` fixture上で対応する製品 service oracleを実行する。SV-OS-01は project.new/view composition/transport receipt、SV-OS-02は arrangement selection→inspection、SV-OS-03は fader receipt/projectionと undo/redo、SV-OS-04は transport receiptと save/reload bundle、SV-OS-05は 2window ownership closeと project truth survivalを検査する。この service oracle は headless OS-L1 の独立 oracleであり、real window上で service-backed input/pixel/close 全量を主張するものではない。real lane reportの `observed.service_oracle` に結果を残し、lane verdictを昇格させない。

## Headless lane

Fake lane は default test targetで実行する。`gpui-test` feature 時のみ、production-like `ViewHost + create_view_window` composition の `GpuiGuiTest` laneが同じ entriesを実行する。matrix projection test は `DriverBinding` declaration だけからrunnable/missing を計算し、何も実行しない。

## Real OS lane

`real-os-scenario = ["native", "ruviform-ui-gpui/real-scenario"]` は non-default featureであり、`[[test]] sv_os_real` は `required-features`で隔離する。`mise.toml` には追加しない。

real lane は Main/Secondary の製品 screen specを `ViewHost` に mountし、`RealWindowMount` 経由で実 GPUI windowへ出す。`RealOsGuiTest::run` の window/session作成自体を interactive preflightとし、失敗時は case bodyを実行せず全件 `NotRun + environment_blocked`とする。成功時のみ同じ catalog entriesを実行し、window geometry/focus request/focus observationを report environment に追加する。SV-OS-05 は両 windowの focus observationがない場合 Pass にしない。

`RealOsGuiTest` は session body 内の `&mut` としてだけ存在するため、test-local `RealGui<'a>` wrapperが `SemanticGuiTest` を委譲実装する。wrapperは port 呼び出しをそのまま転送するだけで、runnerや verdict authorityではない。

## Evidence

real lane evidence は opt-in環境変数 `SYNVELIS_SV114_REAL_EVIDENCE` が指す JSONLに追記する。相対 pathは repository rootに anchorする。schema は `sv114-svos-real/v1`。headless lane は通常の test verdictと catalog report assertionを証拠とし、Fake/GpuiSimulated Passを real/pixel/native dock Passへ昇格させない。
