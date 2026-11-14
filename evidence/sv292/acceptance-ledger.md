# Acceptance Ledger — synvelis#292 RCP-03 headless スライス

Date: 2026-11-14. Slice: RCP-03 Render Policy・
Save/Commit/Share Trigger・Async Job orchestration・
Completeness verdict の `synvelis-render::{policy,render_job}`
への headless 実装。実 OS/filesystem・実 VCS provider
（RCP-05）・実 async job system・RT lanes・GPUI
（sv#295）・cross-gate（RCP-07）は DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/292
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | feat/292-rcp03-render-policy HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 |
| Toolchain | mise 管理 Rust（`mise run` 経由） |
| Project schema | 既存 schema を read adapter で消費（変更なし） |
| Recipe schema | `RECIPE_SCHEMA_VERSION` = 1（RCP-01 由来・変更なし） |
| Artifact manifest schema | `ARTIFACT_MANIFEST_VERSION` = 1（RCP-02 由来・変更なし） |
| Policy/job schema | 本スライスの typed model（永続化 schema は後続 slice） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-render/tests/
  rcp03_render_policy.rs` が `mise run test synvelis-render`
  で green。
- **NotRun**: 実 OS/実 filesystem・実 VCS provider・実
  plugin・実 async scheduler・実 RT lane・GPUI — 本
  スライスでは実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。
- **Unsupported**: typed `Unsupported` 返却が契約の経路
  （Commit API 非対応等）は fixture で検証済み。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| RCP-POL-01 | Pass (headless) | `pol01_mixed_triggers_coalesce_into_one_job_one_artifact`（Manual+OnSave+OnShare 混合 event が同一 `RenderJobKey` に coalesce・artifact publish は recipe 毎 1 回・全 trigger provenance を `JobIntent` が保持）、`pol01_duplicate_events_never_duplicate_work`（同一 event 再送で executor call 数・artifact 数が増えない・冪等）。 |
| RCP-POL-02 | Pass (headless) | `pol02_policy_edits_leave_recipe_key_quality_edits_move_it`（triggers/scope/completion/budget/local override 変更で `RecipeKey` 不変・`policy_version` のみ job identity に反映、音質に効く RenderSettings 変更で `RecipeKey` が移動）。RCP-01 `resolve_recipe` の environment/policy 分離をそのまま消費。 |
| RCP-POL-03 | Pass (headless) | `pol03_save_failure_launches_no_render_job`（`SaveEvent::Failed` は inert な external receipt のみ・job 0 件）、`pol03_saved_revision_pinned_and_never_reassociated`（save 成功 receipt の `ProjectRevision` を job の `saved_revision` に pin・その後 R2 へ編集しても completion/receipt は R1 に留まる）。 |
| RCP-POL-04 | Pass (headless) | `pol04_commit_unsupported_is_typed_not_silent`（Commit capability 非対応 → typed `Unsupported`・job 起動なし・偽成功なし）、`pol04_provider_and_push_failures_stay_off_job_stream`（provider/push 失敗は external receipt stream のみ・render job receipt と混同しない）、`pol04_commit_success_pins_immutable_locator_and_attaches_manifest`（commit 成功 → immutable locator pin・RCP-02 external manifest attach で commit SHA/locator 不変）。 |
| RCP-POL-05 | Pass (headless) | `pol05_besteffort_share_discloses_partial_never_complete`（BestEffort は partial share を許可するが verdict は Complete を名乗らない）、`pol05_requirecomplete_share_blocks_with_typed_gaps`（RequireComplete は typed gap で publish block + 理由返却）、`pol05_missing_plugin_is_a_typed_gap_never_complete`（missing plugin は `CompletenessGapKind` の typed gap・Complete にしない）、`pol05_complete_package_is_allowed`（全 recipe eligible Exact で share 許可）。RCP-02 `lookup()` outcome と RCP-04 語彙（`ArtifactEligibility`/`ReproducibilityClass`）に整合。 |
| RCP-POL-06 | Pass (headless) | `pol06_ondemand_requires_explicit_authorization`（OnDemand は injected authorizer/grant なしに resource 消費しない・拒否は job 0 件）、`pol06_budget_and_permission_denials_reject_before_any_work`（budget/permission 拒否は typed reject・executor call 0）、`pol06_remote_backend_requires_named_consent`（remote backend は named consent なしに実行禁止）、`pol06_storage_quota_pends_until_headroom`・`pol06_concurrency_quota_pends_then_resumes`（環境 quota/storage pending は `drive()` の gate 再評価で resume・`retry()` は bypass しない）、`pol06_retryable_failure_pends_and_retry_completes_once`（transient worker failure は retryable・attempt 会計を保持しつつ 1 回だけ完了）、`pol06_crash_resumes_without_duplicate_artifacts`（crash 観測 → resume で artifact 重複 publish なし）、`pol06_cancel_before_and_during_render_is_typed`（cancel は typed outcome・render 中 cancel も協調的に処理）。 |
| RCP-POL-07 | Pass (headless・port 契約まで) | `pol07_entry_surfaces_share_one_action_and_coalesce`（同一 `JobIntent`/Gateway Action 相当の typed port 経由で headless 投影が 1 action・1 job・同一 receipt に収束することを検証）。実 GUI/CLI/Workflow/Script 投影は lease 外のため NotRun（下記）。 |

## NotRun / Deferred 一覧

- 実 OS/filesystem・実 VCS provider（Save/Commit/Share の
  実 provider、RCP-05/#231 の責務）: NotRun。本 slice の
  trigger port は injectable trait + headless fixture で、
  実 Git/実 FS semantics を検証していない。
- 実 async job system・production scheduler・RT lanes:
  NotRun。`JobOrchestrator::run()`/`drive()` は同期の
  scheduler 等価物であり、実非同期 runtime への配線は
  後続 slice。
- GUI/Headless CLI/Workflow/Script の実 Gateway Action/
  Grant 投影（synvelis-gateway/automation/script/
  workspace は sv#272・sv#279 の active lease）: NotRun。
  本 slice は同一 Action/Grant に投影可能な typed port/
  contract までを検証。
- GPUI Policy UI・Missing Plugin/Render Availability 表示
  （sv#295）: NotRun（read-only 語彙整合のみ）。
- cross-gate（RCP-07）: Deferred。
- `synvelis-project` 側の declared preference/immutable
  intent 本体: 本 slice では `PolicyPatch`/provenance を
  render 側 typed model として実装。project schema への
  永続化配線が必要になれば範囲拡張を coordinator に
  相談する（現状 headless fixture 範囲では不要と判断）。
- Offline render failure の realtime fallback 不在・
  RT blocking 不在は型・経路上保証（executor 注入・
  別 lane なし）だが、実 RT 計測は NotRun。
- Cost estimation model: trigger routing 経路は
  `JobCostEstimate::default()`（宣言なし=0）を積むため、
  resource budget / storage-quota gate は caller-declared
  estimate のみを評価する。実コスト推定は後続 job-gate
  slice の責務（本 slice の gate 構造と typed pending/
  reject は fixture で検証済み）。
