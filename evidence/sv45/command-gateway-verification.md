# Capability / Command Gateway — Verification Record (synvelis#45)

Issue #45 の docs/ 設計契約 slice の検証記録。**本 slice は
docs/ 設計契約 + evidence + headless 回帰実測のみ**であり、
実装 crate・GUI・実 device/plugin/OS/Provider 受入は変更・
実行していない。実行した検証とその結果のみを記録し、推測と
実測を区別する。

## 環境

- OS: Windows（Git Bash 経由）
- 作業 worktree:
  `C:\Users\akiyoshi\orca\workspaces\synvelis\sv45-command-gateway`
- ブランチ: `docs/45-command-gateway`
- Rust toolchain: mise 管理（`mise.ci.toml` pin `1.99.0`・観測
  Cargo `1.99.1`）
- Node: `26.12.1`（doc-maintenance/doc-status scripts 用）
- canonical 検証入口: `mise.toml` の `mise run` タスク
- 補助証拠: docs 検証 script（`.agents/skills/*/scripts/`）

## 実行した検証

### V-1 `mise run fmt-check`（正規検証コマンド）

- コマンド: `mise run fmt-check`
- OS: Windows（Git Bash）
- toolchain: mise 管理 Rust
- exit code: 0
- 結果: 成功（fmt-check 通過）
- 備考: docs/markdown のみの変更のため軽量。Markdown lint は
  doc-status lint で補完。

### V-2 `mise run gw10`（headless 回帰・E2E/証拠生成）

- コマンド: `mise run gw10`
- OS: Windows（Git Bash）
- toolchain: mise 管理 Rust
- exit code: 0
- 結果: 成功。G0〜G6・GTV の全 gate が PASS。
  `target/gw10-evidence/*.jsonl` に verdict record が生成。
  strict report が 5 件の NotRun/Unsupported（SYS-02-ux・
  SYS-03-ux・SYS-05-powerloss・SYS-07-rt・CONSUMER-*）を
  required=false として正直に報告。
- 備考: headless/mock/PoC real transport の検証。実 GUI・
  実 Provider・第三者 MCP Host・実機 lane は対象外。

### V-3 `mise run gw10-report`（集約レポート）

- コマンド: `mise run gw10-report`（`cargo run -p synvelis-
  gateway --bin gw10_report -- --strict`）
- OS: Windows（Git Bash）
- toolchain: mise 管理 Rust
- exit code: 0
- 結果: G0〜G6・GTV 全 PASS・5 件 NotRun/Unsupported（
  required=false）。case ledger の verdict 正本。

### V-4 `mise run check synvelis-gateway`

- コマンド: `mise run check synvelis-gateway`
- OS: Windows（Git Bash）
- toolchain: mise 管理 Rust
- exit code: 0
- 結果: `synvelis-gateway` の check 成功。

### V-5 `mise run test synvelis-gateway`（対象 package）

- コマンド: `mise run test synvelis-gateway`
- 補助: `cargo test --locked -p synvelis-gateway`
- OS: Windows（Git Bash）
- toolchain: mise 管理 Rust（観測 Cargo `1.99.1`）
- exit code: 0
- 結果: 全 test binary 成功。観測された集計で **487 件の
  passing test・0 failure**（library 7・authorization 74・
  contract/project/concurrency/persistence/media suites・
  mcp adapter 27・proposal 30・proposal regressions 16・
  schema compat 7・doc-tests 0）。実装 crate は変更してい
  ないため、この結果は既存実装の headless 回帰証拠。

### V-6 doc-status lint（docs 検証）

- コマンド:
  `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type {spec,design,plan,task,test-spec} --json`
- OS: Windows（Git Bash）
- toolchain: Node `26.12.1`
- exit code: 0
- 結果: 全 docs type で lint 成功（front matter・relations・
  参照整合）。

### V-7 doc-maintenance index apply（index 再生成）

- コマンド: `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index apply`
- OS: Windows（Git Bash）
- toolchain: Node `26.12.1`
- exit code: 0
- 結果: `docs/{specs,designs,plans,tasks,test-specs}/README.md`
  を再生成。手編集なし。

### V-8 impl-doc audit + index sync

- コマンド:
  - `node .agents/skills/impl-doc/scripts/audit_impl_record.js
    --json`
  - `node .agents/skills/impl-doc/scripts/audit_experiment_log.js
    --json`
  - `node .agents/skills/impl-doc/scripts/sync_impl_index.js
    --kind ir/exp --apply`
- OS: Windows（Git Bash）
- toolchain: Node `26.12.1`
- exit code: 0
- 結果: `docs/impl/ir/README.md`・`docs/impl/exp/README.md`
  を再生成。audit 成功。

### V-9 audit_docs（全 docs graph audit）

- コマンド:
  `node .agents/skills/doc-status/scripts/audit_docs.js
  --type all --json`
- OS: Windows（Git Bash）
- toolchain: Node `26.12.1`
- exit code: 0
- 結果: 全 docs type の audit 成功。

### V-10 route_graph（doc graph route）

- コマンド:
  `node .agents/skills/doc-driven-dev-graph/scripts/
  route_graph.js --focus command-gateway --json`
- OS: Windows（Git Bash）
- toolchain: Node `26.12.1`
- exit code: 0
- 結果: command-gateway route が spec→design→plan→task→
  test-spec→impl→evidence の鎖を解決。

### V-11 review-protocol

- 実施: review-protocol skill によるレビュー（spec/design/
  plan/task/test-spec/impl/evidence の一貫性・evidence
  honesty・Owner 境界・relations 整合）
- 結果: blocker なし（レビューで指摘された整合性問題は修正
  済み）。
- 備考: docs のみの slice のため実装 review ではなく契約
  review。

## Pass / Fail / NotRun / Unsupported 集計（本 slice）

| 区分 | 対象 | 結果 |
|---|---|---|
| Pass | docs 検証（fmt-check/lint/index/audit/route） | 全成功 |
| Pass | headless 回帰（gw10/gw10-report/check/test synvelis-gateway） | 全成功 |
| Pass | required case（GT/JM/MCP/SEC/DATA/REC/JOB/SYS/AC/E2E/GTV・G0〜G6） | Pass（mock+PoC real/L1-L4） |
| NotRun | 実 GPUI（SYS-02-ux/03-ux・UX-* 実 UI） | #114 後続 |
| NotRun | 実 Provider・第三者 MCP Host・実機 lane | #46 G4 拡張・#83・実機 lane |
| NotRun | SYS-05-powerloss・CONSUMER-* | required=false・条件未成立 |
| Unsupported | SYS-07-rt（RT budget 未実測） | required=false |

## 判定

本 slice の docs/ 設計契約 + evidence 台帳 + headless 回帰
実測は **完了・全検証成功**。実装 crate は変更していない。
設計完了は実装完了・実機/GUI/Provider 受入を意味しない。
後続 Issue（#46 配下・#83・#114・実機 lane・AUTX #259〜
#267・rf#294/#553 条件付き）が Owner として残る。
