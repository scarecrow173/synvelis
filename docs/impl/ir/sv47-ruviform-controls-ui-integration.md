---
id: "IMPL-0SV47ControlsUiInteg01"
type: "impl"
status: "completed"
title: "SV-47 Ruviform controls/binding headless contract integration — Mixer/Transport/parameter editing surfaces"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/47"
    - "https://github.com/scarecrow173/ruviform/issues/258"
    - "https://github.com/scarecrow173/ruviform/issues/259"
    - "https://github.com/scarecrow173/ruviform/issues/261"
    - "https://github.com/scarecrow173/ruviform/issues/263"
    - "https://github.com/scarecrow173/synvelis/issues/40"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/controls_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv47_controls.rs"
      - type: "file"
        path: "docs/impl/ir/sv47-ruviform-controls-ui-integration.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
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
    - "docs/impl/ir/gw-09-connection-grant-approval-ui-headless-contract-state-fixtures.md"
    - "docs/impl/ir/pc-a-parameter-control-headless-poc.md"
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
    adopted: []
    rejected: []
---

# SV-47 Ruviform controls/binding headless contract integration

## Summary

Synvelis issue #47 — upstream Ruviform `ui/components` ヘッドレス
コントロール契約と `ui/binding` (CommandBinding / ViewLink /
LinkRegistry) の意味論を、Synvelis の Mixer / Transport /
parameter-editing 面に headless contract 層として導入した。
新 module `synvelis_workspace::controls_ui` が generic な
intent / receipt / projection / binding-generation 契約を所有し、
DAW 固有の parameter・automation・transport・undo・recording
意味論は `ControlSession` / `ProjectSession` に留めたまま。
Ruviform の resolved rev (`258df5ae`) は当該 UI module をまだ
含まないため、依存変更は行わず upstream main の契約意味論を
non-RT gateway 境界で mirror した (multi-repo 契約どおり
Synvelis 側は generic gesture/value-mapping を再実装せず、
binding/dispatch/projection 契約のみを適用)。

## Context

Spec/design 文書は issue 起点の直接実装のため新規作成せず、
upstream Ruviform main (`F:/repositry/audio-products/ruviform`) の
`ui/components` (headless control state contract) と `ui/binding`
(`CommandBinding`/`ViewLink`/`LinkRegistry`) を source of truth と
して契約を導出した。GW-09 (`docs/impl/ir/gw-09-*.md`) の
headless contract/state/fixture パターンと PC-A
(`docs/impl/ir/pc-a-*.md`) の `ControlSession` 意味論に従う。
実 device GUI 検証は manual gate であり本 impl のスコープ外。

## Implementation

### Changes

- `synvelis-workspace/src/control.rs`:
  - `PlaybackStatus::Paused` を追加。pause は playhead と
    latch-held 値を保持したまま record/write gate を閉じる
    (paused transport は rolling ではない)。
  - `transport_pause` (`Playing→Paused` のみ遷移) /
    `transport_resume` (`Paused→Playing` のみ) を追加。
  - `gesture_cancel` を追加 — active gesture と preview
    override を破棄し、書き込み・undo entry を生成しない。
- `synvelis-workspace/src/controls_ui.rs` (新規):
  - **Contract types**: `ControlValue` (continuous/discrete/
    toggle)、`IntentKind` (begin/preview/commit/cancel/action)、
    `CommandIntent` + `DispatchContext` (explicit target,
    command id, view/window, generation, issued-at)、
    `ReceiptStatus` (accepted/applied/rejected/stale/timeout)、
    `CommandReceipt`/`WireReceipt`、`CommandPhase`
    (pending/accepted/applied/observed/rejected)、
    `StateProjection`/`ParamProjection`/`TransportProjection`、
    `StripRow`/`TransportRow`、`BindingSnapshot`、`UiDiagnostics`。
  - **SurfaceBinding** — upstream `CommandBinding` mirror:
    generation tracking、active gesture、head command、
    last receipt、last observed projection。revision guard で
    同一・陳腐 projection を drop し、terminal receipt 到着時に
    観測 revision が効果を包含していれば head を `Observed` へ
    settle する (upstream の「newer projection retires terminal
    head」規則)。
  - **ControlsUi** — upstream `ViewLink`/`LinkRegistry` mirror:
    `attach_view` (idempotent、暗黙 dock-transfer detach、
    generation increment、binding re-attach、subscription
    refresh)、`detach_view`/`close_view` (unsubscribe、binding
    detach、in-flight gesture/query cancel、domain entity には
    signal しない)。detach は dying generation で tracked
    `detach-cancel:<gesture>` command + accepted receipt を発行し、
    late receipt/command は stale 分類で落とす。
  - **Dispatch**: `dispatch` が kind/target/generation/window/
    readonly/domain を検証して `ProjectSession`/`ControlSession`
    へ route。preview は coalescible、receipt は command id
    correlation、action は transport discrete 操作。
    `Off` commit は `SetParameterBase` 1 件 (undoable)、
    Touch/Latch/Write は recording transaction (undo 1 step)、
    Read は preview のみ返却。cancel は一切書き込まない。
  - **Projection**: param/transport projection は observed
    truth を preview と分離して運び、subscribe した全 view
    (同一 target を持つ別 window 含む) へ broadcast。
    未観測値を confirmed truth として提示しない。
    `strip_row`/`transport_row` で mixer/transport 面の
    read-model を提供。
- `synvelis-workspace/src/lib.rs`: `pub mod controls_ui` と
  contract 型の re-export。
- `synvelis-workspace/tests/sv47_controls.rs` (新規):
  25 test の headless 受入 + regression fixture — GW-09 パターンで
  deterministic project/target/lane を組み立て、live binding
  generation を `binding(view,target).generation` から採取して
  `DispatchContext` に刻む。

### Decisions During Implementation

- pinned rev `258df5ae` が `ui/components`/`ui/binding` を
  未含有と確認し、依存 rev は変えず upstream main の契約意味論を
  mirror。AGENTS.md の「DAW 固有仕様を Ruviform に持ち込まない /
  generic 契約は Ruviform から採る」を「contract 意味論の採用、
  code copy なし」として解釈。依存 rev 引き上げは follow-up。
- generation は upstream `link.ts` どおり view attach で mint、
  binding は view の epoch を引き継ぐ。binding fresh でも attached
  view の epoch に照合して stale drop。
- detach cancel は upstream の queue-bypassed cancel を
  tracked command として再現 — accepted receipt を wire に出し、
  generation を dead にしてから late landing を防ぐ。
- terminal receipt (applied/rejected) 到着時、効果を含む新 rev の
  projection が観測済みなら head を `Observed` に settle —
  upstream「newer projection retires terminal head」を
  projection-arrival order 非依存で実現するため、command mint 時に
  target revision を記録して照合する方式にした。
- `cancel_gesture` は cancel/detach 両経路で observed override を
  全 bound view へ revert broadcast — 破棄された preview が
  他 window で truth のまま残る不変条件違反を所有境界で修正。
- `Paused` は transport 意味論の最小拡張として `PlaybackStatus`
  追加で実現 — write gate 「transport rolling」の既存不変条件を
  そのまま利用し、pause 経路の例外を増やさない。

### Review Fixes Applied

実装中の自己レビュー/受入テストで検出し修正したもの:

- **`drag_commits` head が `Applied` で stuck**: commit 自身の
  projection が dispatch 中に先着し receipt pump 時点で upstream
  の newer-projection 退避が発火しない。command mint 時の target
  revision を `Tracked` に保持し、terminal receipt 時に観測 rev が
  包含していれば `Observed` へ退避する契約で解消。
- **detach-cancel が他 window を stale のまま残す**:
  `cancel_gesture` が revision bump のみで broadcast せず、
  破棄された override を別 view が truth として保持し続けた。
  cancel 内で param/transport を broadcast するよう修正。
- **`transport_advance` で transport epoch が上がらない**:
  `broadcast_all` を project-truth 用 (param bump + transport
  re-read) に整理した際、advance 経路で transport bump が欠落。
  advance は transport/param 両方を bump するよう個別化。
- **base-edit test が `Read` default で fail**:
  `AutomationMode::Read` が既定値なので base-edit 系 fixture は
  `set_mode(Off)` を明示。
- test ctx helper が generation 0 を刻んで全 intent が stale
  drop — live generation を binding snapshot から採取。
- rejected `begin` は receipt-expected でないため phase を駆動
  しない (upstream-faithful) — 観測は `last_receipt` で検証。

## Review Protocol

2 名の独立 reviewer (A: contract/lifecycle 整合、B: adversarial
state/corner) による review-protocol を実施し、validator で各
candidate を root cause 別に検証した。確認済み P1 と修正:

- **receipt が target を捨てて view+command id で scan していた**:
  sibling target が同一 command id を合法に mint すると誤った
  binding へ receipt が着弾し得た。`deliver` を
  `(view, target, command_id)` 3 項 routing に修正し、
  `WireReceipt.target` を実際に使う。regression:
  `receipts_route_to_their_own_binding`。
- **live gesture id 再利用が owner を上書き**: 既存 id で begin
  すると旧 session gesture が orphan 化し manual override が
  残ったまま別 view の detach で誤 cancel され得た。
  `route_begin` で id 衝突を事前拒否。regression:
  `gesture_id_collision_is_rejected_and_owner_survives`。
- **unregistered view が binding を暗黙生成**: `ensure_binding`
  が dispatch 経路で未 attach view を live 化し session gesture
  まで開始し得た。dispatch を「attached view 必須」にし、
  unknown view は detached として拒否 (generation 照合は
  view epoch に一本化)。regression:
  `unregistered_and_closed_views_are_dropped`。
- **`close_view` が tombstone を残さず late intent で復活**:
  view record 完全削除 → late `Begin` が binding/owner を再生成し
  domain を mutate し得た。tombstone (`attached=false` +
  generation bump) を残し dispatch が先に reject。
  `close_view_drops_everything` を強化。
- **commit 失敗で UI/session gesture が不整合**:
  `control_gesture_end` のエラー経路が UI owner を残したまま
  session gesture を消去し、binding が死んだ gesture を
  previewing し続け得た。失敗時も `end_gesture` で UI record を
  畳み truth を broadcast。
- **`transport_row` が action binding のみ参照**: 未 subscribe の
  action binding が scrub binding の observed truth を mask。
  両 binding を merge し最初に利用可能な observed と
  non-Idle phase を採用。
- **`record_commit` が transport revision を上げない**:
  consumed transaction 後 `recording_active: false` が revision
  guard に掛かって届かない。record ops で transport bump。
- **`detach` が tracked receipts を残したまま epoch を巻き上げ**:
  dead epoch の tracked head が残り remint した同一 command id が
  duplicate 扱い。detach/reattach で tracked を clear。
  regression: `detach_clears_inflight_state_so_reminted_ids_are_fresh`。
- **scrub (seek) gesture が global に exclusive でない**:
  playhead は単一 — transport/position の begin は全 gesture と
  排他。regression: `seek_is_exclusive_across_windows`。
- **`transport_advance` 中の held-write 失敗ループ**: pause で
  record gate が閉じた際 latched 値の再書きが毎 tick Err を返し
  advance が永久に失敗。gate 失敗を非致命とし、playhead/clock
  を進め projection で観測可能に。regression:
  `pause_mutes_held_writes_and_clock_keeps_ticking`。

報告されたが修正対象外 (P2/P3、blocking なし):

- gateway 外 (workspace 直接 mutation) で変更された project
  状態は reattach 時の `refresh_binding` 観測と revision が一致し
  drop され得る — gateway 経路は常に touch→broadcast するため
  内部不変条件は保持される。`resync` API を提供 (強制再観測)。
- receipt rank が `Rejected`/`Unsupported` (rank 3) で `Applied`
  (rank 2) を上書きし得る — 外部注入された矛盾 receipt のみ、
  gateway 自前発行では順序が保証されるため不変条件未損失。
- tracked map の preview 追跡が eviction pressure を増す —
  preview は tracked 対象外に変更済み。
- action が `transport/position` にも受理される (P3) — 対象
  kind 検証を厳格化して `action` は `transport` のみに限定。
- WS-A workspace transport route との機能差は既存境界の別問題。

判定: **ACCEPT** — 確認済み P0/P1 残件なし、regression 追加済み。

## Related Experiments

### Adopted

なし — issue 要件どおりに実装。

### Rejected

- Ruviform dependency を upstream main rev へ引き上げる案 —
  当該 rev 以降の unrelated 変更を lockfile に巻き込み、本
  issue の最小整合範囲を超える。契約 mirror で代替し、rev
  引き上げは別 issue/follow-up とする。
- Synvelis 側に Knob/Fader/Seek の gesture state machine を実装
  する案 — issue 要件「generic control/binding 契約のみ採用」に
  反し、upstream との表現 drift を生む。

## Validation

| 項目 | 結果 |
| --- | --- |
| `mise run fmt` | PASS |
| `mise run check` | PASS (workspace --all-targets) |
| `mise run lint` | PASS (`cargo clippy --workspace --all-targets -- -D warnings`) |
| `mise run test` | PASS (`cargo test --workspace --locked` — sv47_controls 25 test 含む全 crate green) |

受入対応 (`tests/sv47_controls.rs` 25 test):
transport action の applied-vs-noop 区別、seek preview が
playhead を動かさないこと、display-only 操作の project 非変更、
dock-move cancel と stale-generation 抑止、close-view の lifecycle
cleanup、base-edit drag commit の全 window 観測、receipt
ordering/timeout/stale guard、cancel の preview 破棄・非書き込み、
別 window begin 拒否と misroute 防止、duplicate command id の
exactly-once、domain 外 commit 拒否と gesture 生存、detach による
write gesture cancel、unknown target/unsupported kind、
observation feed の projection 更新、latch commit、readonly
target begin 拒否、touch gesture の recording transaction と
1-step undo。

## Risks

- 実 device GUI 検証は manual gate であり本 impl では未実施
  （headless contract/state/fixture のみ）。
- Ruviform pinned rev `258df5ae` は `ui/components`/`ui/binding`
  を未含有 — contract 意味論を mirror したため、rev 引き上げまで
  upstream 型との drift が残る（follow-up で置換検討）。
- gateway 外（workspace 直接 mutation）で変更された project 状態は
  reattach 時の `refresh_binding` 観測と revision が一致し drop
  され得る — gateway 経路は常に touch→broadcast するため内部
  不変条件は保持される。`resync` API で強制再観測可能。
- receipt rank が `Rejected`/`Unsupported`（rank 3）で `Applied`
  （rank 2）を上書きし得る — 外部注入された矛盾 receipt のみで、
  gateway 自前発行では順序が保証されるため不変条件未損失。
- WS-A workspace transport route との機能差は既存境界の別問題
  として残存。

## Follow-ups

- Ruviform dependency rev を `ui/components`/`ui/binding` 含有
  rev へ引き上げ、mirror した contract 型を upstream 型で置換
  検討 (multi-repo 契約どおり generic 側を Ruviform に集約)。
- 実 device GUI 検証 — manual gate (本 impl スコープ外)。
- Mixer/Transport の実 GUI 面 (Custom Elements / Ruviform
  Reference Skin 配線) — headless contract の消費側として
  follow-up。
- `TransportRow`/`StripRow` を実 UI surface が subscribe する
  wiring — 本 impl は headless read-model 提供まで。
