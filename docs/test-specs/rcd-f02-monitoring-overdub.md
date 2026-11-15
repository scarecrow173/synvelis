---
id: "TSPEC-034XJDBoNrTZPeWp6kZe5c"
type: "test-spec"
status: "approved"
title: "RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — Fixture 契約・受入検証仕様（synvelis#120）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-f02-monitoring-overdub.md"
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-f02-monitoring-overdub.md"
    - "docs/tasks/rcd-f02-monitoring-overdub.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-f02-monitoring-overdub.md"
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  defers: []
  deferred-by: []
---

# RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — Fixture 契約・受入検証仕様（synvelis#120）

## Purpose

Issue #120 の設計契約（ADR D1〜D5）に対し、後続実装 slice が満たすべき Rule・検証 Example・Guarantee を固定する。本書は「何を保証すべきか」を定義し、実装方法・数値を確定しない。実装は ADR merge 後の後続 slice、実機/GUI/Plugin Gate は #114 系別 Gate。

## 状態語彙

`POLICIES.md` 準拠。Requirement=`Required`/`Deferred`/`NotApplicable`、実行結果=`Pass`/`Fail`/`NotRun`/`Unsupported`。`Pass (design)` は設計契約の文書化、`Pass (headless)` は headless/unit 契約の実証であり、いずれも製品 GUI・実機・実 Device・実 Plugin Pass ではない。本 slice は docs-only のため全項目 `Pass (design)` または `NotRun`/`Deferred`。

## Feature

### F-ROUTE — Overdub route 判定（AC-F02-02, REQ-F02-01, REQ-F02-06, REQ-F02-07）

- **Rule**: `decide` は既存判定表行を変更せず、`TrackMonitorContext.overdub` 宣言がある場合のみ `MonitorRoute::Overdub` を返し得る。`Overdub` は input leg 可聴かつ playback leg 可聴（mute/solo 解決後）のときのみ。playback leg が `Muted` に解決される場合は `Input` に退化。Monitor Off では overdub は route に影響しない。`input_output_ready == false` では `On`/`Auto` の input leg が成立せず overdub 不成立。
- **Rule**: `MonitorAdapter` は `Overdub` で input_gain→1.0 かつ playback_gain→1.0 に ramp する。input 未供給時は `effective_route=Playback` + `input_missing` に fail-closed 退避（PCM 捏造なし）。
- **Rule**: monitor route・Overdub・handle 判定は raw capture・capture writer を変更しない（REQ-F02-07）。
- **Example**: `decide` に `monitor=On`・`overdub=true`・`input_output_ready=true`・`muted=false` を与えると `Overdub`。`muted=true` を加えると `Input`（playback leg Muted 退化）。`monitor=Off`・`overdub=true`・`arm=On`・`gate_open=true` は `Muted`（baseline 不変）。`monitor=On`・`overdub=true`・`input_output_ready=false` は `pb` + `input_unavailable`。
- **Guarantee**: route は `MonitorRoute` 単一値で決定的。`Muted`+overdub 等の不正組合せは型で排除。monitor 出力は capture/export に非干渉。

### F-NOTICE — Feedback Risk notice（AC-F02-03, REQ-F02-02, REQ-F02-06）

- **Rule**: `MonitorNotices` は既存 3 variant（`double_monitoring`/`hardware_direct_unknown`/`input_unavailable`）を正本として再利用し、新 variant を追加しない。`double_monitoring` の発火条件は input leg が可聴な route（`Input` または `Overdub`）かつ `hardware_direct.observed_active == Some(true)`。`hardware_direct_unknown` は `state == Unknown` で route 不問。`input_unavailable` は `monitor == On` かつ `!input_output_ready`。
- **Rule**: Hardware Direct は `DeviceControlled`/`UserReported`/`Unknown` の 3 分類を維持し、`Unknown` を制御可能と表示しない。外部観測値を DAW 側で変更しない。
- **Example**: `route=Overdub`・`hardware_direct.observed_active=Some(true)` で `double_monitoring=true`。`route=Playback`・`observed_active=Some(true)` では `double_monitoring=false`（input leg 非可聴）。
- **Guarantee**: comb filtering（二重監視）警告は input leg 可聴時に必ず発火し、Overdub で silent にならない。route=`Overdub` は `Input`/`Playback` と視覚的に区別可能に投影される（GUI-L2 表示契約）。

### F-HANDLE — Capture Handle Retention（AC-F02-04, REQ-F02-03, REQ-F02-05, REQ-F02-07）

- **Rule**: Capture Handle は Punch/Record Gate 閉区間のうち明示 consent された session-frame 半開区間のみ capture される。`HandleGrantRecord`（journal kind=4）は consent・capacity・source range・clock・effective ranges を durable に記録し、最初の `ChunkRecord` より前に書かれる。
- **Rule**: effective handle 区間 = 宣言 ∩ gate 閉 を plan 凍結時に解決し durable 記録する。handle 区間は gate 閉側・take 区間は gate 開側のみで構造的 disjoint。
- **Rule**: handle 素材の分類は replay 時に `HandleGrant.effective_ranges` への session-range 包含で決定的に行う（journal 単体で self-describing）。`JournalSpan`/`ChunkRecord`/`CheckpointRecord`/`FinalizeRecord` の format は不変、`VERSION=1` 維持。
- **Rule**: handle 素材は project へ自動 commit されず、session-scoped staging media に保存される。take/comp 昇格は既存の明示 adopt 経路のみ。capacity 超過時は handle capture のみ停止し take capture は継続。打ち切りは journal から検出可能。
- **Rule**: consent なしに punch 外素材を journal 記録・media 保存しない（無許可の隠し Media 生成なし）。
- **Example**: `HandleGrant` なしの session では gate 閉区間の PCM は一切 media/journal に現れない。`HandleGrant` ありでは effective ranges 内の gate 閉区間のみ span が記録され、effective ranges 外の gate 閉区間は記録されない。
- **Guarantee**: handle/take 素材は同一 stream の同一 verified frontier に従う（stream 単位 atomicity）。kind=4 を持つ journal を旧 reader が読むと prefix が切り詰められ fail-closed（handle/take 素材を受理済み capture として露出しない）。

### F-RECOVERY — Undo/Recovery（AC-F02-05, REQ-F02-04, REQ-F02-05）

- **Rule**: Undo（決定的 project 操作）と Recovery（journal verified prefix）は別 semantics。handle 素材の adopt・monitor/overdub 宣言変更・take/comp 操作は既存 project command + stable-ID 境界に従う undo/redo で復元し、undo は session journal・staging media を遡及変更しない。handle capture consent は session 開始時の一回的 grant で undo 対象外（取消は Discard 経路）。
- **Rule**: Recovery は journal replay の verified prefix のみを返し、`HandleGrant` より後の torn tail・未 sync span は受理済み capture として露出しない。handle 分類は `HandleGrant` + verified spans のみで行い、未検証 span を handle と推測分類しない。
- **Example**: device 切断 → fail-stop → verified prefix までの record で recovery review。handle/take は同一 verified frontier に従う。adopt の undo は project 状態を復元するが media を即削除しない。
- **Guarantee**: RCD-02 verified prefix 契約に整合し、handle 専用の undo/recovery モデル・独自永続化経路を新設しない。

### F-PRIVACY — Privacy 境界（AC-F02-06, REQ-F02-05）

- **Rule**: monitor route PCM（`Input`/`Overdub`/`Playback` の monitor 出力）は monitor bus に留まり、capture writer・journal・media・project のいずれにも書き込まない。
- **Rule**: handle 素材（gate 閉 consent 区間の input PCM）の永続化は `HandleGrant` consent が前提。`project.monitoring` に永続化されるのは authored 宣言（`arm`/`monitor`/`lane`/`channel_map`/`punch`/overdub flag）のみで、session 観測値・consent token・PCM は永続化しない。
- **Rule**: 診断/telemetry は handle PCM 内容・monitor PCM を含めない（notice/route/区間メタデータのみ）。削除経路は recovery review の `Discard`（media 参照 probe fail-closed）と staging cleanup に従う。
- **Example**: overdub 宣言を project に保存しても PCM・consent token は含まれない。session 観測 overlay（`MonitorUxState.track_sessions`）は project 再生成で reset され永続化されない。
- **Guarantee**: monitor 内容は consent なしに永続化されない。punch 外素材は明示区間のみ journal/保存される。

## Non-goals

- 製品実装・実機/GUI/Plugin 受入（後続 slice・#114 系別 Gate）。
- 実 OS E2E（Loop/Take/Comp/Device 切断/Undo/Recovery・RT noalloc）— Issue #120 実装 slice の検証。
- RT graph monitor node 搭載（RCD-03 Deferred のまま）。
- Project-level consent 永続化（session-scoped grant で要件充足、YAGNI）。
- handle capacity の具体的既定値・数値（実装 slice の fixture で確定）。
- Rust build/test — docs-only slice のため `NotRun`/`NotApplicable`。

## Risk

- route variant 追加・notice 発火条件拡張・journal kind 追加が既存 consumer を壊す → compile error で検出可能・exhaustive match 更新は実装 slice の範囲。
- handle/take 分類の曖昧さ → effective ranges の構造的 disjoint を plan 凍結時に保証。
- kind=4 journal の旧 reader 互換 → fail-closed（prefix 切り詰め）を仕様として明記。
- headless 証拠を実機 Pass に換算 → 状態語彙で厳密分離。
