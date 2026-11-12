---
id: "IMPL-0SV48VizUiInteg0001"
type: "impl"
status: "completed"
title: "SV-48 Ruviform ui/viz + ui/binding headless contract integration — Waveform/Meter/Spectrum/Timeline into the DAW view layer"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/48"
    - "https://github.com/scarecrow173/ruviform/issues/260"
    - "https://github.com/scarecrow173/ruviform/issues/261"
    - "https://github.com/scarecrow173/ruviform/issues/185"
    - "https://github.com/scarecrow173/synvelis/issues/47"
    - "https://github.com/scarecrow173/synvelis/issues/60"
    - "https://github.com/scarecrow173/synvelis/issues/4"
    - "https://github.com/scarecrow173/synvelis/issues/5"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/11"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/viz_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv48_viz.rs"
      - type: "file"
        path: "docs/impl/ir/sv48-ruviform-viz-ui-integration.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/impl/ir/sv47-ruviform-controls-ui-integration.md"
    - "docs/impl/ir/gw-09-connection-grant-approval-ui-headless-contract-state-fixtures.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "`ruviform-viz` Rust クレート (generic LOD/viewport/lane primitives) を新規 workspace 依存として採用 — lockfile resolved rev `073a3ff6` (全 ruviform git 依存と同 rev。258df5a→073a3ff で viz/ui/media-core/plugin-core の差分なしを確認済み)。generic series/axis/LOD/cache/lane 実装の重複を避け、『generic viz 契約は Ruviform から』の issue 方針をコードレベルで実現。"
      - "TS `ui/viz` element/scene/renderer は移植せず — upstream が canonical。本 module は model+lifecycle (ports/surfaces/pump/link/queries) を headless mirror し、DrawOp scene builder / backend executor / customElements は移植対象外 (renderer fork 禁止の issue 指示)。"
    rejected:
      - "TS `ui/viz` の `AbortSignal`/async query を async mirror として再現 — headless では querySeq+generation+revision 照合で同等の cancel/stale 保証を実現し、テストは完全同期・決定的。"
      - "Timeline 用の upstream Rust `ruviform-viz::timeline` 型をそのまま wire 型に — serde 非対応のため wire 契約は独自 `TimelineItem`/`TimelinePayload` (LaneItem<TimelinePayload> を内部計算に再利用)。"
---

# SV-48 Ruviform ui/viz + ui/binding headless contract integration

## Summary

Synvelis issue #48 — Ruviform `ui/viz` (Waveform / Meter / Spectrum /
Timeline の generic series・axis・viewport・LOD・snapshot 契約) と
`ui/binding` (ViewportQuery/QueryReply・ViewLink generation) の
headless 契約を、Synvelis DAW view layer へ導入した。新 module
`synvelis_workspace::viz_ui` が generic な data/query/snapshot/
lifecycle 契約を所有し、Track/Bus/Clip/Probe 等の DAW 意味論は
canonical target id と timeline item builder (project clip →
`TimelinePayload`) に閉じ込めた。`ruviform-viz` Rust クレート
(waveform LOD, bounded LRU cache, lane/selection/hit-test,
SampleTimebase) を再利用し、generic 実装の重複を避けた。
Arrangement (timeline + clip-window waveform)、Mixer (meter)、
Inspector (probe spectrum) の 3 view が同一 gateway を共有する形で
検証済み。実 device GUI/WebGL2/HiDPI 検証は manual gate で本 impl
スコープ外。

## Context

Spec/design 文書は issue 起点の直接実装のため新規作成せず、upstream
Ruviform `ui/viz` (source.ts・viewport.ts・elements/*.ts, README) と
`ui/binding` (query/link wire 型) を source of truth として契約を導出、
`crates/ruviform-viz` Rust crate (resolved rev `073a3ff6`) を計算層として
採用した。SV-47 (`docs/impl/ir/sv47-*.md`) の contract mirror パターンと
GW-09 の headless fixture 方式に従う。

## Implementation

### Changes

- `Cargo.toml` / `crates/synvelis-workspace/Cargo.toml` —
  `ruviform-viz` workspace 依存を追加 (`ruviform-ui` 等と同じ
  default-branch git 依存、lockfile で `073a3ff6` に解決)。
- `synvelis-workspace/src/viz_ui.rs` (新規):
  - **Wire contract**: `SourceRevision` (untagged number|string)、
    `WaveformAxis`/`WaveformMeta`/`WaveformQuery`/`WaveformBucket`/
    `WaveformView` (Bins|Samples、`count==0` で gap)、`MeterChannel`/
    `MeterFrame`/`MeterMeta` (sequence・dropped・timestamp)、
    `SpectrumBinsAxis`/`SpectrumMeta`/`SpectrumFrame`、
    `ViewportQuery`/`QueryPayload`/`QueryReply`/`QueryErrorKind`/
    `VizQueryError`/`QueryOutcome` (ui/binding mirror)、
    `OverlayState`/`MeterScale`/`SpectrumScale`/`VizStyleInput`
    (paint-only)、`VizStatus` (non-color status channel)、
    `VizViewEvent` (rvf-viewport/rvf-select)、`TimelineAxis`/
    `TimelinePayload`/`TimelineItem`。
  - **Source ports**: `WaveformPort` (meta+bounded query)、
    `MeterPort`/`SpectrumPort` (meta+pull-of-latest-frame、
    `Result<Option<F>, String>` で producer/bridge failure を
    status へ写像)。`subscribe` notify は `VizUi::notify_source_changed`
    へ集約 — port は pull-only で RT-safe。
  - **Fixtures**: `MemoryWaveform` (SliceWaveform + WaveformSeries
    経由の bounded per-bin reduction、`SubrangeWaveform` で clip
    source window 限定 read、`MemoryWaveform::window`)、
    `ManualWaveform` (PCM swap + revision bump)、`ManualMeter`/
    `ManualSpectrum` (complete-frame push、upstream ManualSnapshotSource
    相当)。
  - **`VizUi` gateway**: view attach/detach/close (ViewLink mirror:
    idempotent attach、implicit dock-transfer detach、generation
    bump、resubscribe re-read)、surface bind、span/pan/zoom/
    plot-px ops (`rvf-viewport` event + 即時 re-query)、select
    (rvf-select event のみ発行、committed selection は consumer
    authoritative で `set_overlays` 経路)、overlays/range/labels/
    scales/theme (全て repaint-only、query を発行しない)。
  - **Waveform query path** (`ensure_waveform`): span×plot_px に
    bounded、bucket == plot px、LRU cache
    (`BoundedCache<(source,rev,start,end,buckets,ch)>`) → miss で
    port query → byte budget 検査 → **revision 二重照合** (reply
    revision == issued revision かつ source の現在 revision が
    issued と一致) → stale reply は 1 回 retry の後 `view_stale` +
    `VizStatus::Stale`。
  - **Snapshot pump** (`pump`): attached meter/spectrum だけを走査し
    各 surface が port から最新 complete frame を pull。新規 seq のみ
    landing (republish は新データとして扱わない = producer idle で
    stale へ老化)、producer `dropped` + seq gap を累積 loss として
    bookkeeping、`staleAfterMs` (default 1s) 超過で stale に 1 回だけ
    遷移。pull error は `SourceError` status で pump 自体は継続。
    UI から RT へ同期アクセスしない — producer が port へ push、
    UI は pull のみ。
  - **Timeline**: `sync_timeline` が `ProjectSession` から全 clip を
    Beats (PPQN/TempoMap) または Seconds (resolve_span @ media_rate)
    で `LaneItem<TimelinePayload>` 化、unresolvable clip は skip+count。
    `timeline_row` は current span 内の items のみ返す bounded read
    (`items_in`)。`timeline_stale` は session revision との照合で
    project-source revision guard を実現。
  - **Rows + diagnostics**: `waveform_row`/`meter_row`/`spectrum_row`/
    `timeline_row` read models と `VizDiagnostics`
    (issued/resolved/stale/cancelled/failed・cache hit/miss・
    pumps・frames_pulled・producer_dropped・seq_gaps・
    stale_transitions・repaints・attaches/detaches/resyncs)。
  - **DAW target scheme**: `waveform:clip-<tag>`、
    `waveform:source-<tag>`、`meter:track-<tag>`、`meter:master`、
    `spectrum:<probe>` — logical source id は opaque string、
    DAW 意味論の解決は Synvelis 側。
- `synvelis-workspace/src/lib.rs`: `pub mod viz_ui` + contract 型
  re-export。
- `synvelis-workspace/tests/sv48_viz.rs` (新規): 36 受入テスト —
  3 view 共有、bounded query/LRU、gap/partial coverage、stale
  revision 拒否、budget 超過 outcome、notify resubscribe、seq gap/
  dropped/stale、awaiting-data 対 source error、NaN bin 保持、
  timeline sync/selection/playhead/revision guard、2 window 独立
  state、dock move resubscribe + detached-pump 停止 + revision race、
  repaint-only display ops、bind/detach validation、wire serde
  roundtrip、unregister 劣化、clip window read 限定、plus review
  regression coverage (VizStatus error serde、NaN wire narrowing、
  camelCase wire names、absolute-extent anchor 合成、bind-before-
  register 回復、port swap cache 隔離、unregister NoSource 維持、
  re-attach status 保持、seq/dropped u64::MAX 飽和、非有限 span
  拒否、over-budget bookkeeping 清算、plot_px/buckets 上限、
  partial frame 拒否、event queue 上限) and second-pass coverage
  (swap/late-register status 再導出、stale の swap 越境、
  wrong-kind port SourceError、overlay geometry 検証、
  meter scale 全 field 検証)。

### Decisions During Implementation

- **ruviform-viz 採用**: TS `ui/viz` は locked rev 非含有だが Rust
  crate は同 rev に存在し API 差分なし — LOD/cache/lane の再実装を
  避けるため direct dependency として採用 (SV-47 の「dep bump 回避」と
  は異なり今回は rev 移動も副作用なし)。
- **Synchronous query**: upstream `AbortSignal` は
  querySeq/generation/revision の照合で mirror — stale reply は
  「発行時 revision と不一致」または「resolve 時点で source revision
  が変化」で検出。
- **Republish semantics**: `snapshot()` が同 seq を返しても新着と
  扱わない (last_pull/stale を refresh しない) — これが producer
  idle 時に stale へ老化する唯一の機構であり upstream の aged-
  snapshot 意味論を保つ。
- **Per-bin reduction**: `WaveformSeries::view` は range を clamp
  してから bin するため、requested span の外側を EMPTY bin とする
  ため per-bin `view(c, s_i, e_i, 1)` 経路に変更 — requested 軸の
  bin 境界を維持したまま bounded な read を保つ。
- **Timeline items = beats or seconds**: `ClipPlacement` の mixed
  musical/absolute anchor を `TickTimeMapping`/`resolve_span` で
  解決。MIDI clip も Musical extent として配置可能。
- **NaN ↔ null narrowing**: JSON wire 上の non-finite f64 は
  serde_json で `null` になる (live-path では NaN 保持)。明示的に
  test で document — 実 wire transport は `encoding`/bytes path で
  運ぶ想定 (upstream 同様)。

### Review Fixes Applied

初回テスト実行で検出・修正:

- **Republished seq が stale 化を防ぐ**: pump が同 seq frame でも
  `last_pull_ms` を refresh していた — 「最新 frame が古い」判定が
  起きず producer idle で stale にならない。同 seq は landing せず
  age-check のみへ変更。
- **Out-of-range pan が clamp で data-compress される**:
  `WaveformSeries::view` が end を frames に clamp してから bin
  するため、requested span の外側が EMPTY bin にならず bin が
  clamped range に圧縮される → per-bin reduce に変更して requested
  span をそのまま bin 分割。pan 越境 bucket は count==0 gap として
  観測可能に。
- **Test 側**: dock-move で「cache hit = resync なし」と誤読する
  assertion を修正 (revision race 経路で検証に変更)、two-window
  cache entry 数の前提誤り、NaN→null serde narrowing を finite
  値 + 明示 assert に分離。

## Review Protocol

独立 reviewer 2 名 (agent A `8b96389b` / agent B `58de1694`) が
実装 diff を adversarial review。両者一致の P0/P1 を全件修正し
regression test で固定:

- **P0 — timeline Absolute extent が anchor を落とす**: Beats 軸で
  `ClipExtent::Absolute` が duration だけを tick 変換し anchor
  media offset を加算していなかった。canonical
  `ClipPlacement::resolve_musical_range(TickTimeMapping, rate)` へ
  統一 (Mixed/Absolute 全組合せ)。Musical+Musical のみ direct
  checked 演算を維持。
- **P1 — `VizStatus` serde 不成立**: internally-tagged enum に
  newtype-string variant (`SourceError(String)`) は serialize
  不可能 — struct variant `SourceError { detail }` へ変更。
- **P1 — `last_query` リーク**: over-budget/missing-port 終端経路で
  `w.last_query` が残り stale bookkeeping が生きる — 全終端経路で
  clear。
- **P1 — bind-before-register デッドサーフェス**: source 未登録で
  bound した surface が register 後も `NoSource` のまま —
  `notify_source_changed` で meta 到着時に extent default span を
  埋め requery。
- **P1 — re-attach status 反転**: resync が一律 `AwaitingData` を
  立て landed frame/view/timeline を巻き戻していた — 保持中の
  truth から status を derive (view+stale / frame+stale /
  synced_rev)。
- **P1 — port swap の cache 汚染**: colliding revision token で旧
  source の cache が復活 — `register_*`/`unregister_source` が
  surface の view/cache/frame/liveness を全 reset。
- **P1 — 未 bounded リソース**: `plot_px`/`buckets` を
  `MAX_BUCKETS`、event queue を `MAX_EVENTS` で cap (drop は
  `events_dropped` diag)。`SubrangeWaveform` offset・seq/dropped/
  gap 計数を全て saturating 演算へ。
- **P1 — partial frame が `Ok` に land**: meta channel/bin 数と
  不一致の frame は `SourceError` として reject。
- **P1 — 非有限 span**: `set_span_inner` で start/end を集中
  検証 — pan/zoom が NaN/inf/inverted span を state に書かない。
- **P1 — `synced_rev` sentinel**: `0` は「revision 0 と同期済み」
  と「未同期」を区別不可 — `Option<u64>` へ。
- **Resize event 欠落**: `set_plot_px` が
  `ViewportTrigger::Resize` event を emit するように。
- **NaN wire 縮退**: `serde_f64`/`serde_f64_vec` ヘルパで non-
  finite は JSON `null`、読み戻しは `NaN` — gap marker が wire を
  生き残る。wire 型を camelCase + `deny_unknown_fields` に統一。

その後、修正 diff への verification review (agent `5839567d`) で
第 2 pass を実施 — 新規 P1 1 件と注入可能な状態 gap 2 件を修正:

- **N1 (P1) — meter/spectrum status が再導出されない**:
  `register_*` swap や late-register で meta が `Some` に resolve
  しても status が旧 producer のまま残り、idle port では無限に
  継続した。`snapshot_status`/`absent_or_kind` ヘルパを抽出し
  notify と resync で同一導出を共有 — frame 不在なら
  `AwaitingData`、wrong-kind port は `SourceError`。
- **N2 (P2) — `set_overlays` が非有限 geometry を受理**:
  selection/playhead/marker を境界で検証 (`InvalidSpan`)。
- **N3 (P2) — `set_meter_scale` が `clip_db`/`segments` 未検証**:
  `set_spectrum_scale` と同じ全 field 検証へ。
- 併せて `serde_f64`/`serde_f64_vec` を全 f64 wire field へ展開
  (meta extent・view geometry・bin axis・overlay・scale・
  query/event span) し、struct wire 型へ `deny_unknown_fields` を
  適用 — 全 field の JSON roundtrip が total になった。

未対応 (reported, out of scope): `tempo_map.ppqn` vs
`project.ppqn` の乖離は `synvelis-project` 側の invariant gap
(`SetPpqn` が 960 固定のため現行では一致; crafted snapshot で
乖離可能) — 別 issue として報告対象。`MemoryWaveform::query` の
`channels` リスト乗算増幅は `WaveformPort` trait 経由のみ到達
可能 (gateway は `channels: None` のみ発行) — bounded-read
境界の port 側改善として follow-up。

## Related Experiments

### Adopted

- `ruviform-viz` Rust クレート (generic LOD/viewport/lane primitives) を新規 workspace 依存として採用 — lockfile resolved rev `073a3ff6` (全 ruviform git 依存と同 rev。258df5a→073a3ff で viz/ui/media-core/plugin-core の差分なしを確認済み)。generic series/axis/LOD/cache/lane 実装の重複を避け、『generic viz 契約は Ruviform から』の issue 方針をコードレベルで実現。
- TS `ui/viz` element/scene/renderer は移植せず — upstream が canonical。本 module は model+lifecycle (ports/surfaces/pump/link/queries) を headless mirror し、DrawOp scene builder / backend executor / customElements は移植対象外 (renderer fork 禁止の issue 指示)。

### Rejected

- TS `ui/viz` の `AbortSignal`/async query を async mirror として再現 — headless では querySeq+generation+revision 照合で同等の cancel/stale 保証を実現し、テストは完全同期・決定的。
- Timeline 用の upstream Rust `ruviform-viz::timeline` 型をそのまま wire 型に — serde 非対応のため wire 契約は独自 `TimelineItem`/`TimelinePayload` (LaneItem<TimelinePayload> を内部計算に再利用)。

## Validation

| Check | Command | Result |
| --- | --- | --- |
| fmt | `mise run fmt` | PASS |
| fmt-check | `mise run fmt-check` | PASS |
| check | `mise run check` | PASS |
| lint | `mise run lint` | PASS |
| test | `mise run test` | PASS (66 suites all `ok`; sv48 acceptance 36/36 PASS) |

## Manual Gate / Out of Scope

- 実 device GUI・WebView・GPU (Canvas2D/WebGL2) rendering、HiDPI、
  browser 実測 performance — manual verification gate。本 impl は
  headless contract/state/fixture のみ。
- `ui/viz` element/scene builder/backend executor の Rust 移植 —
  upstream canonical、fork 禁止。
- Meter/Spectrum producer 側の実 RT bridge (engine → port push) —
  port contract は整備済み、実 engine 配線は後続 issue。
- `SpectrumSource`/`Spectrogram` (spectrogram tile) — issue 範囲は
  spectrum display frame のみ、spectrogram 面は follow-up。
- Transport playhead 値の session 連携 (host が `set_overlays` へ
  投影する経路) — view-local overlay contract は整備済み。

## Risks

- 実 device GUI・WebView・GPU (Canvas2D/WebGL2) rendering・HiDPI・
  browser 実測 performance は manual gate で未検証 — headless
  contract/state/fixture のみ。
- TS `ui/viz` element/scene/renderer は upstream canonical で
  移植対象外 — Synvelis 側は model+lifecycle の headless mirror
  のため、将来の renderer 実装で wire 契約との整合確認が必要。
- `tempo_map.ppqn` vs `project.ppqn` の乖離は `synvelis-project`
  側の invariant gap(`SetPpqn` が 960 固定のため現行では一致;
  crafted snapshot で乖離可能)— 別 issue として報告対象。
- `MemoryWaveform::query` の `channels` リスト乗算増幅は
  `WaveformPort` trait 経由のみ到達可能(gateway は `channels:
  None` のみ発行)— bounded-read 境界の port 側改善として
  follow-up。
- JSON wire 上の non-finite f64 は `null` に縮退(NaN↔null
  narrowing)— gap marker の wire 表現として test で document
  済みだが、binary `encoding`/bytes path 未整備の間は JSON
  経路の制約として残る。

## Follow-ups

- Ruviform `ui/viz` showcase/manual visual gate と Synvelis UI の
  実装 (issue #4 Signal Flow・#5 Inspector・#11 Timeline Edit・
  #18 Mixer) との接続。
- Probe/Inspector の ObservationState ↔ spectrum stale/unavailable
  status の統合 (PC-A 意味論との接続は port 経路で可能)。
- `ui/binding` の `encoding`/`Bytes` payload path (binary wire) の
  headless fixture 拡張。
