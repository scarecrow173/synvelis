# Workspace Model — 検証記録 (Issue #2)

作成: 2026-11-13 / branch: `feat/2-workspace-model`
workspace: `C:\Users\akiyoshi\orca\workspaces\synvelis\sv2-workspace-model`
OS: Windows（MINGW64_NT-10.0-26300, MSYS2）/ toolchain: rustc 1.99.1
（`rustc 1.99.1 (58cf8f9c3 2026-10-13)`、mise 管理）

## 1. 実行した検証

| # | コマンド | 目的 | 結果 |
| --- | --- | --- | --- |
| V1 | `mise run fmt-check`（初回 FAIL → `mise run fmt` 適用後に再実行） | canonical format 検査 | **exit 0 / Pass**（新規 `workspace_model.rs` の fmt 差分を `cargo fmt` で修正後） |
| V2 | `mise run check synvelis-workspace` | workspace crate 型検査 | **exit 0 / Pass** |
| V3 | `mise run lint synvelis-workspace` | workspace crate clippy | **exit 0 / Pass** |
| V4 | `mise run test synvelis-workspace` | workspace crate 全 test | **exit 0 / Pass**。`workspace_model.rs::playback_dock_window_churn_preserves_transport_revision_and_history` を含む全 test pass。初回は新規 test の revision 会計ミスで FAIL → `rev0+3`/`builds0+3`（undo+redo+remove の会計、ws_a_05 と同じ）に修正して pass |
| V5 | `mise run check synvelis-app` | app crate 型検査 | **exit 0 / Pass** |
| V6 | `mise run lint synvelis-app` | app crate clippy | **exit 0 / Pass** |
| V7 | `mise run test synvelis-app` | app crate 全 test | **exit 0 / Pass**（38 test binaries 全件 pass。two_window/sv113_dock/dock_ops/close_veto/shell/view_mount/ui_ops/uic5/vcs04 等） |
| V8 | `node .claude/skills/doc-status/scripts/doc_status.js lint --type {spec,design,plan,task,test-spec} --json` | 新規文書の front matter/relation 検査 | exit 0。workspace-model 起因 finding は `index-missing-entry`/`orphan-*`（index 未生成）と `plan-missing-test-spec-evidence` のみ。`verified-by` 追加 + index 再生成で解消 |
| V9 | `node .claude/skills/doc-maintenance/scripts/doc_maintenance.js index apply` | 触れた dir の index 再生成 | 5 dir written（specs/designs/plans/tasks/test-specs）。IR/exp 作成後に impl/ir・impl/exp も再実行 |
| V10 | `node .claude/skills/impl-doc/scripts/audit_impl_record.js --json` | IR 監査 | workspace-model の finding なし（既存 baseline finding は範囲外） |
| V11 | `node .claude/skills/doc-driven-dev-graph/scripts/route_graph.js --focus docs/specs/workspace-model.md --json` | 文書 graph router | **status: blocked**（`current: probe`、pre-existing `artifact-graph`/`broken-relation` — repo baseline、範囲外。evidence/sv2 台帳 §4 に記録） |
| V12 | review-protocol（spec+design、分離 pass → validate → judge） | 仕様・設計レビュー | verdict: **ACCEPT WITH NON-BLOCKING NOTES**。CONFIRMED 3 件修正（非存在 op 名・壊れた節参照・relation 種別）。P0/P1 blocker なし。最終レビューは全成果物完成後に実施 |

## 2. Baseline（既存・本タスク範囲外）の finding

| 項目 | 内容 | 扱い |
| --- | --- | --- |
| `doc_status lint` repo 全体 | 100+ blocking findings（無関係文書の invalid relation 等） | 既存 baseline。本タスクで修正しない。新規文書起因は V8/V9 で解消確認 |
| `doc-driven-dev-graph` route | `status: blocked`、pre-existing broken-relation 多数 | repo baseline。範囲外のため記録のみ |
| `audit_experiment_log.js` | 既存 jsonl の invalid-event-shape（baseline） | 本タスクの exp は repo 慣行の `.md` 記録のため対象外 |

## 3. NotRun / Deferred レーン（POLICIES.md §2/§3 語彙）

| レーン | 状態 | 理由・後続 Owner |
| --- | --- | --- |
| headless（`synvelis-workspace`/`synvelis-app` default feature） | **Pass** | WSM-T-01〜15 の oracle を既存 + 新規 test で実測（V4/V7 exit 0） |
| design-review（文書チェーン静的レビュー） | **Pass** | V8/V9/V12 + review-protocol ACCEPT |
| `native`/`gpui-test` feature lane（`check-app-native`/`lint-app-native`/`check-app-gpui`/`lint-app-gpui`/`test-app-gpui`） | **NotRun（対象外）** | タスク指定の正規検証は Headless のみ。GPUI-simulated/実 driver lane は #112/#114 系 gate。本 slice は contract 層のみ変更（test 追加）で feature lane への影響なし |
| real-os（実 OS Window/Dock/IME/Keyboard/pixel/2 Window/実 plugin） | **NotRun** | synvelis#114（GUI-L2）・#176（EXT-L3）・ruviform#313/#314/#388 consumer が Owner。headless Pass を実機 Pass に換算しない |
| 実 RT 計測（dock/window op 中の glitch/alloc） | **NotRun** | synvelis#294・ruviform consumer gate。stub oracle のみ |
| window-owned job cancel 実挙動 | **NotRun** | `CallOutcome::Job` を返す op 未実装。`UiRuntime` 契約を設計記録 |
| macOS/Linux GUI/EXT | **Deferred** | POLICIES.md — 使用要件未発生 |
| CI（merge 後対象 SHA） | **NotRun pending** | ローカル検証は変更範囲の検証。統合検証は merge 後 main SHA の必須 CI 証拠（Coordinator 確認） |

## 4. 判定まとめ

- 文書チェーン 7 文書 + `evidence/sv2/` 2 件 + `workspace_model.rs` 1 test
  が存在し、WSM-AC-01〜09・WSM-T-01〜20 の写像に漏れがない
  （case-ledger 参照）。
- `mise run fmt-check`/`check`/`lint`/`test`（両 crate）は全て exit 0。
- review-protocol: spec+design で 1 サイクル + 最終レビュー、
  CONFIRMED P0/P1 blocker なし。
- 実装・実 OS 検証の完了は一切主張しない。後続 lane は
  #113/#114/#175/#176/#294 と ruviform#313/#314/#388 に委譲。
