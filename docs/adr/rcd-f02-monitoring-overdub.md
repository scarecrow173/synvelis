---
id: "ADR-034XJBlePY8LREvuEvfvkR"
type: "adr"
status: "accepted"
title: "RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — UX Matrix・Feedback Risk・Capture Handle Retention・Undo/Recovery・Privacy"
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
    - "docs/designs/rcd-03-app-arm-monitoring-ux.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
    - "docs/specs/rcd-f02-monitoring-overdub.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md"
  refines:
    - "docs/designs/rcd-03-app-arm-monitoring-ux.md"
  refined-by: []
  derives-from:
    - "docs/specs/rcd-f02-monitoring-overdub.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-f02-monitoring-overdub.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/120"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/118"
    - "https://github.com/scarecrow173/synvelis/issues/12"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — UX Matrix・Feedback Risk・Capture Handle Retention・Undo/Recovery・Privacy

## Context and Problem Statement

Issue #120（future/recording tracker RCD-F02）は、Advanced Monitoring / Overdub Layering / Pre-Punch Handles の実装に先立ち、次の 5 領域を ADR で合意することを必須化している:

> 実行 Graph の Monitor Route と録音 Capture Gate を分離したまま、Capture Handle には User Consent+保存容量+SourceRange/Clock 証拠を必須にする。Hardware Direct Monitoring は外部/Device 能力であり Synvelis DSP だけで制御を保証しない。
> 初期実装前に UX Matrix（Off/Auto/On×Arm×Overdub×Punch×Playback）、Feedback Risk、Capture Handle Retention、Undo/Recovery と Privacy を ADR で合意する。

受入条件は「二重監視/Comb filtering 警告、Hardware Direct 不可制御の正しい表示、Raw Recorder PCM 不変」「Punch 外素材は明示有効化した区間だけ Journal 記録/保存、無許可の隠し Media 生成なし」「Loop/Take/Comp/Device 切断/Undo/Recovery の実 OS E2E、RT noalloc」を要求する。

現状の契約上の空白:

- `synvelis-recording::monitor::decide` の route は `MonitorRoute::{Input, Playback, Muted}` の排他 1 値で、Overdub（既存 Playback と Software Input の同時混合）を表現できない。
- `synvelis-recording::journal` の record kind は Chunk/Checkpoint/Finalize のみで、「consent」「handle」の語彙が存在しない。
- `synvelis-recording::gate` の `GateSpec` は Record/Punch Gate のみを評価し、gate 閉区間の consent capture（pre/post punch handle）を表現しない。
- RCD-03 で monitor 宣言の session-scope 投影・VCS 分類は確定済みだが、Overdub 宣言・handle consent・persistent Undo の語彙は未定義。

本 ADR はこれら 5 領域の決定を記録し、後続実装 slice（ADR merge 後・直列 dispatch）が従う契約を固定する。**本 slice は docs-only であり、実装・実機受入を含まない。**

## Decision Drivers

* 既存 `decide`/`GateScheduler`/`evaluate_gate`/`journal`/`MonitorAdapter` の権威を第二正本化せず拡張すること。
* Monitor Route と Capture Gate の分離を維持し、monitoring 状態が raw capture/capture writer を変更しないこと。
* Punch 外素材を「明示有効化区間のみ journal 記録・保存」し、無許可の隠し Media を生成しないこと（Issue 受入条件直結）。
* Capture Handle に User Consent・保存容量・SourceRange/Clock 証拠を必須とすること（Issue 後続スコープ原文）。
* Hardware Direct Monitor を `DeviceControlled`/`UserReported`/`Unknown` の 3 分類のまま扱い、Synvelis DSP で制御を保証しない能力を「制御可能」と表示しないこと。
* Undo（決定的 project 操作）と Recovery（journal verified prefix）を別 semantics として RCD-02 契約に整合させること。
* monitor 内容（input PCM・handle 素材）を consent なしに永続化しない privacy 境界。
* 表現ドリフト禁止 — 既存 `GateSpan`/`CapturePiece`/`JournalSpan` の session frame 半開 `[in,out)` + project tick + source frame 表現を再利用し、新しい区間表現を発明しない。

## Considered Options

* **D1 UX Matrix — Overdub の route 表現**: (a) `MonitorRoute` に `Overdub` variant を追加、(b) 既存 route + 別 flag の並列表現、(c) route を集合型に再設計。
* **D2 Feedback Risk — notice semantics**: (a) 既存 `MonitorNotices` 3 variant を再利用し `double_monitoring` の発火条件を input-leg 可聴へ拡張、(b) Overdub 専用の新 notice variant を追加、(c) app 側で独自 notice を発明。
* **D3 Capture Handle — journal/retention 表現**: (a) kind=4 `HandleGrant` record + 既存 `JournalSpan` 分類、(b) `JournalSpan` に material field 追加（VERSION bump）、(c) handle を別 journal/別 media file に分離、(d) handle capture を行わない（pre/post punch を諦める）。
* **D4 Undo/Recovery**: (a) 既存 project undo + RCD-02 verified prefix に整合、(b) handle 専用の undo/recovery モデルを新設、(c) handle を take と同一 commit 単位にする。
* **D5 Privacy**: (a) monitor PCM は永続化しない・handle は session-scoped consent grant のみ、(b) project-level consent 永続化も許可、(c) handle を無期限 staging 保持。

## Decision Outcome

Chosen options: **D1(a), D2(a), D3(a), D4(a), D5(a)**, because 既存権威を壊さず最小の拡張で Issue の 5 領域を契約化でき、fail-closed と privacy 境界を保てるため。

### D1 — UX Matrix: `MonitorRoute::Overdub` variant 追加

`MonitorRoute` に `Overdub` variant を追加する。意味論は「input leg 可聴 **かつ** playback leg 可聴（mute/solo 解決後）」の同時混合。

UX Matrix（`decide` 既存判定表を baseline 転記。`pb` = `playback_route(ctx)` の mute/solo 解決結果）:

| monitor | 条件 | arm | gate_open | phase | Overdub=Off | Overdub=On |
| --- | --- | --- | --- | --- | --- | --- |
| Off | arm && gate_open | On | true | 任意 | `Muted` | `Muted` |
| Off | 他 | 任意 | 任意 | 任意 | `pb` | `pb` |
| Auto | arm && (gate_open \|\| Stopped) | On | 任意 | Stopped or gate_open | `Input` | `Overdub` / `Input`（pb=Muted 時） |
| Auto | 他 | 任意 | 任意 | 他 | `pb` | `pb` |
| On | input_output_ready | 任意 | 任意 | 任意 | `Input` | `Overdub` / `Input`（pb=Muted 時） |
| On | !input_output_ready | 任意 | 任意 | 任意 | `pb` + `input_unavailable` | `pb` + `input_unavailable` |

規則:

- `Overdub` は playback leg が `Muted` に解決される場合 `Input` に退化する（混合の意味が無い）。Monitor Off では overdub 宣言は route に影響しない。
- `TrackMonitorContext` に `overdub: bool`（authored 宣言入力）を追加する。`decide` の純粋関数性・gate 権威非複製・mute/solo が playback leg のみに作用する pre-fader 性は不変。
- `MonitorAdapter` は `Overdub` で input_gain→1.0 かつ playback_gain→1.0 に ramp する。input 未供給時は `effective_route=Playback` + `input_missing` に fail-closed 退避する（`Input` の `Muted` 降格とは異なり playback leg を残す — Overdub の playback は既存 take であり捨てる理由がない）。
- Punch/Loop/Take 数・gate_open 区間は Overdub で変更しない。handle 区間は gate 閉側のみ存在し route 判定の `gate_open` に影響しない。

### D2 — Feedback Risk: 既存 notice 再利用 + `double_monitoring` 発火条件拡張

`MonitorNotices::{double_monitoring, hardware_direct_unknown, input_unavailable}` を正本として再利用し、新 variant は追加しない。

- `double_monitoring`: 発火条件を `route == Input` から **input leg が可聴な route（`Input` または `Overdub`）** へ拡張する。Overdub でも live input が software monitor 経路に乗るため、Software+Hardware 二重監視・comb filtering リスクは `Input` route と同一に成立する。playback leg は既存 take であり入力信号と同一波形ではないため、それ自体は comb filtering を構成しない。
- `hardware_direct_unknown`: 不変（`state == Unknown` で route 不問に発火。`Unknown` を制御可能と表示しない）。
- `input_unavailable`: 不変（`monitor == On` かつ `!input_output_ready`。overdub 宣言があっても input leg が無ければ overdub 不成立として `pb` に退避）。
- Overdub 固有の増分リスク（feedback 露出・monitor latency skew）は input leg 可聴一般に内在し、既存 `LatencyReport`/`LatencyResolution`（Measured/Reported/Estimated/Unknown・Clock Epoch・二重加算禁止）で報告される。新規 notice は追加しない。
- 表示義務: route=`Overdub` は `Input`/`Playback` と視覚的に区別可能に投影する（GUI-L2 表示契約。実装は後続 slice）。Hardware Direct は `DeviceControlled`/`UserReported`/`Unknown` の 3 分類を維持し、Synvelis DSP で制御を保証しない能力を「制御可能」と表示しない。外部観測値を DAW 側で変更しない。

### D3 — Capture Handle Retention: kind=4 `HandleGrant` + 既存 `JournalSpan` 分類

Capture Handle = Punch/Record Gate 閉区間のうち明示 consent された session-frame 半開区間で capture される PCM handle 素材。pre-roll 音声確保・post-roll 余韻の非破壊保存が用途であり、take/comp 化は既存の明示 adopt 経路のみ。

必須属性集合（`HandleGrantRecord` = durable 証拠）:

| 属性 | 内容 |
| --- | --- |
| consent | handle capture の明示 opt-in 記録（grant 識別・session/clock 文脈） |
| capacity | handle 素材の上限（frame/byte budget）。超過時は handle capture のみ停止し take capture は継続。打ち切りは journal から検出可能 |
| source range | 既存 `JournalSpan` field（`session_start`/`frame_count`/`source_start`/`project_in`/`project_out`/`media_offset`）を再利用 |
| clock | session の `ClockEpochId`・sample rate・format（plan 凍結値） |
| effective ranges | plan 凍結時に解決した実効 handle 区間（宣言 ∩ gate 閉）。宣言との差分を durable に記録 |

Journal 表現:

- 新 record kind `KIND_HANDLE_GRANT = 4`。`VERSION=1` を維持する（kind は u8 で拡張可能。VERSION bump は既存 v1 journal 全てを reject するため不採用）。
- `HandleGrantRecord` は当該 stream の最初の `ChunkRecord` より前に書く（capture 開始前の durable consent 証拠）。
- handle piece の PCM span は既存 `JournalSpan` で記録し take span と同一 `ChunkRecord.spans` に含めてよい。分類は replay 時に `HandleGrant.effective_ranges` への session-range 包含で決定的に行う。handle 区間は gate 閉側・take 区間は gate 開側のみで構造的に disjoint（plan 凍結時に保証）。
- 旧 version reader が kind=4 に遭遇した場合: decode 不能 → replay はその位置で停止 → prefix 切り詰め。HandleGrant が先頭のため prefix_len=0 となり recovery は「検証済み journal なし」として fail-closed（handle/take 素材を受理済み capture として露出しない）。downgrade 中の session 継続は非サポート（staging は保持・review 可能）。

Retention 規則:

- handle 素材は session-scoped staging media に保存され、project へは自動 commit されない。take/comp 昇格は既存の明示 adopt 経路（RCD-04 系非破壊契約）のみ。
- `Finalized` 後の handle 素材は uncommitted stream media と同じ lifecycle（RCD-02 review: 検証/取込/保留/破棄）。`Discard` は既存 `MediaReferenceProbe` fail-closed。
- handle 素材の Undo（adopt 取消）は project undo に従い元 media を即削除しない。
- consent は session-scoped。project-level consent 永続化・「常に許可」は Deferred（YAGNI）。

### D4 — Undo/Recovery: 既存契約に整合・別 semantics を維持

- **Undo（決定的 project 操作）**: handle 素材の adopt・monitor/overdub 宣言変更・take/comp 操作は既存 project command + stable-ID 境界に従う undo/redo で復元する。undo は session journal・staging media を遡及変更しない。handle capture consent は session 開始時の一回的 grant であり undo 対象外（取消は Discard 経路）。
- **Recovery（verified prefix）**: crash/device 切断時、journal replay は verified prefix のみを返す。`HandleGrant` より後の torn tail・未 sync span は受理済み capture として露出しない（既存契約のまま handle span も同規則）。handle と take は同一 stream の同一 verified frontier に従う（stream 単位 atomicity）。
- device 切断 → 既存 fail-stop → verified prefix までの record で recovery review。handle 分類は `HandleGrant` + verified spans のみで行い、未検証 span を handle と推測分類しない。
- handle 専用の undo/recovery モデル・独自永続化経路は新設しない。

### D5 — Privacy: consent なし永続化禁止・session-scoped grant

- monitor route PCM（`Input`/`Overdub`/`Playback` の monitor 出力）は monitor bus に留まり、capture writer・journal・media・project のいずれにも書き込まない（既存契約の維持・拡張しない）。
- handle 素材（gate 閉 consent 区間の input PCM）の永続化は `HandleGrant` consent が前提。consent なしに punch 外素材を journal 記録・media 保存しない（Issue 受入条件「無許可の隠し Media 生成なし」）。
- `project.monitoring` に永続化されるのは authored 宣言（`arm`/`monitor`/`lane`/`channel_map`/`punch`/overdub flag）のみ。session 観測値（`MonitorUxState.track_sessions`）・consent token・PCM は永続化しない。
- 診断/telemetry は handle PCM 内容・monitor PCM を含めない。notice/route/区間メタデータのみ。
- 削除経路: handle 素材の破棄は recovery review の `Discard`（media 参照 probe fail-closed）と staging cleanup に従う。undo は media を即削除しない。

### Consequences

* Good, because 既存 `decide`/`GateScheduler`/`evaluate_gate`/`JournalSpan`/`MonitorAdapter`/project undo/RCD-02 recovery の権威を維持したまま、Issue #120 の 5 領域を契約化できる。
* Good, because `MonitorRoute::Overdub` は単一 variant で fail-closed（`Muted`+overdub 等の不正組合せを型で排除）かつ `decide` の純粋関数契約を壊さない。
* Good, because kind=4 record 追加のみで journal format を拡張でき、`JournalSpan`/`ChunkRecord`/verified prefix replay を不変に保てる。
* Good, because handle 素材の分類が `HandleGrant.effective_ranges` への session-range 包含で journal 単体から決定的に復元でき、self-describing を維持する。
* Bad, because `MonitorRoute` variant 追加は既存 consumer（`decide` 呼出側・`MonitorAdapter`・app 投影）の exhaustive match 更新を要求する（compile error で検出可能・実装 slice の範囲）。
* Bad, because kind=4 を持つ journal を旧 version reader が読むと prefix が切り詰められる（downgrade 中 session は非サポートとして明記）。
* Bad, because handle capacity 予算の決定的な既定値は実装 slice の fixture で確定する必要がある（本 ADR は「必須属性として capacity を持つ」契約のみ固定）。

### Confirmation

本 ADR の決定は後続実装 slice で以下により確認される:

- `decide` 拡張（`TrackMonitorContext.overdub`・`MonitorRoute::Overdub`・`double_monitoring` 発火条件）が既存判定表行を変更しないことを headless fixture で検証。
- `HandleGrant` record の encode/decode・replay verified prefix・kind=4 fail-closed を journal test で検証。
- handle effective ranges の plan 凍結解決（宣言 ∩ gate 閉・disjoint 保証）を plan test で検証。
- `MonitorAdapter` の Overdub gain・input 未供給時 Playback 退避を adapter test で検証。
- 実 OS/GUI/Device/Plugin 受入は #114 系別 Gate（本 slice 対象外）。

## Implementation Plan

後続実装 slice（本 slice 対象外・ADR merge 後に直列 dispatch）の想定:

* Affected paths:
  * `../../crates/synvelis-recording/src/monitor.rs` — `TrackMonitorContext` への overdub 宣言入力・`MonitorRoute::Overdub`・`decide` 拡張・`double_monitoring` 発火条件
  * `../../crates/synvelis-recording/src/journal.rs` — `KIND_HANDLE_GRANT=4`・`HandleGrantRecord` encode/decode/replay
  * `../../crates/synvelis-recording/src/gate.rs`・`../../crates/synvelis-recording/src/plan.rs`・`../../crates/synvelis-recording/src/session.rs` — handle 宣言の plan 凍結・effective ranges 解決
  * `../../crates/synvelis-recording` capture engine/writer — handle piece capture（take span 不変）
  * `../../crates/synvelis-player/src/monitor.rs`・`../../crates/synvelis-player/src/monitor_engine.rs` — `Overdub` gain・input 未供給 Playback 退避
  * `../../crates/synvelis-app/src/monitoring.rs` — overdub 宣言・consent UX・notice/route 投影
  * `../../crates/synvelis-project` — project monitoring への overdub field（authored 宣言のみ）
* Patterns to follow: 既存 `decide` 純粋関数・`GateScheduler`/`evaluate_gate` 権威・`JournalSpan`/`ChunkRecord` 構造・`MonitorAdapter` gain ramp・RCD-02 verified prefix recovery・take/comping 非破壊保存。
* Patterns to avoid: route/gate 意味論の app 側再実装、monitor PCM の永続化、consent なし handle capture、`JournalSpan` layout 変更、handle 専用 undo/recovery モデル新設、Hardware Direct を制御可能と表示すること。
* Dependencies or configuration: なし（feature flag 不要。overdub/handle は明示 opt-in 宣言）。

## Verification

* [ ] `decide` 拡張が既存判定表行を変更しないことを headless fixture で検証（実装 slice・`mise run test synvelis-recording` 系）
* [ ] `HandleGrant` record の encode/decode・replay verified prefix・kind=4 fail-closed を journal test で検証（実装 slice）
* [ ] handle effective ranges の plan 凍結解決を plan test で検証（実装 slice）
* [ ] `MonitorAdapter` の Overdub gain・input 未供給時 Playback 退避を adapter test で検証（実装 slice）
* [ ] 実 OS/GUI/Device/Plugin 受入は #114 系別 Gate（本 slice 対象外・NotRun）

## Pros and Cons of the Options

### D1(a) `MonitorRoute::Overdub` variant 追加（採用）

* Good, because `decide` の単一 route 返却・純粋関数契約を維持し、`Muted`+overdub 等の不正組合せを型で排除できる。
* Good, because `MonitorAdapter` の gain 対応が route 1 値で決定的（Overdub → in+pb 両 1.0）。
* Bad, because 既存 consumer の exhaustive match 更新が必要（compile error で検出可能）。

### D1(b) route + 別 flag の並列表現（却下）

* Good, because `MonitorRoute` enum を変更しない。
* Bad, because `decide` が単一 route を返す契約と `MonitorAdapter` の gain 対応を複雑化し、不正組合せ（`Muted`+overdub 等）を型で排除できない。

### D1(c) route を集合型に再設計（却下）

* Good, because 任意の source 組合せを表現できる。
* Bad, because 既存 `decide`/`MonitorAdapter`/app 投影の全面書き換えが必要で、YAGNI（Overdub は in+pb の 1 組合せのみ）。

### D2(a) 既存 notice 再利用 + 発火条件拡張（採用）

* Good, because `double_monitoring` の semantics（Software+Hardware 二重監視・comb filtering）を Overdub にも正しく適用でき、新 variant の投影・永続化・VCS 分類コストを避けられる。
* Bad, because `double_monitoring` の発火条件変更は既存 `Input` route の notice 意味論を拡張する（`Overdub` 追加に伴う必然的変更であり、既存 `Input` の挙動は不変）。

### D2(b) Overdub 専用 notice 追加（却下）

* Good, because Overdub 固有の警告を独立に表現できる。
* Bad, because Overdub の増分リスクは input leg 可聴一般に内在し、既存 latency 品質報告と `double_monitoring` 拡張で表現可能。新 variant は投影・永続化・VCS 分類のコストに見合わない。

### D3(a) kind=4 `HandleGrant` + `JournalSpan` 分類（採用）

* Good, because `JournalSpan`/`ChunkRecord`/verified prefix replay を不変に保ち、journal format 変更を record kind 追加のみに限定できる。
* Good, because `HandleGrant.effective_ranges` への session-range 包含で分類が journal 単体から決定的に復元できる。
* Bad, because kind=4 を持つ journal を旧 reader が読むと prefix が切り詰められる（fail-closed として明記）。

### D3(b) `JournalSpan` material field 追加（却下）

* Good, because span 単位で material が self-describing になる。
* Bad, because payload layout 変更 → journal VERSION bump → 既存 v1 journal 全てが reject され、verified prefix replay の互換性を失う。

### D3(c) handle を別 journal/別 media file（却下）

* Good, because handle を take から完全分離できる。
* Bad, because stream atomicity（verified frontier の単位）を崩し、RCD-02 recovery review の単位を二重化する。

### D3(d) handle capture を行わない（却下）

* Good, because journal/retention の拡張が不要。
* Bad, because Issue #120 の Pre-Punch Handles 要件を満たさない。

### D4(a) 既存 project undo + RCD-02 verified prefix（採用）

* Good, because Undo（決定的 project 操作）と Recovery（verified journal/media prefix）を別 semantics として既存契約に整合させられる。
* Bad, because handle 素材の分類・retention を recovery review に追加する必要がある（`HandleGrant` + effective ranges の解釈）。

### D5(a) monitor PCM 非永続化・session-scoped grant（採用）

* Good, because privacy 境界が明確（consent なし永続化禁止）で、project-level consent の複雑さを回避できる。
* Bad, because session ごとに consent が必要（利便性は下がるが privacy 優先）。

## More Information

* Issue #120 — https://github.com/scarecrow173/synvelis/issues/120
* 関連 Issue: synvelis#117（RCD-03 monitor/punch/latency）・synvelis#116（RCD-02 durability/recovery）・synvelis#118（RCD-04 comp/midi/recovery E2E）・synvelis#12（recording-take-comping 基本契約）・SF-02 synvelis#105・SV-GUI-05 synvelis#114
* 関連文書: `docs/specs/rcd-f02-monitoring-overdub.md`・`docs/designs/rcd-f02-monitoring-overdub-design.md`・`docs/designs/rcd-03-*`・`docs/designs/rcd-02-*`・`docs/{specs,designs}/recording-take-comping*`
* Evidence: `evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md`（5 領域の決定要約 + 採用/却下選択肢）・`evidence/sv120/rcd-f02-monitoring-overdub-verification.md`（実行検証記録）
* 実装 slice は本 ADR merge 後・直列 dispatch。Issue #120 全体の close は Coordinator/Owner 判断（本 slice merge のみでは完了を宣言しない）。
