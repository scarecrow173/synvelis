# SV-AUTX-00 タスク分解（Task）

[plan.md](plan.md) の Slice を Owner Issue 単位のタスクに落としたもの。
各タスクは対応する契約文書の節を acceptance の根拠とする。

## AUTX-00（本タスク / #259）

- [x] T-00-1: `docs/automation/` に契約文書群を作成（spec/design×7/plan/tasks/README）
- [x] T-00-2: Owner 表・DAG・非重複境界・Case ID registry を統合
- [ ] T-00-3: `doc-maintenance` index 再生成 + doc-status 検査（PR で結果記録）
- [ ] T-00-4: review-protocol レビュー（blocker なしを確認）
- [ ] T-00-5: PR 作成（`Refs #259`。umbrella は close しない）

## AUTX-01 Action（#260）— S1

- [ ] T-01-1: Action catalog を `CapabilityRegistry` 投影として実装
      （第二 registry なし。根拠: design-capability-action §1/§6）
- [ ] T-01-2: Action Metadata（検索名/カテゴリ/UI hint）のみを保持
- [ ] T-01-3: `Availability` 表示 status と実行権限の分離を UI/CLI で確認
      （case: AUTX-ACT-CAP-01, AUTX-ACT-AV-01）

## AUTX-02 CLI issuer（#261）— S2

- [ ] T-02-1: trusted local session の attest と `AdapterSeal` 付き
      `CallerIdentity` 発行を adapter に実装
- [ ] T-02-2: Human/Unattended/CI/Agent の session 種別→Principal/Origin
      マッピング（design-trust-boundary §2）
- [ ] T-02-3: `--as human`/`--yes`/TTY/env のみで Human が生成されない
      ことを試験（case: AUTX-TRU-HUM-01, AUTX-TRU-AGT-01）

## AUTX-03 Headless CLI（#262）— S6（#261 受入待ち）

- [ ] T-03-1: `action run --file` / `workflow run --file` の CLI 面
      （`script run`/`--script` は作らない）
- [ ] T-03-2: 機械可読 Outcome/Error（`GatewayOutcome`/`ErrorEnvelope` を
      そのまま serialize。GUI/MCP と同一語彙）
- [ ] T-03-3: Native/WebView 非依存の独立 adapter（GW-R-13）

## AUTX-04 Atomic Macro（#263）— S5

- [ ] T-04-1: 既存 `synvelis.operation.batch` の境界契約を確認
      （≤64・非 nested・単一 Project・1 CAS/1 Undo）
- [ ] T-04-2: scratch-ID/rewrite seam の拡張または typed Unsupported
      （design-atomic-macro §3。case: AUTX-MAC-SID-01）
- [ ] T-04-3: 65/nested/cross-project/Query 混在の typed 拒否試験
      （case: AUTX-MAC-BND-01, AUTX-MAC-CAS-01）

## AUTX-05 Workflow Plan（#264）— S3（Start Blocker）

- [ ] T-05-1: `WorkflowDefinition` → 型付き Plan Compiler（7 step kind:
      Query/AtomicBatch/JobSubmit/Await/Result/If/BoundedLoop）
- [ ] T-05-2: plan 署名・revision・idempotency key・cycle/再帰拒否・
      bounded loop 上限（case: AUTX-WFL-STEP-01, AUTX-WFL-CYC-01）

## AUTX-06 Workflow Coordinator（#265）— S4（#264 待ち）

- [ ] T-06-1: Run/Step 状態機械と journal・resume cursor
      （design-workflow-run §3/§4。case: AUTX-WFL-RUN-01）
- [ ] T-06-2: Fault Matrix F-1〜F-10 の fixture 試験・二重副作用 0
      （case: AUTX-WFL-FLT-01）
- [ ] T-06-3: cancel request 追跡・`CancelLink` の正直な状態
- [ ] T-06-4: 接続寿命からの独立（resume/cancel/再照会）

## AUTX-07 GPUI UX（#266）— S8（並行）

- [ ] T-07-1: Action catalog・Workflow 状態・Approval/結果の GPUI surface
      （Mock/Fixture 契約から着手）
- [ ] T-07-2: GUI 独自の実行経路を作らない（Gateway 経由のみ）

## AUTX-08 E2E（#267）— S9

- [ ] T-08-1: GUI/MCP/CLI/AI の同一 CapabilityRef→同一 Outcome/Error
      （case: AUTX-E2E-SAME-01）
- [ ] T-08-2: 独立 Policy/Commit oracle（case: AUTX-E2E-ORACLE-01）
- [ ] T-08-3: 実 OS 証拠の Required/Deferred/NotRun 分離記録

## SV-SCRIPT-01〜04（#271〜#274）— S7

- [ ] T-S1-1 (#271): `ScriptActionDefinition` schema・backend registry・
      source/version drift 拒否（case: AUTX-SCR-DEF-01）
- [ ] T-S2-1 (#272): script→typed Invocation Document/WorkflowDefinition
- [ ] T-S3-1 (#273): YAML/Luau/Rhai typed port・共通 Plan 収束・
      異種言語参照の digest pin（case: AUTX-SCR-TYP-01, AUTX-SCR-REF-01）
- [ ] T-S4-1 (#274): `--file` 面・Analyzer/Runtime 独立
      （case: AUTX-SCR-ANA-01, AUTX-CLI-FILE-01）

## Ruviform 依存（Synvelis 側は契約参照のみ）

- [ ] RF#553: 共通化の設計評価（結果を #271〜#274 の採否に反映）
- [ ] RF#558/#559: Luau+Rhai 共存 PoC → #273 backend 要件の確定
- [ ] RF#560: native Analyzer/FFI → #274 の Analyzer 独立境界の確定
