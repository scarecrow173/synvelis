# CB-08 Comparison Orchestrator headless coverage 棚卸し (Issue #285 / synvelis#285)

CB-08（Semantic/Signal/Perceptual Diff・Context 固定・Cache）
の acceptance case-ID を、merge 済み CB-01..07 adapter・
VCS-02 semantic diff・RCP branch render-pin 解決の上で
headless に棚卸しした結果。sv109/sv288 と同じ Lane 語彙を
使う。

凡例: ✅ headless 証拠あり（本 slice の `mise run test` で
実測 green）/ ◐ 部分的 / ⛔ 外部 Gate または未 merge 前提
（`external-gates.md` 参照）

## 計測環境（2026-11-16）

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `16c8144`（branch `scarecrow173/sv285-cb08-compare` base）+ 本 slice の commit |
| Ruviform resolved SHA | `Cargo.lock` git dep（`ac10a2f7` checkout） |
| OS / CPU / FS | Windows 11 / worktree `sv285-cb08-compare` |
| Device + Plugin | なし（headless — audio device・native plugin host 未接続） |
| Toolchain | Rust（mise 管理。CI pin `1.99.0`） |
| Project schema | v14 envelope |

## Lane 定義（sv109/sv288 と同一）

| Lane | 環境 | Pass にできるもの |
| --- | --- | --- |
| T0 headless (synvelis-project) | deterministic unit/integration test | ID/ref/schema/CAS・pin/context/stale/cache/layer 契約 |
| T2 headless E2E (synvelis-player/render) | lowering/offline render/probe | PCM・source map — **本 slice 対象外** |
| T3 real OS GUI/Device | Windows/macOS/Linux + device/plugin | 実 Render/試聴 — **全て NotRun** |
| T4 both consumers | Synvelis + AudioLens | public API/precision — **全て NotRun** |

状態語彙: `Pass`/`Fail`/`NotRun`/`Unsupported`（実行結果）と
`Required`/`Deferred`/`NotApplicable`/Blocked-by-issue
（要件・前提状態）を分離する（`POLICIES.md`）。

---

## Case → fixture → oracle → result

### CB-CMP-01（全 7 scope で pinned revision・固定 context・
semantic diff・unsupported は reason 付き）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| 7 scope pin + semantic diff | `cb08_compare_orchestrator.rs`（`gate_fixture` 6 scope + `take_fixture` TakeUsage） | `ComparisonSession` pin/context 独立 oracle + `compare_snapshots` 系 `ChangeSet` | ✅ T0 headless Pass |
| unsupported domain の reason | `LayerState::Unavailable`/`Unknown`/`Unsupported` typed reason（provider なし signal・perceptual） | typed reason 非 collapse | ✅ T0 headless Pass |
| 実 signal/PCM/perceptual evidence | — | 実 renderer・試聴 | ⛔ NotRun（`external-gates.md`） |

### CB-CMP-02（context 変更で旧 Audio Diff/cache が stale・
false Active/Observed を出さない）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| branch/scope/project revision stale | `cb08_cmp_02_branch_edit_marks_session_and_result_stale` | `StaleDimension` 分類（`BranchRevision`/`ScopeContent`/`BranchPayload`/`ProjectRevision`） | ✅ T0 headless Pass |
| shared-context/tempo/source/plugin stale | `cb08_cmp_02_shared_context_edit_marks_stale_without_scope_move`・`cb08_cmp_02_tempo_and_source_moves_classify_independently`・`cb08_cmp_02_plugin_insert_marks_plugin_dimension_stale` | 次元別 `StaleDimension`（`SharedContext`/`Tempo`/`Source`/`Plugin`/`TimeMapping`） | ✅ T0 headless Pass |
| stale evidence が current を自称しない | 同上 + `cb08_cmp_signal_unaligned_policy_and_stale_flag`（`stale` flag） | freshness/stale flag 独立 oracle | ✅ T0 headless Pass |
| 実 Audio Diff 再測定・Observed 表示 | — | 実 render/probe・UI | ⛔ NotRun（CB-09/CB-10・外部 gate） |

### CB-CMP-03（同一 PCM fixture で zero difference・gain/
latency/PDC 分類・missing PCM は明示）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| signal provider seam の honest gap | `cb08_cmp_signal_provider_gates_plugin_state_and_quality`（NoProvider/PluginStateUnknown/EvidenceQuality）・`cb08_cmp_signal_unaligned_policy_and_stale_flag`（AlignmentPolicy） | typed `LayerState`/`EvidenceReason` | ✅ T0 headless Pass（seam 契約のみ） |
| 同一 PCM zero diff・gain/latency/PDC 分類 | — | 実 PCM render/probe・alignment oracle | ⛔ NotRun（実 signal provider 前提 — Ruviform #308/#309・CB-10） |
| missing PCM の明示 | provider 非供給 → `Unavailable{NoProvider}` | typed reason | ✅ T0 headless Pass（provider 不在の形で） |

### CB-CMP-04（非整列/非決定/非同一 source の delta を
正しい処理差と報告しない・loudness match は原信号を変えない）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| alignment policy による signal 拒否 | `cb08_cmp_signal_unaligned_policy_and_stale_flag`（`AlignmentPolicy::Unaligned` → `Unavailable{AlignmentPolicy}`） | policy refusal 独立 oracle | ✅ T0 headless Pass（宣言 policy の refusal のみ） |
| loudness match 非破壊・実 delta 分類 | — | 実 signal・試聴 A/B | ⛔ NotRun（`MonitoringPolicy` は audition path 宣言のみ・実測は外部 gate） |

### CB-CMP-05（MIDI 固定 instrument/FX 入力・arrangement
tempo/length 変更の整列または明示 Unsupported）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| MIDI/Arrangement scope の pin+semantic diff | `cb08_cmp_01_six_gate_scopes_pin_context_and_semantic_diff`（`MidiClip`/`Arrangement` 含む 6 scope） | `ChangeSet` scoped kind oracle | ✅ T0 headless Pass（semantic 層のみ） |
| MIDI alignment・tempo/length 変化の signal 整列 | — | 実 MIDI render・time mapping provider | ⛔ NotRun（`AlignmentPolicy::Mapped` の実 provider 前提） |

### CB-CMP-06（非 adopted candidate の連続 RT 実行なし・
Live/Offline resource policy・plugin failure fallback の独立表現）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| compare は read-only derived（RT 実行しない） | `cb08_cmp_open_is_read_only_and_session_is_derived`（open/evaluate が live revision・`Project`・branch content 不変） | Project bitwise 不変 oracle | ✅ T0 headless Pass |
| Live/Offline resource policy・plugin failure fallback | — | 実 RT/host・fallback 実測 | ⛔ NotRun（外部 gate — `RuntimePublish`/plugin host 前提） |

### CB-CMP-07（read-only VCS snapshot・probe capture が
audio truth を書き換えない・AudioLens f64 精度は独立 gate）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| compare/probe が project truth を変更しない | `cb08_cmp_open_is_read_only_and_session_is_derived` | Project bitwise 不変 oracle | ✅ T0 headless Pass |
| derived cache が truth を装わない | `cb08_cmp_cache_hit_reuse_and_content_invalidation`（key digest が content 変化で必ず変わる） | `ComparisonKey`/`CacheStatus` oracle | ✅ T0 headless Pass |
| AudioLens f64/native 精度非回帰 | — | consumer 側 gate | ⛔ NotRun（`scarecrow173/ruviform#294`/`#308`/`#309`） |

### CB-CMP-RCP-01（Verified Artifact vs Derived Probe/
Preview/Unknown・shared-upstream/tempo/sidechain 変更で
stale・固定 context で false Measured/Exact を出さない）

| 範囲 | 被覆 fixture | oracle | 判定 |
| --- | --- | --- | --- |
| evidence origin の区別（Pipeline/Derived/Mock） | `EvidenceOrigin` enum + `cb08_cmp_signal_provider_gates_plugin_state_and_quality`（Mock origin 転記） | origin 独立 oracle | ✅ T0 headless Pass（seam 契約のみ） |
| shared-upstream/tempo/sidechain stale | `cb08_cmp_02_*`（`SharedContext`/`Tempo`/`Plugin`/`Source`/`TimeMapping` 次元） | `StaleDimension` 分類 oracle | ✅ T0 headless Pass |
| 固定 context で false Measured/Exact を出さない | provider gate（plugin opaque・非有限 metric・Unaligned）+ `NoProvider`/`RequiresAudition` 固定 gap | typed `LayerState` oracle | ✅ T0 headless Pass（headless 範囲） |
| Verified Artifact 判定・実 render/probe artifact | — | 実 renderer・RCP artifact | ⛔ NotRun（RCP-07/#296・外部 gate） |

---

## 正直な分類まとめ

- **✅ headless Pass（T0）**: pin/CAS・context 固定・3 層分離・
  typed staleness 分類・derived cache・provider seam gate・
  read-only 契約・7 scope semantic diff — `cb08_compare_
  orchestrator.rs` 13 test が `mise run test synvelis-project`
  で green。
- **◐ 部分的**: signal/perceptual 層は seam・gate・honest gap
  の契約のみ。実測 evidence の供給・分類は provider 実装と
  外部 gate の責務。
- **⛔ NotRun/Blocked/Deferred**: 実 signal render・PCM delta・
  試聴/知覚受入・plugin GUI・AudioLens consumer・RT publish・
  macOS/Linux・実機 device — `external-gates.md` 参照。
