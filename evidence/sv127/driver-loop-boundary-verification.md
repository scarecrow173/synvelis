# Evidence — synvelis#127 TIM-03 driver-side loop-wrap boundary slice

Issue #127（TIM-03 tracker）の driver 側 slice。
`HeadlessDriver` が attach 済み node ごとに `WrapSim` を保持し、
`ProcessBoundarySession::stage_window` の `boundaries` 引数へ
位置由来の `BoundaryPoint { kind: LoopWrap }` を供給する。
routing 側 slice（`verification.md`、同 dir・別ファイル）の
`WrapSim`/`walk_window` を再利用し、driver 経路の 3 箇所の
`stage_window` 呼出（activation anchor・定常 transport
staging・seek re-anchor）全てに適用した。

## Anchor — 対象 SHA・環境

- branch: `scarecrow173/sv127-driver-loopbound`（worktree
  `G:\orca\workspaces\synvelis\sv127-driver-loopbound`）。
  base は dispatch 時点の `origin/main`
  `34387c4e31afb90d7e0d5266d4e87d81d4e8b329`。
- OS: Windows。toolchain: rustc 1.99.1
  （`rustc -V`: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`、
  mise 管理）。
- 変更面: `crates/synvelis-player/src/driver.rs`・
  `crates/synvelis-player/tests/tim03_driver_loop_boundary.rs`
  （新規）+ docs チェーン（slug `tim-03-driver-loop-boundary`）
  + `docs/*/README.md` 管理 index 再生成 + `evidence/sv127/`。
  `process_boundary.rs`・`routing_control.rs`・`dispatch.rs`・
  `Cargo.lock`・他 crate の変更なし（WrapSim API 追加不要だった
  ため `process_boundary.rs` も未変更）。

## 検証コマンドと結果（canonical = mise、最終 tree で実測）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（`cargo fmt --all -- --check`、exit 0） |
| `mise run check synvelis-player` | Pass（`cargo check --locked --all-targets -p synvelis-player`、exit 0） |
| `mise run lint synvelis-player` | Pass（`cargo clippy --locked --all-targets -p synvelis-player -- -D warnings`、exit 0） |
| `mise run test synvelis-player` | Pass（`cargo test --locked -p synvelis-player`、exit 0） |
| `mise run test-rt` | Pass（workspace `rt_` filter、exit 0） |
| `mise run check-headless-deps` | Pass（`node tools/check-headless-deps.mjs`、exit 0 — 12 headless crates に GUI framework 依存なし） |
| `mise exec -- prek run --all-files` | Pass（exit 0） |

`mise run test synvelis-player` の内訳（抜粋）:

- `synvelis-player` lib unittests: 9 tests all ok
  （`process_boundary::loop_wrap_tests` — WrapSim algebra、
  routing slice からの既存 test・非回帰）。
- `tests/tim03_driver_loop_boundary.rs`（新規）: 6 tests all ok
  — `tim03_dlb_01_activation_anchor_and_steady_loop`・
  `tim03_dlb_02_window_end_wrap_belongs_to_next_window`・
  `tim03_dlb_03_no_loop_and_stopped_have_no_wraps`・
  `tim03_dlb_04_set_loop_change_and_clear_loop`・
  `tim03_dlb_05_seek_into_active_loop_cascades`・
  `tim03_dlb_06_sessionless_node_stays_legacy`。
- 既存 TIM-03 suite（`tim03_loop_boundary` 7、
  `tim03_process_boundary` 17、`tim03_process_boundary_rt` 1、
  `tim03_request_plan` 10、`tim03_session_wiring` 10、
  `tim03_attach_wiring` 10、`tim04_plugin_state` 9 等）全 pass
  — routing 側挙動の非回帰。

## 導出結果の突合証拠（R9 相当・driver 経路）

`tim03_driver_loop_boundary.rs` の recording plugin は scheduler
が transport span ごとに呼ぶたび folded
`TransportInfo.position` と frame 数を記録する。call head は
観測した最初の node head と各 call の累積 frame 数から再構成し、
window 内 span cut（= 実 scheduler の wrap frame）を
`HeadlessDriver::attached_window_plan(node).boundaries`
（oracle が実際に評価した applied offset）と照合する。

- DB-01 activation anchor + steady loop: `SetLoop` を
  `stage_window` の pending command として preview に含め、
  anchor の `Seek+Play` batch でも導出 wrap offset が plugin
  観測 interior cut と一致。
- DB-02 window-end wrap: window 末尾ちょうどの wrap は offset
  が window 長と等しくなるため供給されず、次 window の head
  wrap（offset 0）として扱われる — plugin 観測と一致。
- DB-03 no loop / stopped: wrap point ゼロ、
  `plan.boundaries` 空 — 非 playing 状態で誤導出しない。
- DB-04 `SetLoop` 変更 / `ClearLoop`: pending に既 stage 分を
  含めた preview が mid-window command 適用後の wrap を
  scheduler と同じ順序で評価 — plugin 観測と一致。
- DB-05 seek into active loop: seek quiesce pass が node cursor
  を driver cursor より先行させる区間でも `sim_window` が
  node cursor domain を追従し、`set_clock` re-anchor 後の
  cascade（same-head 複数 wrap の 1 point collapse）が
  plugin 観測と一致。
- DB-06 sessionless node: `boundary: None` の node は
  `stage_window` を通らず legacy lenient 経路のまま —
  `attached_window_plan` は `None`、挙動非回帰。

## 設計上の要点（証拠の読み方）

- `AttachedDispatch.wrap_sim` は `WrapSim::new(frame_base)` で
  初期化し、唯一の mutation point は
  `observe_attached_block`（各 processed window の walk）。
  preview（`loop_wrap_boundaries`）は clone に対する read-only
  walk で、live state を消費しない。
- `sim_window` は node cursor domain — seek quiesce pass で
  driver `cursor` と一時乖離するため独立に保持し、
  `set_clock` 成功時に即時追従する。
- `staged_transport` は node に queue 済み・未観測の command
  を push 順で保持し、全て `due = 0`（次 callback head apply）
  として pending に解決する — routing の
  `RoutingControlDispatch::staged_transport` と同じ契約。

## 既知の盲点（review-protocol で CONFIRMED・非 blocking）

`ParamDispatch::dispatch`（params write 経路）が node queue に
直接積む内部 `Play` は driver の `staged_transport` を通らない。
node が `Playing` の間は re-anchor が位置 no-op で無害だが、
`panic` 後 `resume_attached_transport` 前の非 playing 状態で
consumer が `attached_dispatch` / `attached_boundary` 経由で
param dispatch すると wrap 導出が古いまま残る。routing は自ら
の call site でこの `Play` を補記するが、driver 経路の call
site は consumer 所有の accessor であり driver.rs 内では捕捉
できない — `driver.rs` の doc comment と design doc に契約を
明記済み。根本対応（dispatch 層からの command 可視化）は別
slice の検討事項。

## NotRun / 証拠分類

- 本 slice の Pass は headless lane（Fake Plugin + 実
  `HeadlessDriver`/`ProcessBoundarySession`/`WrapSim`/
  `TransportScheduler`）の実測のみを意味する。
- 実 CLAP/VST3/AU plugin process・実 OS/GUI・RT
  NoAlloc/NoBlock の実機計測・consumer 照合
  （AL f64/native PCM）・precision certification/
  CapabilityMatrix・Ruviform #630 実機・EXT-L3 は本 slice の
  Out of Scope であり `NotRun`（POLICIES.md の
  OS-L1/GUI-L2/EXT-L3 区分では headless fixture = L1 相当）。
- `*-msrv`・`*-full` 系は Daily Full CI 委譲のため未実行
  （NotRun）。
