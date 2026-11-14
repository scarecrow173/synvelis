# SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract — Verification Record

実行した検証の記録。docs-only スライスのため、コード系
check/lint/test/build・`*-full`・`*-msrv`・実 GUI/実 OS
レーンは対象外(タスク指定)。`mise run fmt-check` のみ
mise 経由で実行し、それ以外は doc-* Skill の node script
を直接実行した(canonical = mise タスクは `fmt-check` のみ
該当。docs 検証は repo の doc-* Skill が正本手順)。

## 実行環境

- OS: Windows(worktree `C:\Users\akiyoshi\orca\workspaces\
  synvelis\sv112-keyboard-gesture`)
- toolchain: mise 管理(node + rust)。`mise run fmt-check`
  は cargo fmt --all -- --check を実行
- 実行日: 2026-11-14

## 実行した検証

| # | コマンド | 目的 | exit | 結果 |
| --- | --- | --- | --- | --- |
| V-1 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/{specs,designs,plans,tasks,test-specs,impl,impl/ir,impl/exp}` | 生成 index の差分 preview | 0 | specs/designs/plans/tasks/test-specs/impl-ir に changes=1、impl/exp は in-sync |
| V-2 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply --dir docs/{specs,designs,plans,tasks,test-specs,impl/ir}` | index 再生成(手編集なし) | 0 | 各 dir 1 written・0 blocked |
| V-3 | `node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/{specs,designs,plans,tasks,test-specs,impl}` | 新規文書の deterministic finding 確認 | 0 | sv-gui-03-06 由来の finding 0(既存の無関係文書の finding は多数・範囲外) |
| V-4 | `node .agents/skills/doc-status/scripts/doc_status.js audit` | repo 全体 audit | 0 | 1061 docs・885 findings・664 blocking — 全て既存文書由来。sv-gui-03-06/evidence/sv112 の finding 0 |
| V-5 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | IR/EXP 監査 | 0 | 初回: IR の required section 不足 + verification.md 未作成を検出 → IR を規定 section 構成へ修正・verification.md 作成後に再監査(下記 V-8) |
| V-6 | `mise run fmt-check` | cargo fmt --check(no-op 確認) | 0 | 差分なし(コード変更なし) |
| V-7 | `git diff --check` / `git status --short` | 空白エラー・変更範囲確認 | 0 | 許可範囲 + 生成 index + evidence/sv112/ のみ |
| V-8 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`(修正後再実行) | IR section 修正の確認 | 0 | sv-gui-03-06 由来の error 0 を確認 |

## 未検証の項目(NotRun / Deferred / Unsupported)

| 項目 | 状態 | 理由・委譲先 |
| --- | --- | --- |
| 実 GUI key dispatch(実 Window/実キー入力) | **NotRun** | docs-only スライス。#114 GUI-L2 lane |
| 実 OS 既定バインド確認(Win Ctrl/macOS Cmd) | **NotRun** | 実機 lane。#114 |
| IME/Focus/DPI 実機挙動 | **NotRun** | #114。`Ime` Undo 配送の文書照合もここで判断 |
| 実 Plugin GUI キー所有 | **NotRun** | #176 EXT-L3 lane |
| headless 契約試験(TS-EX-01〜11)の実走 | **Deferred** | keymap dispatch 実装 slice で実施。契約試験の仕様のみ test-spec で定義 |
| keymap dispatch 実装・`spec_for` 登録・リマップ UI | **Deferred** | #112 実装 slice。コード変更禁止の docs-only |
| `check`/`lint`/`test`/`build`・`*-full`・`*-msrv` | **NotRun** | docs-only 変更のため対象外(タスク指定)。Workspace 全体検証は merge 後 CI が担う |
| macOS/Linux GUI-L2/EXT-L3 | **Unsupported→Deferred** | POLICIES.md: macOS/Linux は現状 OS-L1 要件、GUI-L2/EXT-L3 は使用要件発生まで Deferred |

## headless 検証と実機 Pass の区別

本スライスで Pass としたのは **文書契約の整合性**(割当表の
§7.2 網羅・`captures_input`/`dispatch`/`dispatch_undo` との
整合・index/lint/audit/fmt)のみ。実 GUI key dispatch・実 OS
既定バインド・実機 IME/Plugin GUI の挙動は一切検証しておらず、
headless/文書の成功を実機 Pass に昇格しない(POLICIES.md §2/§3、
KG-AC-07)。

## 既知の範囲外 blocker

- **doc-driven-dev-graph router**: 既存の無関係な broken
  relation で `probe` node が blocked を返す(repo 全体の
  既存 graph 債務)。本スライスは宣言済み文書順で手動継続し、
  router 結果を偽装しなかった。修復は範囲外。
- **doc_status audit の既存 blocking findings**: 664 件全て
  既存文書由来(vcs-*/sv-gui-02/test-specs の ID 形式・
  relation 不整合等)。本スライスの文書は finding 0。
