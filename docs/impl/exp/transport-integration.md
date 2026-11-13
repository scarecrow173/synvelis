---
id: "EXP-034X7wokQkqbBkxJk5SmEd"
type: "experiment"
status: "completed"
title: "Transport Integration — Issue #20 設計契約文書化と TT 写像の試行"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/20"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/transport-integration.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/transport-integration-design.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/transport-integration-task.md"
    - "docs/test-specs/transport-integration.md"
  defers: []
  deferred-by: []
---

# Transport Integration — Issue #20 設計契約文書化と TT 写像の試行

## 問題設定

Issue #20 の Time / Transport / MIDI / Automation / Plugin
Integration v0.5（Span・Event・State Contracts）と TT-025〜030
監査補遺を canonical 文書鎖として確立する必要があった。上流
baseline（v0.5 architecture・#19 PPQN architecture）と TIM-01〜04
（#125〜128）の headless 実装済み slice・Ruviform pinned `fd1d4d5`
の generic 契約は存在するが、Issue #20 が要求する TT-001〜030 の
各ケースに対応する正本 + evidence 台帳がなかった。本 slice は
docs のみで crate を変更しないため、(a) 7 文書鎖の生成、
(b) TT→既存 test/oracle/状態の写像、(c) doc graph/index/review
の整合確認が試行対象となった。

## 試行と結果

### E1: 上流契約と実装 fixture の照合

`docs/designs/time-transport-midi-automation-plugin-integration-
architecture.md`（v0.5 Approved — 14 合意項目・TT-001〜024・
§14 TT-025〜030）・`docs/designs/configurable-ppqn-musical-time-
architecture.md`（#19）・TIM-01〜04 spec/design/test-spec/impl・
`crates/synvelis-player` の `process_boundary.rs`・`tim03_*`/
`tim02_*`/`tim_02_*`/`tim04_*` fixture・`crates/synvelis-project`
の `tim_01_*`/`tim_02_*`/`plugin_state.rs`・`crates/synvelis-
workspace` の `aut_01_*`/`aut_02_*`/`aut_03_*`・`crates/
synvelis-render` の `tim04_*` を照合し、TT-001〜030 の各ケースに
対応する既存 headless 証拠を特定した。Ruviform `fd1d4d5` の
`ruviform-media-core`/`plugin-core`/`plugin-host` が
`BeatPosition`/`EngineFrame`/`TransportScheduler`/`AppliedReceipt`/
`BlockContract`/`InBlockTransportSupport`/`NoteLedger`/
`build_call_event_list` を提供し、TT-ID を冠した upstream test
（`tt008`/`tt009`/`tt010`/`tt012`/`tt025`〜`tt030`）が独立
oracle として使えることを確認した。

**結果**: TT→test 写像が可能。ただし多くのケースが headless
関連契約の確認に留まり「Pass(部分)」となる。実 VST3/CLAP・
実 OS MIDI・実 device・GUI は EXT-L3/GUI-L2 で別途 NotRun/
Deferred。TT-003 の SMF SMPTE Division は #129 で Unsupported。

### E2: Owner 分離・時間/世代 domain の境界確認

Synvelis semantic（Project PPQN・Rational Beat・TempoMap・
Source Native Tick・Automation Write・Recording）と Ruviform
generic（Beat/Clock/Transport/Event/PluginHost/Probe）の依存
方向が `synvelis-* → ruviform-*` 一方向であること、DAW 型が
generic runtime に逆流しないことを design の Components 表で
固定した。Device Clock Epoch・Engine Frame・Recording Session
Frame・Project Musical/Media Time・Source Native Time・
TempoMapRevision・TransportGeneration・PlanGeneration を別
domain として分離し、異 domain の無検証比較・加算を禁じる
契約（TT-R-01）を確認した。

**結果**: Issue #20 の Owner 分離・時間/世代分離要求と一致。
Ruviform generic 不足は Synvelis 側で恒久重複実装せず upstream
Issue（ruviform#190/#188/#186/#189/#307/#196）へ起票する方針を
記録した。

### E3: Capability/Degraded・Offline Exact の分類規則

`Exact/FrameAccurate/BlockOnly/DegradedWithConsent/Unsupported/
Unknown`（能力 Class）と `Pass/Fail/NotRun/Unsupported`（検証
結果）を別語彙で記録する規則を test-spec TT-T-R-03 として固定
した。`InBlockTransportSupport` は `Verified` のみ in-block
transport 対応とみなし、規格に存在するだけで Sample-accurate
を保証しない（TT-027）。Offline Exact は未保証精度で
Fail-closed、Realtime Degraded は `DegradedConsent` +
Capability 判定 + receipt 記録が必須（TT-030）。

**結果**: TT-025〜030 の process legality（`min_frames ≤
frames ≤ max_frames`）・process-start offset・Offline
Fail-closed を契約化できた。headless mock の Pass を実 plugin
受入に繰り上げない規則を明文化した。

### E4: 文書鎖生成と index/relation 整合

`new_spec`/`new_design`/`new_plan`/`new_task`/`new_test_spec`/
`new_impl_record` の scaffold で 7 文書を生成し、front matter の
`source`/`depends-on`/`related`/`derives-from`/`derived-by`/
`implements`/`verifies`/`references`/`defers` を相互に整合させた。
IR は `docs/impl/ir/transport-integration.md` へ rename して
scope の file map に合わせた。`doc_maintenance.js index apply`
で触った dir の index を再生成した。

**結果**: 7 文書 + evidence/sv20 2 件が存在。index は生成物とし
て再生成。`doc_status lint`/`audit_impl_record`/`route_graph`/
`review-protocol` の結果は `evidence/sv20/transport-integration-
verification.md` に記録。

## 採用した判断

- TT-001〜024（TT-A/B/C/D 群）と TT-025〜030（監査補遺）を一意
  ID として固定し、重複・欠番を排除した。test-spec で一意性を
  検証。
- 「Pass(部分)」表記を採用し、headless で関連契約が確認できる
  が TT 全体の独立 oracle を満たさないケースを限定した。
- Plugin Event offset の起点を「各物理 process 呼出し開始」と
  明記し、Callback-relative/Span-relative の転送を禁じた
  （`frame_offset = event_engine_frame − process_start`）。
- Logical Transport Span ≠ Physical Plugin Process Call を分離
  し、min_frames 制約下での合法分割・Verified in-block
  transport・明示的 degraded の選択を TT-025/026/027 で固定。
- Offline Exact の Fail-closed と Realtime Degraded の consent
  必須を TT-030 で固定し、consent が Offline Exact 違反を
  救済しないことを明記。

## 結論

上流契約（v0.5 architecture・#19 PPQN architecture）と実装済み
headless fixture（TIM-01〜04・sf-03・pc-c4-07・aut/ctl・recording）
が Issue #20 の要求と整合することを確認し、TT-001〜030 を既存
証拠へ写像した canonical 文書鎖を確立した。実装・実 plugin・
実 OS・実 MIDI device・GUI・OS 受入は一切主張せず、それらは
#125〜128・#129/#130・#114・ruviform#190/#196/#253・EXT-L3/
GUI-L2 に委譲する。
