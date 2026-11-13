---
id: "SPEC-034X6Wo5jGoog4i61651vc"
type: "spec"
status: "approved"
title: "Playback / Mixer / Generic Execution Graph — Typed Port, PDC, Plan Activation (EX-001〜020 design contract)"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/18"
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
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "https://github.com/scarecrow173/synvelis/issues/18"
  derived-by:
    - "docs/designs/playback-graph-design.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Playback / Mixer / Generic Execution Graph — Typed Port, PDC, Plan Activation

## What Should Be Built

Issue #18 の canonical 設計契約: Synvelis の DAW 意味論
（Track/Bus/Send/Automation・Project state・routing 意味論・source
mapping）と、Ruviform の再利用 generic primitive（typed Graph /
Plan / RT / Compensation / Plugin Compiler）の Owner 分離を固定し、
Typed Port・Channel 契約、PDC 責務分割、Configured/Prepared/
Published/Active/Observed 世代、ProjectController CAS と speculative
plan 規則、Last-good 保持、失敗分類と回復、EX-001〜020 受入
マトリクスを一つの正本として文書化する。

本 slice は **docs/ 設計契約のみ**。実装は #104〜#109（Player/Render
Owner）、#114（GUI）、Ruviform #294（両 consumer 受入）が担う。
設計完了を実装・実 PCM・実 Plugin・実 device・GUI・OS 受入と
みなさない。

## Why Now

- 既存 `docs/designs/playback-mixer-generic-execution-graph-architecture.md`
  （Approved v0.4）は 12 原則と EX-001〜020 マトリクスを持つが、
  canonical spec/design/plan/task/test-spec/impl の 7 文書鎖と
  evidence 台帳が未整備。
- SF-03（#106 系）が semantic→runtime 写像・activation receipt・
  fault 分類を headless 実装済みで、Issue #18 の設計契約はそれを
  Playback/Mixer/Generic Graph の正本として再確認・拡張する必要がある。
- 実装 Owner（#104〜109）が着手する前に、Typed Port/PDC/Plan
  Activation/Last-good/失敗分類の判断基準を固定しないと、consumer
  ごとの解釈差異が生じる。

## Users and Value

- 実装者（#104〜109）: lowering・activation・fault・検証の判断基準。
- Reviewer: Owner 分離と evidence の誠実さ（headless≠実機受入）を
  一意に判定できる。
- 将来の consumer（probe/control/render/UI）: 同一 generation
  namespace と receipt 契約を共有できる。

## Scope

### In Scope

- Typed Port / Channel 契約（format・layout・明示 converter）。
- Generic Graph / Plan の責務と、Synvelis semantic lowering。
- Track/Bus/Send/Automation の意味論と generic runtime への非漏洩。
- PDC 責務分割（lowering/runtime contract vs DAW 第二権威の禁止）。
- Configured/Prepared/Published/Active/Observed 世代と receipt。
- 単一 RT process による stateful ownership、plan retirement、
  dynamic recompilation。
- Structural 変更 vs Fast Control の分類。
- Source mapping / probe evidence。
- Playback/prefetch/underrun/seek/loop/MIDI note-off/offline/
  master/stem export/capture-monitor-playback 分離。
- 失敗分類と回復（stale revision, capability drift, invalid
  graph/port/channel, cycle, missing source/plugin, queue full,
  pending activation, retirement overflow, latency change,
  unsupported mapping, prefetch underrun, device/clock epoch）。
- ProjectController CAS と speculative plan 規則。
- Project revision と execution-plan revision の区別。
- Last-good plan / configured project の保持。
- EX-001〜020 の受入ケース写像と capability 別状態
  （Pass/Fail/Unsupported/NotRun）の記録規則。

### Out of Scope

- 実装コード（`crates/`、`Cargo.toml`、`Cargo.lock`、CI、Ruviform）。
- GUI 実装（#114）、実 device/plugin 受入（EXT-L3）。
- Ruviform Graph/RT/Plugin Compiler の DAW 独自コピー。
- Project Source / Host Parameter の第二正本。
- 設計だけによる実 PCM / 実 Plugin PASS の宣言。

## Requirements

- **PG-R-01 Owner 分離**: Synvelis は DAW 意味論（Track/Bus/Send/
  Automation、Project state、routing 意味論、source mapping）の
  authoritative owner。Ruviform は再利用 generic typed Graph/Plan/
  RT/Compensation/Plugin Compiler primitive の owner。DAW 固有型を
  generic runtime graph に逆流させない。generic runtime に不足が
  あれば Synvelis 側で恒久的重複実装せず Ruviform Issue へ起票する。
- **PG-R-02 Typed Port / Channel 契約**: 全 edge は型付き Port を
  持ち、format/channel/layout の互換性を lowering/compile 時に
  検証する。channel layout 不一致は reject または明示 converter
  のみ許容し、暗黙 upmix/downmix・resample をしない。
- **PG-R-03 Generic Graph**: arbitrary-port DAG を表現し、決定的
  lowering 順序を持つ。cycle（zero-delay feedback を含む）は
  compile/validation で拒否する。
- **PG-R-04 Semantic Lowering**: Track/Bus/Send/Automation の意味を
  決定的に generic graph へ lower する。main+send fan-out は source
  を一度だけ実行し、branch→return は決定的な mix 順序を持つ。
  aux sidechain は capability 検証付きで分離する。
- **PG-R-05 PDC 責務分割**: PDC（plugin delay compensation）の機構は
  Ruviform typed runtime の Slot/PDC と `synvelis-render` の offline
  render job が所有する。semantic lowering は PDC を直接符号化せず、
  realtime/offline で同一 topology を lower することで同一意味の
  compensation を可能にする。latency 変更は非 RT recompile を伴う。
- **PG-R-06 Plan 世代**: Configured → Validated → Prepared →
  Published → Active（RT 観測後のみ mint）→ Observed/Retired を
  区別し、各遷移を receipt で記録する。Published ≠ Active。
  frames=0・経過時間・EngineStub build count は Active 証拠にしない。
- **PG-R-07 CAS / Speculative Plan**: `ProjectController.apply` の
  CAS commit 前に speculative compiled plan を publish しない。
  commit 後に capability/format/clock を再照合して正式 plan を
  作成・publish する。RT Applied 前は Active と主張しない。
- **PG-R-08 Stateful 単一 RT Ownership**: stateful processor/plugin
  instance は単一 RT process が所有し、plan 交換時は世代連動で
  lease/teardown する。retirement queue は bounded で、飽和は
  `RetireOverflow` fault（clean success ではない）。
- **PG-R-09 Structural vs Fast Control**: fader/send gain/automation
  等の fast control は topology recompile なしで適用する。
  structural 変更（routing/insert/plugin 構成）は再生中でも RT
  thread を block せず非 RT で recompile→publish する。
- **PG-R-10 Playback / Media**: prefetch miss は明示 underrun、
  EOF/seek/loop は決定的。seek/loop 後の MIDI note-off 残りを
  0 にする（panic/chase 契約）。capture/monitor/playback を分離する。
- **PG-R-11 Offline / Export**: 安定 node で live/offline PCM が
  同値。master/stem export は分離された job。stateful master
  render は逐次 execution を維持し、独立解析・cache 作業と混同
  しない。実 WAV export は Finalize→別 Reader Reopen の証拠を伴う。
- **PG-R-12 Probe / Source Map**: consumer（probe/control/PCM/
  render/undo/save）は `ProjectRevision`+plan `generation` を
  相関キーとし、独自の active 判定を持たない。source map は
  generation-scoped で publish 時に検証され、plan swap/plugin
  reload/port reorder/generation 遷移で無効化される。
- **PG-R-13 Failure Taxonomy / Recovery**: stale revision・
  capability drift・invalid graph/port/channel mapping・cycle・
  missing source/plugin・queue full・pending activation・
  retirement overflow・latency change・unsupported mapping・
  prefetch underrun・device/clock epoch change を型付きで区別し、
  Plan/Plugin Process/RT fault で Last-good graph・configured
  project・receipt を保持する。GUI/Project 保存と RT 実行世代の
  相違を観測可能にする。
- **PG-R-14 Evidence Honesty**: 実 device/実 plugin/OS が必要な
  gate は capability 別に Pass/Fail/Unsupported/NotRun を明示し、
  headless 証拠だけで実機受入とみなさない。設計レビュー Pass と
  実測実装 Pass を区別する。
- **PG-R-15 Revision 区別**: Project revision（semantic 状態）と
  execution-plan revision/generation（runtime 状態）を別物として
  記録し、NoRebuild（semantic 等価）でも新 Revision が Active に
  なったという偽 receipt を出さない。

## Acceptance Criteria

- [ ] **PG-AC-01**: EX-001〜006 — Audio Source 決定的 PCM、Send
  分岐/合流の fan-out 一回実行、branch→return 決定的 mix、aux
  sidechain 分離+capability 検証、異種 channel layout の reject/
  明示 converter、zero-delay feedback cycle 拒否を Typed Graph と
  PCM fixture で検証する設計・写像が存在する。
- [ ] **PG-AC-02**: EX-007〜011 — 128/512 frame 経路と 384 frame
  PDC 補償、plugin latency 動的変更の非 RT recompile、再生中
  graph 構造変更の RT block なし、stateful processor の単一 RT
  実行、retirement queue 飽和時の安全性を検証する設計・写像が
  存在する。
- [ ] **PG-AC-03**: EX-012〜015 — fader/send automation が topology
  recompile なし、seek/loop 時の MIDI note-off 残り 0、prefetch
  miss の明示 underrun、recording raw capture と
  monitor/playback/PDC の独立性を確認する設計・写像が存在する。
- [ ] **PG-AC-04**: EX-016〜020 — 安定 node の live/offline 同値、
  非決定的 plugin の非保証明示、master/stem export 分離、2 地点
  probe evidence、Project revision と Active plan revision の
  相違記録を確認する設計・写像が存在する。
- [ ] **PG-AC-05**: Synvelis semantic routing/Track/Bus/Clip 意味論と
  Ruviform generic typed graph/compiler/RT/PDC/publish の Owner
  分離が文書化され、generic 公開型への DAW 意味論逆流を防ぐ規則が
  ある。
- [ ] **PG-AC-06**: CAS commit 前の speculative plan 非公開、
  commit 後の capability/format/clock 再照合、RT Applied 前の
  Active 非主張が文書化されている。
- [ ] **PG-AC-07**: Plan/Plugin Process/RT fault で last-good
  graph・configured project・receipt を保持し、GUI/Project 保存と
  RT 実行世代の相違を観測できる設計がある。
- [ ] **PG-AC-08**: stateful master render の逐次 execution 維持と、
  実 WAV export の Finalize→別 Reader Reopen 証拠要件が明記されて
  いる。
- [ ] **PG-AC-09**: 実 device/実 plugin/OS 必須の EX gate が
  capability 別 Pass/Fail/Unsupported/NotRun として分類可能で、
  headless 証拠を実機受入と混同しない記録規則がある。
- [ ] **PG-AC-10**: EX-001〜020 の各ケースが test-spec case・既存
  Synvelis/Ruviform test・独立 oracle・現在の観測状態・
  Required/Deferred/NotApplicable・Pass/Fail/NotRun/Unsupported に
  写像され、仕様だけの EX ID を Pass 扱いしない。

## Deferred Design Concerns

- 実 device/実 plugin/OS GUI の EX gate 実測 | headless 設計契約のみ
  | #114 GUI gate・EXT-L3 実機タスク・Ruviform #294 | 実装 Owner が
  headless Pass を実機受入と誤読する。
- Ruviform generic typed graph の任意 port DAG 化の完成度 | pin
  `0f5b5737` 時点の API で本契約は記述可能だが、upstream 完成は別
  Issue | ruviform#184/#308/#309 | Synvelis 側で DAW 独自コピーを
  恒久実装する誘惑。
- SF-ACT-07 preview vs export fail-closed policy | policy 未規定 |
  後続 Issue | preview/export で失敗時挙動が分岐する。

## Sources and Relations

- Issue #18 本文（EX-001〜020 受入・Owner 分離・禁止事項）。
- `docs/designs/playback-mixer-generic-execution-graph-architecture.md`
  （Approved v0.4 — 12 原則・PoC order・EX matrix の上流 baseline）。
- `docs/specs/sf-03-map-semantic-runtime-contract.md` と
  `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
  （semantic→runtime 写像・activation receipt・fault 18 分類の
  headless 実装済み契約）。
- `docs/specs/l1-*`/`l2-*`/`x-*`/`tim-02-*`（playback/export/MIDI
  の確定済み縦断契約）。
- `docs/specs/sf-06-cross-gate-oracle-case-ledger.md` と
  `evidence/reports/sf-06-cross-gate-ledger.md`（T0〜T4 gate
  分類・headless Pass の部分性）。
- 関連 Issue: synvelis #4/#6/#8/#10/#12/#17/#19/#65/#84/#100〜109/
  #112/#114/#145〜167/#184/#219/#230/#277〜284/#294/#330/#365/#368、
  ruviform #59/#113/#184〜189/#193/#219/#294/#308/#309。
