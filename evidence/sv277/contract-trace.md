# SV-277 Contract Trace — Issue #277 合意項目 → lease 3 ファイル記述箇所

DESIGN-ONLY slice の contract-trace。Issue #277 本文（2026-11-11 合意）の
各合意項目が、lease 済み 3 文書のどこに記述されたかを対応付ける。
実装・受入の PASS を意味しない。

## 文書

- Spec: `docs/specs/creative-branch.md`（SPEC-034Xe4RFguDtxkqBKi6834, draft）
- Design: `docs/designs/creative-branch-design.md`（DESIGN-034XecqdTFl5brsp93RKYN, draft）
- ADR: `docs/adr/cb-00-creative-branch.md`（ADR-034Xe4SL0Sj3r7V85VOaFO, proposed）

## Issue #277 合意項目 → 記述箇所

| Issue 合意項目 | Spec | Design | ADR |
| --- | --- | --- | --- |
| 概念型 VariantSet/Branch/VariantScope/ComparisonSession・SnapshotIdentity（ProjectId+固定 locator+bytes digest+schema/projection、Revision 単独不可） | §「製品契約となる概念」CB-R-04〜05、受入 CB-AC-01 | §3.1 概念型、§3.2 既存型写像、§3.3 ID/Lineage 規則 | Decision Outcome Option 1、Context |
| Branch-local 生成物と Live の Typed Stable ID 分離・Adopt 時の決定的 ID mapping/参照再結線 | CB-R-05、CB-AC-04 | §3.3、§7.2、CB-ID-01 | Consequences、Patterns to avoid |
| 唯一の Writer = ProjectController・versioned persistent domain 真値 | CB-R-02、CB-AC-07 | §2.1、§4.2、§9.1 | Decision Drivers、Option 3/4 却下理由 |
| Fork/Edit/Adopt/Undo 状態機械・Editing/Comparing/Auditioning/Adopted/Runtime Active 別状態 | CB-R-03、CB-AC-03 | §4.1〜4.3（T-1〜T-10 遷移表） | Consequences |
| 採用候補編集=Live と同一 Controller 操作で同期（二重正本なし）・非採用候補=永続候補のみ | CB-R-03、CB-AC-03 | §4.2、§4.3 T-2/T-3、CB-SM-01/02 | Decision Drivers |
| 7 Domain Scope→shared/owned→Fork/Edit/Compare/Adopt/Discard 表を型・Owner・状態遷移・失敗型へ | CB-R-04、CB-AC-02 | §5 Domain Adapter Contract（7+1 行） | Context、Implementation Plan |
| Processing 分岐境界（Stable Semantic Processor/Port・初期範囲・外部 Send/Sidechain/多出力は検証または Reject/Unsupported） | CB-R-06、CB-AC-05 | §6 Processing Branch Boundary Contract、CB-PROC-01/02 | Considered Options、Patterns to avoid |
| Compare 3 層（Semantic/Signal/Perceptual）・固定 Evaluation Context・時間整合・失効・Audio Diff≠Perceptual 表示 | CB-R-07、CB-AC-05 | §7.1、CB-CMP-01〜03 | Consequences（Bad: 実信号依存） |
| Configured/Measured/Inferred/Unknown・Aligned/Approximate/Unaligned/Unavailable 区別 | CB-R-07 | §7.1 証拠状態語彙 | — |
| Source/Raw Take 非破壊・Recording 所有は #12 系 | CB-R-08、CB-AC-06 | §5 補足、§10 非破壊 | Decision Drivers |
| Adopt=単一 ProjectController CAS/Composite + 1 Undo・Preflight fingerprint・Stale/Conflict は自動 Rebase/Merge せず拒否 | CB-R-07、CB-AC-04 | §7.2、CB-ADP-01 | Decision Outcome Option 1 |
| Project CAS 成立と RT Plan Publish/Activation は非原子的別段階・別 Receipt・Last-good/Pending/Retry・Project 巻き戻しなし | CB-R-07、CB-AC-01 | §7.3、CB-ADP-02 | Consequences、related ADR sf-03/pc-c1 |
| Missing Plugin で Edit 可能/Exact 不可を区別 | CB-R-07、CB-AC-01 | §7.4、CB-RCP-01、CB-ADP-04 | Consequences |
| Evaluation/Resource 契約（Live vs Offline/Cache・CacheHit 条件・Compare 専用と共有 Verified Artifact の区別） | CB-R-07 | §7.4 | — |
| RCP/2026-11-11 合同契約（Branch=Project versioned 正本・Artifact=非正本派生・cross-branch CacheHit・closure 検証・RCP-05 #294 接続） | CB-R-07 関連、§Sources | §2.2 前提、§7.4 | More Information（#289/#294） |
| Owner/DAG（Synvelis/Ruviform/AudioLens 責務分離・CB-01 先行正本・並行開始規則・Start/Integration/Acceptance 区別・#288 Cross-Gate 合流） | CB-R-08、CB-AC-08 | §9.1〜9.3 | Decision Drivers、§9.2 表 |
| 既存 Issue Owner 重複・DAG 循環なし（#10/#12/#17/#7/#104/#105/#106/#121/#122/#128/#114・Ruviform #308/#309/#294） | CB-AC-07 | §9.3 監査表 | More Information |
| 通常 DAW Gate A 独立・VCS optional・AI OFF・AudioLens 非回帰を下流 Issue に明記 | CB-R-08、CB-AC-08 | §9.2 Issue 表、§10 | Decision Drivers、Option 2 却下 |
| Oracle/Case ID（CB-RCP-01 先頭・Reject/Degraded/Unsupported 失敗行列・二重正本禁止・無断誤参照/無断変更 Reject） | CB-R-08、CB-AC-04〜06 | §8.1〜8.3 | Confirmation |
| Legacy v5 migration 別実装せず #17/#86/#92 versioned reader 再利用・旧 Schema projection 不明は Ambiguous/Unknown | CB-R-05 | §3.3、§8.3、CB-ADP-05 | Patterns to follow/avoid |
| Graph Spec/Node ID/Port ID/Plan Generation は Ruviform Runtime 識別子で Project semantic ID へ昇格しない | CB-R-05 | §6 Runtime 識別子の分離 | Patterns to avoid |
| 旧 WebView/SolidJS を正式 UI 契約にしない | CB-R-08 | §10 UI 技術 | Patterns to avoid |
| Semantic Merge/Cherry-pick/Blend/Mix は初期対象外 | CB-R-01 Out of Scope | §1、§12 Deferred | Option 5 却下 |

## 既存コード根拠（READ-ONLY 照合）

| 設計上の主張 | 確認した既存根拠 |
| --- | --- |
| 唯一 Writer・expected_revision CAS・validate→scratch→atomic swap・undo/redo 新 revision | `crates/synvelis-project/src/controller.rs` |
| SnapshotIdentity = ProjectId+SnapshotOrigin+FixedSnapshotLocator+ContentDigest+schema/projection | `crates/synvelis-project/src/snapshot/types.rs` |
| Semantic Diff engine・ComparisonSpec/Domain・tri-state・AmbiguityReason | `crates/synvelis-project/src/diff/{engine,adapters,types}.rs` |
| TakeGroup/Comp/CompRegion/CompCandidateRef/CompTransition/CompRegistry・active_comp_id | `crates/synvelis-project/src/comp.rs` |
| MIDI comp・MidiCompSelection・compile_midi_comp | `crates/synvelis-project/src/midi_comp.rs` |
| StateCaptureQuality・PluginStateFailureKind・Receipt/RestoreReceipt | `crates/synvelis-project/src/plugin_state.rs` |
| PluginInstanceId・PluginInstanceIdentities（schema v13） | `crates/synvelis-project/src/plugin_instance.rs` |
| RoutingModel/ChannelStrip/ProcessorChain/Processor/PluginRef/SemanticPort/RoutingConnection/SendConfig・RoutingEditIntent/preflight/RoutingPlan/RoutingEditError | `crates/synvelis-project/src/routing.rs`・`routing_tx.rs`・`routing_build.rs` |
| 順序付き migration registry・SCHEMA_VERSION=13・head CAS save | `crates/synvelis-project/src/migration.rs`・`store.rs` |

## 既存 docs 根拠

- `docs/specs/project-model.md` PM-R-06〜R-12（lineage・非破壊・単一 Writer・CAS・Undo/Redo）。
- `docs/specs/project-persistence.md`・`docs/designs/project-model-design.md`（snapshot/head CAS/migration）。
- `docs/specs/vcs-01-immutable-snapshot-resolver.md`・`vcs-02-semantic-diff-changeset.md`（read-only Snapshot・Semantic Diff）。
- `docs/specs/vcs-integration.md`・`docs/designs/vcs-integration-design.md`（Creative Branch ≠ VCS branch・VCS Off）。
- `docs/specs/sf-03-map-semantic-runtime-contract.md`・`playback-graph.md`（Configured→Active・last_good・generation）。
- `docs/specs/tim-04-plugin-state-snapshot-consistency.md`（Plugin State 品質 Receipt）。
- `docs/specs/recording-take-comping.md`・`mda-takegroup-comp-effective-projection-placement-contract.md`（Take/Comp 非破壊参照）。
- `docs/specs/musical-time.md`・`tim-02-smf-format01-import-export.md`（MIDI Source/SMF native fidelity）。
- `docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md`（semantic port/processor ID）。
- `docs/specs/media-render.md`・`command-gateway.md`（render pipeline・Gateway Action）。
- `docs/designs/execution-plan-design.md`（SV-CB DAG node/edge の正本）。

## 判定

- Issue #277 合意項目は全て lease 3 文書内に記述箇所を持つ（本表で網羅）。
- 設計・実装・Gate PASS ではない。status は spec=draft / design=draft /
  adr=proposed のまま人間承認待ち。
