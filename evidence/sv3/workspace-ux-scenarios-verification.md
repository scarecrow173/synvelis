# Workspace UX Scenarios — 検証記録 (Issue #3)

作成: 2026-11-14 / branch: `docs/3-workspace-ux` / OS: Windows
（workspace: `C:\Users\akiyoshi\orca\workspaces\synvelis\sv3-workspace-ux`）

## 1. 実行した検証（文書チェーンの静的検査）

| # | コマンド | 目的 | 結果 |
| --- | --- | --- | --- |
| V1 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type spec --json` | spec front matter/relation/index の検査 | exit 0。`workspace-ux-scenarios.md` の finding は `index-missing-entry`/`orphan-index`/`orphan-navigation` のみ（index 再生成前）。blocking な front matter/relation エラーなし |
| V2 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type design --json` | design 同上 | exit 0。同上（index/orphan のみ） |
| V3 | review-protocol（spec+design、独立 Reviewer A/B pass → Validator → Judge） | 仕様整合・反例レビュー | verdict: **ACCEPT WITH NON-BLOCKING NOTES**。CONFIRMED P0/P1 なし。Issue Candidates C1〜C6（全て P2/P3）を Validator で CONFIRMED 判定後、Worker 共通契約に基づき全件修正（design §6.2/§6.4/§7/§9.1/§9.4 + Decision Log D21〜D24）。review 回数: 1 サイクル（fix は構造・責務・境界を変えない局所修正のため full re-review 不要と判定） |
| V4 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan` → `index apply --expect <planHash>` | 触れた dir の index 再生成 | 10 target 中 6 written（designs/impl-ir/plans/specs/tasks/test-specs）。`docs/impl/exp` は in-sync（同 index は `.jsonl` のみ対象で、repo 慣行の `.md` exp 記録は index 対象外 — 既存 rcd-03 等と同じ扱い） |
| V5 | `doc_status.js lint --type {plan,task,test-spec} --json`（index 再生成後） | 新規 plan/task/test-spec の検査 | exit 0。`workspace-ux` 関連 finding **0 件**（index/orphan 解消済み） |
| V6 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | IR 監査 | `workspace-ux-scenarios.md` の finding なし |
| V7 | `node .agents/skills/impl-doc/scripts/audit_experiment_log.js --json` | exp JSONL 監査 | 本タスクの exp は `.md` 記録（repo 慣行）のため対象外。既存 `issue-147-sccache.jsonl` の invalid-event-shape は **baseline 既存問題**（本タスク無関係） |
| V8 | `mise run fmt-check` | canonical 検証コマンド（`cargo fmt --all -- --check`） | **exit 0 / Pass**。コード変更なしのため Rust 差分への影響なし |
| V9 | `git status --porcelain` / `git diff --stat` | 変更範囲の確認 | 変更は `docs/` 新規 7 文書 + `evidence/sv3/` 新規 2 ファイル + 再生成 index README 6 件のみ。`crates/`・`Cargo.*`・`mise.toml`・CI・`docs/mockups/`・他 docs への変更なし |

## 2. Baseline（既存・本タスク範囲外）の finding

| 項目 | 内容 | 扱い |
| --- | --- | --- |
| `doc_status lint` repo 全体 | 122 blocking findings / 156 docs（invalid ID・invalid date・missing upstream relations・unresolved provenance 等、無関係文書由来） | 既存 baseline。本タスクで修正しない（編集範囲外）。新規文書起因の finding は V5 で 0 件を確認 |
| `doc-driven-dev-graph` route | `status: blocked`、`current: probe`、`edgeId: null`、blockers に `artifact-graph`/`focus-required` + 多数の既存 broken relations | repo 全体の baseline graph 問題。新規文書の relation は個別 lint（V1/V2/V5）で解決確認済み |
| `audit_experiment_log.js` | 既存 `docs/impl/exp/issue-147-sccache.jsonl` の invalid-event-shape | 既存 baseline。本タスクの exp は `.md` のため無関係 |

## 3. NotRun / Deferred レーン（POLICIES.md §2/§3 語彙）

| レーン | 状態 | 理由・後続 Owner |
| --- | --- | --- |
| design-review（本書・spec・test-spec の静的レビュー） | **Pass**（V3 review-protocol + V1/V2/V5 lint） | 文書契約の検収のみ。UX 挙動の実測ではない |
| headless（`synvelis-workspace` WS-A/ED-A 契約） | **Pass（既存固定済み分のみ）** / 新規 UI 挙動は **NotRun** | WS-A-01〜06・ED-A 契約は既存 test で固定済み。本 UX 正本の新規 UI 挙動（Drop Preview・閉鎖復帰 UI・Inspector モード UI 等）の headless scenario 化は #112/#113 後続 slice |
| gpui-test（`mise run test-app-gpui`、GPUI-simulated） | **NotRun** | 本タスクは文書のみ。scenario 接続は #112/#114 |
| real-os（実 OS Window/Focus/IME/DPI/2 Window/実 Plugin GUI） | **NotRun** | #114（GUI-L2）・#176（EXT-L3 Plugin GUI）が Owner。設計評価を実機 Pass に換算しない（Issue #3 明示禁止） |
| macOS/Linux GUI/EXT | **Deferred** | Windows 実機需要が確定するまで Deferred（POLICIES の OS 別規定） |
| 製品コード check/test/build | **NotRun（対象外）** | `crates/` 変更なし。`mise run fmt-check` のみ実施（V8） |

## 4. 判定まとめ

- 文書チェーン（spec→design→plan→task→test-spec→ir/exp）と
  `evidence/sv3/` 追跡表が存在し、WUX-AC-01〜10・WUX-SCN-A〜F・
  Issue #3 §1〜§10 の写像に漏れがないことを確認（traceability
  記録参照）。
- review-protocol: 1 サイクル、CONFIRMED P0/P1 なし、
  blocker 残存なし → Worker 共通契約により承認代替。
- 実装・実 OS 検証の完了は一切主張しない。後続 lane は
  #112/#113/#114/#176/#5/#11/#12 と Ruviform #181/#182/#386/#388
  に委譲。
