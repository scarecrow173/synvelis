# Evidence — synvelis#127 TIM-03 Loop boundary 自動導出 slice

Issue #127（TIM-03: Plugin Process境界／Loop・Tempo・
Automation精度 Capability/Degraded Policy）の
loop-boundary 自動導出 slice。`WrapSim`（scheduler 等価
scalar state）を `NodeSurface` に維持し、
`RoutingControlDispatch::boundary_loop_wraps` が
upcoming window 内の loop wrap `BoundaryPoint` を
非 RT で導出する。

## Anchor — 対象 SHA・環境

- branch: `feat/127-loop-boundary-derivation`（worktree
  `scarecrow173/sv127-loopbound`）。base は dispatch 時点の
  `origin/main` `89c519c`。
- OS: Windows（worktree `G:\orca\workspaces\synvelis\
  sv127-loopbound`）。toolchain: rustc 1.99.1
  （mise 管理、`mise.toml` pin）。
- 変更面: `crates/synvelis-player/src/process_boundary.rs`・
  `crates/synvelis-player/src/routing_control.rs`・
  `crates/synvelis-player/tests/tim03_loop_boundary.rs`（新規）
  + docs チェーン + `evidence/sv127/`。`dispatch.rs`・
  `Cargo.lock`・他 crate の変更なし。

## 検証コマンドと結果（canonical = mise）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（`cargo fmt --all -- --check`、exit 0） |
| `mise run check synvelis-player` | Pass（`cargo check --locked --all-targets`、exit 0） |
| `mise run lint synvelis-player` | Pass（`cargo clippy --locked --all-targets -- -D warnings`、exit 0） |
| `mise run test synvelis-player` | Pass（全 test binary ok、exit 0） |

`mise run test synvelis-player` の内訳（抜粋）:

- `synvelis-player` lib unittests: 9 tests all ok
  （`process_boundary::loop_wrap_tests` — WrapSim algebra:
  steady loop 複数 wrap・no loop/not-playing・window
  end-exclusive・head apply tie・mid-window Seek/SetLoop・
  invalid command・same-frame cascade・consecutive window
  scheduler-faithful）。
- `tests/tim03_loop_boundary.rs`: 7 tests all ok。
- 既存 TIM-03 suite（`tim03_process_boundary` 17、
  `tim03_process_boundary_rt` 1、`tim03_request_plan` 10、
  `tim03_session_wiring` 10、`tim03_attach_wiring` 10、
  `tim04_plugin_state` 9 等）全 pass — 非回帰。

## R9 実 scheduler 一致の証拠

`tim03_loop_boundary.rs` の Fake Plugin は scheduler が
transport span ごとに呼ぶたび `(folded position,
frames)` を記録する。call head は最初の観測 node head
と各 call の累積 frame 数から再構成し、span の interior
境界（= 実 scheduler の wrap frame）を導出 offset と
照合する。

- LB-01 steady loop: `SetLoop [0,100)` が head 512 で apply
  され、position 256 が loop end を越えているため head
  wrap（offset 0）+ cascade で anchor 56 に着地。導出
  offset `[0, 44, 144, 244]`（wrap frame 556/656/756、
  856 は次 window）が plugin 観測 span cut
  `[(56,44) (0,100) (0,100) (0,100) (56,44)…]` と
  frame-exact に一致。次 window `[88,188]`、3 番目
  `[32,132,232]` も同様に一致。
- LB-03 mid-window `SetLoop`: 旧 region の wrap は
  apply 前まで有効、新 region install 後の wrap は
  新 region で評価 — plugin 観測の span 分割と一致。
- LB-04 mid-window `Seek`: 予測 wrap が seek apply で
  消え、seek 先が loop end を越える場合の same-frame
  cascade が 1 point に collapse — plugin 観測と一致。
- LB-05 `ClearLoop`: clear 前の wrap は残り、clear 後は
  position が unfold して span が分割されない —
  plugin 観測 `(56,44) (0,84) (84,128) (212,256)` と一致。

## 証拠分類

- 本 slice の Pass は headless lane（Fake Plugin +
  実 `RoutingEngine`/`RoutingControlDispatch`/
  `TransportScheduler`）の実測のみを意味する。
- 実 CLAP/VST3/AU plugin process・実 OS/GUI・RT
  NoAlloc/NoBlock の実機計測・consumer 照合
  （AL f64/native PCM）・precision certification は
  本 slice の Out of Scope であり `NotRun`。
  POLICIES.md の OS-L1/GUI-L2/EXT-L3 区分では
  headless fixture = L1 相当の検証として記録する。
