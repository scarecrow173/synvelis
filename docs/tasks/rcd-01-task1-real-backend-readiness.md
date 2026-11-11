---
id: "TASK-GJlWE8j5emvVOKFMtbTXHR"
type: "task"
status: "in_progress"
title: "RCD-01 Task — 実Audio Device 到達準備（stamp placement fallback + real backend 配線）"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  implements:
    - "docs/plans/rcd-01-real-backend-readiness-plan.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-01-real-backend-readiness.md"
    - "docs/designs/rcd-01-real-backend-readiness-design.md"
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

# RCD-01 Task — 実Audio Device 到達準備

## Checklist

### Slice A — stamp placement fallback（jitter 許容の累積配置）

- [x] `crates/synvelis-recording/Cargo.toml`: `ruviform-media-core`
      を dev-deps → deps へ昇格。
- [x] `tests/common/mod.rs`: `FakeStreamShared` に capture stamp
      発射（`report_capture`/`stamp_epoch_ticks`/`stamp_base`/
      `now_ticks` knob）と決定的 jitter knob（`stamp_jitter_ns`）を追加。
- [x] `tests/input_adapter.rs`: stamp fallback 系 test を先に書く
      （TDD）。
- [x] `src/input.rs`: `StampPlacement`/`calibrate_stamp_placement`/
      `StampState`/`resolve_first_frame`/`stamp_rejection_fault`/
      許容閾値定数 実装、callback 組込み、`#[cfg(test)]` unit test
      （5 種 MapRejection + seed/累積配置/jitter/delta gap/drift/逆行）。
      ※実機実測の jitter で初版「block 毎独立写像」が
      `FAULT_INPUT_DISCONTINUITY` となったため累積配置へ改訂。
- [x] `tests/input_adapter.rs`: `missing_frame_position_faults` を
      「position も stamp も無い場合に fault」へ整合更新、
      `capture_stamp_jitter_still_records_verified_pcm` 追加。
- [x] `tests/device_recording.rs`: stamp fallback の実 writer 統合
      test 追加。
- [x] `tests/input_adapter_rt_alloc.rs`: stamp 経路の no-alloc 計測。

### Slice B — real backend 配線 + opt-in lane

- [x] ルート `Cargo.toml`: `ruviform-audio-device` workspace dep 追加。
- [x] `crates/synvelis-recording/Cargo.toml`: optional dep +
      `real-device` feature。
- [x] `src/real_input.rs` 新規: `negotiate_input_format`/
      `select_f32_input_format`/`real_input_config` + feature-gated
      `default_host` re-export。
- [x] `src/lib.rs`: `real_input` module export。
- [x] `tests/real_input.rs` 新規: headless test + `#[ignore]` 実機 lane
      （timestamp fields probe 含む）。
- [x] `mise.toml`: `test-recording-real-input`/
      `lint-recording-real-input` opt-in task。
- [x] stale「ruviform#251 待ち」コメント更新（3 test file）。
- [x] `Cargo.lock` 再生成。
- [x] ruviform 上流 issue 起票: cpal WASAPI adapter が
      `IAudioClock::GetPosition` の device frame position を破棄
      → ruviform#565（`frame_position` 供給で stamp fallback は
      不要になる）。

### 検証・提出

- [x] `mise run fmt` → `fmt-check`/`check`/`lint`/`test`/`test-rt`/
      `test-msrv` 全パス。
- [x] `cargo clippy -p synvelis-recording --all-targets
      --features real-device -- -D warnings`（補助証拠・pass）。
- [x] 実機 lane `cargo test -p synvelis-recording --locked
      --features real-device --test real_input -- --ignored
      --test-threads=1`: Windows/WASAPI（Audient iD4）で
      2 test pass（jitter 下の連続録音を確認）。
- [x] `git fetch origin main && git merge origin/main` 後に再検証
      （Already up to date・再検証不要と判断）。
- [x] review-protocol レビュー（separated passes・blocker 無し。
      非 blocking note: frame_position/stamp 混在 backend での
      stamp state 陳腐化は保守的 fault、長時間 session の drift
      閾値は設計上の許容範囲、Cargo.lock の ruviform rev 更新は
      PR に記録）。
- [ ] Conventional Commits で論理分割 commit、push、PR 作成
      （本文は日本語・実測/NotRun を区別）。
