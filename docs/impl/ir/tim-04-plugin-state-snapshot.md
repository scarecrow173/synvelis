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

## Summary

Issue #128 の headless/mock slice: plugin state を「exact と
偽らない」品質証拠モデルとして `synvelis-project` に実装し、
`synvelis-player` の非 RT adapter（`PluginStateHost`）と
`synvelis-render` の offline recipe gate（`PluginStateRecipe` /
`preflight_plugin_state`）で消費した。品質語彙
（`StateCaptureQuality`/`StateQualityPolicy`/
`PluginStateFailureKind`）・content-addressed store
（`InMemory`/`Directory`）・`PluginStateSession` save/restore・
receipt 系を additive に追加。Ruviform #196 の generic
`HostCommand::SaveState`/`RestoreState` を消費するのみで、
host/lifecycle/state API の複製はしていない。mise canonical
検証は全 pass（`mise run test` 146 test binaries・0 failed、
`plugin_state` 14 + player `tim04_plugin_state` 9 + render
`tim04_plugin_state` 5 を含む）。

## Context

- Issue: https://github.com/scarecrow173/synvelis/issues/128
- 上流 doc chain:
  `docs/specs/tim-04-plugin-state-snapshot-consistency.md` /
  `docs/designs/tim-04-plugin-state-snapshot-consistency-design.md` /
  `docs/plans/tim-04-plugin-state-snapshot.md` /
  `docs/tasks/tim-04-plugin-state-snapshot.md` /
  `docs/test-specs/tim-04-plugin-state-snapshot.md`。
- 依存（task doc Dependencies より）: Ruviform #196
  `HostCommand::SaveState`/`RestoreState`（消費のみ）、VCS-01
  `ContentDigest`/`OpaqueStateRef`（再利用・変更しない）、
  SYN-IO-02 staged publish（`ruviform-io-file` 再利用）、
  TIM-03 process boundary（非 RT policy 先例）。
- 範囲外（front matter `defers`）: 実 VST3/AU/CLAP plugin での
  state save/restore・実 OS filesystem atomicity・GUI 統合は
  NotRun/Unsupported — headless mock slice の範囲外。

## Implementation

以下、crate ごとの additive 実装内容（host/lifecycle/state
API の複製なし・Ruviform #196 の generic command を消費
するのみ）。

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

## Related Experiments

### Adopted

- なし（experiment log なし — `metadata.experiments.adopted` は空、
  `docs/impl/exp/` に関連 experiment なし）。

### Rejected

- なし。

## Validation

実施済み — canonical = mise。

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

## Risks

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

## Notes

- `Cargo.toml`/`Cargo.lock`/`mise.toml` 無変更。依存追加なし。
- `plugin_state.rs` は単一 module file（submodule 分割は
  行数が更に増えた場合に検討）。
- `PluginStateHost` は `Rc<RefCell<PluginHost>>` 共有のため
  `!Send` — host 同様 single-owner 非 RT object。
- `plugin_state_handles` は published generation の
  `RetainedConnection` からしか vend しないため、retired
  generation の handle は host 側 generation counter が
  reject する（test で Destroy 後 fail-closed を確認済み）。

## Follow-ups

- 実 VST3/AU/CLAP plugin での state save/restore・実 OS
  filesystem atomicity・GUI/実 audio device/RT thread からの
  state save は NotRun/Unsupported — headless mock slice の
  範囲外であり、実 plugin 統合・OS レベル fault 注入・GUI
  統合は別 slice/gate へ委譲（front matter `defers` と同一
  範囲）。
- `plugin_state.rs` は単一 module file — 行数が更に増えた
  場合に submodule 分割を検討（上記 Notes）。
- Ruviform #253 OS MIDI/VST3 翻訳を state-save owner とする
  変更は対象外（scope 禁止事項どおり未着手 — 上記 Risks）。
