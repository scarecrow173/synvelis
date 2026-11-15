# RCD-F02 ADR 決定記録 — Advanced Monitoring / Overdub / Pre-Punch Handles（synvelis#120）

対象: `docs/adr/rcd-f02-monitoring-overdub.md`（`ADR-034XJBlePY8LREvuEvfvkR`、status: accepted）
Slice: docs-only 設計 slice（実装・実機受入を含まない）
Date: 2026-11-15

Issue #120 が必須化する 5 領域の決定要約と、採用/却下した選択肢を記録する。
詳細は ADR 本文・`docs/designs/rcd-f02-monitoring-overdub-design.md` を正本とする。

## D1 — UX Matrix: `MonitorRoute::Overdub` variant 追加（採用）

Monitor Off/Auto/On × Arm × Overdub × Punch/gate × Playback の可聴 route 行列を、
既存 `decide` 判定表（`crates/synvelis-recording/src/monitor.rs` L177-186）を
baseline 転記し、Overdub 次元を `MonitorRoute::Overdub` variant 追加で表現する。

- `Overdub` = input leg 可聴 **かつ** playback leg 可聴（mute/solo 解決後）の同時混合。
- playback leg が `Muted` に解決される場合は `Input` に退化。Monitor Off では overdub は route に影響しない。
- `TrackMonitorContext` に `overdub: bool` を追加。`decide` の純粋関数性・gate 権威非複製・pre-fader mute/solo は不変。
- `MonitorAdapter` は `Overdub` で input+playback 両 gain→1.0。input 未供給時は `effective_route=Playback` + `input_missing` に fail-closed 退避。

採用理由: `decide` の単一 route 返却・純粋関数契約を維持し、`Muted`+overdub 等の不正組合せを型で排除できる。

却下した選択肢:
- route + 別 flag の並列表現 — 不正組合せを型で排除できず `decide`/`MonitorAdapter` 契約を複雑化。
- route を集合型に再設計 — 既存 consumer の全面書き換え・YAGNI。

## D2 — Feedback Risk: 既存 notice 再利用 + `double_monitoring` 発火条件拡張（採用）

`MonitorNotices::{double_monitoring,hardware_direct_unknown,input_unavailable}` を
正本として再利用し、新 variant は追加しない。

- `double_monitoring`: 発火条件を `route == Input` から input-leg 可聴 route
  （`Input` または `Overdub`）へ拡張。Overdub でも live input が software monitor
  経路に乗るため Software+Hardware 二重監視・comb filtering リスクは同一に成立。
- `hardware_direct_unknown`/`input_unavailable`: 不変。
- Overdub 固有の増分リスク（feedback 露出・latency skew）は既存
  `LatencyReport`/`LatencyResolution` で報告。新規 notice は追加しない。
- Hardware Direct は `DeviceControlled`/`UserReported`/`Unknown` の 3 分類を維持し、
  `Unknown` を制御可能と表示しない。外部観測値を DAW 側で変更しない。

採用理由: 増分リスクは input leg 可聴一般に内在し、既存 latency 品質報告と
`double_monitoring` 拡張で表現可能。新 variant は投影・永続化・VCS 分類の
コストに見合わない。

却下した選択肢:
- Overdub 専用 notice variant 追加 — 表現可能だが投影/永続化/VCS コストに見合わない。
- app 側で独自 notice を発明 — 第二正本を作るため禁止。

## D3 — Capture Handle Retention: kind=4 `HandleGrant` + `JournalSpan` 分類（採用）

Capture Handle = Punch/Record Gate 閉区間のうち明示 consent された
session-frame 半開区間で capture される PCM handle 素材。

必須属性集合（`HandleGrantRecord` = durable 証拠）:
consent・capacity（frame/byte budget）・source range（既存 `JournalSpan` field
再利用）・clock（`ClockEpochId`・sample rate・format）・effective ranges
（宣言 ∩ gate 閉、plan 凍結時に解決・durable 記録）。

- journal kind=4 `HandleGrant` record を追加。`VERSION=1` 維持。
- `HandleGrantRecord` は最初の `ChunkRecord` より前に書く。
- handle span は既存 `JournalSpan` で記録し take span と同一
  `ChunkRecord.spans` に含めてよい。分類は replay 時に
  `HandleGrant.effective_ranges` への session-range 包含で決定的に行う
  （構造的 disjoint を plan 凍結時に保証）。
- handle 素材は project へ自動 commit されず session-scoped staging media。
  take/comp 昇格は既存の明示 adopt 経路のみ。capacity 超過時は handle
  capture のみ停止し take capture は継続。
- 旧 reader が kind=4 に遭遇 → prefix 切り詰め → fail-closed
  （handle/take 素材を受理済み capture として露出しない）。

採用理由: `JournalSpan`/`ChunkRecord`/verified prefix replay を不変に保ち、
journal format 変更を record kind 追加のみに限定。分類が journal 単体から
決定的に復元でき self-describing を維持。

却下した選択肢:
- `JournalSpan` material field 追加（VERSION bump）— payload layout 変更で
  v1 journal 全てが reject され verified prefix replay の互換性を失う。
- handle を別 journal/別 media file — stream atomicity（verified frontier
  の単位）を崩し recovery review の単位を二重化。
- handle capture を行わない — Issue #120 の Pre-Punch Handles 要件を満たさない。

## D4 — Undo/Recovery: 既存契約に整合・別 semantics を維持（採用）

- **Undo（決定的 project 操作）**: handle 素材の adopt・monitor/overdub 宣言
  変更・take/comp 操作は既存 project command + stable-ID 境界に従う
  undo/redo で復元。undo は session journal・staging media を遡及変更しない。
  handle capture consent は session 開始時の一回的 grant で undo 対象外
  （取消は Discard 経路）。
- **Recovery（verified prefix）**: journal replay は verified prefix のみを返し、
  `HandleGrant` より後の torn tail・未 sync span は受理済み capture として
  露出しない。handle/take は同一 stream の同一 verified frontier に従う
  （stream 単位 atomicity）。handle 分類は `HandleGrant` + verified spans
  のみで行い、未検証 span を handle と推測分類しない。

採用理由: RCD-02 verified prefix 契約・take/comping 非破壊保存に整合し、
handle 専用の undo/recovery モデル・独自永続化経路を新設しない。

却下した選択肢:
- handle 専用 undo/recovery モデル新設 — 第二正本・契約重複。
- handle を take と同一 commit 単位にする — consent 区間と take の
  lifecycle が異なり retention 契約を曖昧にする。

## D5 — Privacy: consent なし永続化禁止・session-scoped grant（採用）

- monitor route PCM（`Input`/`Overdub`/`Playback` の monitor 出力）は monitor
  bus に留まり、capture writer・journal・media・project のいずれにも
  書き込まない（既存契約の維持・拡張しない）。
- handle 素材（gate 閉 consent 区間の input PCM）の永続化は `HandleGrant`
  consent が前提。consent なしに punch 外素材を journal 記録・media 保存
  しない（Issue 受入条件「無許可の隠し Media 生成なし」）。
- `project.monitoring` に永続化されるのは authored 宣言
  （`arm`/`monitor`/`lane`/`channel_map`/`punch`/overdub flag）のみ。
  session 観測値・consent token・PCM は永続化しない。
- 診断/telemetry は handle PCM 内容・monitor PCM を含めない。
- 削除経路: recovery review の `Discard`（media 参照 probe fail-closed）
  と staging cleanup。undo は media を即削除しない。

採用理由: privacy 境界が明確で、project-level consent の複雑さを回避できる。

却下した選択肢:
- project-level consent 永続化（「常に許可」）— privacy 境界が複雑化・YAGNI。
- handle を無期限 staging 保持 — retention 契約を曖昧にする。

## Honest-capability（全領域共通）

Hardware Direct Monitor は `DeviceControlled`/`UserReported`/`Unknown` の
3 分類（`monitor.rs` L70-79）を維持し、Synvelis DSP で制御を保証しない
能力を「制御可能」と表示しない。`Unknown` を制御可能と表示せず、外部観測値を
DAW 側で変更しない。これは D1〜D5 の全決定に共通する制約として ADR/design
に明記した。

## 状態分類

- ADR/design/spec/plan/task/test-spec/ir/exp の文書化: `Pass (design)`。
- 実装・headless fixture・実 OS/GUI/Device/Plugin 検証: `NotRun`/`Deferred`
  （後続 slice・#114 系別 Gate）。本 slice は docs-only であり、
  headless/design 証拠を実機 Pass に換算しない。
