# Asset Library — Verification Record (synvelis#9)

本 slice（docs/ 設計契約 + evidence）で実際に実行した検証の
記録。**実行していない検証は Pass としない。** 実装 crate の
check/test/build・実 Provider・実 GUI・実 OS/device の受入は
本 slice の対象外であり、#85〜#91 の別 gate で実施する。

## 実行環境

| 項目 | 値 |
|---|---|
| OS | Windows（worktree `sv9-asset-library`） |
| repo/commit SHA | branch `docs/9-asset-library`（base `origin/main` = `86715f9ac2761f9f275e885e0bbd7ee775d9633e`） |
| Ruviform resolved SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock` git dep） |
| toolchain | mise 管理（`mise.toml` 正本）。docs 検証は node script + `mise run fmt-check` |
| 変更範囲 | docs + evidence のみ（crate/CI/Cargo/Ruviform 共通契約は無変更） |

## 実行した検証

| # | command | lane | expected | observed | exit | status |
|---|---------|------|----------|----------|------|--------|
| V-1 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply --json` | docs/L1 | 触れた dir の index が再生成・blocked なし | `written`: designs/impl-ir/plans/specs/tasks/test-specs の 6 README、`blocked`: [] | 0 | **Pass** |
| V-2 | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` | docs/L1 | impl IR index が in-sync | `IN-SYNC docs/impl/ir`（pins 183 sources） | 0 | **Pass** |
| V-3 | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind exp --apply` | docs/L1 | impl exp index が in-sync | `IN-SYNC docs/impl/exp`（files=1: `.jsonl` のみ管理。`.md` experiment doc は doc_status 側で走査） | 0 | **Pass** |
| V-4 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type spec --json`（asset-library 関連 findings を抽出） | docs/L1 | 新規 doc に error なし | 当初 `defers` prose target・`derived-by` 非相互・`references` 未作成 file の指摘 → 修正後 clean | 0 | **Pass**（修正後） |
| V-5 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type design --json`（同上） | docs/L1 | 同上 | 同上 → clean | 0 | **Pass**（修正後） |
| V-6 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type plan --json`（同上） | docs/L1 | 同上 | 同上 → clean | 0 | **Pass**（修正後） |
| V-7 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type task --json`（同上） | docs/L1 | 同上 | 同上 → clean | 0 | **Pass**（修正後） |
| V-8 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type test-spec --json`（同上） | docs/L1 | 同上 | 同上 → clean | 0 | **Pass**（修正後） |
| V-9 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`（asset-library findings 抽出） | docs/L1 | 新規 IR に error なし | `broken-relation-link` warning 1 件（verification.md 未作成時）→ 本 file 作成で解消。error なし | 0 | **Pass** |
| V-10 | `node .agents/skills/impl-doc/scripts/audit_experiment_log.js --json`（asset-library findings 抽出） | docs/L1 | 新規 exp に finding なし | `none` | 0 | **Pass** |
| V-11 | `node .agents/skills/doc-status/scripts/audit_docs.js --type all --json`（asset-library findings 抽出） | docs/L1 | 新規 doc に finding なし | `none` | 0 | **Pass** |
| V-12 | `mise run fmt-check` | docs/L1 | exit 0 | `cargo fmt --all -- --check` exit 0 | 0 | **Pass** |

注: V-4〜V-8 の lint は repo 全体では既存 doc（alib-* 等）に
多数の error/warning があるが、いずれも本 slice 以前からの
既存 findings であり対象外。本 slice の新規 file
（asset-library*）に限定した findings は修正後 clean。

## 未実行・対象外（NotRun/Deferred/Unsupported）

| 項目 | 状態 | 理由・後続 |
|---|---|---|
| 実装 crate の `check`/`test`/`build`/`test-rt` | **NotRun** | docs-only slice。crate 変更なし。実装検証は #85〜#91 Owner |
| LIB-ID/BROWSE/PIN/IMPORT/MIG/CACHE/SEC/QOS 全 case | **NotRun** | 実装未着手（#85〜#91）。fixture/oracle/Owner は `asset-library-case-ledger.md` に台帳化 |
| 実 Provider（実 Git remote・実 WebDAV・実 credential store）E2E | **Deferred** | #88/#90/#331 の provider gate |
| 実 GPUI Browser window 操作受入 | **Deferred** | #114 |
| OS 別実機・実 device・実 network の L2/L3 | **Deferred** | POLICIES.md 検証レベル。利用される機能のみ Required |
| SVN/P4/SMB/S3 等の追加 Provider | **NotImplemented** | typed `Unsupported`/`NotImplemented` のまま段階化 |
| Ruviform Rooted File/IO/Security 共通契約変更 | **NotApplicable** | 本 slice は参照のみ。変更は Ruviform 側 Issue Owner |
| `doc-driven-dev-graph` route | **blocked（既存）** | repo 全体に既存の broken-relation findings があり route が blocked を返す。本 slice の docs 追加が原因ではない。新規 file 固有の findings は lint/audit で clean を確認 |

## 診断・復旧

- lint で指摘された `defers` prose target・`derived-by`
  非相互・`references` 未作成 file は、relations を canonical
  link のみに修正（`defers: []`、Deferred 項目は本文
  「Deferred Design Concerns」に記述）し、verification.md
  作成で解消した。
- `docs/impl/exp/asset-library-synvelis-9.jsonl`（scaffold が
  生成した空 JSONL）は canonical 名 `asset-library.md`（type
  `experiment` doc）へ置き換えのため削除した。`docs/impl/
  exp/README.md` は `.jsonl` のみを管理する generated-index
  であり、`.md` experiment doc は doc_status 側で走査される
  （command-gateway 先例と同じ）。
