---
id: "DESIGN-sv115VirtualEndpointRcd01"
type: "design"
status: "approved"
title: "Issue #115 RCD-01 Consumer Virtual-WASAPI EXT-L3 Harness 設計"
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
    modified:
      - type: "file"
        path: "crates/synvelis-recording/tests/real_input.rs"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-115-virtual-endpoint.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-115-virtual-endpoint.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-virtual-endpoint.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-115-virtual-endpoint.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/ruviform/issues/1001"
  defers: []
  deferred-by: []
---

# Issue #115 RCD-01 Consumer Virtual-WASAPI EXT-L3 Harness 設計

## 1. 責務境界

本 harness は test-local stimulus pump と本番 Recorder capture の 2 面を持つ。

- output 側は製品入力経路ではなく stimulus pump なので、test-local output
  stream を開く。callback は RT 境界であり、allocation・blocking・I/O・panic
  を行わず、atomic cursor と事前構築済み `PcmFixture` だけで fill する。
- input 側は production `negotiate_input_format` / `real_input_config` と
  `RealDeviceRig` を通し、`InputSession`/`InputAdapter`/実 `StreamWriter`/
  journal/staging/project commit の本番経路を使う。test が生 capture buffer
  のみを観測して終わる fake harness にはしない。
- production `src/` は変更しない。endpoint negotiation や multi-track tap 構成は
  既存 test helper と production API で組み、ruviform の恒久 API 変更も行わない。

## 2. Endpoint 候補化と fail-closed 分類

OS enumeration direction（render list / capture list）を権威とし、名前は候補化
だけに使う。stem matcher は `ruviform` を含み、かつ `cable` または
`virtual audio device` を含む名前だけを許可する。`input`/`output` 等の
direction token を除いた stem が一致する render/capture candidate を pair 化する。
VB-CABLE・Voicemeeter・物理 device 名は reject する。

分類は次の通り。

- host 作成または device 列挙が失敗: provisioning 状態を判定できないため
  `result=not-run`。
- Ruviform stem pair が 0 件: `result=not-run`・`NOT_PROVISIONED`。
- pair 候補が存在し、F32 format 交渉・open/start/callback/oracle/commit/reload
  が失敗: `result=fail`。
- pair のいずれかが全 oracle を通過: `result=pass`。

## 3. Stimulus と RT callback

stimulus は `PcmPattern::Impulse`（amplitude 0.8、100ms 周期）を使う。
impulse は OS SRC に対して正弦波 bit pattern より頑健で、marker 間隔・個数・
RMS band を独立に検算できる。output callback は callback ごとの local frame
位置に依存せず、stream 開始からの absolute render-frame cursor を atomic に
進めて `PcmFixture::fill_view` へ渡す。callback 観測（channel mismatch、
timestamp regression、fill/error 回数）は atomic counter にのみ記録する。

## 4. Capture・multi-track・commit/reload

capture endpoint は production negotiation の結果を使う。session frame 数と
`plan_at` の rate には共通 test helper の固定 rate ではなく negotiated input
rate を使う。stereo 以上の input channel を `ChannelMap::mono(0)` と
`ChannelMap::mono(1)` の 2 tap に分け、2 track/2 lane/2 stream に記録する。
これにより 1 入力 stream から複数 track への分離と、別 staging media・別
journal verified prefix を検証する。

capture input start → render output start → bounded pump → output stop → input
stop → finish の順で駆動し、watchdog と bounded duration で hang を防ぐ。
finish 後は各 stream で journal coverage/source contiguity/integrity を確認し、
`commit_session` が `Applied`、reload 後 recommit が `AlreadyApplied` である
ことを確認する。

## 5. Oracle の分離

virtual cable には OS SRC と clock/latency 差があるため、render stimulus と
capture PCM の bit-exact 一致は要求しない。oracle は 2 層に分ける。

1. 信号内容 oracle: marker 個数（8 個以上）、平均 marker 間隔 100ms±10%、
   最初の marker latency、RMS/level ratio、非 finite sample、output callback
   contract。
2. 記録媒体 oracle: journal span/CRC/verified prefix、`verified_sha256` の
   独立 SHA-256 再計算、commit manifest/source/asset digest chain、
   save→reload 後の entity 整合。

この分離により、SRC を通る EXT-L3 の信号観測と、Synvelis 記録媒体の内部
整合を混同しない。

## 6. RunMetadata artifact

terminal 状態ごとに `run_metadata:` JSON を stderr へ出力し、
`CARGO_TARGET_TMPDIR` が利用可能なら JSON file も書く。vocabulary は
`ruviform-audio-test` の Case Schema v1 に合わせ、`case_id`/`requirement`/
`result`/`level`/`environment_kind`/`oracle`/`parameters`/`repo`/`sha`/`os`/
`arch`/`expected`/`observed`/`evidence` を記録する。repo/SHA は env または git
metadata から取得し、取れない場合は `unknown` とする。
