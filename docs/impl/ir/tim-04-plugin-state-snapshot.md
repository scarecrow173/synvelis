---
id: "IMPL-Tim04PluginStateSnapshot"
type: "impl"
status: "completed"
title: "TIM-04 Plugin State Snapshot 一貫性 — quality receipt / save-reload / HEAD / offline recipe（headless mock slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/plugin_state.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/plugin_state.rs"
      - type: "file"
        path: "crates/synvelis-player/src/plugin_state.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/tim04_plugin_state.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/tim04_plugin_state.rs"
      - type: "file"
        path: "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
      - type: "file"
        path: "docs/designs/tim-04-plugin-state-snapshot-consistency-design.md"
      - type: "file"
        path: "docs/plans/tim-04-plugin-state-snapshot.md"
      - type: "file"
        path: "docs/tasks/tim-04-plugin-state-snapshot.md"
      - type: "file"
        path: "docs/test-specs/tim-04-plugin-state-snapshot.md"
      - type: "file"
        path: "docs/impl/ir/tim-04-plugin-state-snapshot.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/host.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_engine.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/sf03_l3.rs"
      - type: "file"
        path: "crates/synvelis-render/src/error.rs"
      - type: "file"
        path: "crates/synvelis-render/src/export.rs"
      - type: "file"
        path: "crates/synvelis-render/src/job.rs"
      - type: "file"
        path: "crates/synvelis-render/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-render/src/preflight.rs"
      - type: "file"
        path: "crates/synvelis-render/src/spec.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/l2_e2e.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/x_e2e.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim-04-plugin-state-snapshot.md"
    - "docs/designs/tim-04-plugin-state-snapshot-consistency-design.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/tasks/tim-04-plugin-state-snapshot.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-04-plugin-state-snapshot-consistency-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/tim-03-process-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-04-plugin-state-snapshot.md"
  derived-by: []
  verifies:
    - "docs/test-specs/tim-04-plugin-state-snapshot.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/196"
    - "https://github.com/scarecrow173/ruviform/issues/253"
  defers:
    - "実 VST3/AU/CLAP plugin での state save/restore・実 OS filesystem atomicity・GUI 統合（NotRun/Unsupported — headless mock slice の範囲外）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-04 Plugin State Snapshot 一貫性 — Implementation Record

## What was built

Issue #128 の headless/mock slice。plugin state を「exact と
偽らない」品質証拠モデルとして `synvelis-project` に実装し、
`synvelis-player` の非 RT adapter と `synvelis-render` の
offline recipe gate で消費した。Ruviform #196 の generic
`HostCommand::SaveState`/`RestoreState` を消費するのみで、
host/lifecycle/state API の複製はしていない。

### `synvelis-project::plugin_state`（新設・additive）

- 品質語彙: `StateCaptureQuality`（`Coordinated`/`Quiescent`/
  `BestEffort`/`Unsupported`）、`CaptureMode`、
  `StateQualityPolicy`（`Standard`/`Strict`）、
  `PluginStateFailureKind`（`CaptureFailed`/`Timeout`/
  `PermissionDenied`/`Crashed`/`Corrupt`/`Unavailable`/`Missing`）。
  `exact_eligible()` は `Coordinated`/`Quiescent` のみ true。
- port 契約: `PluginStatePort`（`describe`/`capture`/`restore`）
  と `PluginStateStore`（content-addressed blob/manifest/HEAD）。
- store: `InMemoryPluginStateStore` と
  `DirectoryPluginStateStore`（`blobs/<digest>`・
  `manifests/ps-<digest>.json`・HEAD pointer、SYN-IO-02 の
  staged publish を `ruviform-io-file` 経由で再利用）。
  digest は VCS-01 の `ContentDigest`（SHA-256）を再利用。
- session: `PluginStateSession::save`/`restore`。
  - save は `routing.processors` を `ProcessorId` 順に走査。
    `Unresolved` binding・live instance 不在は
    `Failed(Unavailable)` で port を呼ばない。plugin
    `Unsupported`・typed failure は blob 無しで記録。
  - `Standard` は degraded/unsupported/failed をそのまま記録して
    publish。`Strict` は非 exact-eligible または失敗があれば
    `QualityRejected` で blob/manifest/HEAD の store 接触前に
    fail-closed。
  - indeterminate publish（blob/manifest）は read-back + digest
    照合で reconcile。未解消は error のまま。
  - restore は manifest schema/digest を検証し、各 blob の
    SHA-256 を port restore 呼出前に検証。manifest に無い live
    processor は `missing_states` に報告。copy-restore
    （undo/history 相当の record コピー）は品質・digest を
    そのまま保持。
- receipt: `PluginStateReceipt`/`PluginStateRestoreReceipt`/
  `PluginStateQualitySummary` — File Save 成功・Plugin State
  exactness・RT Active を混同しない別証拠。

### `synvelis-player`（additive）

- `plugin_state.rs`: `PluginStateHost` — 共有 `PluginHost` を
  包む `PluginStatePort` 実装。`HostCommand::SaveState`/
  `RestoreState` を発行し、前後の `lifecycle_state` から
  `processing`/`stable_boundary` を観測して achieved quality を
  分類（processing 中は `BestEffort` 上限、安定非処理は
  mode に応じ `Coordinated`/`Quiescent`）。`HostError` は
  `Unsupported`/`Unavailable`/`Crashed`/`CaptureFailed` に
  typed 写像。全 method は非 RT。
- `host.rs`: `ProcessorConnection.state_handle:
  Option<PluginHandle>`（plugin 接続のみ `Some`）。
- `routing_engine.rs`: `RetainedConnection.state_handle` と
  `plugin_state_handles(generation)` — `&self` copy vend で
  stale/未知 generation は空を返し fail-closed。

### `synvelis-render`（additive）

- `spec.rs`: `PluginStateRecipe`（policy + evidence summary +
  optional manifest digest）を `ExportRequest.plugin_state` に追加。
- `preflight.rs`: `preflight_plugin_state` — `Strict` かつ
  `!exact_eligible()` は `PreflightError::PluginStateQuality` で
  output/staged destination 生成前に拒否。`Standard` は
  evidence をそのまま通す。
- `job.rs`/`export.rs`: `ExportReceipt.plugin_state` に recipe を
  verbatim 記録（degraded を exact に格上げしない）。
- 既存 `ExportRequest` 構築箇所（l2_e2e/x_e2e 他）は
  `plugin_state: None` で無変更動作を維持。

## Verification（実施済み — canonical = mise）

| Task | Result |
| --- | --- |
| `mise run fmt` / `fmt-check` | Pass |
| `mise run check` | Pass（workspace --all-targets） |
| `mise run lint` | Pass（-D warnings） |
| `mise run test` | Pass（workspace --locked。146 test binaries・0 failed。`plugin_state` 14 + player `tim04_plugin_state` 9 + render `tim04_plugin_state` 5 を含む） |
| `mise run test-rt` | Pass（rt_ contract tests） |
| `mise run test-msrv` | Pass（rust 1.89.0 toolchain で workspace test） |
| `mise run check-app-native` | Pass |
| `mise run lint-app-native` | Pass |
| `mise run build-app-native` | Pass |

Fixture coverage（headless/mock のみ）: 全 4 品質 class・
Crash 相当（`Crashed`）/Timeout/PermissionDenied/Corrupt/
Unavailable/Unsupported の typed 記録・Strict fail-closed・
indeterminate blob/manifest publish reconcile・HEAD protection・
save/reload round-trip・restore digest 検証・missing/corrupt
blob・copy-restore 品質維持・実 `PluginHost` + mock plugin の
SaveState/RestoreState round-trip・render Strict 拒否 /
Standard evidence 保持。

### NotRun / Unsupported（明示）

- 実 VST3/AU/CLAP plugin での state save/restore: **NotRun** —
  mock plugin のみ。実 plugin 統合は別 slice。
- 実 OS filesystem atomicity・permission fault の実挙動:
  **NotRun** — `DirectoryPluginStateStore` は実 FS を使うが
  OS レベル fault 注入は mock store のみ。
- GUI / 実 audio device / RT thread からの state save:
  **Unsupported** — adapter は非 RT のみ。RT callback からの
  plugin state save・FS IO は構造上存在しない。
- Ruviform #253 OS MIDI/VST3 翻訳を state-save owner とする
  変更: **対象外**（scope 禁止事項どおり未着手）。

## Notes / follow-ups

- `Cargo.toml`/`Cargo.lock`/`mise.toml` 無変更。依存追加なし。
- `plugin_state.rs` は単一 module file（submodule 分割は
  行数が更に増えた場合に検討）。
- `PluginStateHost` は `Rc<RefCell<PluginHost>>` 共有のため
  `!Send` — host 同様 single-owner 非 RT object。
- `plugin_state_handles` は published generation の
  `RetainedConnection` からしか vend しないため、retired
  generation の handle は host 側 generation counter が
  reject する（test で Destroy 後 fail-closed を確認済み）。
