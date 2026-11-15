# RCD-F02 検証記録 — docs-only 設計 slice（synvelis#120）

Slice: docs-only（`docs/` 文書チェーン + `evidence/sv120/` + 生成 index のみ）
Date: 2026-11-15
OS: Windows（worktree `G:/orca/workspaces/synvelis/sv120-rcd-f02`）
Toolchain: mise 管理（`mise.toml` が正本）

## 実行した検証

| # | コマンド | exit | 結果 |
| --- | --- | --- | --- |
| 1 | `mise run fmt-check` | 0 | Pass（`cargo fmt --all -- --check`） |
| 2 | `mise exec -- prek run --all-files` | 0 | Pass（trim/eof/yaml/toml/mixed-line-ending/utf-8-bom/private-key/illegal-windows-names/actionlint 全て Passed or Skipped） |
| 3 | `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --cwd . --json` | 0 | `status: blocked`（pre-existing repository-wide broken relation。slice 起因でない。`docs/impl/exp/rcd-f02-monitoring-overdub.md` E1 に記録） |
| 4 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan` | 0 | 10 target(s)、触れた全 dir が `sync`（in-sync 化が必要なのは新規追加分のみ） |
| 5 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply --dir <dir>` × 6（specs/designs/plans/tasks/test-specs/adr） | 0 | 各 dir `1 written, 0 blocked`（`docs/*/README.md` 再生成） |
| 6 | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` | 0 | `docs/impl/ir/README.md` に `rcd-f02-monitoring-overdub.md` を add |
| 7 | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind exp --apply` | 0 | `docs/impl/exp/README.md`（exp index は `.jsonl` のみ管理。`.md` exp memo は index 対象外 — `recording-take-comping` 先例と同型） |
| 8 | `node .agents/skills/doc-status/scripts/doc_status.js audit --type all --profile artifact` | 0 | 1184 document(s), 2011 finding(s), 711 blocking — **大半は pre-existing**（legacy `invalid-id-prefix`/`invalid-id-format`/`orphan-navigation`/`test-spec-missing-verifies` 等）。slice 起因 finding は下記「slice 起因 finding の対応」で全て解消 |
| 9 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type <spec\|design\|plan\|task\|test-spec\|adr>` | 0 | 各 type で pre-existing finding は多数。rcd-f02 新規分は evidence target 未作成・task status 誤記・spec upstream 欠落・ADR reciprocal のみで、全て修正済み（下記） |
| 10 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js` | 0 | rcd-f02 の finding は evidence target 未作成のみ（本ファイル作成で解消）。他は pre-existing（vcs-* defers missing target 等） |
| 11 | `node .agents/skills/impl-doc/scripts/audit_experiment_log.js` | 0 | `docs/impl/exp` Files: 2（`.jsonl` のみ監査対象）。`issue-147-sccache.jsonl` の `invalid-event-shape` error は **pre-existing**（本 slice 非起因）。新規 `.md` exp memo は監査対象外 |
| 12 | `node .agents/skills/adr-doc/scripts/audit_adr.js --dir docs/adr` | 0 | rcd-f02 に error なし（他 ADR の `Missing section` error は pre-existing） |
| 13 | `node .agents/skills/adr-doc/scripts/check_code_links.js --dir docs/adr` | 0 | rcd-f02 の `missing-implementation-path` warning を `../../crates/...` 相対 path に修正して解消 |
| 14 | `node .agents/skills/adr-doc/scripts/review_adr.js --dir docs/adr` | 0 | rcd-f02 に blocker なし |

## slice 起因 finding の対応

| finding | 対応 |
| --- | --- |
| `broken-relation-link`: plans/tasks/test-specs/ir の `related` → `evidence/sv120/rcd-f02-monitoring-overdub-verification.md` 未作成 | 本ファイルを作成して解消 |
| `invalid-status`: task `status: completed` | `done` に修正（task-doc 有効値: todo/in-progress/blocked/done/wont-do） |
| `traceability-missing-upstream`: spec に upstream relation なし | `derives-from: issue#120` を追加（`ai-native-baseline` 先例と同型） |
| `inconsistent-reciprocal-relation`: ADR `derives-from` → spec に対し spec `derived-by` が design のみ | spec `derived-by` に ADR を追加 |
| `index-metadata-mismatch`/`index-tier-mismatch`: task status 変更後の index 不整合 | `index apply --dir docs/tasks` で再生成 |
| `missing-implementation-path`: ADR Implementation Plan の path が `docs/adr/` 相対で解決不能 | `../../crates/...` 相対 path に修正 |

## 未実行の検証（正直な記録）

- **cargo test/build/lint・`mise run test/check/build` 系**: `NotRun`/`NotApplicable`。docs-only slice であり `crates/**` を変更していない（POLICIES §5 比例原則 — 文書のみの変更は軽量チェックが原則）。
- **実 OS/実 GUI/実 Device/実 Plugin/EXT-L3 検証**: `NotRun`/`Deferred`。#114 系別受入 Gate の責務。
- **RT noalloc・実 OS E2E（Loop/Take/Comp/Device 切断/Undo/Recovery）**: `Deferred`。Issue #120 実装 slice の検証。
- **`*-full`/`*-msrv`/Feature Matrix/3OS 横断**: `NotRun`。タスク指定になし・日常必須検証に非該当。

## 差分要約

- 新規: `docs/{specs,designs,plans,tasks,test-specs,adr,impl/ir,impl/exp}/rcd-f02-monitoring-overdub*.md` 8 文書 + `evidence/sv120/rcd-f02-monitoring-overdub-{adr-decisions,verification}.md`。
- 変更: 触れた dir の管理 index（`docs/{specs,designs,plans,tasks,test-specs,adr,impl/ir}/README.md`）の再生成のみ。
- `crates/**`・`Cargo.toml`/`Cargo.lock`・`mise.toml`・CI・`POLICIES.md`・`AGENTS.md`・他 slug docs: 変更なし。
