# Capability / Command Gateway G0-G6 受入照合 — 検証記録 (synvelis#46)

`evidence/sv46/command-gateway-gates-ledger.md` の照合に
用いた実行検証の記録。全コマンドは `mise.toml` が正本の
mise task 経由で実行した（cargo 等の直接実行は補助証拠）。

## 環境

| 項目 | 値 |
|---|---|
| 対象 Revision | `86715f9ac2761f9f275e885e0bbd7ee775d9633e`（`origin/main` HEAD・PR #408 merge commit） |
| Ruviform resolved | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock`） |
| OS / Arch | Windows / x86_64（Git Bash） |
| Rust toolchain | cargo/rustc `1.99.1` |
| Node | `26.12.1` |
| 実行日 | 2026-11-14 |
| worktree | `C:\Users\akiyoshi\orca\workspaces\synvelis\sv46-gateway-gates` |
| ブランチ | `docs/46-gateway-gates-audit` |

## 実行した検証（実測）

### `mise run fmt-check`

- 内容: `cargo fmt --all -- --check`
- exit code: **0**
- 結果: 成功（差分なし）

### `mise run gw10`

- 内容: GW-10 harness suite（`cargo test -p synvelis-gateway
  --test gw10_*` 系 + `cargo test -p mcp-poc --test
  gw10_transport` + `cargo run -p synvelis-gateway --bin
  gw10_report -- --strict`）
- exit code: **0**
- 結果:
  - `gw10_report -- --strict`: **G0 PASS・G1 PASS・G2 PASS・
    G3 PASS・G4 PASS・G5 PASS・G6 PASS・GTV PASS**・
    required cases passed。
  - optional/missing evidence（required=false・gate を保持
    しない）:
    - `SYS-02-ux`: NOT-RUN（実 GPUI UX・#114）
    - `SYS-03-ux`: NOT-RUN（実 GPUI UX・#114）
    - `SYS-05-powerloss`: NOT-RUN（電源断・harness 非対応・
      実機 lane）
    - `SYS-07-rt`: UNSUPPORTED（RT budget 未実測・実機 lane）
    - `CONSUMER-RUVIFORM`: NOT-RUN（条件付き・rf#294）
    - `CONSUMER-AUDIOLENS`: NOT-RUN（条件付き・rf#294）
  - `mcp-poc/tests/gw10_transport.rs`（`mise run gw10` 内で
    実行）: 4 tests pass —
    `gw10f_three_modes_over_real_transports`・
    `gw10f_delegated_apply_commits_and_survives_restart_real`・
    `gw10f_boundary_attacks_fail_closed_real`・
    `gw10f_permit_cli_lifecycle_real`。
    real stdio + 認証 loopback transport・real
    `DirectoryBundleStore`+`commit.journal`+`FilePermitStore`
    を使用。**PoC real transport であり第三者 MCP Host では
    ない**。

### `mise run gw10-report`

- 内容: `cargo run -p synvelis-gateway --bin gw10_report`
  （`target/gw10-evidence/*.jsonl` 集約・strict なし表示）
- exit code: **0**
- 結果: gate verdict G0〜G6・GTV 全 PASS。missing evidence:
  `SYS-02-ux`・`SYS-03-ux`・`SYS-05-powerloss`・
  `CONSUMER-RUVIFORM`・`CONSUMER-AUDIOLENS`（NotRun/
  Unsupported）。`SYS-07-rt` は Unsupported として記録。

### `mise run test synvelis-gateway`

- 内容: `cargo test --locked -p synvelis-gateway`
- exit code: **0**
- 結果: 25 test binary 全成功・**406 passed / 0 failed**
  （lib 7・bin 0・integration 22 file・doc-tests 0）。
  sv45 verification 記録の 487 passed とは別観測（対象
  package・実行構成の差）であり混同しない。

## 文書検証（docs/impl・docs/test-specs 追加に伴う）

| コマンド | exit | 結果 |
|---|---|---|
| `node .agents/skills/doc-status/scripts/doc_status.js lint --type {spec,design,plan,task,test-spec} --json` | 0 | lint エラーなし（対象 type 全て） |
| `node .agents/skills/doc-status/scripts/audit_docs.js --type all --json` | 0 | audit エラーなし |
| `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | 0 | IR audit エラーなし |
| `node .agents/skills/impl-doc/scripts/audit_experiment_log.js --json` | 0 | exp audit エラーなし |
| `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` / `--kind exp --apply` | 0 | impl index 再生成（両 kind とも IN-SYNC） |
| `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply` | 0 | docs index 再生成 |
| `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus command-gateway --json` | 0 | exit 0・status=`blocked`。blocker は repo 既存の broken-relation 146 件（automation-modulation/rcd-*/vcs-* 等・他 doc）+ `focus-invalid` であり、本 slice の新規 file 起因の blocker は 0 件 |
| `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus docs/test-specs/command-gateway-gates.md --json` | 0 | exit 0・status=`blocked`（同上・repo 既存 blocker のみ。本 file 起因 0 件） |

（上記は文書追加・index 再生成後に実行した最終値。詳細は
`docs/impl/ir/command-gateway-gates.md` Validation 節と
PR 本文を参照。）

## 補助証拠（canonical = mise ではない実行）

- `mise run gw10` 内で `cargo test -p mcp-poc --test
  gw10_transport` が実行され 4 tests pass（上記）。
- `mcp-poc/tests/stdio_e2e.rs` は本 slice の正規コマンド
  （`test synvelis-gateway`）の対象外 package のため今回
  未再実行。同一 wire/transport を駆動する既存 fixture と
  して `gw10_transport.rs` と併記するが、未実行である
  ことを明記する（推測で Pass にしない）。
- `mise run check synvelis-gateway` は本 slice では未実行
  （docs/evidence のみの変更・コード変更なし。`test` が
  compile を含む）。

## 未検証（NotRun/Unsupported — 台帳に詳細）

- 実 GUI/GPUI 承認 UX（SYS-02-ux・SYS-03-ux・UX-01〜06
  実画面）→ #114
- 第三者 MCP Host での G4 受入・実 Host 互換・Transport
  最終決定 → #46 G4 拡張・別設計
- 実 Provider・実生成 Media・Provider/Model/Rights 実契約
  → 別設計（#46 G5 注記）
- 電源断 durability（SYS-05-powerloss）・OS 別 RT budget
  実測（SYS-07-rt）・大規模 Scope 負荷試験 → 実機 lane・
  #83・#46 G6 残件
- Grant の product 配線での永続化（FilePolicyStore→serve
  統合）→ #46 G6 残件
- Ruviform/AudioLens Consumer 回帰（CONSUMER-RUVIFORM/
  AUDIOLENS）→ Ruviform 変更時のみ #294・#553（条件付き・
  本 slice で Ruviform 変更なし）
- AUTX #259〜267 の GW 接合 → AUTX Issue scope（本台帳の
  Gate 判定対象外）
