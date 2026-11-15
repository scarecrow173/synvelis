---
id: "DESIGN-6b7yO4AkTMb19oy5Wz3k1L"
type: "design"
status: "approved"
title: "Issue #115 rf#628 record-after-recovery Design"
created: "2026-11-16"
updated: "2026-11-16"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-115-rf628-record-after-recovery.md"
  implemented-by:
    - "docs/plans/issue-115-rf628-record-after-recovery.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-115-rf628-consumer-oracle.md"
    - "docs/designs/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-rf628-record-after-recovery.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 record-after-recovery Design

## 判断

SV-7 の核心は「re-open した session で record する」こと。候補は
2 つあった。

- **(a) 手組み rig**: hook で persistent open error →
  `InputSession::open` rejection → clear → 手動 `InputSession::
  open` で `(session, ends)` を取得し、`RealDeviceRig` の pub
  field を手で組み立てる。`ends`→`StreamWriter` 変換と
  `StagingArea::create` の stream 設定は `tests/common/mod.rs`
  の private `open_rig_core`/`staging_streams` が担うため、
  test file 側に ~50 行の複製が必要になる。private 実装の
  ドリフト温床であり、rig 構築経路（本番に近い側）を迂回する。
- **(b) pre-open hook（採用）**: `rf628_record` に rig open 直前
  の hook を追加し、hook 内で rejection→clear までを実行。
  `RealDeviceRig::open` → `open_rig_core` → `InputSession::open`
  が recovery 後の re-open を兼ね、その session がそのまま
  record に使われる。`open_rig_core` は `&dyn Device` 経路で
  既に正しく（PR #486 design 判断どおり）、staging/writer 構築を
  一切複製しない。

(b) を採用する。re-open は文字通り `InputSession::open` の
呼出しであり、同じ `ScriptedDeviceHandle`（`Clone` で state
共有）・同じ `InputStreamSpec` に対して error clear 後に成功
する — SV-5c が手動 open で確認したのと同じ経路が、本番
rig 構築の中でそのまま再現される。

SV-8 は既存 helper のみで実現可能 — `channels: 2`・
`contiguous_scenario(2, …)`・`taps: [(0, mono(0)), (1, mono(1))]`
は SV-2（4ch→mono(1)/mono(3)）と同構造で、`validate_spec` の
channel range 検査（`src < layout.channels()`）と backend の
scenario/format 整合検査（fixture channels == negotiated
channels）を共に通る。新規 helper・新規 assertion は不要。

## 主要決定

- **D1 hook 位置**: `rf628_record` の device/spec 構築直後・
  `RealDeviceRig::open` 直前に `pre_open(&devices, &specs)` を
  1 度呼ぶ。`StagingArea::create` は rig open 内で session open
  より先に走るが device に触れないため、hook→rig open の順序で
  open fault のライフサイクルを完全に制御できる。
- **D2 既存呼出しの非回帰**: `rf628_record` は
  `rf628_record_with_pre_open(…, |_, _| {})` へ委譲する薄い
  wrapper とし、既存 7 呼出し（SV-1〜SV-5d）の signature・
  挙動を変えない。
- **D3 rejection 分類の再利用**: hook 内の失敗側検証は SV-5c と
  同一 — `set_open_input_error(Some(DeviceError::Backend(
  "permission_denied")))` → `InputSession::open` が
  `Err(InputBindError::Device(_))` → `set_open_input_error(None)`。
  recovery 専用の新経路・新 assertion を発明しない。
- **D4 oracle 厳密さの踏襲**: SV-7 の record 後検証は SV-1 と
  同じ — `assert_finish_complete` → `rf628_commit_reload`
  （`CommitOutcome::Applied`・`sources_committed == 1`・
  `recoverable` 空）→ `assert_scenario_stream`（journal media の
  segment 単位 `pcm_reference_block` + `pcm_crc32` 検算 +
  `verified_sha256` `sha2` 再計算 + manifest↔project digest 連鎖）
  を commit controller と reload controller の両方で実施 →
  reload 後 recommit が `AlreadyApplied`。
- **D5 SV-8 構成**: `Noise` pattern（channel ごとに独立な
  (seed, channel, frame) hash）で L/R tap の PCM 分離を digest
  差で立証。`assert_ne!(media_path(0), media_path(1))` で別
  staging media、両 stream で `assert_scenario_stream` +
  `AlreadyApplied` 冪等まで SV-2 と同じ監査を行う。
- **D6 honesty**: scripted-backend の Pass は deterministic-
  backend headless lane のみ。rf#629/physical/GUI/real-input
  lane の verdict を変更しない。production `src/`・
  `tests/common/mod.rs`・依存・pin は非変更。

## リスク

| Risk | 対応 |
| --- | --- |
| hook が rig open より後に呼ばれ open fault が効かない | `pre_open` を `dyn_inputs` 構築前・`RealDeviceRig::open` 前に配置。失敗時は `.expect("session open")` が panic して test が落ちるため silent pass しない |
| 失敗した `InputSession::open` が device 側に残留 state を残す | backend `open_input` は open fault を Phase A で先に評価し `d.open_input` を触らずに return（`stream.rs` 実測）。persistent error は clear まで全 open を拒否するだけで消費されない |
| SV-8 で stereo 両 channel の PCM が一致し分離を立証できない | `Noise` は channel 次元を持つ hash（SV-2 先例）のため ch0/ch1 で bit 単位に異なる。`assert_ne!(digest)` で直接確認 |
| `rf628_record` refactor が既存 test を壊す | wrapper は no-op hook 委譲のみ。`mise run test synvelis-recording` で全 11 test の非回帰を確認 |
