# sv514 decision ledger — SessionRoutingEngine RCP selection 配線採否

Issue: https://github.com/scarecrow173/synvelis/issues/514
Branch: `feat/514-session-engine-selection`
Base: `3e206c03` (origin/main at dispatch)
Date: 2026-11-18

## 判定

**「RoutingEngine レベルで責務が閉じる（実装不要）」** — 分岐 B 採用。

`SessionRoutingEngine` は RCP selection を消費しない。rebuild 経路の
未配線は「意図的」であり、Deferred residual を解消した。

## 判定材料 (a)–(d) の評価

| # | 判定材料 | 結果 | 根拠 |
| --- | --- | --- | --- |
| (a) | missing-plugin Blocked/substitution semantics が app 層で要求されるか | **要求されない** | substitution は `ExecutionSelection.substitutions` の decision 証拠に留まり、`lower_semantic` は常に `project.routing` 全体を lowering するため published plan は変わらない（`crates/synvelis-workspace/src/rcp_selection.rs`・`synvelis-player` lowering）。substitution の実再生反映には engine 側 plan shaping が必要で `synvelis-player` は lease 外。verdict を hard gate にすると `ProcessorHost::connect`（実 instantiate = ground truth）より弱い caller 申告情報で false-block を持ち込む |
| (b) | `render.rs` `RenderStatusScreenState.selection` との整合 | **整合する（producer 待ちのまま）** | `selection: Option<RenderSelectionView>` は「selection が走った時」の投影で、live `ExecutionSelection` の producer を待つ構成（sv#295 面）。自然な producer は per-intent selection service であり、intent 非依存・`BuildMode::Realtime` 固定の rebuild hot path ではない。OfflineRender/Export intent は synvelis-render 側 lowering を通り SessionRoutingEngine を経由しない |
| (c) | `SelectionInputs` の全 pin を `SessionEngineConfig`/composition 経路で供給できるか | **供給不能** | `plugin_evidence`: `ProcessorBinding` は descriptor/version/capability_revision のみで `PluginIdentity`/`PluginStateDecl`/automation completeness の producer なし。`environments`: `PluginHost` は Scan/Instantiate のみで `PluginEnvironment` facts API なし。`render_candidates`: `CandidateReport` は synvelis-render lookup 由来、`SemanticRenderCoverage` producer なし（RCP-01/02/05 領域）。app は synvelis-render 非依存設計で dep 追加も避けるべき |
| (d) | 現行 rebuild が selection 非消費のまま契約を満たすか | **満たす** | `EnginePort::on_project_revision(&Project, ProjectRevision)` は intent・証拠・candidate を運ばない。SF-03 slice5 設計（honest input 供給 + lifecycle 転送、activation/last-good/stale/equivalence は RoutingEngine 内）に適合。Issue 要求の安全性 pin は既存: stale 非 publish（`selection_pin_fault` + `rcp_ex05_stale_selection_never_publishes`）、generation/epoch/build-key（`checked_key` + publish 再検証 + `ex05_*` + player `rcp_04` oracle 26 件）、QueueFull/RetireOverflow last-good（`ex05_queue_full_and_retire_failure_retain_last_good`）、adapter pin（`sf03_session_engine.rs`） |

## 既存 pin の所在（実測）

- workspace `crates/synvelis-workspace/tests/rcp_selection_integration.rs`:
  semantic model→snapshot→selection→reconciliation→engine plan の
  10 case（MainPlayback exact・missing-plugin blocked・substitution・
  intent 別・stale・NoRebuild/ControlOnly/Rebuild 分類・last-good）。
- player `crates/synvelis-player` `rcp_04` oracle: 26 件。
- app `crates/synvelis-app/tests/sf03_session_engine.rs`: session
  apply→publish→run→Active・failure/last-good・rename→Equivalent・
  stale revision/reconcile — 非 selection 経路の adapter pin。

## 「意図的な未配線」の明記箇所

- `docs/adr/issue-514-session-engine-selection.md` — 決定記録。
- `crates/synvelis-app/src/engine.rs` module doc — コードサイトでの
  明示（selection 非消費は意図的、trigger 条件を記載）。
- 本 ledger — 判定根拠の集約。

## 将来の配線 trigger（design 文書と同一）

1. `SelectionInputs` の production 供給経路が存在する
   （plugin env scan API・TIM-04 `PluginStateDecl` 供給・artifact
   coverage mapping）。
2. `ExecutionSelection` の consumer が確定する（sv#295 表示面、
   または substitution を実再生に反映する plan shaping — 後者は
   player 契約変更を伴う可能性）。
3. 対象 intent が確定する。

## AC4 — stateless `PluginStateDecl` の帰属

- 状況: `PluginStateDecl.digest: Option<String>` は「stateless」と
  「未 capture」を型上区別不能。`verify_plugin` の Live 条件は
  `digest.is_some()` を無条件要求し `digest: None` → `StateUnknown`
  fail-closed（`crates/synvelis-player/src/rcp/verify.rs`）。
  `StateCaptureQuality::Unsupported` の語彙は存在するが
  `StateRestoreSupport` に「適用外」variant がなく、判定は player
  内のため workspace 側に lever なし。
- PR #498 時点: 未消化（`evidence/sv293/rcp-04-fault-matrix.md` が
  deferred residual として記録）。
- 本 Issue での処理: **player 契約変更が必要と確定 → 実装せず別
  Issue に帰属**。起票 Issue: #530

## lane Pass/NotRun 簿記

| lane | OS | toolchain | 結果 | 備考 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | rustc 1.99.1 / rustfmt 1.10.0 | **Fail（環境起因・既存）** | `cargo fmt --all -- --check` が os error 206（filename too long）で失敗。clean tree（stash 検証済み）でも同一失敗のため本変更と無関係。全 15 package を `cargo fmt -p <pkg> -- --check` で個別検証し全て pass（補助証拠） |
| `mise run check synvelis-app` | Windows | rustc 1.99.1 | Pass (exit 0) | |
| `mise run lint synvelis-app` | Windows | rustc 1.99.1 | Pass (exit 0) | -D warnings |
| `mise run test synvelis-app` | Windows | rustc 1.99.1 | Pass (exit 0) | headless/fixture のみ・全 suite 0 failed |
| `mise run check synvelis-workspace` | Windows | rustc 1.99.1 | Pass (exit 0) | |
| `mise run lint synvelis-workspace` | Windows | rustc 1.99.1 | Pass (exit 0) | -D warnings |
| `mise run test synvelis-workspace` | Windows | rustc 1.99.1 | Pass (exit 0) | `rcp_selection_integration` 10/10 pass・回帰なし |
| T3/T4 実 device/plugin・実 OS・RT/PCM | — | — | **NotRun** | gate: #109・#296・`scarecrow173/ruviform#630`・`scarecrow173/ruviform#631` |
| `check-headless-deps` | — | — | **NotRun** | dep 追加なしのため対象外（dep 追加時のみ必須） |
| `prek run --all-files` | — | — | **NotRun** | dep 追加なしのため対象外 |

## 関連 Issue / PR

- 本 Issue: #514（Refs のみ、Close は Coordinator）
- 前提: #293（RCP-04 semantic→selection・PR #498 merge `8d0e31ba`）、
  #106（SF-03 SessionRoutingEngine）
- 調整: #517（RuntimePublish 本番配線）、#294（BranchAudition/
  provenance）、#295（live selection 表示面）、#109/#296（T3/T4 gate）
- AC4 follow-up: #530（stateless `PluginStateDecl` 型表現 — player
  契約変更）
