# SV-AUTX-00 仕様: Action / Macro / Workflow / Script / Headless CLI 統合契約

- Issue: <https://github.com/scarecrow173/synvelis/issues/259>
- 種別: umbrella 設計契約（DESIGN-ONLY。実装を含まない）
- 前提正本: `crates/synvelis-gateway/`（GW-01〜10）、`docs/specs/command-gateway.md`、
  `docs/designs/execution-plan-design.md`

## 1. 目的

Synvelis の自動化呼び出し面（GUI・MCP・Headless CLI・Script・Workflow）が
それぞれ独自の Capability Registry・認可系・Job 正本・Project Writer・
Workflow Engine を持つことを禁止し、単一の統合契約の下で収束させる。

本仕様が固定する不変条件:

1. Capability の正本は `synvelis-gateway` の `CapabilityRegistry` のみ。
   Action はその presentation 投影であり、第二 Registry を作らない。
2. 認可の正本は GW-02 の `PolicyEvaluator` + Grant/Approval（GW-05/06）のみ。
   CLI・Script・Workflow はいずれも迂回経路を持たない。
3. Project 変更の正本は GW-06 `CommitGate` / ProjectController の単一 writer
   のみ。Atomic Macro も Workflow も直接 mutate しない。
4. Job 実行の正本は provider/coordinator（`JobPort` 先）であり、adapter は
   correlation record のみを持つ。第二 Job truth を作らない。
5. GUI / MCP / CLI は同一の `GatewayOutcome` / `ErrorEnvelope` /
   Receipt 契約を共有する。

## 2. 対象範囲

### 含む

- Action / Atomic Macro / Workflow / Script / Headless CLI の統合契約
- Owner・依存 DAG（AUTX-01〜08、SV-SCRIPT-01〜04、Ruviform #553/#558〜560）
- Run/Step 状態機械、Fault/Recovery 契約
- Trust/OS Boundary（CLI の Human/Unattended/CI/Agent 分離）
- Script → 型付き Plan → Gateway の収束契約
- Case ID レジストリ、検証マトリクスの枠組み

### 含まない（各 Owner Issue の責務）

- AUTX-01〜08（#260〜#267）と SV-SCRIPT-01〜04（#271〜#274）の実装
- Ruviform 側の実装（#553/#558/#559/#560 は依存・境界の参照のみ）
- `crates/**` の一切の変更

## 3. 要件（Issue #259 受入条件の転記）

- R-1: AUTX-01〜08 と RF #558〜560 / SV #271〜274 の Owner・DAG・唯一正本・
  Script→Typed Plan→Gateway を整理し、Project/Job/Grant/Action Registry の
  第二正本を 0 件とする。
- R-2: YAML/Luau/Rhai の共通 Plan、異種言語参照、Analyzer 独立、Rhai PoC 限定、
  Lua 5.4 除外が受入仕様と CLI に一致する。
- R-3: AUTX-00 と #271〜274 / Ruviform #558〜560 の Owner 表・DAG・
  Trust/Schema/Plan/Artifact 境界を統合し、第二 Gateway/Job/Project Writer/
  Action Registry 0 件を確認する。
- R-4: YAML/Luau/Rhai を共通型付き Plan へ接続するケース、異種言語 Workflow
  参照、Rhai 未正式サポート、Lua 5.4 除外、Analyzer と Runtime 独立を
  実装 Owner 別に検証可能にする。
- R-5: 実在する `CapabilityDescriptor` / `OperationDomain` / `Gateway::bind` /
  `CommitGate` / `JobPort` と対応する Action/Workflow/CLI 型・Owner を表で
  照合し、二重 Registry/Writer/Job/Grant 作成 0 件である。
- R-6: Atomic Batch（≤64・非 Nested）/ 単一 Project / 1 CAS / 1 Undo・
  作成直後 ID 参照の未対応/対応を、実装可能な Input/Output/Unsupported 契約
  として定義する。
- R-7: Workflow の Run/Step 状態遷移、再起動/Cancel/Grant Revoke/
  Indeterminate 時の再照会、外部副作用非 Atomic・計画署名/Revision/
  Idempotency を Fault Matrix と独立 Oracle で定義する。
- R-8: ローカル CLI の Human/Unattended/CI/Agent を
  Origin/Principal/AdapterSeal 発行権限と分離し、任意引数・TTY・環境変数
  だけで Human 承認が生成されない契約にする。
- R-9: 各 AUTX-01〜08 の成果物/受入 Owner・開始に必要な条件と統合関係を
  DAG に反映し、GUI/CLI/AI の意味論が同一 Capability/Project 正本に一致する。

## 4. 受入条件（本タスクの完了条件）

- [x] AC-1: `docs/automation/` に上記 8 成果物相当の契約文書を作成した
  （design-*.md 7 件 + spec/plan/tasks/overview + 生成 index README.md）。
- [x] AC-2: Owner 表・DAG・Trust/Schema/Plan/Artifact 境界を
  [design-owner-dag.md](design-owner-dag.md) に統合した。
- [x] AC-3: 第二 Gateway/Job/Project Writer/Action Registry 0 件の照合表を
  各 design 文書と DAG 文書に記した。
- [x] AC-4: Atomic Batch / Run-Stop 遷移 / CLI Trust / Script 契約を
  実装 Owner がテスト可能な形で記述した。
- [x] AC-5: `doc-maintenance` の index 再生成と doc-status 検査を実行した
  （`index apply --dir docs/automation` で `docs/automation/README.md` を
  生成・in-sync を確認。`doc_status lint` の findings は unmanaged 契約文書
  共通の front-matter 系であり、既存の非 canonical 文書と同種。
  実測値は PR 本文に記録）。
- [x] AC-6: review-protocol レビューで blocker なしを確認した
  （subagent 不使用のため Reviewer A/B・Validator・Judge を分離パスで実施。
  確認済み修正: case ID 衝突・batch 契約精度・DAG #263 欠落・Script Batch
  規則の明記。残存 P0/P1 なし。詳細は PR 本文）。

## 5. 検証方法

- 本タスクは文書のみの変更であり、Workspace 全体の Build/Test は不要
  （POLICIES.md §5、タスク共通契約「検証」節）。
- 実施する検証:
  - Markdown リンク・相対参照の整合
  - `doc-maintenance` `index plan` → `index apply`（生成物の再生成）
  - `doc-status` の lint/audit（新規文書・影響 index の範囲）
  - `mise run fmt-check`（文書のみの変更だが実行可能なら実施）
- 設計レビュー観点: `mise run check` / `mise run test` / `mise run gw10` の
  既存 Contract との齟齬がないことを**契約照合**で確認する。これらの
  コマンド実行・実 OS 証拠は別 Owner の実施であり、本タスクでは **NotRun**。
- セキュリティ/RT 観点: `mise run test-rt` および G0〜G6 Grant・
  Commit/Recovery Fault Matrix との齟齬を反例表で確認するが、
  実装未完のため **NotRun**（契約レベルの整合のみ確認）。

## 6. ポリシー語彙

- 要件状態: `Required` / `Deferred` / `NotApplicable`
- 実行結果: `Pass` / `Fail` / `NotRun` / `Unsupported`
- 実測と推測を区別する。headless/fake 成功を実 Windows GUI-L2・EXT-L3 の
  証拠とみなさない。
