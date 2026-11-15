---
id: "EXP-cb08-compare-orchestrator"
title: "CB-08 Comparison Orchestrator headless 実験メモ (synvelis#285)"
created: "2026-11-16"
updated: "2026-11-16"
---

# 実験記録: CB-08 Comparison Orchestrator headless slice（synvelis#285）

## E1 shared-context drift をどう分類するか

**仮説**: `EvaluationContext` が保持する shared-context digest
が動いたとき、issue が名指す次元（tempo/time mapping/plugin/
source）と「その他 shared context」を区別して `StaleDimension`
へ落とせる。

- 調査: `BranchRenderPin.shared_context_digest` は scope 外の
  全 semantic object を束ねた 1 digest であり、そのままでは
  「tempo が動いた」のか「無関係な strip が動いた」のかを
  区別できない。CB-CMP-02 は context 変更で旧 Audio
  Diff/cache が stale になることを要求するが、分類粒度は
  issue の dimension 名（shared-upstream・tempo・sidechain
  等）を反映すべき。
- 案 A（shared_context_digest の一致/不一致のみ）: drift の
  有無は分かるが次元分類ができず、`StaleDimension` の網羅性
  を満たせない。却下。
- 案 B（shared object を kind で 5 分割して個別 fingerprint）:
  `Project`→tempo、`ClockEpoch`→time mapping、
  `PluginInstance`→plugin、`Asset`/`MediaSource`/
  `MidiSource`/`MidiNote`/`SmfDocument`→source、残り全部→
  remainder。`shared_moved` が真のときだけ各 digest を比較し、
  不一致の次元だけを `StaleDimension` として列挙する。

**観察**: 案 B を採用。ただし初版は「reasons の最後の要素が
shared-context 系か」で `SharedContext` を発火する実装で、
named 次元のいずれかが動いた場合に `SharedContext` が誤って
も/出ない分類 bug があった。`remainder_digest` を
`ContextDigests`/`EvaluationContext` に追加し、remainder
だけが動いたときに限り `SharedContext` を発火するよう修正
（`cb08_cmp_02_shared_context_edit_marks_stale_without_scope_move`
で named 非該当の strip fader edit が `SharedContext` のみを
返し、`cb08_cmp_02_tempo_and_source_moves_classify_
independently` で tempo/source が `SharedContext` を発火
しないことを確認）。

## E2 cache hit/miss をどう観測するか

**仮説**: semantic `ChangeSet` の derived cache 再利用を
`CacheStatus` で報告できる。

- 調査: `ChangeSetCache::get_or_compare` は Arc 共有の結果を
  返すが hit/miss を返さない。呼び出し側で「今回計算したか」
  を知る手段が必要。
- 案 A（`get_or_compare` の戻り値に flag を足す）: 既存
  caller（vcs02 系）の signature を壊す。却下。
- 案 B（`contains` probe を追加）: 同一 `ComparisonKey`
  digest で entry 存在を non-mutating に確認し、
  `get_or_compare` の直前に呼ぶ。LRU 順序を更新せず計算も
  しない — 「これから hit するか」の正確な事前 probe。

**観察**: 案 B を採用。`cb08_cmp_cache_hit_reuse_and_content_
invalidation` で Miss→Hit→別 session でも Hit→content 変化で
Miss+別 key を確認。probe と `get_or_compare` の間に entry が
消える競合は single-threaded orchestrator では起きず、起きても
`CacheStatus` が Hit を過剰申告するだけで結果の正しさには
影響しない（`get_or_compare` が権威）。

## E3 signal provider report をどこで gate するか

**仮説**: provider が `LayerState::Available` を申告しても、
orchestrator 側の既知条件（plugin opaque state・metric 有限性・
alignment policy）で降格できる。

- 調査: provider は外部実装であり、その report は「この比較の
  evidence である」という主張に過ぎない。headless では plugin
  opaque state が `Known` でない view があり得るため、provider
  が measured-exact を申告しても検証不能。
- 案 A（provider の state をそのまま採用）: `Mock`/外部
  provider が `Available` を申告すればそのまま表示され、
  plugin 未検証の measured claim が成立してしまう。却下。
- 案 B（orchestrator が report を gate）: `Unaligned` policy →
  `AlignmentPolicy`、plugin opaque 非 `Known` →
  `PluginStateUnknown`、非有限 metric → `EvidenceQuality`。
  provider の `origin`/`alignment`/metrics/provenance は
  そのまま転記し、state のみを gate 結果で上書きする。

**観察**: 案 B を採用。`cb08_cmp_signal_provider_gates_plugin_
state_and_quality` で plugin-free project の Mock report が
`origin: Mock` 付きで流れ、processor 持ち project では
`Unavailable{PluginStateUnknown}`、非有限 metric で
`Unknown{EvidenceQuality}` になることを確認。
`cb08_cmp_signal_unaligned_policy_and_stale_flag` で
`Unaligned` policy が provider 有無に関わらず signal を
拒否することを確認。

## E4 composed branch view をどう snapshot 化するか

**仮説**: `resolve_render_project` が返す composed `Project`
を、そのまま `SnapshotResolver` で immutable snapshot 化できる。

- 調査: `capture_live` は `ProjectController` と live revision
  を要求し、stale-revision 検査を行う。composed view は
  controller を持たない transient 値であり、live authority
  ではない。
- 案 A（composed view を live に一時適用して capture）:
  project を変更するため read-only 契約違反。却下。
- 案 B（`capture_project` を追加）: envelope/digest/manifest
  の組立経路を共有し、revision claim と locator pin のみを
  引数化。stale-revision 検査は live 専用のため除外。

**観察**: 案 B を採用。`open_session` は両 endpoint を
`resolve_render_project` → `capture_project` で pin し、
semantic diff はその immutable view 上で実行される。
`cb08_cmp_open_is_read_only_and_session_is_derived` で
open/evaluate が live revision・`Project`・branch content を
変えないことを確認。
