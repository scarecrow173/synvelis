---
id: "EXP-102UiCfInt007GgHhIiJjK"
title: "UI-C5 製品 GUI 統合 実験メモ (synvelis#102)"
created: "2026-11-14"
updated: "2026-11-14"
---

# UI-C5 製品 GUI 統合 — experiment log

## E1. `ControlEntityRef` の serde 不在への対処

`ControlEntityRef` は `synvelis-project` 所属で serde impl を持たず、
本 slice の編集範囲外。per-view focus の永続化には tagged 文字列
（`param-…`/`automation-lane-…`/`mod-source-…`/`mod-assign-…`）を
採用し、`control_entity_tag`/`control_entity_parse` で round-trip
させた。4 種の id parser は prefix で一意に確定するため順次試行で
曖昧性がない。malformed tag は `None` へ劣化し、entity を推測しない。

## E2. Explore 履歴の永続化境界

`capture()`/`restore()` は in-memory の typed struct をそのまま
受け渡すため serde attribute を通らず、`#[serde(skip)]` の
explore stacks も生き残る。永続化契約が意味を持つのは JSON 境界
（`to_json`/`from_json`）のみ — `signal_flow` の explore stacks と
同じ構造であり、テストも JSON 往復で固定した。当初 in-memory
restore で空を期待して 1 度失敗し、境界を確認して修正した。

## E3. `deny_unknown_fields` と tolerant decode の組み合わせ

`ControlRelationViewState` を `#[serde(default)]` のみで定義した
初版では `{"bogus": true}` が未知 field を無視して default へ
デコード成功し、malformed 劣化テストが失敗した。
`SignalFlowViewState` と同じ `#[serde(default, deny_unknown_fields)]`
に揃えて、未知 shape は decode error → `deserialize_control_relation`
の `Option` フォールバックで `None` へ落ちる契約に統一した。

## E4. `Prepare` state の構造的非到達

`recording_status` は `ProjectSession` のみを観測し、caller-held
`PreparedRecordingCommit` を受け取れないため `Prepare` は op 経由で
到達不能。これは module docs が明示する契約であり、無理に payload を
op state に持ち込まず、op doc に非到達を明記した。
将来 GUI が prepare→apply を駆動する場合は `recording_status_with`
相当の param を別途設計する。

## E5. outcome と error の分離

`control_edge_commit` の `Stale` は異常ではなく gesture の契約上の
終了形態。`Result<Value, ErrorBody>` の error にせず、応答の
`outcome` field に typed 値（`applied`/`cancelled`/`stale`）として
載せた。`RecordCommitError`（Conflict/Rejected 等）は真の失敗として
error detail に `Debug` verbatim で残す — typed rejection を
潰さない。

## E6. graph router の blocked 状態

`doc-driven-dev-graph` の route 実行は `artifact-graph` および
`docs/` 全体の既存 broken relations（defers/related/source）により
`blocked` を返した。本 slice 起因ではなく repository 全体の既存状態
（先行 slice も同じ状態で着地）。文書チェーンは先行 slice と同じ
規約で spec→design→plan→task→test-spec→ir→exp を作成し、
無関係な docs の修復は行わなかった。
