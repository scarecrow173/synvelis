---
id: "DESIGN-034XJyqSj5ERrc0PHD2tDm"
type: "design"
status: "approved"
title: "RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles design"
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
  implements:
    - "docs/specs/rcd-f02-monitoring-overdub.md"
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-f02-monitoring-overdub.md"
    - "docs/designs/rcd-03-app-arm-monitoring-ux.md"
    - "docs/designs/rcd-03-monitor-punch-design.md"
    - "docs/designs/rcd-03-monitoring-punch-latency.md"
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/designs/recording-take-comping-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/rcd-f02-monitoring-overdub.md"
    - "docs/plans/rcd-f02-monitoring-overdub.md"
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
  defers: []
  deferred-by: []
---

# RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles design

## Context

RCD-03（synvelis#117）で monitor route 判定・gate・latency の baseline は確定済み。本 design は Issue #120 が要求する Advanced Monitoring / Overdub Layering / Pre-Punch Handles の拡張境界を、既存契約を第二正本化せずに定義する。**docs-only slice であり、コード変更・実装を含まない。**

既存契約（read-only input、全て実在確認済み）:

- `crates/synvelis-recording/src/monitor.rs`
  - `decide(ctx, phase, gate_open) -> MonitorDecision` — 純粋関数。route は `MonitorRoute::{Input, Playback, Muted}` の排他 1 値。混合 route は未存在。
  - `TrackMonitorContext { arm, monitor, input_output_ready, muted, soloed, any_solo, hardware_direct }`。
  - `MonitorNotices { double_monitoring, hardware_direct_unknown, input_unavailable }`。
  - `HardwareDirectState::{DeviceControlled, UserReported, Unknown}` + `observed_active: Option<bool>` — 外部観測値。DAW は変更も制御保証もしない。
  - `MonitorProbe` — `GateScheduler`/`evaluate_gate` を共有し gate piece から `gate_open` を供給。monitoring は raw capture/capture writer に影響しない。
- `crates/synvelis-recording/src/gate.rs` — `GateSpec::{Normal, Punch, Loop, PunchInLoop}`、`GateSpan`/`CapturePiece` は session frame 半開 `[in,out)` + `project_in`/`project_out`（MusicalTick）を持つ。`GateScheduler`/`evaluate_gate` が区間評価の唯一の権威。
- `crates/synvelis-recording/src/journal.rs` — append-only binary framed record。`MAGIC=0x4A52`、`VERSION=1`、kind `1=Chunk / 2=Checkpoint / 3=Finalize`。`JournalSpan { pass_index, session_start, frame_count, source_start, project_in, project_out, media_offset }` — 既に SourceRange/Clock 証拠（source frame・project tick・media offset）を保持。replay は torn tail・bad header/version/length/CRC・sequence gap で停止し verified prefix のみ返す（破損 byte を truncate しない）。「consent」「handle」の語彙は未存在。
- `crates/synvelis-app/src/monitoring.rs` — `MonitorUxState`（session 観測 overlay、非永続）、`project.monitoring`（authored 宣言の正本: `lane`/`arm`/`monitor`/`channel_map`/`punch`）。session 宣言は project 再生成で reset。persistent Arm/Monitor 宣言・Undo・loop/punch UX・実 device/GUI 受入は RCD-03 時点で未対応/Deferred。
- `crates/synvelis-player/src/monitor.rs` — `MonitorAdapter` は route に応じ input/playback gain を ramp。`route==Input` で input 未供給 → `Muted` + `input_missing`（PCM 捏造なし）。native placement 保持、latency/PDC 補正なし。
- `crates/synvelis-player/src/monitor_engine.rs` — recording `MonitorProbe` + player `MonitorAdapter` を合成。monitor 出力は通常 engine 出力・raw capture・export と独立。現状 RT callback 外。
- RCD-02（`docs/designs/rcd-02-*`）— staged publish・journal replay verified prefix・recovery review（Verified/Partial/Invalid と実無音の区別・検証/取込/保留/破棄）。
- take/comping（`docs/{specs,designs}/recording-take-comping*`）— 非破壊保存・Comp は指定候補区間のみ・Undo は元 media を即削除しない。

## Scope

### In Scope

- UX Matrix（Off/Auto/On × Arm × Overdub × Punch/gate × Playback）の route 意味論
- Overdub の route 表現拡張の決定（`MonitorRoute` variant 追加）
- Feedback Risk の notice semantics（既存 notice 再利用 + 発火条件拡張）
- Capture Handle の consent/capacity/SourceRange/Clock 証拠・journal 表現・retention
- Undo/Recovery semantics（RCD-02 verified prefix 整合）
- Privacy 境界（consent なし永続化禁止）
- 後続実装 slice の想定ファイル・検証方針

### Out of Scope

- Rust 実装・テスト・fixture（後続 slice）
- 実 OS/GUI/Device/Plugin 受入（GUI-L2/EXT-L3）
- RT graph monitor node 搭載（RCD-03 Deferred のまま）
- Project-level consent 永続化（session-scoped grant で要件充足、YAGNI）
- 既存 broken relation 修復・第二正本の作成

## Components and Boundaries

| 層 | crate/module | RCD-F02 での責務 |
| --- | --- | --- |
| route 判定 | `synvelis-recording::monitor::decide` | Overdub 次元を含む route 判定の拡張点（`TrackMonitorContext` に overdub 宣言入力を追加、`MonitorRoute::Overdub` を返し得る）。純粋関数性・gate 権威非複製を維持 |
| gate 区間 | `synvelis-recording::gate` | handle 区間の評価も同じ `GateScheduler`/`evaluate_gate` 系の区間機械を再利用（第二 gate 実装を作らない）。handle は「gate 閉の consent 区間」として plan 凍結時に解決 |
| session/plan | `synvelis-recording::plan`/`session` | `PreparedRecordingPlan` に handle 宣言（consent 参照・容量・区間）を凍結。session 遷移 `Created→Open→Closed→Finalized` は不変 |
| journal | `synvelis-recording::journal` | kind=4 `HandleGrant` record 追加（consent・容量・有効 handle 区間・clock epoch・format の durable 証拠）。`JournalSpan`/`ChunkRecord`/`CheckpointRecord`/`FinalizeRecord` は不変 |
| capture | `synvelis-recording` capture engine/writer | take piece（gate open）に加え handle piece（consent 区間 ∩ gate 閉）を同一 span 機械で capture。take span の内容・順序・PCM を変更しない |
| monitor PCM | `synvelis-player::monitor`/`monitor_engine` | `MonitorRoute::Overdub` の gain 対応（input+playback 同時）。input 未供給時の fail-closed 規則。monitor 出力は capture/export に非干渉を維持 |
| UX/投影 | `synvelis-app::monitoring` | overdub 宣言の session/persisted 扱い、notice/route の投影、consent UX の要求契約。route/gate/latency 意味論を再実装しない |
| project | `synvelis-project` | `project.monitoring` への overdub 宣言 field 追加（authored 宣言のみ、PCM/consent token 非含有）。VCS diff 分類は既存 `TrackMonitorDecl`/`PunchMarks` 機構に乗せる |

## Data and Control Flow

### Overdub route 判定

```
TrackMonitorContext { arm, monitor, input_output_ready, muted, soloed,
                      any_solo, hardware_direct, overdub: bool }
        │
        ▼  decide(ctx, phase, gate_open)  — 純粋関数（既存を拡張）
MonitorDecision { route: MonitorRoute, notices: MonitorNotices }
        │
        ▼  MonitorProbe.feed → MonitorSegment（session frame 精度）
        ▼  synvelis-player MonitorAdapter.process_block
           Overdub → input_gain→1.0 かつ playback_gain→1.0（ramp）
           input 未供給 → effective Playback + input_missing（fail-closed）
```

### Capture Handle

```
ユーザー consent（session 開始前の明示 opt-in）
        │
        ▼  HandleSpec { 宣言 handle 区間（tick/session 相当）, capacity budget,
        │              clock_epoch, sample_rate/format }
        ▼  PreparedRecordingPlan 凍結時に gate schedule と照合
           → effective handle 区間 = 宣言 ∩ gate 閉（重複は plan で解決・記録）
        │
        ▼  journal 先頭に kind=4 HandleGrant record（consent・容量・
           effective 区間・clock epoch・format）— capture 開始前に durable 記録
        │
        ▼  capture engine: take piece（evaluate_gate）+ handle piece
           （consent 区間）を同一 JournalSpan 機械で media へ書込
        │
        ▼  replay/recovery: HandleGrant + spans を verified prefix で復元
           → span の session range が effective handle 区間に含まれるかで
             take/handle を分類（journal 単体で self-describing）
```

## Non-Functional Constraints

- **RT/noalloc**: monitor route 判定・handle 区間解決は plan 凍結時（非 RT）。RT callback への影響は handle piece の span 生成が既存 bounded 機械に乗る範囲に限定。実 RT noalloc 検証は実装 slice の `test-rt` 系で行う（本 slice NotRun）。
- **Fail-closed**: consent/capacity/区間証拠の無い handle capture は存在しない。journal replay は verified prefix のみ。未検証/torn data を受理済み capture として露出しない。
- **Privacy**: monitor route PCM（Input/Overdub の monitor 出力）はいかなる経路でも永続化しない（既存契約の維持）。handle 素材の永続化は HandleGrant consent が前提。
- **Honest capability**: Hardware Direct は `DeviceControlled`/`UserReported`/`Unknown` の 3 分類を維持。`Unknown` を制御可能と表示しない。外部観測値を DAW 側で変更しない。

## UX Matrix（baseline 転記 + Overdub 次元）

`decide` 既存判定表（monitor.rs L177-186）を baseline として転記。`pb = playback_route(ctx)` = mute/solo で `Muted`/`Playback` に解決される playback leg。

| monitor | 条件 | arm | gate_open | phase | Overdub=Off route | Overdub=On route（input leg 可聴時） |
| --- | --- | --- | --- | --- | --- | --- |
| Off | arm && gate_open | On | true | 任意 | `Muted` | `Muted`（input leg 無し → overdub 無効） |
| Off | 他 | 任意 | 任意 | 任意 | `pb` | `pb` |
| Auto | arm && (gate_open \|\| Stopped) | On | 任意 | Stopped or gate_open | `Input` | `Overdub`（pb 可聴時）/ `Input`（pb=Muted 時） |
| Auto | 他 | 任意 | 任意 | 他 | `pb` | `pb` |
| On | input_output_ready | 任意 | 任意 | 任意 | `Input` | `Overdub` / `Input`（pb=Muted 時） |
| On | !input_output_ready | 任意 | 任意 | 任意 | `pb` + `input_unavailable` | `pb` + `input_unavailable`（input leg 不可 → overdub 不成立） |

補足規則:

- `Overdub` route は「input leg 可聴 **かつ** playback leg 可聴（mute/solo 解決後）」のときのみ返る。playback leg が `Muted` に解決される場合は `Input` に退化する（混合の意味が無いため）。
- Monitor Off では overdub 宣言は route に影響しない（input leg が存在しない）。
- `gate_open` は `arm==On` のときのみ意味を持つ（既存契約）。handle 区間は gate 閉側にのみ存在し、route 判定の `gate_open` には影響しない。
- Punch/Loop の `gate_open` は `GateScheduler`/`evaluate_gate` の piece から供給される既存経路のみ。Overdub は punch 範囲・loop pass 数・take 数を変更しない。

## Feedback Risk（notice semantics）

既存 `MonitorNotices` を正本として再利用。新 variant は追加しない（決定: 下記「検討して却下した選択肢」参照）。

| notice | 発火条件（拡張後） | Overdub での扱い |
| --- | --- | --- |
| `double_monitoring` | **input leg が可聴な route**（`Input` または `Overdub`）かつ `hardware_direct.observed_active == Some(true)` | `route == Input` → `route ∈ {Input, Overdub}` に拡張。Overdub でも live input が software monitor 経路に乗るため Software+Hardware 二重監視・comb filtering リスクは同一に成立 |
| `hardware_direct_unknown` | `hardware_direct.state == Unknown`（route 不問） | 不変。`Unknown` を制御可能と表示しない |
| `input_unavailable` | `monitor == On` かつ `!input_output_ready` | 不変。overdub 宣言があっても input leg が無ければ overdub 不成立として `pb` に退避 |

- **Comb filtering**: 同一入力信号が software monitor 経路と hardware direct 経路で異 latency にて到達する場合に発生。`double_monitoring` がこの警告の正本 semantics であり、Overdub route でも input leg が可聴なら発火する（上表の拡張）。Overdub の playback leg は既存 take であり入力信号と同一波形ではないため、それ自体は comb filtering を構成しない。
- **Overdub 固有の追加リスク**: live input + playback の同時モニタは feedback 露出を増やすが、これは input leg 可聴一般（`Input` route と同じ）に内在するリスクであり、latency 品質は既存 `LatencyReport`/`LatencyResolution`（Measured/Reported/Estimated/Unknown・Clock Epoch・二重加算禁止）で報告される。新規 notice は追加しない。
- **表示義務**: route=`Overdub` は `Input`/`Playback` と視覚的に区別可能に投影すること（GUI-L2 の表示契約。実装は後続 slice）。`double_monitoring`/`hardware_direct_unknown`/`input_unavailable` は既存どおり投影する。

## Capture Handle Retention

### 定義

Capture Handle = Punch/Record Gate 閉区間のうち、**明示 consent された session-frame 半開区間**で capture される PCM handle 素材。用途は punch-in 時の pre-roll 音声確保・post-roll 余韻の非破壊保存であり、後続の take/comp 化は既存の非破壊経路（明示 adopt）を通る。

### 必須属性集合（HandleGrant record = durable 証拠）

| 属性 | 内容 | 根拠 |
| --- | --- | --- |
| consent | handle capture の明示 opt-in 記録（grant 識別・宣言時点の session/clock 文脈） | Issue「User Consent 必須」 |
| capacity | handle 素材の上限（frame/byte budget）。超過時は handle capture のみ停止し take capture は継続。打ち切りは journal から検出可能でなければならない | Issue「保存容量必須」・無制限の隠し Media 防止 |
| source range | `JournalSpan` 既存 field（`session_start`/`frame_count`/`source_start`/`project_in`/`project_out`/`media_offset`）を再利用。新しい区間表現を発明しない | Issue「SourceRange/Clock証拠必須」・表現ドリフト禁止 |
| clock | session の `ClockEpochId`・sample rate・format（plan 凍結値を記録） | clock 証拠 |
| effective ranges | plan 凍結時に解決した実効 handle 区間（宣言 ∩ gate 閉）。宣言と実効の差分を durable に残す | consent 範囲の自己記述性 |

### Journal 表現

- 新 record kind `KIND_HANDLE_GRANT = 4`（`HandleGrantRecord`）。`VERSION=1` は維持 — kind は u8 で拡張可能であり、version bump は既存 v1 journal 全てを reject するため不採用。
- `HandleGrantRecord` は当該 stream の最初の `ChunkRecord` より前に書く（capture 開始前の durable consent 証拠）。
- handle piece の PCM span は既存 `JournalSpan` で記録し、take span と同一 `ChunkRecord.spans` に含めてよい。分類は replay 時に `HandleGrant.effective_ranges` への session-range 包含で決定的に行う（handle 区間は gate 閉側のみ・take 区間は gate 開側のみで構造的に disjoint）。
- 旧 version の reader が kind=4 に遭遇した場合: decode 不能 → replay はその位置で停止 → verified prefix が当該 record 以前に切り詰められる。HandleGrant が先頭にあるため prefix_len=0 となり、recovery は「検証済み journal なし」として fail-closed に扱う（handle/take 素材を受理済み capture として露出しない）。downgrade 中の session 継続はサポートしない（staging は保持され review 可能）。

### Retention 規則

- handle 素材は session-scoped の staging media に保存され、project へは**自動 commit されない**。take/comp への昇格は既存の明示 adopt 経路（RCD-04 系の非破壊 take/comp contract）のみ。
- session `Finalized` 後の handle 素材は uncommitted stream media と同じ lifecycle（RCD-02 review: 検証/取込/保留/破棄）。`Discard` は既存 `MediaReferenceProbe` fail-closed に従う。
- handle 素材の Undo（adopt の取消）は project undo に従い、元 media を即削除しない（take/comping 契約）。
- consent は session-scoped。project への consent 永続化・「常に許可」は Deferred（YAGNI）。

## Undo/Recovery

Undo と Recovery は別 semantics:

- **Undo（決定的 project 操作）**: handle 素材の adopt・monitor/overdub 宣言変更・take/comp 操作は既存 project command + stable-ID 境界に従う undo/redo で復元する。undo は session journal・staging media を遡及変更しない。handle capture consent 自体は session 開始時の一回的 grant であり undo 対象外（取消は Discard 経路）。
- **Recovery（verified prefix）**: crash/device 切断時、journal replay は verified prefix のみを返す。HandleGrant より後の torn tail・未 sync の span は受理済み capture として露出しない（既存契約のまま、handle span も同じ規則）。handle 素材と take 素材は同一 stream の同一 verified frontier に従う — handle が verified なら take も同 frontier まで verified、という単位で扱う（stream 単位の atomicity を維持）。
- device 切断 → 既存 fail-stop → verified prefix までの Chunk/Checkpoint/Finalize で recovery review。handle 分類は HandleGrant + verified spans のみで行い、未検証 span を handle と推測分類しない。

## Privacy

- monitor route PCM（`Input`/`Overdub`/`Playback` の monitor 出力）は monitor bus に留まり、capture writer・journal・media・project のいずれにも書き込まない（既存契約の維持・拡張しない）。
- handle 素材（gate 閉 consent 区間の input PCM）の永続化は `HandleGrant` consent が前提。consent なしに punch 外素材を journal 記録・media 保存しない（Issue 受入条件「無許可の隠し Media 生成なし」）。
- `project.monitoring` に永続化されるのは authored 宣言（`arm`/`monitor`/`lane`/`channel_map`/`punch`/overdub flag）のみ。session 観測値（`MonitorUxState.track_sessions`）・consent token・PCM は永続化しない。
- 診断/telemetry は handle PCM 内容・monitor PCM を含めない。notice/route/区間メタデータのみ。
- 削除経路: handle 素材の破棄は recovery review の `Discard`（media 参照 probe fail-closed）と staging cleanup に従う。undo は media を即削除しない。

## Risks and Trade-offs

| Risk | 対応 |
| --- | --- |
| `MonitorRoute::Overdub` 追加が既存 consumer を壊す | exhaustive match の compile error で検出可能。adapter/engine/app の対応は実装 slice で網羅。`decide` の戻り型・純粋性は不変 |
| handle 区間と take 区間の重複で分類が曖昧になる | plan 凍結時に effective ranges = 宣言 ∩ gate 閉を解決し durable 記録。構造的 disjoint を契約化 |
| kind=4 追加で旧 reader が journal を読めない | fail-closed（prefix 切り詰め）を仕様として明記。downgrade 中 session は非サポート。VERSION 維持で v1 journal 互換は保つ |
| handle capacity 超過で take 録音を道連れにする | budget 超過は handle capture のみ停止。take は継続。打ち切りは journal の effective ranges と span 被覆差で検出可能 |
| overdub が feedback risk を過小表示する | `double_monitoring` を input-leg 可聴に拡張。route=Overdub の視覚的区別を表示義務化。latency 品質は既存報告を使用 |
| consent を project に残すと privacy 境界が複雑化 | session-scoped grant のみ。project-level consent は Deferred |

## Deferred Design Concerns

- 実装 slice（`synvelis-recording` Overdub/Handle/Take Layer、`synvelis-player` Monitor Cue、`synvelis-app` Consent/Warning UX・Fixture）| 本 slice は設計のみ | ADR merge 後の直列 dispatch | 実装契約が未確定のままでは手戻りが大きい
- 実 OS E2E（Loop/Take/Comp/Device 切断/Undo/Recovery・RT noalloc）| GUI-L2/EXT-L3 | #114 系受入 slice | headless 証拠を実機 Pass に換算できない
- RT graph monitor node | RCD-03 Deferred | RT graph slice | monitor engine は現状 RT callback 外
- Project-level consent 永続化 | YAGNI | 実利用での要望 | privacy 境界の複雑化を回避

## 検討して却下した選択肢

- **Overdub を `MonitorRoute` variant 追加ではなく「route + 別 flag の並列表現」にする案**: `decide` が単一 route を返す純粋関数契約と `MonitorAdapter` の gain 対応を複雑化し、`Muted`+overdub 等の不正組合せを型で排除できない。単一 variant `Overdub` の方が fail-closed で明示的。
- **`MonitorNotices` に `overdub_*` variant を追加する案**: Overdub の増分リスク（feedback 露出・latency skew）は input leg 可聴一般に内在し、`double_monitoring` の発火条件拡張と既存 latency 品質報告で表現可能。新 variant は投影・永続化・VCS 分類のコストに見合わない。
- **handle span を `JournalSpan` に material field 追加で区別する案**: payload layout 変更 → journal VERSION bump → 既存 v1 journal 全てが reject される。effective ranges による session-range 包含分類なら format 変更は kind=4 record 追加のみで済み、journal 単体で self-describing を維持できる。
- **handle を別 journal/別 media file にする案**: stream atomicity（verified frontier の単位）を崩し、RCD-02 recovery review の単位を二重化する。consent 区間も同一 stream の span として扱う方が契約が単純。
- **consent を project.monitoring に永続化する案**: session-scoped で要件を満たし、永続化は privacy 境界を曖昧にする。Deferred。

## Verification Notes

- [ ] `decide` の拡張（`TrackMonitorContext.overdub`・`MonitorRoute::Overdub`・`double_monitoring` 発火条件）が既存判定表行を変更しないことを headless fixture で検証（実装 slice）
- [ ] `HandleGrant` record の encode/decode・replay verified prefix・kind=4 の fail-closed を journal test で検証（実装 slice）
- [ ] handle effective ranges の plan 凍結解決（宣言 ∩ gate 閉・disjoint 保証）を plan test で検証（実装 slice）
- [ ] `MonitorAdapter` の Overdub gain・input 未供給時 Playback 退避を adapter test で検証（実装 slice）
- [ ] 実 OS/GUI/Device/Plugin 受入は #114 系別 Gate（本 slice 対象外・NotRun）
