---
id: "EXP-sf05-residual-coverage-audit"
title: "SF-05 residual headless 被覆監査 — 実験ログ (synvelis#108)"
created: "2026-11-14"
updated: "2026-11-14"
---

# SF-05 residual headless 被覆監査 — experiment log

## E1. 監査の進め方

Issue #108 の受入条件 SF-UX-01〜07 を、merge 済み slice1..3 の
正本文書（spec/design/plan/task/test-spec）と既存テスト
（`crates/synvelis-workspace/tests/sf_05.rs`・`sf_05_s2.rs`、
`crates/synvelis-app/tests/sf05_signal_flow.rs`・
`sf05_probe_share.rs`）へ写像した。監査は「各受入節の sub-condition
がどの既存テストまたは未被覆分岐に対応するか」を先に固定し、
重複テストを追加しない方針で進めた。

結果: SF-UX-01（hierarchy 投影・二重 edge なし）・SF-UX-02
（visual-only 不変）・SF-UX-05（Configured..Observed と
Unavailable..Unsupported の状態分離）は `sf_05.rs`・
`sf05_signal_flow.rs` で十分に被覆。SF-UX-06（probe point 共有・
capture dedup）は `sf05_probe_share.rs` で被覆。

未被覆として残ったのは SF-UX-03/04/07 の以下 6 分岐:

- concurrent window revision（2 view が同一 revision に fixate
  した gesture の後着が retarget せず `Stale` になること）
- capability-generation staleness（plugin port reconfigure 相当の
  generation drift で confirm が fail-closed になること）
- late receipt（Applied 終端後の gesture API 呼出しが `NoGesture`
  で fail-closed になり slot が解放されること）
- input-context 喪失（Keyboard/IME/PluginGui/Modal への遷移で
  gesture が drop され focus channel が誤転送されないこと）
- dock/window move・pool roundtrip・view close での Signal Flow
  state 保持/破棄の境界
- visual layout の project semantic diff 除外の明示的証明
  （`compare_snapshots` の `Equal`）

## E2. concurrent-window / capability-generation テストの設計

`routing_gesture_begin` が fixation に `revision`・
`capability_generation`・`window` を記録し、`confirm` が
window → capability → CAS revision の順で検査する実装を確認。
2 window fixture は `create_window` × 2 + view × 2 で構成し、
各 gesture が独立に合法な route（bass→kick、kick→vox）を
fixate するよう 3 track fixture を用意した。先着 `Applied` で
revision が進み、後着は revision CAS で `Stale` — これが
「Concurrent window revision」で要求される「Target swap なし・
成功 1 Undo」の headless 証拠になる。

capability-generation は public API のみで「gesture 中に
`InsertProcessor`→`RemoveProcessor`→`InsertProcessor`（
`capability_revision` bump）」を行い
`port_capability_generation(project)` が fixation 値から動く
ことを assert してから confirm を呼ぶ。revision も同時に進むが
検査順により capability 分岐が proximal cause であり、観測
契約（`Stale`・project 不変）は同一。実 plugin の dynamic port
再構成そのものは EXT-L3 の外部 Gate に記録した。

## E3. late receipt / input-context / lifecycle テストの設計

late receipt は `routing_gesture_*` の 4 API（hover/drop/confirm/
cancel）を Applied 終端後に呼び、全て `WorkspaceError::NoGesture`
を返すこと・project/revision が不変なこと・新規 gesture が
開始できることを検証。Cancelled 終端は既存テストが部分被覆する
ため Applied 終端に絞った。

input-context は `WorkspaceManager::set_input_context` の
View→非 View 遷移が進行中 gesture を全 drop する契約を利用。
`InputContext::{TextInput,Ime,PluginGui,Modal}` の 4 variant で
反復し、各遷移後に gesture 不在・late confirm `NoGesture`・
project 不変・3 focus channel（editing selection・arrangement
inspection・Signal Flow inspection）の非転送を確認した。
実 OS の keyboard/IME/plugin GUI focus event は headless では
再現できないため GUI-L2 として分離記録。

lifecycle は `ViewInstanceId` key の `ViewState.signal_flow` が
dock move・`detach_to_pool`・`reattach` を越えて保持され、
`remove_view`（ReleaseViews 相当）で view と共に破棄される
境界を検証。gesture 中の `remove_view` で preview が project に
波及しないことも同時に固定した。

## E4. semantic diff 除外テストの設計

`compare_snapshots`（`ComparisonSpec::new` + 既定
`ComparisonDomains`）で `capture_live` の `ImmutableSnapshot`
before/after を比較する。visual mutation は `state_mut` 経由で
`SignalFlowVisualState` の全 surface（positions/groups/fold/
zoom/pan/link_focus/scope）を適用し、inspection も
`signal_flow_inspect`/`signal_flow_set_inspection_mode` で変更。
`ComparisonEquality::Equal`・`summary.total == 0` を確認後、
`SetProjectName` を適用して diff が出ることを陽性対照とした。
これにより「visual state が project snapshot に構造的に含まれ
ない」ことを空虚でない oracle で固定した。

## E5. src 変更を行わなかった判断

監査で `routing_gesture_*`・`signal_flow_*`・`set_input_context`・
dock/pool/remove_view・`compare_snapshots` の contract 違反は
見つからなかった。全て既存実装が要求契約を正しく実現しており、
テスト追加のみで証拠を固定できた。編集範囲の制約
（`crates/synvelis-project/` read-only）とも整合する。
コード差分は `crates/synvelis-workspace/tests/sf_05_s2.rs` 1 件
＋文書/evidence のみ。
