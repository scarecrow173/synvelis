# SV293 — RCP-04 Missing Plugin Resolver Fault Matrix (synvelis#293)

Issue #293（RCP-04）headless slice の oracle 検証台帳。
`crates/synvelis-player/tests/rcp_04.rs`（26 test）の実行結果と、
Issue 受入条件 RCP-EX-01〜06 の headless 対応状況を記録する。
本台帳は **Fake/Domain Oracle の判定契約検証** であり、実
Plugin・実 OS・実 RT・PCM 実測の Pass を意味しない（POLICIES.md
§検証レベル: 本 slice は OS-L1 相当の非 RT 決定契約）。

## Environment

| 項目 | 値 |
| --- | --- |
| Repo | scarecrow173/synvelis |
| Branch | `feat/293-missing-plugin-resolver`（worktree `sv293-rcp4`） |
| OS | Windows (win32) |
| Toolchain | mise 管理 Rust 1.99（`mise.toml`/`mise.ci.toml` 正本） |
| Ruviform resolved | `fd1d4d5f`（`Cargo.lock` pin・変更なし） |
| 検証入口 | `mise run fmt-check` / `check` / `lint` / `test`（canonical） |

## 実行した検証

| # | コマンド | 目的 | 終了コード | 結果 |
| --- | --- | --- | --- | --- |
| 1 | `mise run fmt-check` | 体裁検査 | 0 | Pass |
| 2 | `mise run check synvelis-player` | 型検査（lib+test） | 0 | Pass |
| 3 | `mise run lint synvelis-player` | clippy 全 target | 0 | Pass（warning 0） |
| 4 | `mise run test synvelis-player` | 全 test suite | 0 | Pass — `tests/rcp_04.rs` 26/26、既存 suite も全 pass |
| 5 | `mise run check synvelis-render` | 下流 build integrity | 0 | Pass |
| 6 | `mise run check synvelis-app` | 下流 build integrity | 0 | Pass |

## Fault matrix / Case 台帳

凡例: **Pass** = 実装済み oracle test が検証 / **NotRun** = 本
slice 対象外・後続 Gate 待ち / **Deferred** = 別 lease・後続
Issue へ明示的に送った残件。

### RCP-EX-01 — Intent ごとの Plan/Blocked/Preview

| Case | Fixture/状況 | Expected | Test | 結果 |
| --- | --- | --- | --- | --- |
| EX-01a | Missing instrument + checkpoint→Live EQ、MainPlayback、EligibleExact artifact | instrument を substitution、EQ は live region、Exact Plan | `ex01_missing_instrument_checkpoint_substitutes_exact` | Pass |
| EX-01a' | 同上で artifact が preview class のみ | 偽 Exact なしで Blocked | `ex01_preview_intent_requires_opt_in_for_preview_artifact`（MainPlayback 側）・`ex01_missing_plugin_no_artifact_blocks_main_playback` | Pass |
| EX-01b | Missing reverb + final checkpoint→Live Compressor | reverb 置換・compressor live、Exact Plan | `ex01_missing_reverb_final_checkpoint_keeps_compressor_live` | Pass |
| EX-01c | OfflineRender / Export intent | exact のみ許容・非等価は fail-closed | `ex01_offline_render_and_export_require_exact` | Pass |

### RCP-EX-02 — cutset 境界完全性

| Case | Fixture/状況 | Expected | Test | 結果 |
| --- | --- | --- | --- | --- |
| EX-02a | multi-out の片出力のみカバー | 未カバー必須 output で Rejected | `ex02_multiout_requires_every_output_port_covered` | Pass |
| EX-02b | 必須 MIDI/Event output を持つ node | typed support なし artifact は Rejected | `ex02_required_midi_output_needs_typed_artifact_support` | Pass |
| EX-02c | Send/Return + parallel + sidechain + fan-in PDC | 境界 pin/domain/format/latency 不一致は Rejected、誤配送 0 | `ex02_send_return_boundary_input_must_be_pinned`・`ex02_latency_mismatch_rejected` | Pass |
| EX-02d | region feed が未カバー | dangling input で Rejected | `ex02_dangling_input_when_region_feed_uncovered` | Pass |
| EX-02e | 同一入力への二重 feed | double-add で Rejected | `ex02_double_feed_rejected` | Pass |
| EX-02f | 外部 sidechain が pin されていない | 旧 render 再利用を Rejected | `ex02_unpinned_sidechain_input_rejected` | Pass |
| EX-02g | optional `min==0` 入力が未接続の正常系 | dangling 誤判定しない（regression guard） | `ex02_send_return_boundary_input_must_be_pinned` 内包 | Pass |

### RCP-EX-03 — 偽 Exact なし / Preview 明示

| Case | Fixture/状況 | Expected | Test | 結果 |
| --- | --- | --- | --- | --- |
| EX-03a | Unsupported/Corrupt/Stale/ACL Denied artifact | MainPlayback で Exact を名乗らない | `ex03_bad_artifact_states_never_exact_for_main_playback`・`ex03_acl_denied_and_stale_pin_rejected` | Pass |
| EX-03b | Unknown plugin state が required path 上 | Blocked + typed verdict | `ex03_unknown_plugin_state_blocks_required_path` | Pass |
| EX-03c | 同上を preview + 明示 opt-in | NonEquivalent + skipped node 記録（Exact 偽装なし） | `ex03_unknown_plugin_state_blocks_required_path`（preview 側 assertion） | Pass |
| EX-03d | plugin verdict の型付け | Missing/Denied/Drift 等が typed | `ex03_missing_plugin_verdicts_are_typed` | Pass |

### RCP-EX-04 — Edit 分類（render stale と独立）

| Case | Fixture/状況 | Expected | Test | 結果 |
| --- | --- | --- | --- | --- |
| EX-04a | rename / UI-only 変更 | NoRebuild | `ex04_rename_is_no_rebuild` | Pass |
| EX-04b | bound fast parameter / automation event | ControlOnly（graph rebuild なし） | `ex04_bound_parameter_and_automation_are_control_only` | Pass |
| EX-04c | structural / capability / render substitute 変化 | Rebuild | `ex04_structural_capability_and_render_changes_rebuild` | Pass |
| EX-04d | source media content 変更（pin 非対象） | pins 同一でも Rebuild（fail-closed） | `ex04_source_content_edit_rebuilds` | Pass |

### RCP-EX-05 — Safe Plan Reconciliation / stale publish

| Case | Fixture/状況 | Expected | Test | 結果 |
| --- | --- | --- | --- | --- |
| EX-05a | R1→R2→R3 連続編集、古い prepare が後着 | Stale/Superseded で publish 拒否・誤 Activation 0 | `ex05_rapid_edits_never_activate_stale_plan`・`ex05_out_of_order_publish_rejected` | Pass |
| EX-05b | clock epoch 変更 / plugin restore failure | DeviceEpochChanged 等 typed fault・last-good 保持・旧 gen 配送遮断 | `ex05_plugin_restore_failure_and_epoch_change_block`・`ex05_reconcile_detects_drift_and_gates_delivery` | Pass |
| EX-05c | QueueFull / RetireOverflow | Failed receipt・last-good 保持・current mismatch | `ex05_queue_full_and_retire_failure_retain_last_good` | Pass |
| EX-05d | RT 観測 generation/epoch と plan pin の照合 | epoch 不一致・stale generation は Active にしない | `ex05_active_requires_matching_generation_and_epoch` | Pass |

### RCP-EX-06 — 実機 Gate（PCM/実 Plugin/実 OS/実 RT）

| Case | 内容 | 結果 | 理由 |
| --- | --- | --- | --- |
| EX-06a | Seek/Loop/Tail/PDC/frame accuracy の PCM 実測 | **NotRun** | 実 Plugin・実 OS・実 RT が必要。RCP-GATE（synvelis#296）へ送る |
| EX-06b | 実 Plugin binary での capability/restore 検証 | **NotRun** | 同上。headless verdict は Fake capability snapshot のみ |
| EX-06c | RT thread の alloc/FS/block 計測 | **NotRun** | 本 slice の resolver は非 RT 純粋関数であり RT 配置は未実施。tim03 系の既存 RT oracle が別途担保 |

## Semantic dependency / 対応表

| 依存 | 関係 | 本 slice での扱い |
| --- | --- | --- |
| synvelis#106（semantic lowering / ExecutionPlan） | `RoutingBuildKey`・`RoutingFault`・`RoutingActivationState` 等の既存契約を **READ-ONLY 参照** して reconcile に再利用。二重の activation 正本を作らない | Pass（参照のみ・変更なし） |
| synvelis#104 / #105 / #109（safe activation・fault oracle 統合） | plan activation/last-good の意味論を既存 `synvelis-project` 型に揃えた | Pass（型整合を test で確認） |
| synvelis#290 RCP-01（recipe/dependency closure） | `RenderCandidate`/`ArtifactOutput` は eligibility state を **入力として消費** する typed contract。RecipeKey/ContentHash の正本は #290 領域 | Deferred（本 slice は消費側のみ） |
| synvelis#291 RCP-02（immutable render media/manifest/store） | artifact content hash・state 検証は manifest 契約を仮定 | Deferred（同上） |
| synvelis#292 RCP-03（render policy/job/completeness） | candidate eligibility 分類の供給元 | Deferred（同上） |
| synvelis#294 RCP-05（snapshot references） | `ResolvedGraphSnapshot` の provenance は #294 契約に従う想定 | Deferred |
| synvelis#295 RCP-06（UX/observability） | verdict/receipt の UI 露出は未実装 | Deferred |
| synvelis#296 RCP-07 / RCP-GATE | 実機 acceptance・PCM oracle | Deferred（EX-06 全件） |
| synvelis#297 RCP-F01 | remote renderer PoC | 対象外 |

## Deferred residual（lease 外・後続作業）

| 残件 | 必要なファイル/範囲 | 理由 |
| --- | --- | --- |
| workspace semantic→selection integration | `crates/synvelis-workspace/` | 別 Worker の lease。Issue #293 本文が許容する統合は本 slice では未実施。`ExecutionSelection` を workspace の plan 構築へ接続する配線が残る |
| 実 Plugin/実 OS/実 RT/PCM Gate | `crates/synvelis-render/`・test lane・#296 | RCP-EX-06 全件。headless Pass を実機 Pass に昇格しない |
| Render eligibility evaluator 正本 | #290/#291/#292 各 crate | 本 slice は `RenderCandidateIndex` を入力として消費するのみ |
| GUI/CLI/Script 露出 | #295 RCP-06 範囲 | UX/observability/authorization は別 Issue |

## Review 記録（review-protocol）

subagent を使えない worker 契約のため cooperative / adversarial
の 2 pass を分離して実施した。objective checks（上表 1〜6）実施後。

### Cooperative pass（整合性）

- spec→test-spec→IR→exp→evidence の relations が相互に一貫
  （`implements`/`verified-by`/`references`）。docs index 3 dir
  を `doc-maintenance index apply` で再生成し `index plan` が
  `in-sync` であることを確認。
- `verify.rs` の検査順 doc comment と実装順の不一致を修正
  （layout → automation → state）。
- `cutset.rs` の required port 会計で `BTreeMap::insert` が
  multiplicity 下限を上書きし得た件を `entry().or_insert(1)` に修正。

### Adversarial pass（欠陥探索）→ 修正済み

- `EditKind::SourceContent` が pin 非対象の media 変更を
  `NoRebuild` と誤分類し得た → `Rebuild` 固定 + oracle test 追加。
- greedy loop が同一 artifact の reject を各 round 重複記録 →
  `recorded` set で初回 reject のみ evidence 化。
- `reconcile` が `capabilities.generation` と snapshot pin の
  不一致を見ていなかった → `check_pins` と同規則で
  `CapabilityGeneration` mismatch を報告。
- `observe_active` が clock epoch を照合せず stale-epoch 観測で
  `Active` 化し得た → generation + epoch の両方一致を要求 +
  oracle test 追加。

### Non-blocking 観察（対応見送り・記録のみ）

- stateless plugin（opaque state を持たない processor）の
  `PluginStateDecl` 表現は `digest: None` = 未 capture と区別
  不能で fail-closed（`StateUnknown`）に倒れる。実用上 Plugin
  class は state chunk を持つ前提で、workspace integration 側で
  表現を確定する deferred residual。
- `NodeClass::Source` の media content 可用性は snapshot 構築側
  （deferred integration）の前提であり、本 slice の model は
  content pin を持たない — 上記 SourceContent→Rebuild がその
  補完。
- multi-node region の effective latency は境界 edge の
  `pdc_frames` + source node `latency_frames` で近似。領域内部
  latency の合成は semantic graph の latency propagation 依存で
  deferred。fail 方向は reject 側（安全側）に倒れる。
- `BranchAudition` の `fixed_inputs`/`target_revision` は
  headless slice では未消費の intent 証拠（writer 隔離は runtime
  責務）。
- `AttemptOutcome::Ineligible.reason` は自由文字列（deterministic
  だが enum ではない）。証拠用の粗い分類として許容。

## 判定まとめ

- RCP-EX-01〜05 の headless 検証可能分: **Pass**（26 oracle
  test・typed verdict/cutset/reconcile の決定契約）
- RCP-EX-06: **NotRun**（実機 Gate・#296 へ送付）
- 偽 Exact / 古い plan 誤 Activation / last-good 喪失: oracle
  上 **0 件**を確認。実機での再確認は RCP-GATE の責務。
