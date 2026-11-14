# AUTX-04-F01 Composite forward reference — Case Ledger（synvelis#432 headless slice）

Date: 2026-11-14。Slice: `ProjectOp::Composite::validate` の
逐次 dry-run 化（`crates/synvelis-project/src/ops.rs`）と
headless 回帰/新規テスト（`crates/synvelis-project/tests/
composite.rs`）。**本 slice は project crate 内の Domain
Oracle のみ**を対象とし、GW-04 scratch-ID/rewrite seam・
`synvelis.operation.batch` 非空 `bindings` の昇格
（`crates/synvelis-gateway`、sv#265 リース）・実 OS/実
Plugin/実 RT 経路は NotRun/Deferred である。headless の
Pass を runtime/実機の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/432
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `feat/432-composite-sequential-validate`（worktree `sv432-compfwd`。base = origin/main `1721deb`） |
| OS / CPU / FS | Windows 11 / NTFS（C:） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89（`*-msrv` レーンは対象外） |
| Build | mbx managed target（worktree `target` リンク先。`CARGO_TARGET_DIR`/`MBX_TARGET_ROOT`/`--target-dir` 未設定） |
| Project schema | v14（変更なし — validate 意味論のみ） |

## 判定語彙

- **Pass**: 当該 SHA で検証が exit 0 で完了し、対象
  test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 OS 結合）。
- **Deferred**: 別 Issue/別リースが Owner の項目
  （gateway `bindings` 昇格 = sv#265 リース、#265/#266/
  #267/#272/#273）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-project` | Pass（exit 0） |
| `mise run lint synvelis-project` | Pass（exit 0、clippy `-D warnings`・`--all-targets`） |
| `mise run test synvelis-project` | Pass（43 test binaries・0 failure。新規 `tests/composite.rs` 6 件含む） |
| `mise run check synvelis-workspace` | Pass（exit 0。`synvelis-project` 直接依存の依存先整合） |
| `mise run check synvelis-render` | Pass（exit 0。同上） |

補助証拠（canonical = 上記 mise task）: `cargo test -p
synvelis-project --test composite` の直接実行（新規 test
file 反復中）— 結果は mise task と一致。

注: Issue 本文の dependent crate 記述は `synvelis-automation`
だが、origin/main（`1721deb`）で `synvelis-automation` は
`synvelis-project` に依存しない。実依存先である
`synvelis-workspace`・`synvelis-render` の `check` を依存先
整合として実行した（`synvelis-automation`・`synvelis-gateway`
はリース外のため未検証 — 下記 Deferred 参照）。

## 受入条件 → case 対応表

### AC-1 `Composite::validate` の逐次 dry-run 化・前方参照許可

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| CMP-FWD-01 | `[AddTrack, AddLane{predicted}, AddLane{predicted}]` が `ProjectController::apply` で commit まで通り、member 0 の mint ID と参照 ID が一致 | commit 成功・track が 2 lane を保有・`live_revision` +1 | Pass | `composite_create_then_reference_commits_as_one_undo_step` |
| CMP-FWD-02 | track/lane 以外の kind（asset→source）でも create-then-reference が成立 | `RegisterAsset`→`RegisterSource{predicted_asset}` が commit・参照反映 | Pass | `composite_forward_reference_works_for_asset_source_pair` |
| CMP-FWD-03 | dry-run 採番が `project.id_watermark` シードで live ID を消費しない | validate 失敗時に live/undo/revision/generator が不変（CMP-REG-03/04 と併せて確認） | Pass | `ops.rs` validate arm + `CounterIdGenerator::new(project.id_watermark)`、`docs/impl/exp/autx-04-f01-composite-forward-ref.md` E2 |

### AC-2 member 毎の早期 validate・API・非 composite・単一 undo の不変

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| CMP-REG-01 | member 0 失敗時に member 1 を validate/apply しない | `NotFound` が member 0 の track を指し、live に track/lane なし・undo entry なし | Pass | `composite_member_failure_never_reaches_later_members` |
| CMP-REG-02 | mid-member 失敗で全体 reject・部分 commit なし | member 0 の dry-run mint は破棄、live/revision/undo 不変 | Pass | `composite_mid_member_failure_discards_the_whole_dry_run` |
| CMP-REG-03 | 空/nested composite の reject 語彙不変 | `Validation("empty composite")`/`Validation("nested composite")` を完全一致で確認 | Pass | `composite_rejects_empty_and_nested_with_stable_vocabulary` |
| CMP-REG-04 | `StaleRevision` は validate 前に reject・状態不変 | `Err(StaleRevision)`・live/undo 不変 | Pass | `composite_stale_revision_is_rejected_before_validation` |
| CMP-REG-05 | composite は単一 undo/redo step | 1 回の `undo` で track+2 lane がまとめて復元前へ、`redo` で復帰 | Pass | `composite_create_then_reference_commits_as_one_undo_step` |
| CMP-REG-06 | 非 composite 挙動・既存契約の非回帰 | `mise run test synvelis-project` 43 binaries・0 failure（既存 `routing_sf01` composite test・`controller_store` 等を含む） | Pass | 同上 test run |

### AC-3 forward-reference composite の headless commit

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| CMP-HL-01 | create-then-reference composite が commit まで通る headless テスト | `tests/composite.rs` 6 件すべて green | Pass | `mise run test synvelis-project`（`composite.rs` 6/6） |

### AC-4 sv#263 bindings 経路の実 commit 到達（本スライス範囲の切り分け）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| CMP-GW-01 | `synvelis.operation.batch` 非空 `bindings` → placeholder 解決 → `Composite` commit が end-to-end で通る | `crates/synvelis-gateway` は sv#265 リースかつ `synvelis-project`→`synvelis-gateway` 依存が存在しないため本スライスでは実施不能 | Deferred | project 側前提（CMP-FWD-01/02・CMP-HL-01）は Pass。gateway 側昇格試験は sv#265 Worker または後続統合スライスへ引き継ぎ。`prepare_engine.rs` ~L794-815 の `PreparationRejected` fallback は依然として非空 `bindings` を reject する（本 slice では未変更・未検証） |
| CMP-OS-01 | 実 OS window/GUI 経路での batch 実行 | 対象外（headless slice） | NotRun | POLICIES.md OS-L1/GUI-L2/EXT-L3 — 実 GUI 結合は提供機能の受入 Gate |
| CMP-RT-01 | 実 Plugin・実 RT engine 経路 | 対象外（headless slice） | NotRun | 同上 EXT-L3 |
| CMP-MSRV-01 | `*-msrv` レーン | 対象外（ユーザー指定で除外） | NotRun | タスク指定により MSRV lane は実行しない |

## 未検証・引き継ぎ

- **GW-04 scratch-ID/rewrite seam と `bindings` 昇格**:
  `crates/synvelis-gateway/src/prepare_engine.rs`（sv#265
  リース）の後続 slice。必要ファイル:
  `crates/synvelis-gateway/src/prepare_engine.rs`（batch
  `bindings` gate 撤去・placeholder rewrite 接続）、
  必要なら `crates/synvelis-automation`（placeholder
  発行側の契約確認）。本 slice は project 側前提のみを
  証明した。
- **`draw_raw_id` 予約窓を跨ぐ採番系列**: watermark シード
  dry-run が fail-closed reject する既定契約として許容
  （誤 commit なし）。予約を跨ぐ end-to-end は gateway
  seam 側の責務。
- **統合検証**: ローカル検証は変更範囲の検証であり、
  merge 後の対象 SHA の必須 CI 証拠が統合受入
  （Coordinator 確認）。
