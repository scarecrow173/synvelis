# DAW Workflow — 検証記録 (Issue #8)

作成: 2026-11-14 / branch: `docs/8-daw-workflow` / OS: Windows
（workspace: `C:\Users\akiyoshi\orca\workspaces\synvelis\sv8-daw-workflow`）

## 1. 実行した検証（文書チェーンの静的検査）

| # | コマンド | 目的 | 結果 |
| --- | --- | --- | --- |
| V1 | `node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/specs --json` | spec front matter/relation/index の検査 | exit 0。`daw-workflow.md` の finding は `broken-relation-link`（plan/task 未作成時点）・`index-missing-entry`/`orphan-index`/`orphan-navigation`・`traceability-missing-upstream`。upstream は `derives-from`（issue #8/#1）追加で解消、残りは後続工程で解消 |
| V2 | `node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/designs --json` | design 同上 | exit 0。同上（index/orphan・未作成 plan/task relation のみ） |
| V3 | review-protocol（spec+design、独立 Reviewer A/B pass → Validator → Judge） | 仕様整合・反例レビュー | verdict: **ACCEPT WITH NON-BLOCKING NOTES**。CONFIRMED P0/P1 なし。確認された問題は全て P2 の文書バグ（design §2.2/§3 節参照誤り・§7.3 gesture ラベル崩れ・spec implemented-by 欠落・制作領域表/P0-P1-P2 表欠落）で全件修正済み。review 回数: 1 サイクル（fix は構造・責務・境界を変えない局所修正のため full re-review 不要と判定） |
| V4 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan` → `index apply --expect <planHash>`（specs/designs/plans/tasks/test-specs/impl-ir の 6 dir） | 触れた dir の index 再生成 | 6 target 全て written、0 blocked。`docs/impl/exp` は in-sync（同 index は `.jsonl` のみ対象で repo 慣行の `.md` exp 記録は index 対象外 — sv3 と同じ扱い） |
| V5 | `doc_status.js lint --dir docs/{specs,designs,plans,tasks,test-specs} --json`（index 再生成後） | 新規文書の検査 | exit 0。`daw-workflow` 関連 finding **0 件**（index/orphan/relation 全解消）。plans の `inconsistent-reciprocal-relation`（非 blocking）1 件を spec `implemented-by` 追加で解消 |
| V6 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | IR 監査 | `daw-workflow.md` の finding なし（`## Context`/`## Implementation` 節を sv3 慣行に合わせて補正後） |
| V7 | `node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/impl/{ir,exp}` | impl dir lint | exp `.md` は index/lint 対象外（repo 慣行）。ir は V6 の audit で検証 |
| V8 | `mise run fmt-check`（`cargo fmt --all -- --check`） | canonical 検証コマンド | **exit 0 / Pass**。コード変更なしのため Rust 差分への影響なし |
| V9 | `git diff --check` | 空白エラー検査 | **exit 0 / Pass** |
| V10 | `git status --porcelain` / `git fetch origin main` | 変更範囲・main 取り込み確認 | 変更は `docs/` 新規 7 文書 + `evidence/sv8/` 新規 2 ファイル + 再生成 index README 6 件のみ。`crates/`・`Cargo.*`・`mise.toml`・CI・`docs/mockups/`・他 docs への変更なし。origin/main は前進（86715f9..9c9278b）しており merge 時に index README が競合 → merge 後に index を再生成して解消 |

## 2. Baseline（既存・本タスク範囲外）の finding

| 項目 | 内容 | 扱い |
| --- | --- | --- |
| `doc_status lint` repo 全体 | designs dir で 175 blocking findings / 187 docs（既存文書由来の broken relation・index 不整合等） | 既存 baseline。本タスクで修正しない（編集範囲外）。新規文書起因の finding は V5 で 0 件を確認 |
| `doc-driven-dev-graph` route | sv3 記録と同様、repo 全体の baseline graph 問題で fail-closed する可能性あり | 今回は文書を宣言済み順（spec→design→plan→task→test-spec→ir/exp）で作成。router 実行結果を偽装しない |

## 3. NotRun / Deferred レーン（POLICIES.md §2/§3 語彙）

| レーン | 状態 | 理由・後続 Owner |
| --- | --- | --- |
| design-review（本書・spec・test-spec の静的レビュー） | **Pass**（V3 review-protocol + V5 lint） | 文書契約の検収のみ。UX 挙動の実測ではない |
| headless（Project/Undo/Export/WAV decode の論理検証） | **NotRun** | 本タスクは文書のみ。WF-AC の headless scenario 化は #10/#11/#12/#21/#114 後続 slice |
| gpui-test（GPUI-simulated） | **NotRun** | 本タスクは文書のみ。scenario 接続は #112/#114 |
| real-os（実 OS Window/Focus/IME/DPI/2 Window/実 Plugin GUI） | **NotRun** | #114（GUI-L2）・#176（EXT-L3 Plugin GUI）が Owner。設計評価を実機 Pass に換算しない |
| macOS/Linux GUI/EXT | **Deferred** | Windows 実機需要が確定するまで Deferred（POLICIES の OS 別規定） |
| 製品コード check/test/build | **NotRun（対象外）** | `crates/` 変更なし。`mise run fmt-check` のみ実施（V8） |
| Keyboard 具体的キー割当 | **Deferred** | #112 で確定（design D-3） |
| Tool HitTest/Snap 数値・Take/Comp Editor UI | **Deferred** | #11/#12 が正本 |
| Remote Browser 詳細 | **Deferred** | #9/ALIB 後続 |
| Signal Flow/Inspector/Version Diff detour 詳細 | **Deferred** | #4/#5/#7（Gate B/C） |
| 新規製品トレードオフ・契約破壊・定量 SLO・OS サポート宣言 | **Deferred** | 人間決定待ち（Issue 2026-11-10 実行判断） |

## 4. 判定まとめ

- 文書チェーン（spec→design→plan→task→test-spec→ir/exp）と
  `evidence/sv8/` 追跡表が存在し、WF-AC-01〜10・WF-SCN-
  MIDI/AUDIO/MIX・GA-01〜08・Issue #8 §別項目の写像に漏れが
  ないことを確認（traceability 記録参照）。
- review-protocol: 1 サイクル、CONFIRMED P0/P1 なし、
  blocker 残存なし → Worker 共通契約により承認代替。
  spec/design/test-spec = `approved`、task = `done`、
  plan = `approved`、ir/exp = `completed`。
- 実装・実 OS 検証の完了は一切主張しない。後続 lane は
  #9/#10/#11/#12/#17/#18/#20/#21/#112/#114/#176/#23 に委譲。
