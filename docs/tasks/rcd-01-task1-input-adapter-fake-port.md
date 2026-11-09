---
id: "TASK-LRVMzVtzazw5gBnDcKYhLk"
type: "task"
status: "in_progress"
title: "RCD-01 Task — input adapter + fake port 検証"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  implements:
    - "docs/plans/rcd-01-input-adapter.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md"
    - "docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 Task — input adapter + fake port 検証

## Checklist

- [ ] `crates/synvelis-recording/Cargo.toml` に `ruviform-audio-core.workspace = true` 追加、`Cargo.lock` 再生成。
- [ ] `src/input.rs`: `InputBinding`/channel map/Arm/Monitor/clock domain 状態、
      `InputCallbackInfo`→`InputBlock` 変換、planar→interleaved 有界コピー、
      fault 写像。
- [ ] `src/lib.rs` export。
- [ ] fake `DeviceEnumerator`/`Device`/`InputStream` harness（scripted port）。
- [ ] `tests/input_adapter.rs`: PCM fidelity（Mono/Stereo/MultiChannel/2Track）、
      binding/channel map、monitor 独立、Arm Off/Monitor On、same-clock 受理/
      拒否、fail-stop（drop/epoch/rate/domain/error/queue-full）、
      Journal Verified Prefix 保持。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` 全パス。
- [ ] `git fetch origin main && git merge origin/main` 後に再検証。
- [ ] Conventional Commits で論理分割 commit、push、PR 作成。

## Notes

- 実機検証は Unsupported（ruviform#251）。fake port 契約 Pass として記録し、
  実 Device Pass と偽装しない。
- `synvelis-app`・他 crate・Ruviform は変更しない。
