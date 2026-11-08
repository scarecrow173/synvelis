---
id: "PLAN-034UfDsBnYf8HjGm7Hvr0N"
type: "plan"
status: "in-progress"
title: "SF-03 slice3 実装計画 — 実 plugin host 接続・世代連動 teardown"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-slice3-plugin-host-connect-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
    - "docs/designs/sf-03-slice3-plugin-host-connect-design.md"
  derived-by:
    - "docs/tasks/sf-03-slice3-host-boundary.md"
    - "docs/tasks/sf-03-slice3-engine-integration.md"
    - "docs/tasks/sf-03-slice3-acceptance-tests.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice3-plugin-host-connect.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice3 実装計画 — 実 plugin host 接続・世代連動 teardown

## 対象ファイル

| 変更 | ファイル |
| --- | --- |
| 新規 | `crates/synvelis-player/src/host.rs` — `ProcessorHost`/`ProcessorConnectRequest`/`ProcessorConnection`/`ProcessorLease`/`ProcessorConnectError`、`StaticProcessorHost`、`PluginHostProcessors`（`Rc<RefCell<PluginHost>>` adapter + best-effort teardown closure） |
| 変更 | `crates/synvelis-player/src/lib.rs` — `host` module 公開、`host` 系型を再 export |
| 変更 | `crates/synvelis-player/src/routing_lower.rs` — `engine_format` を `pub(crate)` 化（host request 用 `port_format` 生成） |
| 変更 | `crates/synvelis-player/src/routing_engine.rs` — `RoutingDeps.processors` 撤去、`host` フィールド追加、`stage_build` で semantic processor ごとに `host.connect`、offline gate を接続済み processor に対して実施、`PreparedRouting.connections` + publish/observe_activation/seek での世代連動 teardown |
| 変更 | `crates/synvelis-player/tests/sf03_l1.rs`・`sf03_l2.rs`・その他 `RoutingDeps` 構築箇所 — `deps.processors` を `StaticProcessorHost` へ移行 |
| 新規 | `crates/synvelis-player/tests/sf03_l3.rs` — slice 3 受入テスト（static + 実 `PluginHost`） |
| 新規 | `docs/tasks/sf-03-slice3-host-boundary.md`・`docs/tasks/sf-03-slice3-engine-integration.md`・`docs/tasks/sf-03-slice3-acceptance-tests.md` |
| 新規 | `docs/test-specs/sf-03-slice3-plugin-host-connect.md` |

## 順序

1. `host.rs` — boundary 型 + `StaticProcessorHost` + `PluginHostProcessors`。
   host 単体で lifecycle 順序（Instantiate→Initialize→Activate→
   StartProcessing→attach）と teardown を検証する下位 test を先に書く。
2. `routing_engine.rs` — `RoutingDeps`/`stage_build`/`publish`/
   `observe_activation`/`seek` の接続・世代 teardown 統合。
3. 既存 test の `RoutingDeps` 移行（`sf03_l1.rs`/`sf03_l2.rs`/他）。
4. `sf03_l3.rs` 受入テスト（static fault・real host・rapid generation・
   teardown・last-good・offline）。
5. `docs/test-specs`・tasks 文書仕上げ、index 更新。
6. `git fetch origin main && git merge origin/main` 後、
   `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
   `check-headless-deps`。

## リスク

- `Rc<RefCell<PluginHost>>` により `RoutingEngine`/`PreparedRouting` が
  `!Send` 化 → headless 単一スレッド契約内では問題なし。RT 側に
  `PluginNode` のみが乗るため RT `Send` 要件は維持される。
  GUI/app 側で engine を別スレッドへ移す変更が来た場合は別 slice で
  boundary 再検討（PR 本文に残件として明記）。
- `validate_buses` の per-direction 順序不一致で実 plugin が
  `Incompatible` になる可能性 → semantic port 順（port_ids 順）と
  plugin `buses()` 報告順をテストで固定し、報告順は `BusReport::
  Reported` を返す test plugin で検証。`Unavailable` は unverified
  扱いで失敗にしない（upstream 契約）。
- lease drop の reentrancy（`try_borrow_mut` 失敗時の静黙 skip）→
  teardown は best-effort として設計済み。`connect` 中に lease drop
  が走る経路は engine 内に存在しないことを code review で確認。
- publish 失敗パスで `PreparedRouting` が部分 move される →
  `connections` field drop は残フィールド単位で正しく走ることを
  test（compile 失敗後の host instance 解放）で実測。
- `PluginHostProcessors` の `ProcessConfig` 生成は engine
  `block_frames`/`sample_rate`/channels から行い、`AttachConfig.bounds`
  と一致させる必要がある → `pc_b.rs` fixture を template にし、
  mismatch は `Incompatible` fault として test で固定。

## Task Breakdown

- task 1（`sf-03-slice3-host-boundary`）: `host.rs` 新規 + lib.rs export +
  host 単体 test。
- task 2（`sf-03-slice3-engine-integration`）: `routing_lower.rs` /
  `routing_engine.rs` 改修 + 既存 test 移行。
- task 3（`sf-03-slice3-acceptance-tests`）: `sf03_l3.rs` + test-spec。

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` / `mise run test-rt` / `mise run check-headless-deps`
      が全て exit 0。
- [ ] `cargo test -p synvelis-player --test sf03_l3` が green。
- [ ] `RoutingDeps.processors` が撤去され、全 `RoutingDeps` 構築箇所が
      `ProcessorHost` boundary 経由になっている。
- [ ] `sf03_l3.rs` で以下を実測: static 接続成功、PluginHost 実接続で
      audio 処理結果が出力へ反映、未登録 plugin → `PluginUnavailable`、
      bus/bounds 不一致 → `UnsupportedFormatOrTimeMap`、lifecycle 失敗 →
      `PluginUnavailable`、pending 世代置換で旧 lease 解放、Active 観測で
      旧世代 lease 解放、接続失敗で last-good 維持。

## Self-Review

- spec R1–R4 は task 1（boundary）と task 2（engine 統合）に写像済み。
- placeholder なし。ファイルパスは既存 crate 構成に一致。
- `docs/test-specs/sf-03-slice3-plugin-host-connect.md` は task 3 で作成し
  plan の `verified-by` に登録する。

## Review Handoff

- 前提: slice 1（`a652123`）・slice 2（`3fabfef`）が main に存在。
- upstream API の根拠は ruviform rev `863a7a9` の
  `ruviform-plugin-host`/`ruviform-audio-plugin-bridge` ソース。
- 未解決の残件（scope 外）: GUI/app/session への host 配線、
  `EngineStub` 置換、実 OS/デバイス検証、NoRebuild 最適化。
