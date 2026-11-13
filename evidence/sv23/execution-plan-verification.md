# SV23 Execution Plan — Verification Record

Issue #23 docs slice（横断 DAG・Owner/Gate/Acceptance
Matrix・OS 検証レベル・evidence 台帳）の検証実行記録。
本記録は **docs/ 設計契約 + evidence 台帳のローカル検証**
であり、統合受入・GUI-L2/EXT-L3 実機受入ではない。
merge 後 main SHA の必須 CI 結果が統合検証の証拠
（POLICIES.md・共通契約）。

## Environment

| 項目 | 値 |
| --- | --- |
| Repo | scarecrow173/synvelis |
| Branch | scarecrow173/sv23-execution-plan（push 前に `docs/23-execution-plan` との関係を確認） |
| OS | Windows（worktree 環境） |
| Toolchain | mise 管理（`mise.toml` 正本）+ Node.js（doc ツール） |
| 対象 SHA | branch HEAD（commit 後に確定・本記録は commit 前の作業 tree に対する検証） |
| Ruviform resolved | `Cargo.lock` pin（本 slice で変更なし） |

## 実行した検証

| # | コマンド | 目的 | 終了コード | 結果 |
| --- | --- | --- | --- | --- |
| 1 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply` | docs index 再生成 | 0 | Pass — 1 回目は IR front matter の `changes.added` 形式不正で `docs/impl/ir/README.md` が blocked。`{type,path}` object 形式へ修正後、再実行で 1 written / 0 blocked |
| 2 | `node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/{specs,designs,plans,tasks,test-specs}` | front matter/relation lint | 0 | Pass — `execution-plan*` 6 文書に finding なし。test-spec の `derives-from`/`verifies` 不整合 2 件を修正後に再 lint で 0 件。dir 全体には既存文書の baseline finding（vcs-*/tim-* 等の invalid-id-format・missing-upstream 等）が残存（範囲外・未修復） |
| 3 | `node .agents/skills/doc-status/scripts/audit_docs.js --type all` | docs 監査 | 0 | Pass（範囲内）— `execution-plan*` に finding なし。repo 全体には既存文書の baseline error/warning が残存（範囲外・未修復） |
| 4 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | IR 監査 | 0 | Pass（範囲内）— `execution-plan.md` に finding なし。既存 IR の baseline finding（missing-section・broken-relation-link 等）が残存（範囲外・未修復） |
| 5 | `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus execution-plan --json` | graph route | 0 | Blocked（baseline）— `status: blocked`・`current/next: probe`。blocker は repo 全体の既存 `artifact-graph` + `broken-relation` findings（他文書の `defers`/source 関係）で、本 slice の文書に起因する finding はない。`--focus docs/specs/execution-plan.md` では focus が解決され focus-invalid finding なし（同じく baseline blocked）。既存 blocker は範囲外のため未修復・記録のみ |
| 6 | `mise run fmt-check` | 体裁検査（`cargo fmt --all -- --check`） | 0 | Pass |
| 7 | `review-protocol`（下記 2 pass） | レビュー | — | Pass — blocker なし（詳細は「Review 記録」） |

## Review 記録（review-protocol）

Objective checks（上表 1〜6）実施後、subagent を使えない
worker 契約のため cooperative / adversarial の 2 pass を
分離して実施した。

### Cooperative pass（整合性レビュー）

- spec→design→plan→task→test-spec→impl(ir/exp) の
  relations が相互に一貫し、全 target が実在することを
  確認（lint 0 findings と併せて確認）。
- `evidence/sv23/` 3 件が spec/design/plan/task/test-spec/
  IR の `references` から到達可能であることを確認。
- Issue #23 の 7 受入条件が EP-AC-01〜07 に写像され、
  EP-AC-08/09 が evidence schema・非巡回性を補完する
  ことを確認。
- 発見: test-spec の `derives-from` が spec を指し spec
  の `derived-by` に戻りリンクがない警告、`verifies` が
  plan（不可型）を含む blocking 警告 → sv#20 慣行に
  合わせ `derives-from: [design]`・`verifies: [spec,
  design]` へ修正。

### Adversarial pass（欠陥探索レビュー）

- 「Pass」を自称する記述を全台帳で走査し、実測してい
  ない `check-ruviform-rev` が `Pass` と記録されている
  のを発見 → `NotRun`（理由明記）へ修正。
- DAG edge 表の `SV-RCP-01〜07 →acc RCP-07-GATE(#296)`
  が gate node 自身を source に含む表記揺れを発見 →
  `SV-RCP-01〜06 →acc SV-RCP-07(#296)` へ修正。
- Issue #23 の non-goal/provisional 値（SMF SMPTE・
  MIDI2/MPE・VCS three-way merge・Comp crossfade 20 ms・
  Touch return 150 ms・capture queue・journal checkpoint・
  monitor ramp・plugin processing accuracy・GUI LOD SLO）
  の明示が不足 → design の Deferred Design Concerns に
  追記。
- `NotRun`/`Deferred`/`Unsupported` を `Pass` に読み替
  える記述、headless/Fake 成功を GUI-L2/EXT-L3 Pass に
  する記述、新規 CI Workflow 増設を示唆する記述がない
  ことを確認。

### 判定

- CONFIRMED P0/P1: なし。P2/P3 相当の指摘は全て修正
  済み。blocker は残っていない。

## NotRun / Unsupported / Deferred

| 項目 | 状態 | 理由・後続 |
| --- | --- | --- |
| 実装コード・CI・Cargo/mise 変更を伴う検証 | NotRun | docs-only slice。変更範囲に比例した軽量チェックのみ実施 |
| `check`/`lint`/`test`/`build <pkg>`・`test-rt`・`gw10`・`check-headless-deps`・`check-ruviform-rev` | NotRun | 対象 crate/依存の変更なし。Daily/手動 Full CI に委譲 |
| `check-app-native`/`lint-app-native`/`check-app-gpui`/`check-ui`/`check-app-webview` 系 | NotRun | 対象外（docs-only）。GUI-L2 は別 Issue |
| `run-app-native`（実窓） | NotRun | 手動・CI 対象外。GUI-L2-WIN は synvelis#114/#176・ruviform#313/#243 が担う |
| `test-recording-real-*` | NotRun | opt-in 手動・CI 対象外。EXT-L3-WIN は synvelis#115/#117・ruviform#251/#253 が担う |
| macOS/Linux の GUI-L2/EXT-L3 | Deferred | 利用要件発生まで Deferred（POLICIES.md）。未実測を Pass にしない |
| `test-msrv` | NotRun | ユーザー指定により本 slice では実行しない |
| Daily/手動 Full CI の対象 SHA 実行 | NotRun | merge 後 main SHA の必須 CI 結果が統合検証の証拠（Coordinator が確認） |
| 実 Window/実 Device/実 Plugin/実 Provider の L2/L3 実測 | NotRun | synvelis#114/#176・ruviform#313/#243/#251/#253/#388/#389/#532 が担う |
| graph router の baseline blocker（`artifact-graph`・`broken-relation`） | NotRun（未修復） | repo 全体の既存 findings。他文書の `defers`/source 関係が原因で本 slice 範囲外。修復は別タスク |

## DAG/matrix 不変条件の確認（test-spec EP-TS-R01〜R07 の適用）

- EP-TS-R01（非巡回・edge 型）: `docs/designs/execution-
  plan-design.md` の edge 表を (from,to,type) として
  確認。edge type は `start`/`integration`/`acceptance`
  の 3 値のみ。`acceptance` edge の to は Gate node のみ
  で、Gate node から `start` edge が出ない。全 lane を
  通じて Acceptance→Start の逆流・同一 node 間循環・
  未定義 node 参照がないことを確認。
- EP-TS-R02（Owner 一意）: `execution-plan-gate-ledger.md`
  の各 node が Owner を一意に持ち、共通基盤能力を
  Synvelis で重複実装しない規則を確認。
- EP-TS-R03（OS Level）: `execution-plan-entry-gate-
  ledger.md` の各入口が OS Level・Gate/Feature・要求
  状態・結果状態を別欄で持ち、`gpui-test`/`webview`/
  Fake Driver の成功を GUI-L2/EXT-L3 Pass にしていない
  ことを確認。
- EP-TS-R04（正規入口・schema）: 正規入口を網羅し、
  未実施は `NotRun`/`Unsupported`/`Deferred` である
  ことを確認。
- EP-TS-R05（tuple 追跡）: #262/#419・#294・#114/#109/
  #103 の tuple registry が `execution-plan-gate-
  ledger.md` に存在し、片 repo の結果を他 repo へ
  転用していないことを確認。
- EP-TS-R06（fail-closed）: `NotRun`/`Fail`/`Unsupported`
  が残る node を Acceptance 済みと記録していないこと、
  CI 未実装の検証を「委譲済み」としていないことを確認。
- EP-TS-R07（文書鎖整合）: spec→design→plan→task→
  test-spec→impl(ir/exp) の relations が相互に一貫し、
  sv#21/#22 並行 Worker の文書を編集・作成していない
  ことを確認。

## 結果サマリ

- docs/ 設計契約 + evidence/sv23 台帳のローカル検証:
  index apply・lint・audit_docs・audit_impl_record・
  fmt-check は Pass。route_graph は baseline の
  repo-wide blocker で `blocked`（本 slice 起因の
  finding なし）。review-protocol は blocker なし。
- 統合受入・GUI-L2/EXT-L3 実機受入: **NotRun**（別
  Issue・別 Gate が担う。本 slice は計画・台帳のみ）。
