---
id: "EXP-3FNMUpjRuCrvGC3Y3XRPMa"
title: "SF-04 probe adapter 受入集約 — 実装メモ (synvelis#107)"
created: "2026-11-14"
updated: "2026-11-14"
---

# SF-04 probe adapter 受入集約 — experiment log

## E1. 被覆監査の進め方

Issue #107 の SF-PROBE-01..08 を既存 slice 実装
（slice1 semantic resolve / slice2 trace bridge / slice3 clock
epoch / slice5 workspace lease / slice6 compare / slice8 faults /
slice9 discontinuity declare / slice10 compare op）と `tests/
sf04_*` の headless テストへ写像した。監査は「受入条件の各節が
どの既存テストまたは未被覆分岐に対応するか」を先に固定し、
重複テストを追加しない方針で進めた（タスク指針「既存テストで
被覆済みのケースは重複追加せず被覆表に記録」）。

監査で見つかった未被覆分岐は 2 つ:

- **entity 消失時の unbind**: `sf04_probe_adapter` の moved-point
  テストは「semantic point が別 port へ移動」するケースを被覆
  するが、「対象 entity 自体が project から消える」ケース
  （`RemoveProcessor` 等による SourceMap 失効）は未被覆。
  Issue #107 の副次条件「port disappearance/re-attach」
  「old slot reuse で誤観測 0」に対応する。
- **per-key subscription dedup 境界**: `sf04_probe_adapter` の
  remove/lifecycle テストは binding 単位の動作を被覆するが、
  同一 `ProbeSubscriptionKey` への重複 request の冪等性と、
  `session_id` 違いが独立 subscription になる境界は未被覆。
  SF-PROBE-06 の player 層側の契約を固定する必要があった。

## E2. entity 消失テストの設計

`RemoveProcessor` → republish で `ProbeSession::on_publish` が
`resolve_probe_point` を再実行し、entity 不在で `Unresolved`
を返す経路を使う。broker probe は取り外され
`runtime_probe_id = None` になる。検証は RT-observed の
`SemanticProbeBinding`（`probe_bindings()` の public read model
＋ `probe_drain` の status event）で行い、内部状態への直接
アクセスはしない。

re-attach は「同じ semantic point が復活」ではなく「新しい
entity id = 新しい semantic point」として扱うべきかを確認した
ところ、`SemanticProbePoint` は entity id を含むため新 id は
別 key であり、旧 binding の `Unresolved` 状態と混同されない
ことをテストで固定した。これが「old slot reuse で誤観測 0」
の headless 証拠になる。

## E3. per-key dedup テストの設計

`probe_request` の冪等 early-return（既存 binding があれば
何もしない）を利用し、同一 key の重複 request が
`runtime_probe_id`・`requested_mode` を変更しないことを検証。
`probe_capture` は binding あたり open segment を 1 つしか
持たないため、capture 中の再 `probe_capture` は
`AlreadyCapturing` で fail-closed になる。

`session_id` 違いの key が独立 install されることを確認した
上で、「cross-window/inspector 間の lease 集約は session 層
（slice5, synvelis-workspace）の契約であり、player 層は
per-key dedup のみを保証する」という境界をテストコメントと
assertion で明示した。これにより SF-PROBE-06 の責務分界が
コード上で固定される。

## E4. src 変更を行わなかった判断

監査で `resolve_probe_point`・`ProbeSession::on_publish`・
broker install/rebind・trace bridge・compare の contract 違反は
見つからなかった。entity 消失時の unbind・per-key dedup・
capture session 排他は全て既存実装が正しく実現しており、
テスト追加のみで契約を固定できた。spec の Out of Scope
（src 変更は監査で違反が見つかった場合の最小修正に限定）に
従い、コード差分はテストファイル 1 件に留めた。
