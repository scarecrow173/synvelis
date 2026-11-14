---
id: "EXP-WDBkr2wia8rpphV4034lTq"
title: "Recording / Take / Comping 設計定義 実験メモ (synvelis#12)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: Recording / Take / Comping（synvelis#12）

## E1 doc-driven-dev-graph router の状態

`route_graph.js --cwd . --json` を実行。`status: blocked`、
`current/next: probe`、`edgeId: null`。blocker は **既存の
repository-wide broken relation**（`docs/designs/automation-
modulation-design.md`・`rcd-*`・`tim-02-*`・`vcs-*`・
`docs/impl/ir/*` の deferred relation が存在しない path を指す）
であり、本 slice が新たに作ったものではない。sv#5/sv#6/sv#11
先例と同様、この blocked は slice 起因でないため、router が返す
delegate が無いまま `spec → design → plan → task → test-spec →
impl` の文書チェーンを規約どおり作成した。無関係な broken
relation は **修復しない**（範囲外）。exp に記録して後続の
doc-status 修復 slice に委譲。

## E2 artifact ID の生成

先行 slice（sv#5/sv#6/sv#11）に倣い、`SPEC-`/`DESIGN-`/`PLAN-`/`TASK-`/
`TSPEC-`/`IMPL-`/`EXP-` + 22 文字 Base62 の artifact ID を生成
（crypto random 22 bytes を base62 化）。生成値:

- `SPEC-QIpUDjWLLusOo4TEiv6CjZ`
- `DESIGN-jrb8PxsJgSO7YaSuwiQUcL`
- `PLAN-xOv3jxNuAQWUfmfAuMGI8P`
- `TASK-fQ5GG20T5UOhj5Xy7za6XD`
- `TSPEC-a6ZCHY6LRL2KeFER0sLwl7`
- `IMPL-C1ZTg7tLJ3bWNVepfbzpSs`
- `EXP-WDBkr2wia8rpphV4034lTq`

## E3 AC-01〜69 の採番と状態分類

Issue #12 §13 の 69 受入条件を **Issue 本文順に AC-01〜69** で採番し、
spec・`evidence/sv12/owner-matrix.md`・test-spec で同一 ID を維持した。

状態分類の判断:

- `Pass (headless)` 44 件 — 既存 headless/unit fixture（`vs_b`/`vs_c`/
  `vs_d`/`device_recording`/`input_adapter`/`monitor_gate`/
  `monitor_punch_session`/`recovery_review`/`md_a`/`rcd_04_*`/`tim_02_*`/
  `pc_c1`/`ed_a`/`uic5`/`ws_a` 等）が契約を実証。
- `Partial` 22 件 — 機構は headless 実証済みだが literal 件数（8 track・
  120BPM/48kHz/4小節）・実画面経路・実 plugin capability 等の残部が
  後続 Gate。
- `Deferred` 2 件（AC-28/29）— Take Lane/Comp Editor UI が未実装で
  #114/#118 の GUI Gate のみ。
- `Pass (design)` 1 件（AC-69）— API 適合チェックの process AC で本
  chain が証拠。実装時の適合チェックは #115〜#118 が実施。
- `NotRun`/`Unsupported` 0 件 — 設計契約のみで未検証の AC は無く、
  capability 非対応の明示は無し。

`Pass (headless)`/`Partial` の GUI・実機・実 Device・実 Plugin 面は
個別に `Deferred`（#114・EXT-L3・ruviform capability Issues）として
扱い、headless 成功を製品受入に換算しない（POLICIES.md 準拠）。

## E4 depends-on/related path の実在確認

当初 spec/design の `depends-on` に推測した filename（`vs-a-recording-
session-pass-take-contract.md` 等）を記述したが、実ファイル名と不一致
だったため、`ls docs/specs|designs` で実名を確認して修正した。MIDI
Source Note 編集は専用 spec が無く `docs/designs/midi-source-note-
editing-design.md`（implements issue #52）を depends-on に採用。
TIM-02 の MIDI phrase は `tim-02-player-midi-rt-offline.md`・
`tim-02-smf-format01-import-export.md` を採用。修正後、全 local
relation path が実在することを確認した（broken relation を新たに
増やさない）。

## E5 creative branch・Ruviform 境界の確認

Issue #12 の境界を確認: Creative Branch Take/Comp 候補（Fork/Adopt/
Audio Diff）は #279 CB-02・#278 CB-01・#288 CB-11 の別作業であり、
Raw Session/Source/Take Capture/Journal/Recovery は #12 が所有し
Fork/Adopt/Audio Diff は書き込まない。Recording/Monitor/Punch/
Automation Write active 中の Adoption は明示的安全検証が必要で非対応は
reject/defer。本設計は #12 が所有する基本契約のみを固定し、CB 系・
Ruviform Generic Device Input/File/Clock の DAW 固有化は行わない。
