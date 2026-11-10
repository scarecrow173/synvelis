---
id: "IMPL-034Vg2bnZtbJvdYWRJIUM3"
type: "impl"
status: "completed"
title: "RCD-03 app Arm/Monitoring UX slice 実装記録"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/src/monitoring.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/monitor_arm.rs"
      - type: "file"
        path: "docs/specs/rcd-03-app-arm-monitoring-ux.md"
      - type: "file"
        path: "docs/designs/rcd-03-app-arm-monitoring-ux.md"
      - type: "file"
        path: "docs/plans/rcd-03-app-arm-monitoring-ux.md"
      - type: "file"
        path: "docs/test-specs/rcd-03-app-arm-monitoring-ux.md"
      - type: "file"
        path: "docs/tasks/rcd-03-task2-app-arm-monitoring-ux.md"
      - type: "file"
        path: "docs/impl/ir/rcd-03-app-arm-monitoring-ux-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "Cargo.lock"
  implements:
    - "docs/tasks/rcd-03-task2-app-arm-monitoring-ux.md"
    - "docs/plans/rcd-03-app-arm-monitoring-ux.md"
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-03-task2-app-arm-monitoring-ux.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-03-monitoring-punch-latency-crate-slice.md"
    - "docs/impl/ir/rcd-01-audio-device-headless-slice-fake-port-streamwriter-fixture-recording-stat.md"
    - "docs/impl/ir/rcd-02-project-library-adapter-verified-sha-256-mediareferenceprobe-saved-head.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
    - "docs/designs/rcd-03-app-arm-monitoring-ux.md"
    - "docs/plans/rcd-03-app-arm-monitoring-ux.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-03-app-arm-monitoring-ux.md"
  verified-by:
    - "docs/test-specs/rcd-03-app-arm-monitoring-ux.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-03 app Arm/Monitoring UX slice 実装記録

## Summary

Issue #117（RCD-03 Monitoring / Punch / Latency UX）のうち
`synvelis-app` が所有する headless な Arm/Monitoring UX・状態投影
slice を実装した。`syn.monitoring.set|punch|transport|session|
latency|state` の 6 op を `syn.*` サービス面へ追加し、
session-scope の monitor 宣言（Arm/Monitor mode/channel binding/
`input_output_ready`/Hardware Direct 観測）・monitor session
（`synvelis-recording::preflight` が確定する `PreparedRecordingPlan`
+ transport/punch マーク）・latency 注入点を保持・投影する。
route/gate/latency の意味論は recording/player crate の
`decide`/`MonitorProbe`/`LatencyReport::resolve`/
`prepare_low_latency` を消費し、app 側で再実装しない。
宣言・session・punch・latency は全て session-only であり、
`Project`/revision/Undo/capture/writer/file bytes を一切変更
しない（`project.new`/`open`/`reload` で reset）。
実 Device/Capture/Software Monitor PCM・実 GUI/multi-window/
focus 受入は対象外（NotRun — 外部 gate）。本 slice は Issue #117
を Close しない。

## Context

- spec `docs/specs/rcd-03-app-arm-monitoring-ux.md`
- design `docs/designs/rcd-03-app-arm-monitoring-ux.md`
- plan `docs/plans/rcd-03-app-arm-monitoring-ux.md`
- test-spec `docs/test-specs/rcd-03-app-arm-monitoring-ux.md`
- task `docs/tasks/rcd-03-task2-app-arm-monitoring-ux.md`

先行 slice（`rcd-03-monitoring-punch-latency-crate-slice.md`、
PR #214 merge 済み）が `synvelis-recording` の decide/MonitorProbe/
HardwareDirect notice と `synvelis-player` の MonitorAdapter/
LatencyReport/low-latency readiness を確定済み。本 slice はそれらを
消費する app 側 UX op と読み取り投影のみを追加し、crate 層の
意味論は変更していない。`synvelis-player` は読み取り利用のみで
無変更、`synvelis-project`/`synvelis-workspace`/`synvelis-library`/
`mise.toml`/`evidence/` も無変更。

## Implementation

### Changes

- `crates/synvelis-app/src/monitoring.rs`（新規、約 900 行）:
  `MonitorUxState`（`BTreeMap<TrackId, MonitorTrackDecl>` 宣言 +
  `Option<MonitorSessionState>` + `PunchMarks` + `LatencyReport` +
  `LowLatencyReadiness` + `tone_difference`）を実装。
  `set_track`/`clear_track`/`punch_set`/`punch_mark`/`punch_clear`/
  `transport`/`start_session`/`stop_session`/`set_latency`/`state`
  を提供。`state()` は `contexts()` で宣言 track の
  `TrackMonitorContext`（strip 由来 muted/soloed、any_solo は
  宣言 track の strip 集合で評価 — design 記載の決定）を構築し、
  position→session frame 変換・`MonitorProbe` 全区間 feed
  （`push_segment_merged` で同一決定の隣接 segment を併合）・
  live route（`decide(ctx, Playing|Stopped, gate_open_now)`）・
  `latency_json`（`LatencyReport::resolve` +
  `prepare_low_latency` の verbatim 投影）を合成する。
  `gate_open_now` は position を含む segment の `gate_open` を
  採用し、session 外・停止中・未カバーは `false`（tick 空間で
  gate を再実装しない）。session 未開始は `session.active:false`
  ・空 segments・decide の Stopped 判定のみを返し捏造しない。
- `crates/synvelis-app/src/app.rs`: `monitoring` field 追加、
  `project_new`/`project_open`（`project_reload` は open 経由）で
  reset。
- `crates/synvelis-app/src/lib.rs`: `pub mod monitoring`。
- `crates/synvelis-app/src/service.rs`: 6 op の `OPS` 登録 +
  dispatch 配線。
- `crates/synvelis-app/src/ui_ops.rs`: 6 op ハンドラ +
  param helper（tick/tagged enum/`LatencyReport`/
  `LowLatencyReadiness` wire 変換）。malformed 値は
  `INVALID_PARAMS`。
- `crates/synvelis-app/Cargo.toml` + `Cargo.lock`:
  `synvelis-recording`・`ruviform-media-core` を workspace 依存に
  追加（ruviform rev は不変 `0f5b5737`）。
- `crates/synvelis-app/tests/monitor_arm.rs`（新規、11 test）:
  E1 route 行列・E2 punch 半開 segment・E2b 長尺 session
  gate_open regression・E3 Hardware Direct/二重監視・E4
  input_unavailable・E5 view/window/transport 不変・E6 latency/
  low-latency 投影・E7 未開始の正直投影・param 検証
  （foreign lane/未知 track/非 bool）・project 非破壊・
  service 登録。
- `docs/` チェーン 5 文書 + 各 README index + 本 impl record。

### Decisions During Implementation

- route/gate/latency の権威は recording/player crate に限定し、
  app は `decide`/`MonitorProbe`/`LatencyReport::resolve`/
  `prepare_low_latency` の呼出しと wire 変換のみを行う。
  `gate_open` は probe segment（scheduler 由来）から引き、
  tick 空間で gate を再実装しない。
- monitor session は `preflight` で plan を凍結し、以後の
  project/tempo 変更で再計算しない（録音 pass の binding 時
  凍結と同じ契約）。punch マーク変更は session active 中を
  `INVALID_PARAMS` で拒否（gate は開始時確定済み）。
  punch の順序・`stop > transport_start` 等の範囲検証は
  `preflight` が権威として行い、`syn.monitoring.punch` は
  マーク保持のみ（開始時に初めて gate へ写像される）。
- 宣言 lane は `track.lane_ids` 所属を要求する（review 指摘:
  他 track の lane を誤 bind すると session が別 track の
  入力を capture する）。
- segment 投影は `MonitorProbe` の feed block 粒度出力を
  `push_segment_merged` で同一決定（`gate_open`/`route`/
  `notices` 一致・半開連続）の隣接区間へ併合する。併合しないと
  単純 gate でも session 長比例の本数となり `SEGMENT_CAP=256`
  が実用 session（旧 feed 4096 frame で約 22 秒分）を打ち切り、
  cap 以遠の live route が `gate_open=false` に誤投影される
  （review で発見・修正。`FEED_BLOCK` は 2^24 frame に拡大し
  feed 回数も削減）。併合は gate 意味論の再実装ではなく
  decide 結果の verbatim 比較による表示上の区間統合。
- `any_solo` は宣言 track の strip 集合で評価する
  （design 記載の決定 — monitor session の solo 文脈を
  project 全 strip ではなく宣言集合に限定）。
- `syn.monitoring.latency` は session 非依存の注入点
  （headless — 実 device 層が値を提供するまでの seam）。
  `report` は全体置換、`readiness`/`tone_difference` は field
  patch。非 bool 等の malformed は `INVALID_PARAMS`。
- Hardware Direct は外部観測を verbatim に保持・投影し、
  `Unknown` を制御可能と偽らない（`hardware_direct_unknown`
  notice を decide 経由で常時投影）。DAW Mute/Solo でその値を
  変更しない。

## Related Experiments

### Adopted

<!-- 実験 log なし（既知解の機械的実装 + fixture 作成）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、Windows 11 x64・
rust 1.99.0 pinned）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  `monitor_arm.rs` 11 test を含む）
- `mise run test-rt` — Pass
- `mise run check-app-native` — Pass（`synvelis-app --features
  native`、依存 `proc-macro-error2` の future-incompat warning
  は既存・本変更と無関係）
- `mise run test-msrv` — Pass（rust 1.89.0、
  `cargo test --workspace --locked`）

補助証拠（canonical = mise、直接 cargo は補助）:
`cargo test -p synvelis-app --test monitor_arm` — 11 passed。

review-protocol（coordinator 契約に基づく承認代替）: 実施 1 回。
Reviewer A（spec 適合）/ Reviewer B（敵対的正当性）の分離 pass で
Issue Candidate を洗い、確認済み指摘を修正した。残 blocker なし
— verdict ACCEPT（non-blocking note のみ）。

NotRun / Unsupported（test-spec/spec 宣言どおり、headless
fixture は以下を証明しない）:

- 実 Device/Capture/Software Monitor の可聴結果・実 PCM 計測 —
  NotRun（実機 gate・後続 slice）
- 実 GPUI GUI・multi-window/focus 受入・AudioLens — NotRun
  （#114 系の外部 gate）
- RT graph への monitor node 搭載・low-latency route の実
  Prepared Route 構築・player RT 搭載 — 対象外（宣言 scope 外）
- #115 実 Input 同 Clock・#116 Writer/Recovery 連携 — 対象外
- Loop/PunchInLoop gate・count-in/pre-roll UX — 本 slice 非対応
  （`GateSpec` は投影で正直に名前を返すが構築は Normal/Punch のみ）
- Mock/headless fixture を実入力 Pass としない区分を維持

## Risks

- `syn.monitoring.state` は宣言 track ごとに `MonitorProbe` を
  全 session 分 drive する（非 RT・bounded）。`FEED_BLOCK` 拡大と
  segment 併合で実用 session では数 feed/track に収まるが、
  極端に長い session（time 符号的に巨大な stop）では反復回数が
  増える。`SEGMENT_CAP`/`PROBE_TRACK_CAP`/`tracks_omitted`/
  `truncated` で出力は bounded のまま。
- `position_session_frame` は `tick_to_media` 失敗・session 外
  position で `None` → `in_session:false`・`gate_open:false` の
  正直な投影に落ちる。seek は範囲無制限（session 外は投影上
  「範囲外」として扱う）。
- `any_solo` を宣言 track 集合に限定する設計決定は、project 内の
  非宣言 strip が solo でも宣言 track を mute しない。monitor
  session を独立文脈と見なす design 判断であり、実 GUI/mixer
  統合時に再検討の余地あり。
- punch 順序等の gate 検証は `syn.monitoring.punch` ではなく
  session start の `preflight` が権威 — マーク保持中は不正順序を
  許容し、開始時に `INVALID_PARAMS` で顕在化する（設計どおり）。

## Follow-ups

- Issue #117 残件（本 slice で Close しない）: 実
  Device/Capture/Software Monitor/GUI 実測、実 GPUI GUI・
  multi-window/focus 受入（#114）、RT graph monitor node 搭載、
  low-latency route の実 Prepared Route 構築、project/library
  への monitor/latency 永続投影、#115/#116 連携。
- `syn.monitoring.*` の wire 仕様は headless 契約 — 実 GUI shell
  が消費する際に op 面の見直しが必要になり得る。
