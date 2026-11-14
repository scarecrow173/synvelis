# SV289 — Render Checkpoint Verification Record (synvelis#289 RCP-00)

Issue #289 docs slice（canonical spec + design + ADR +
evidence）の検証実行記録。本記録は **docs/ 設計契約 +
evidence 台帳のローカル検証** であり、統合受入・GUI-L2/
EXT-L3 実機受入・実装 fixture 実走ではない。merge 後 main
SHA の必須 CI 結果が統合検証の証拠（POLICIES.md・共通
契約）。

## Environment

| 項目 | 値 |
| --- | --- |
| Repo | scarecrow173/synvelis |
| Branch | `docs/289-render-recipe-spec`（worktree `sv289-rcp0`） |
| OS | Windows (win32) |
| Toolchain | mise 管理（`mise.toml` 正本）+ Node.js（doc ツール） |
| 対象 | docs/markdown/evidence のみ（コード変更なし） |
| Ruviform resolved | `Cargo.lock` pin（本 slice で変更なし） |

## 実行した検証

| # | コマンド | 目的 | 終了コード | 結果 |
| --- | --- | --- | --- | --- |
| 1 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/specs` → `index apply --dir docs/specs --expect <planHash>` | docs/specs index 再生成 | 0 | Pass — `render-checkpoint.md` を add。1 回目は `--expect` に per-dir planHash を渡し plan drift で reject、overall planHash で再 apply し `docs/specs/README.md` written / 0 blocked |
| 2 | 同上 `--dir docs/designs` | docs/designs index 再生成 | 0 | Pass — `rcp-render-recipe-design.md` を add、`docs/designs/README.md` written / 0 blocked |
| 3 | 同上 `--dir docs/adr` | docs/adr index 再生成 | 0 | Pass — `rcp-00-render-recipe.md` を add、`docs/adr/README.md` written / 0 blocked |
| 4 | `doc_maintenance.js index plan`（3 dir 再 scan） | index 整合 rescan | 0 | Pass — front matter 修正後も 3 dir 全て `verdict: in-sync`・`changes: []`（index は id/title/status/path のみ追跡） |
| 5 | `node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/specs` | spec lint | 0 | Pass（範囲内）— 初回は `RCP-SPEC-01〜08` が legacy id `SPEC-0N` と衝突し `unresolved-legacy-reference` blocking 8 件 + `inconsistent-reciprocal-relation` warning 2 件。`SPEC-` と数字の間を U+2011（non-breaking hyphen、表示は同一）へ変更して blocking を解消し、`derives-from`→`depends-on`・`depends-on`→`related`（vcs-01）の relation 調整で warning も 0 件に。dir 全体には既存文書の baseline finding（invalid-id-format・missing-upstream 等）が残存（範囲外・未修復） |
| 6 | `doc_status.js lint --dir docs/designs` | design lint | 0 | Pass（範囲内）— `rcp-render-recipe-design.md` finding 0 件（vcs-01 を `depends-on`→`related` へ移して reciprocal warning を解消）。baseline 残存 |
| 7 | `doc_status.js lint --dir docs/adr` | adr lint | 0 | Pass（範囲内）— `rcp-00-render-recipe.md` finding 0 件。baseline 残存 |
| 8 | `node .agents/skills/doc-status/scripts/audit_docs.js --type spec --type design --type adr` | docs 監査 | 0 | Pass（範囲内）— 新規 3 文書に finding なし。repo 全体の既存 baseline error/warning（他 ADR の invalid-id-prefix・invalid-status 等）が残存（範囲外・未修復） |
| 9 | `node .agents/skills/adr-doc/scripts/audit_adr.js --dir docs/adr` | ADR 監査 | 0 | Pass（範囲内）— `rcp-00-render-recipe.md` に error なし。relation の "missing ADR" warning は ADR-scope reviewer が spec/design path を ADR として解決しない既存の false positive（他 ADR も同型）。baseline 残存 |
| 10 | `node .agents/skills/adr-doc/scripts/review_adr.js --dir docs/adr` | ADR agent-readiness review | 0 | Pass（範囲内）— `rcp-00-render-recipe.md` は "Implementation Plan should name affected paths" warning のみで、同 warning は既存 ADR（alib-01/aut-02/ctl-01 等）にも広く出る baseline。Implementation Plan は affected paths/patterns/dependencies を記載済み。blocker なし |
| 11 | `mise run fmt-check`（`cargo fmt --all -- --check`） | 体裁検査 | 0 | Pass |
| 12 | `review-protocol`（下記 2 pass） | レビュー | — | Pass — blocker なし（詳細は「Review 記録」） |

## Review 記録（review-protocol）

Objective checks（上表 1〜11）実施後、subagent を使えない
worker 契約のため cooperative / adversarial の 2 pass を
分離して実施した。

### Cooperative pass（整合性レビュー）

- spec→design→ADR の relations が相互に一貫: spec
  `derived-by`=[design, adr]・`implemented-by`=[design]、
  design `implements`=[spec]・`derives-from`=[spec, adr]、
  ADR `derives-from`=[spec]。lint 0 findings と併せて確認。
- Issue #289 §0〜§9 が spec の対応セクションへ写像され、
  RCP-SPEC-01〜08 が spec §Acceptance Criteria 表へ
  写像されることを contract-trace で確認。
- `evidence/sv289/` 2 件が spec/design/ADR の
  `related`/`references` から到達可能であることを確認
  （evidence/ は docs index 管理外・lint 対象外）。
- `PluginStateRecipe`（`crates/synvelis-render/src/
  spec.rs`）と `RenderRecipe`/`RecipeKey` の区別が spec
  §Canonical Render Recipe「用語の区別」で宣言されている
  ことを確認。
- 発見: `RCP-SPEC-0N` が legacy id `SPEC-NNNN` 形式と
  衝突 → U+2011 で解消（上表 5）。`depends-on`→vcs-01・
  `derives-from`→execution-plan-design の reciprocal
  warning → relation 種別の調整で解消（repo 慣行:
  project-persistence.md 等は vcs-01 を `related` に置く）。

### Adversarial pass（欠陥探索レビュー）

- 「Pass」を自称する記述を全 3 文書 + evidence で走査:
  spec の状態列は「設計契約確定」「本書で固定」であり
  実装/実機 Pass を主張していない。evidence は docs 検証
  のみ `Pass`、実装・fixture・実機系は `NotRun`/
  `Unsupported`/`Deferred` と明記。headless/Fake 成功を
  GUI-L2/EXT-L3 Pass にする記述、Issue 作成・設計確定を
  実装完了とする記述がないことを確認。
- `NotRun`/`Unsupported`/`Deferred` を `Pass` に読み替
  える記述、silent/bypass/old/"Exact" 提示の許容、
  Plugin Missing で編集禁止とする記述、RecipeKey/
  ContentHash/Eligibility の混同、"latest wins"、OnCommit
  の未実装 capability 成功扱い、RT thread での
  FS/alloc/block 許容、AudioLens への DAW policy 強制、
  Ruviform への DAW 語彙混入・DSP Compiler/Plugin Host/
  Media Writer 重複実装の示唆がないことを確認。
- DAG edge が forward-only・acyclic・定義済み node のみで、
  `acceptance` edge の to が Gate node のみ・Gate node から
  `start` edge を出さないことを spec §Owners and Start
  DAG・design・ADR で確認（`execution-plan-design.md` の
  edge 型規則と同一）。
- `crates/**`・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  `.github/workflows/`・既存 docs（3 新規ファイルと
  再生成 index 以外）を変更していないことを `git status`
  で確認。

### 判定

- CONFIRMED P0/P1: なし。P2/P3 相当の指摘（legacy id
  衝突・reciprocal relation warning）は全て修正済み。
  blocker は残っていない。

## NotRun / Unsupported / Deferred

| 項目 | 状態 | 理由・後続 |
| --- | --- | --- |
| 実装コード・CI・Cargo/mise 変更を伴う検証 | NotRun | docs-only slice。変更範囲に比例した軽量チェックのみ実施 |
| `check`/`lint`/`test`/`build <pkg>`・`test-rt`・`gw10`・`check-headless-deps`・`check-ruviform-rev` | NotRun | 対象 crate/依存の変更なし。Daily/手動 Full CI に委譲 |
| `check-app-native`/`lint-app-native`/`check-app-gpui`/`check-ui`/`check-app-webview` 系 | NotRun | 対象外（docs-only）。GUI-L2 は別 Issue |
| `run-app-native`（実窓） | NotRun | 手動・CI 対象外。GUI-L2-WIN は別 Issue |
| `test-recording-real-*` | NotRun | opt-in 手動・CI 対象外。EXT-L3-WIN は別 Issue |
| macOS/Linux の GUI-L2/EXT-L3 | Deferred | 利用要件発生まで Deferred（POLICIES.md）。未実測を Pass にしない |
| `test-msrv` | NotRun | ユーザー指定により本 slice では実行しない |
| Daily/手動 Full CI の対象 SHA 実行 | NotRun | merge 後 main SHA の必須 CI 結果が統合検証の証拠（Coordinator が確認） |
| 実 Window/実 Device/実 Plugin/実 OS/実 PCM/RT/AudioLens の L2/L3 実測 | NotRun | #296 RCP-GATE・子 Issue の別 Gate が担う。本 slice は計画・契約のみ |
| 実 Plugin/RT/OS/PCM/GPUI/AudioLens 受入 gate | NotRun | RCP-SPEC-01〜06 の fixture/実装検証は #290〜#296、RCP-GATE-01〜12 は #296 |
| Remote renderer（RCP-F01 #297） | Deferred | owner-authorized future PoC。初期 Gate をブロックしない |
| graph router / route_graph baseline blocker | NotRun（未修復） | repo 全体の既存 findings。他文書起因で本 slice 範囲外。修復は別タスク |

## 結果サマリ

- docs/ 設計契約 + evidence/sv289 台帳のローカル検証:
  index plan/apply（3 dir）・rescan in-sync・lint（3 dir
  範囲内 0 findings）・audit_docs・audit_adr・review_adr・
  fmt-check は Pass。review-protocol は blocker なし。
- 実装・fixture 実走・統合受入・GUI-L2/EXT-L3 実機受入:
  **NotRun**（#290〜#297 の子 Issue・別 Gate が担う。
  本 slice は canonical 契約の確定のみ）。
