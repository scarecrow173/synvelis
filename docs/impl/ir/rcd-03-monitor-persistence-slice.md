---
id: "IMPL-034VpRmK9xT2wQ8nB4vC6d"
type: "impl"
status: "completed"
title: "RCD-03 Arm/Monitor/Punch Project 永続投影 slice 実装記録"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/monitoring_data.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/monitoring_persist.rs"
      - type: "file"
        path: "docs/specs/rcd-03-monitor-persistence.md"
      - type: "file"
        path: "docs/designs/rcd-03-monitor-persistence-design.md"
      - type: "file"
        path: "docs/plans/rcd-03-monitor-persistence-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-03-task1-monitor-persistence.md"
      - type: "file"
        path: "docs/test-specs/rcd-03-monitor-persistence.md"
      - type: "file"
        path: "docs/impl/ir/rcd-03-monitor-persistence-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
      - type: "file"
        path: "crates/synvelis-app/src/monitoring.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/monitor_arm.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
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
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-03-task1-monitor-persistence.md"
    - "docs/plans/rcd-03-monitor-persistence-plan.md"
    - "docs/designs/rcd-03-monitor-persistence-design.md"
    - "docs/specs/rcd-03-monitor-persistence.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-03-task1-monitor-persistence.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-03-app-arm-monitoring-ux-slice.md"
    - "docs/impl/ir/rcd-03-monitoring-punch-latency-crate-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitor-persistence.md"
    - "docs/designs/rcd-03-monitor-persistence-design.md"
    - "docs/plans/rcd-03-monitor-persistence-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-03-monitor-persistence.md"
  verified-by:
    - "docs/test-specs/rcd-03-monitor-persistence.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-03 Arm/Monitor/Punch Project 永続投影 slice 実装記録

## Summary

Issue #117（RCD-03 Monitoring / Punch / Latency）の残 headless
スライスとして、authored monitor 宣言（Arm/Monitor mode・
channel_map・lane）と punch marks を `synvelis-project` の永続
モデルへ POD 投影し、schema を v11 → v12 へ進めた。
`synvelis-app` の `syn.monitoring.set`/`punch` は宣言変更を
`ProjectOp`（`SetTrackMonitor`/`ClearTrackMonitor`/
`SetPunchMarks`）として `ProjectSession::apply` 経由で
write-through し、`MonitorUxState` は session 観測値
（`input_output_ready`/`hardware_direct`/稼働 session/latency
注入）のみを保持する read-through 構成へ再編した。
session 観測値は永続化しない契約を維持し、
`project.new`/`project.open` は観測値のみ reset して load 済み
宣言をそのまま投影する。route/gate/latency の意味論は引き続き
`synvelis-recording`/`synvelis-player` が所有し、app/project 側に
複製しない。本 slice は Issue #117 を Close しない
（実 Device/Capture/Software Monitor・実 GUI・#114 Scenario・
MonitorAdapter の実 RT graph 搭載は外部 gate）。

## Context

- spec `docs/specs/rcd-03-monitor-persistence.md`
- design `docs/designs/rcd-03-monitor-persistence-design.md`
- plan `docs/plans/rcd-03-monitor-persistence-plan.md`
- test-spec `docs/test-specs/rcd-03-monitor-persistence.md`
- task `docs/tasks/rcd-03-task1-monitor-persistence.md`

先行 slice（`rcd-03-app-arm-monitoring-ux-slice.md`）が
`syn.monitoring.*` op・`MonitorUxState`・`decide`/`MonitorProbe`/
`preflight`/`LatencyReport` 消費を session-only として確定済みで、
「永続化・Undo 対象化」を deferred として残していた。本 slice は
その deferred 分のみを回収する。`synvelis-recording`/
`synvelis-player`/`synvelis-workspace`/`synvelis-library`/`ui/` は
無変更。`synvelis-gateway` は新規 `ProjectOp` variant が
`op_name` の網羅 match を破壊するため、Coordinator 承認済みの
範囲拡張として命名 arm 3 件のみ追加した。

## Implementation

### Changes

- `crates/synvelis-project/src/monitoring_data.rs`（新規）:
  `ArmState`/`MonitorState`（recording と同名・同値・別型の POD
  コピー — 依存方向 `project ← recording` で型共有不可）、
  `MonitorChannelMap`（非空検査は参照 oracle が権威、default は
  stereo identity `[0,1]`）、`TrackMonitorDecl`、`PunchMarks`、
  `MonitoringData`（`BTreeMap<TrackId, TrackMonitorDecl>` +
  project 全体 punch）。全て serde derive + `deny_unknown_fields`。
- `crates/synvelis-project/src/project.rs`: `monitoring` field
  （`#[serde(default)]` で v11 body 互換）、`Project::new` 初期化、
  `validate_references` に宣言 track 存在・lane 帰属・channel_map
  非空の検査を追加。
- `crates/synvelis-project/src/store.rs`: `SCHEMA_VERSION` 12 +
  履歴コメント。
- `crates/synvelis-project/src/migration.rs`: STEPS 末尾に
  `from: 11` の identity step（`#[serde(default)]` が既に正しい
  空値を与えるため実質 no-op。`migrate_v8_smf_documents` と同型で
  version 前進の監査証跡）。
- `crates/synvelis-project/src/ops.rs`: `SetTrackMonitor`
  （full-decl upsert — patch 型を避け app が畳んで送る）、
  `ClearTrackMonitor`、`SetPunchMarks`（verbatim 置換）。
  `SetTrackMonitor::validate` は track 存在・lane 帰属・channel_map
  非空を検査。`RemoveTrack::apply` は decl を cascade 除去
  （undo は snapshot 復元で戻る）、`RemoveLane::validate` は
  monitor 宣言参照中の lane を拒否。
- `crates/synvelis-project/src/lib.rs`: `monitoring_data` module +
  型 export。
- `crates/synvelis-project/tests/monitoring_persist.rs`（新規、
  10 test）: op upsert/clear・validate 原子性・punch verbatim・
  undo/redo・save/load round trip・v11→v12 migration・
  RemoveTrack cascade・RemoveLane 拒否・`validate_references`
  oracle。
- `crates/synvelis-project/tests/md_a.rs`・`tim_02_smf.rs`:
  literal `SCHEMA_VERSION == 11` pin を 12 へ更新（schema bump の
  既存慣行どおり）。
- `crates/synvelis-app/src/monitoring.rs`: `MonitorUxState` を
  session 観測値のみに再構成（`track_sessions` overlay・
  `session`・`latency`・`low_latency`・`tone_difference`）。
  宣言・punch の保持 map を廃止し `project.monitoring` を
  read-through。`merged_decl` は persisted field
  （`lane`/`arm`/`monitor`/`channel_map`）がある場合のみ宣言を
  畳む — session 観測 field のみの patch は `None` を返し
  default decl を生やさない（save/load 後に幻の宣言が残るのを
  防ぐ）。`contexts()`/`state()` は decl ∪ overlay の和集合を
  `BTreeSet` で走査し、宣言 field は project（無ければ default +
  `declared:false`）、観測 field は overlay から投影。
  `start_session` は宣言・lane・channel_map・punch を
  `project.monitoring` から読み `preflight`/`GateSpec` 構成は不変。
- `crates/synvelis-app/src/ui_ops.rs`: `op_monitoring_set` は
  persisted field が宣言を変える場合のみ `SetTrackMonitor` を発行
  （同一値は revision を消費しない）、成功後に観測 field を overlay
  へ書く（op 失敗時は overlay も不変）。`clear:true` は decl が
  あれば `ClearTrackMonitor` + overlay 除去、overlay のみなら
  op を発行しない。`op_monitoring_punch` は `require_no_session`
  通過後に project punch へ action を畳み `SetPunchMarks` を発行
  （session active 中の拒否は従来どおり app 層の制約）。
  param 解析は `monitoring_track_patch` helper へ抽出。
- `crates/synvelis-app/src/app.rs`: `project_new`/`project_open` で
  `MonitorUxState::default()`（観測値 reset）のコメントを
  read-through 契約へ更新 — 宣言・punch は load 済み project が
  そのまま正本。
- `crates/synvelis-app/tests/monitor_arm.rs`: 11 → 14 test。
  write-through + undo、save/open rehydrate、session active 中の
  punch 拒否、観測値非永続を追加。`monitoring_ops_do_not_mutate_
  project` は session 観測値のみが revision を消費しない契約へ
  更新。
- `crates/synvelis-gateway/src/prepare.rs`: `op_name` に
  `set-track-monitor`/`clear-track-monitor`/`set-punch-marks` の
  arm を追加（Coordinator 承認済み範囲拡張 — 追加しないと
  workspace `cargo check` が E0004 で失敗する）。
- `docs/` チェーン 5 文書 + 各 README index + 本 impl record。

### Decisions During Implementation

- authored 宣言のみを永続化し、session 観測値
  （`input_output_ready`/`hardware_direct`/latency/稼働
  session/latency 注入）は `MonitorUxState` に残す。
  `Project::monitoring` は外部観測・実行時計測を持たない
  authored intent のみを保持する。
- project POD は recording 型を参照しない別型として定義し、
  app 境界で `ArmState`/`MonitorState`/`ChannelMap` へ写像する
  （依存方向 `project ← recording`）。wire/serde 語彙は同一を維持。
- `SetTrackMonitor` は patch 型ではなく full-decl upsert。
  app が現在 decl（無ければ default）に patch を畳んで送るため、
  `Option<Option<LaneId>>` の入れ子を避け Composite 合成も
  「最終形を構築してから積む」明確な意味論になる。
- session 観測 field のみの patch は宣言を生やさない。
  旧 session-only 実装は任意の set で decl エントリを作っていたが、
  永続化後にそのまま真似ると観測値の set が save/load 後に幻の
  宣言として残る。宣言無し track の可視性は `state()` の
  decl ∪ overlay 和集合で維持する（投影される track 集合は旧実装と
  同一）。
- `session_json` の `phase`（`punch_wait`/`punch_open`/
  `post_roll`）は live の `project.monitoring.punch` ではなく
  session 開始時に `plan.gate` へ確定したマークと比較する。
  稼働中の `syn.project.undo` で project punch が変わっても
  表示が凍結済み gate と乖離しない（review で発見・修正）。
- `RemoveTrack` は decl を cascade 除去、`RemoveLane` は宣言参照中
  の lane を拒否。`validate_references` が load 経路の最終防衛線
  として同じ不変条件を強制する。
- VCS semantic diff（`diff/engine.rs`）への monitor 宣言分類追加は
  本 slice 対象外 — diff は curated projection のため未列挙でも
  破損しない。follow-up として spec/design に記録。

## Related Experiments

### Adopted

<!-- 実験 log なし（既知解の機械的実装 + fixture 作成）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、Windows 11 x64・
rust 1.99.0 pinned、`git merge origin/main` 取り込み後）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`、exit 0）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`、exit 0）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  exit 0。`monitor_arm.rs` 14 test・`monitoring_persist.rs`
  10 test を含む全 workspace 緑）
- `mise run test-rt` — Pass（`cargo test --workspace --locked
  rt_`、exit 0）
- `mise run check-app-native` — Pass（`synvelis-app --features
  native`、exit 0。依存 `proc-macro-error2` の future-incompat
  warning は既存・本変更と無関係）
- `mise run lint-app-native` — Pass（exit 0、同上 warning のみ）
- `mise run test-msrv` — Pass（`mise exec rust@1.89.0 -- cargo
  test --workspace --locked`、exit 0）
- `mise run build` — Pass（`cargo build --workspace --locked`、
  exit 0）
- `mise run sync-ui-deps` + `mise run check-ruviform-rev` —
  Pass（merge で取り込んだ ruviform pin `b02942c2` へ vendor
  再同期、REV match 確認）

補助証拠（canonical = mise、直接 cargo は補助）:
`cargo test -p synvelis-app --test monitor_arm` — 14 passed、
`cargo test -p synvelis-project --test monitoring_persist` —
10 passed。

検証中に観測した非 scope 事象（いずれも再実行で緑・本変更と
無関係と判断）:

- `synvelis-library/tests/webdav.rs::browse_cursor_pages_listing`
  が初回 `mise run test` で `os error 10053`（socket abort）に
  より 1 度失敗。単独再実行・workspace 再実行とも pass —
  ローカル socket の一過性 flake。
- `synvelis-player/tests/sf03_l2.rs` が初回 workspace 実行で
  `memory allocation failed`/`STATUS_STACK_BUFFER_OVERRUN` により
  1 度異常終了。単独再実行・workspace 再実行とも pass —
  大規模並列 test 実行時のリソース逼迫と判断。
- `md_a_05`/`smf_document_survives` の literal schema pin
  （`== 11`）は本 slice の v12 bump が直接原因のため修正済み
  （上記 Changes 参照）。

review-protocol（worker 契約に基づく承認代替）: 実施 1 回。
Reviewer A（spec 適合）/ Reviewer B（敵対的正当性）の分離 pass で
Issue Candidate を洗い、確認済み指摘を修正した:

- CONFIRMED P2 相当: `session_json` の phase が live project
  punch を参照し稼働中 undo で gate と乖離し得た → `plan.gate`
  凍結値へ修正。
- CONFIRMED P3 相当: session active 中の punch 拒否に専用 fixture
  が無かった → `punch_rejected_while_monitor_session_active`
  追加。
- CONFIRMED P2 相当: literal schema pin 2 件が v12 で失敗 →
  `mise run test` で検出・修正。
- その他の候補（overlay 残存 track の投影、session-only patch の
  default decl 生成、ClearTrackMonitor の no-op 意味論、
  `#[allow]` 使用、serde 形状）は実装・既存慣行と照合し
  REJECTED/non-issue と判定。

残 blocker なし — verdict ACCEPT。

NotRun / Unsupported（test-spec/spec 宣言どおり、headless
fixture は以下を証明しない）:

- 実 Device/Capture/Software Monitor の可聴計測・実 PCM 経路 —
  NotRun（実機 gate・後続 slice）
- 実 GPUI GUI・multi-window/focus 受入・#114 Scenario —
  NotRun（外部 gate）
- `MonitorAdapter` の実 RT graph monitor node 搭載・low-latency
  route の実 Prepared Route 構築（#106/#115/Ruviform#251 の
  統合契約依存）— 対象外
- Loop/PunchInLoop gate・count-in/pre-roll UX — 対象外
- VCS semantic diff への monitor 宣言分類 — follow-up
  （diff engine は curated projection のため未対応でも破損しない）

## Risks

- `syn.monitoring.state` は decl ∪ overlay の和集合を毎回
  project から走査する。宣言 track 数は mixer 規模で bounded、
  `PROBE_TRACK_CAP`/`SEGMENT_CAP` の既存 cap がそのまま効く。
- `RemoveTrack` の cascade は decl を暗黙に消すが undo で復元
  される。`validate_references` が crafted snapshot の孤立 decl
  を拒否するため不変条件は維持される。
- session 観測 overlay は track 削除時に自動除去されない。
  `state()` は `in_project:false` で正直に投影するため幻の
  route は捏造されないが、overlay エントリ自体は session
  reset（project.new/open）まで残る — 旧 session-only 実装と
  同じ挙動であり回帰ではない。
- v12 snapshot を v11 build で読むことは `deny_unknown_fields`
  で拒否される意図的な前方非互換（contract_change として
  spec/design に記録済み）。

## Follow-ups

- Issue #117 残件（本 slice で Close しない）: 実
  Device/Capture/Software Monitor/GUI 実測、実 GPUI GUI・
  multi-window/focus 受入（#114）、MonitorAdapter の実 RT graph
  搭載、low-latency route の実 Prepared Route 構築、#115/#116
  連携。
- VCS semantic diff（`diff/engine.rs`）への monitor 宣言分類
  追加 — authored 宣言が diff に現れない follow-up。
- `synvelis-library` webdav / `synvelis-player` sf03_l2 の
  環境依存 flake 観測（本 slice 範囲外・記録のみ）。
