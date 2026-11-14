# SV20 Transport Integration — Verification Record

Issue #20 docs slice（canonical 7 文書鎖 + evidence/sv20 台帳）の
検証実行記録。実装 crate の検証は対象外（docs/設計スライス）。

- 実行環境: Windows / MINGW64（worktree
  `C:\Users\akiyoshi\orca\workspaces\synvelis\sv20-transport-
  integration`）
- toolchain: mise 管理（`mise.toml` 正本）。node スクリプトは
  `.agents/skills/*/scripts/*.js` を直接 `node` で実行。
- Synvelis revision: `58538fb`（docs slice 前の worktree head）
- Ruviform pinned: `fd1d4d5`

| # | コマンド | 目的 | 結果 |
| --- | --- | --- | --- |
| V1 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply` | 触れた dir の index 再生成 | **exit 0 / 6 written・0 blocked**。`docs/{designs,impl/ir,plans,specs,tasks,test-specs}/README.md` を再生成。`docs/impl/exp` は `.jsonl` のみ index 対象の既存規約のため `.md` experiment record は index されず差分なし（sv18 と同一規約） |
| V2 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type {spec,design,plan,task,test-spec} --json` | 新規文書の front matter/relation 検査 | 初回 transport-integration 起因 finding を修正後 **0 件**。修正内容: (a) design `self-relation` → `defers` の自己参照を除去、(b) design `inconsistent-reciprocal-relation` → `derived-by` に test-spec を追加、(c) plan `inconsistent-reciprocal-relation` → `derives-from` を design のみに修正、(d) plan `plan-missing-test-spec-evidence`(blocking) → `verified-by` に test-spec を追加、(e) test-spec `inconsistent-reciprocal-relation`×2 → `derives-from` を design のみに修正、(f) task/test-spec `broken-relation-link`(blocking)×2 → 本 verification file を作成して解消 |
| V3 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | IR 監査 | **transport-integration 起因 0 件**（repo 全体 107 warning は既存 baseline） |
| V4 | `node .agents/skills/doc-status/scripts/audit_docs.js --type all --json` | 文書 audit | **transport-integration 起因 0 件**（本 file 作成後。作成前は missing target×2 のみで他になし） |
| V5 | `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus transport-integration --json` | 文書 graph router | **status: blocked**（`current: probe`、`edgeId: null`、pre-existing `artifact-graph`/`broken-relation` — repo baseline、範囲外。sv18 と同一 baseline。case-ledger §未実施範囲に記録） |
| V6 | `mise run fmt-check` | canonical format 検査 | **exit 0 / Pass**（`cargo fmt --all -- --check`） |
| V7 | `review-protocol` | spec/design/文書鎖レビュー | **blocker なし**（下記 §Review） |

## Baseline blockers（範囲外・修復しない）

- `route_graph.js` の `status: blocked` は `artifact-graph` と多数の
  `broken-relation`（他文書の `relations.defers` の prose 文字列等）
  による repo baseline。sv18 でも同一 baseline が記録されており、
  本 slice の対象ファイル外のため修復しない。
- `doc_status lint`/`audit_impl_record`/`audit_docs` の repo 全体
  warning/error は既存 baseline。transport-integration 起因は
  V2/V3/V4 で 0 件に修正済み。

## 未検証の項目（NotRun / Unsupported / Deferred）

- 実 VST3/CLAP plugin transport/state/offline: **NotRun**
  （EXT-L3・ruviform#253/#196・synvelis#114）。
- 実 OS MIDI backend・実 MIDI device: **NotRun**（ruviform#253・
  synvelis#12 recording Owner）。
- 実 GUI automation record・Inspector gesture: **Deferred**(#114)。
- SMF SMPTE Division/Format2: **Unsupported**(#129)。
- MIDI2/MPE/Per-note: **Deferred**(#130)。
- 実 plugin latency/state/offline・全 plugin bit-perfect offline
  再現性: **NotRun**（契約上の Non-goal）。
- post-merge CI: merge 後の main SHA に対する必須 CI 結果が
  統合検証の証拠（本 slice では未実施）。
- cargo test/build/lint: docs slice のため対象外（`mise run
  fmt-check` のみ実施）。

## Review

`review-protocol` に従い、objective checks（V1〜V6）を先に実行し、
spec→design→plan→task→test-spec→ir/exp→evidence の文書鎖を
cooperative/adversarial の 2 pass でレビューした。確認した観点:

- TT-001〜030 の一意性・Owner 分離・fixture/oracle/状態の写像が
  spec/design/test-spec/evidence で一致。
- headless Pass を実機受入に繰り上げていない（capability gate で
  NotRun/Deferred/Unsupported を明示）。
- `Exact/Degraded/Unsupported/Unknown`（能力 Class）と
  `Pass/Fail/NotRun`（結果状態）を混同していない。
- Plugin Event offset=process-start 相対・half-open span・Offline
  Exact Fail-closed・Realtime Degraded consent が契約として
  一貫。
- Owner 分離（Synvelis DAW 意味論 vs Ruviform generic）が
  components 表で固定され、generic 不足は upstream Issue へ。

**結果**: confirmed P0/P1 blocker なし。P2 指摘（fixture 名の
代表例表記・数値未確定の継承）は evidence/Deferred Design
Concerns に記録済み。
