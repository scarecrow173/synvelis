# Project Model — 検証記録 (Issue #10)

作成: 2026-11-13 / branch: `scarecrow173/sv10-project-model`
workspace: `C:\Users\akiyoshi\orca\workspaces\synvelis\sv10-project-model`
OS: Windows（MINGW64）/ toolchain: rustc（mise 管理、`mise run test` が
`cargo test --locked -p synvelis-project` を実行）

## 1. 実行した検証

| # | コマンド | 目的 | 結果 |
| --- | --- | --- | --- |
| V1 | `mise run test synvelis-project` | 既存 fixture の oracle 実測（PM-T 写像の根拠） | **exit 0 / Pass**。41 test binary・460 test 全件 pass（lib unittests・39 integration・2 doc-test）。`cargo test --locked -p synvelis-project` |
| V2 | `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus project-model --json` | 文書 graph router | **status: blocked**（`current: probe`、`edgeId: null`、pre-existing `artifact-graph`/`broken-relation` — repo baseline、範囲外。case-ledger §4 に記録） |
| V3 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply --dir docs/{specs,designs,plans,tasks,test-specs,impl/ir}` | 触れた dir の index 再生成 | **6 dir written・0 blocked**。`docs/impl/exp` は `.jsonl` のみ index 対象の既存規約のため `.md` experiment record は index されず差分なし（`0 written`） |
| V4 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type {spec,design,plan,task,test-spec} --json` | 新規文書の front matter/relation 検査 | project-model 起因 finding は **warning 2 件のみ（non-blocking）**: `inconsistent-reciprocal-relation` — spec/design の `depends-on` が `vcs-01-immutable-snapshot-resolver(.md/-design.md)` を指し、target が `blocks` を宣言しつつ逆リンクを持たない（target 側の既存 relation 不整合であり、本 slice の編集範囲外。修復しない） |
| V5 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | IR 監査 | 初回 project-model finding 5 件（`defers` に docs パスでない文字列・`adopted` に EXP ID）→ front matter を修正（`defers: []`・`adopted: [docs/impl/exp/project-model.md]`・`references: [test-spec]`）後 **0 件** |
| V6 | `mise run fmt-check` | canonical format 検査（docs slice） | **exit 0 / Pass**（`cargo fmt --all -- --check`） |
| V7 | review-protocol（spec+design+chain 全体、分離 pass → validate → judge） | 文書チェーンレビュー | CONFIRMED 3 件を修正: (a) `ClipPlayback`/`PlaybackMapping`/`Clip` の実 field 名を実装に合わせて訂正（gain/fade は clip field に存在しないことを明記）、(b) `Command` が `ProjectId` を持たない実装事実に合わせ spec/design を訂正、(c) `SplitClip` に明示 `DerivedFrom` field がない gap を design/ledger/test-spec に記録。P0/P1 blocker なし（verdict: ACCEPT WITH NON-BLOCKING NOTES 相当） |

## 2. Baseline（既存・本タスク範囲外）の finding

| 項目 | 内容 | 扱い |
| --- | --- | --- |
| `doc_status lint` repo 全体 | spec 171/design 245/plan 124/task 187/test-spec 131 の finding（大半は既存文書の warning/error） | 既存 baseline。本タスクで修正しない。project-model 起因は V4 の warning 2 件のみ |
| `doc-driven-dev-graph` route | `status: blocked`、pre-existing broken-relation 多数 | repo baseline。範囲外のため記録のみ（case-ledger §4） |
| `inconsistent-reciprocal-relation`（vcs-01） | vcs-01 spec/design が `blocks` を宣言しつつ逆リンクなし | vcs-01 文書側の既存 relation 不整合。編集範囲外のため記録のみ |

## 3. NotRun / Deferred レーン（POLICIES.md §2/§3 語彙）

| レーン | 状態 | 理由・後続 Owner |
| --- | --- | --- |
| headless（`synvelis-project` default test） | **Pass** | V1 exit 0・460 test 全件 pass。PM-T-01〜22 の oracle を実測 |
| design-review（文書チェーン静的レビュー） | **Pass** | V4/V5/V7 + review-protocol。PM-T-21/22・gap 項目を設計レビューで確認 |
| 実 GUI Clip/Track 編集・Timeline tools | **NotRun** | synvelis#112/#114（GUI-L2）・#11 |
| 実 plugin state/instance・実 plugin GUI | **NotRun** | synvelis#176・tim-04・ruviform plugin host（EXT-L3） |
| 実 RT no-alloc/no-block・execution graph publish・sample 一致 | **NotRun** | synvelis#294・ruviform#219/#294・#18 |
| 実 storage durability・電源断・stale lock 実測・real FS fault | **NotRun** | synvelis#84/#330・ruviform-io-file real FS・rcd-02 |
| remote provider fetch/cancel 実装・offline 実 fetch | **NotRun** | ALIB 系別 slice（alib-04/06/07）・synvelis#9 |
| Creative Branch 実装（VariantScope/BranchContentRef/Adopt） | **NotRun** | synvelis#277/#278 別 slice（本書は契約のみ） |
| cross-consumer/RT gate | **NotRun** | ruviform#219/#294・synvelis consumer gate |
| macOS/Linux lane | **Deferred** | POLICIES.md — 使用要件未発生 |
| CI（merge 後対象 SHA） | **NotRun pending** | ローカル検証は変更範囲の検証。統合検証は merge 後 main SHA の必須 CI 証拠（Coordinator 確認） |

## 4. 判定まとめ

- 文書チェーン 7 文書 + `evidence/sv10/` 2 件が存在し、PM-AC-01〜13 の
  全受入条件が PM-T-01〜22・既存 test・設計レビュー・NotRun/Deferred
  （PM-T-23〜30）に漏れなく写像される（case-ledger 参照）。
- `mise run fmt-check`/`test synvelis-project` は exit 0。
- review-protocol: spec+design+chain 全体で 1 サイクル、CONFIRMED P0/P1
  blocker なし。確認された実装事実との差異（clip field・Command 構造・
  DerivedFrom gap）は修正・記録済み。
- 実装・実 OS 検証の完了は一切主張しない。後続 lane は #11/#12/#17/#18/
  #84/#112/#114/#176/#277/#278/#294/#330 と ruviform#189/#219/#294、
  ALIB 系に委譲。
