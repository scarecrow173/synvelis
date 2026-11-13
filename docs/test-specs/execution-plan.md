---
id: "TSPEC-034X83U7r7ZrZZjmIhd8Em"
type: "test-spec"
status: "approved"
title: "Execution Plan Test Spec — DAG acyclicity・Owner/Gate/Acceptance matrix invariants・OS level rules"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
    - "docs/test-specs/transport-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/execution-plan-design.md"
  derived-by: []
  verifies:
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
  verified-by: []
  references:
    - "evidence/sv23/execution-plan-gate-ledger.md"
    - "evidence/sv23/execution-plan-entry-gate-ledger.md"
    - "evidence/sv23/execution-plan-verification.md"
  defers: []
  deferred-by: []
---

# Execution Plan Test Spec — DAG acyclicity・Owner/Gate/Acceptance matrix invariants・OS level rules

## Features / Rules

本 test-spec は Issue #23 docs slice の不変条件を定義する。
対象は文書・台帳の構造検証であり、実装コード・実機
受入を対象としない（実装 Gate は各 workstream Issue と
Daily/手動 Full CI が担う）。

### EP-TS-R01 単一 DAG・非巡回（EP-R-01・EP-AC-01/09）

- `docs/designs/execution-plan-design.md` の edge 表と
  `evidence/sv23/execution-plan-gate-ledger.md` が定義
  する全 edge を (from, to, type) の組として抽出したとき:
  - edge type は `start`/`integration`/`acceptance` の
    3 値のみ。
  - `acceptance` edge の to は Gate node のみで、
    Gate node から `start` edge が出ない（Start への
    逆流なし）。
  - 全 node/edge の到達可能性に循環がない
    （topological sort が完了する）。
  - 未定義 node への edge・定義済み node の孤立参照が
    ない。
- Issue #23 本文の Start/Integration/Acceptance 分離を
  3 相に型付けし、参照先 Issue の全 Close を Start
  条件にしていないこと（例: #103 Oracle PASS は #133
  の acceptance 条件であり start 条件ではない）。

### EP-TS-R02 Owner/Gate/Acceptance Matrix 一意性（EP-R-03）

- `evidence/sv23/execution-plan-gate-ledger.md` の各
  node が Owner（Synvelis 製品 Domain / Ruviform 共通
  基盤 / AudioLens Consumer / CI Owner）を一意に持つ。
- 同一 capability の実装 Owner が Synvelis/Ruviform で
  重複しない（共通基盤能力は Ruviform Issue へ委譲）。
- Gate node の受入条件が Required OS Level・証拠種別・
  独立 Oracle を明記する。

### EP-TS-R03 OS 検証レベル規則（EP-R-04/05・EP-AC-02〜05）

- `evidence/sv23/execution-plan-entry-gate-ledger.md` の
  各入口行が OS Level（OS-L1/GUI-L2/EXT-L3）と Gate/
  Feature 範囲を持つ。
- Windows: GUI-L2/EXT-L3 は「実際に提供・利用する」
  機能のみ Required。macOS/Linux: GUI-L2/EXT-L3 は
  Deferred（利用要件発生まで）であり、NotRequired/
  Deferred を Pass にしていない。
- 要求状態（Required/Deferred/NotApplicable）と実行
  結果（Pass/Fail/NotRun/Unsupported）が別欄で記録され
  ている。
- `gpui-test`/`webview`/Fake Driver/Headless/Browser の
  成功が GUI-L2/EXT-L3 の Pass として記録されていない
  （capability 分離）。

### EP-TS-R04 正規入口対応と Evidence Schema（EP-R-06/08・EP-AC-08）

- `evidence/sv23/execution-plan-entry-gate-ledger.md` が
  `mise.toml` の正規入口を網羅し、各行が対象 SHA・
  OS・Gate/Feature・結果状態（実測 Pass または
  NotRun/Unsupported/Deferred）を持つ。
- evidence record が schema
  （case_id|gate|repo|candidate_sha|
  consumer_resolved_sha|os/arch|feature/test|
  requirement_state|result_state|expected|observed|
  run_url/device_log|owner_issue|notes）に従う。
- 未実施入口は `NotRun`/`Unsupported` であり `Pass`
  ではない。

### EP-TS-R05 候補 SHA tuple 追跡（EP-R-07・EP-AC-07）

- `evidence/sv23/execution-plan-gate-ledger.md` に
  #262/#419・#294（Ruviform）・#114/#109/#103
  （Synvelis）の tuple registry が存在し、同一候補
  SHA/OS/Feature/結果状態を相互参照できる構造を持つ。
- 片 repo の結果を他 repo/他 Consumer へ転用していない。

### EP-TS-R06 Fail-closed 進捗判定（EP-R-12）

- node の Acceptance 到達記録は、対応 Gate の Required
  項目が全て `Pass` かつ証拠（CI Run URL/実機ログ/
  Case ID）を持つ場合のみ許可される。
- `NotRun`/`Fail`/`Unsupported` が残る node を
  Acceptance 済みと記録していない。
- CI に実装されていない検証を「委譲済み」としていない
  （新規 Workflow 増設なし・EP-R-09）。

### EP-TS-R07 文書鎖整合（EP-R-11）

- spec→design→plan→task→test-spec→impl(ir/exp) の
  relations が相互に一貫（implements/derived-by/
  verifies/verified-by/references が実在ファイルを指す）。
- sv#21/#22 並行 Worker の文書を編集・作成していない
  （参照のみ）。

## Examples

- 正例: SV-SF-03(#106) →start SV-SF-04(#107 headless)
  は `start` edge。SV-SF-06(#109) は start edge を持たず
  並行開発・Acceptance のみ。
- 正例: #103 Oracle PASS は SV-AUT-03(#133) の
  `acceptance` edge であり `start` edge ではない。
- 誤例（禁止）: GUI-L2-WIN の `Pass` を macOS/Linux の
  EXT-L3 `Pass` と読み替える、`gpui-test` Pass を #114
  製品 E2E の GUI-L2 Pass とする、Daily CI 未実行 SHA を
  `Pass` と記録する。

## Guarantees

- 本 slice の成果物は docs/ 設計契約 + evidence/sv23 台帳
  のみであり、実装・CI・Cargo/mise 変更を含まない。
- 文書承認・計画確立は実装完了でも GUI/Device/Plugin/
  製品受入でもない。各 workstream の Acceptance は
  対応 Gate の Required 証拠が揃うまで未受入。
- 台帳の `NotRun`/`Deferred`/`Unsupported` は誠実に
  記録され、後続 Owner が証拠を追跡できる。
