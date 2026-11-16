# SV-106 Evidence — ポインタ（synvelis#106）

Issue #106「[impl/signal-flow][SF-03] Semantic→Ruviform Typed
Graph Lowering・Revision Map・Plan Activation/Last-good
Consumer」の証拠ポインタ。sv106 専用の evidence ディレクトリは
存在しない — 被覆証拠は以下の既存資産に分散している
（2026-11 追記・bookkeeping）。

## 被覆証拠の所在

- **`evidence/sv109/coverage.md`** — SF-06 cross-gate headless
  fixture 被覆棚卸し（現行被覆の正本）。SF-03 系 fixture の
  **SF-ACT-01〜09** 行が sv106 受入ケースをカバーする:
  `sf03_l1.rs`（semantic lowering deterministic・
  semantic↔legacy PCM equivalence・two-track additive mix・
  send gain・pre/post tap 区別・processor chain gain PCM）、
  `sf03_l2.rs`（prepare/lower/publish fault taxonomy・
  last-good・reconcile drift・pending publish retry・
  QueueFull・offline capability）、`sf03_l3/l4/l6/l9.rs`
  等。T2 headless Pass、実 device/plugin/PCM 系は T3 gate。
- **`crates/synvelis-workspace/tests/rcp_selection_integration.rs`**
  — sv#293 residual slice（PR #498、merged 2026-11）で追加
  された semantic→selection integration oracle。**10 tests**
  （`rcp_ex01_*` ×4・`rcp_ex03_*`・`rcp_ex04_*` ×2・
  `rcp_ex05_*`・`rcp_ex02_*`・`rcp_producer_*`）が、sv106 の
  SF03-RCP-01/02（render-checkpoint integration legs —
  `docs/impl/ir/sf-03-slice8-revision-fixture-slice.md` で
  「#293 未実装」と記録されていた残件）を被覆する。
  test-spec 写像は
  `docs/test-specs/sv293-semantic-selection-integration.md`
  （TS-01〜14 → `rcp_*` test 名）。RCP-EX-06 実機計測は
  #296 で NotRun 継続。

## 注意

- 本 README はポインタのみであり、新規の実測証拠を主張しない。
  各行の判定はリンク先の台帳・test-spec が正本。
- sv106 の残 gate（実 device・実 plugin・PCM/latency 実測）は
  T3/実機 gate 側の責務 — `evidence/sv109/external-gates.md`
  を参照。
