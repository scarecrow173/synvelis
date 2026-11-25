---
id: "SPEC-sv115VirtualEndpointRcd01"
type: "spec"
status: "approved"
title: "Issue #115 RCD-01 Consumer Virtual-WASAPI EXT-L3 Harness"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/virtual_endpoint.rs"
      - type: "file"
        path: "docs/specs/issue-115-virtual-endpoint.md"
      - type: "file"
        path: "docs/designs/issue-115-virtual-endpoint.md"
      - type: "file"
        path: "docs/test-specs/issue-115-virtual-endpoint.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/tests/real_input.rs"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/issue-115-virtual-endpoint.md"
  depends-on:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md"
    - "evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/issue-115-virtual-endpoint.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-115-virtual-endpoint.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/628"
    - "https://github.com/scarecrow173/ruviform/issues/629"
    - "https://github.com/scarecrow173/ruviform/issues/1001"
  defers: []
  deferred-by: []
---

# Issue #115 RCD-01 Consumer Virtual-WASAPI EXT-L3 Harness

## What Should Be Built

`Ruviform Cable Input`（OS render/output endpoint）へ deterministic PCM
stimulus を本番 OS output callback から供給し、`Ruviform Cable Output`
（OS capture/input endpoint）に現れた PCM を Synvelis 本番 Recorder 経路で
capture する consumer virtual-WASAPI EXT-L3 harness を提供する。

対象は B1 方針の Ruviform Virtual Audio Device のみであり、VB-CABLE・
Voicemeeter・その他の仮想 audio driver を provisioning 済み証拠とみなさない。
endpoint 名一致は候補であって証拠ではない。実際に open/start し、output
callback・input stream・Recorder commit→save→reload を観測できた run のみが
`level=ext-l3` / `environment_kind=virtual-os-endpoint` を名乗る。

## Acceptance Criteria

1. `crates/synvelis-recording/tests/virtual_endpoint.rs` が、Ruviform Cable
   render/capture endpoint の stem pair を候補化し、不在時は fail-closed に
   `result=not-run` / `NOT_PROVISIONED` を terminal metadata へ記録する。
2. stimulus は `ruviform-audio-test` の `PcmFixture` / `PcmPattern::Impulse`
   を使い、production `AudioBufferViewMut` を受け取る output callback が
   absolute render-frame cursor から fill する。callback 内で allocation・
   blocking・I/O を行わない。
3. capture は production `negotiate_input_format` / `real_input_config` /
   `InputSession` / `InputAdapter` / `RealDeviceRig` / 実 `StreamWriter` /
   journal/staging / `commit_session` / `DirectoryBundleStore` save / 別
   `ProjectController` load を通る。stereo capture を mono tap 2 本へ分ける
   multi-track 構成を少なくとも 1 つ通す。
4. virtual cable の OS SRC・latency を前提に、render stimulus と capture PCM
   の bit-exact 一致は Pass 条件にしない。信号内容は marker 間隔・個数・
   RMS/level band・非 finite sample を独立 oracle で判定し、journal の
   bytes/CRC/`verified_sha256` は記録媒体の内部整合として独立再計算する。
5. commit→save→reload 後に manifest/source/asset digest chain、entity 数、
   `AlreadyApplied` recommit を照合する。
6. endpoint pair 不在・host 列挙失敗は NotRun、候補 pair があるのに
   open/start/callback/oracle/commit/reload が失敗した場合は Fail として
   区別する。RunMetadata 風 JSON を terminal output と一時領域へ出す。
7. feature に依存しない pure/unit test が Ruviform stem matcher、他社 virtual
   cable/物理名 reject、RunMetadata round-trip、NotRun/Fail 分類を既定 laneで
   回帰検証する。
8. `mise run test-recording-virtual-endpoint` は opt-in lane であり、既定
   `mise run test synvelis-recording` に provisioned driver を要求しない。
9. 物理 real-input lane を PCM fidelity Pass 権威へ昇格しない。deterministic
   source 検証は本 virtual endpoint harness に分離されたことを probe 出力へ
   反映する。

## Non-goals

- driver package の install/provisioning、管理者権限を要する OS 設定変更
- CI workflow/job 追加、cross-repo artifact download、driver retention 運用
- `Cargo.toml`/`Cargo.lock`/ruviform pin の変更
- Punch/Monitor/Arm 全 case、reconnect/xrun/permission fault injection、
  cross-clock multi-device、物理 USB 抜去、ADC/DAC 特性
- 物理 device lane や fake/deterministic-backend lane の Pass を EXT-L3 Pass
  へ転用すること
