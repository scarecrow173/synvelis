# Owner / Dependency DAG・Case ID・非重複境界（AUTX-00 中核）

一次正本: `docs/designs/execution-plan-design.md`（AUTX DAG 行）、
`docs/specs/command-gateway.md` GW-R-13（Boundary Non-duplication）、
Issue #259〜#267・#271〜#274・Ruviform #553/#558〜560。
本書はそれらを統合した Owner 表・DAG・境界照合の canonical とする。

## 1. Owner 表

| ID | Issue | 成果物（所有する契約） | 触れてはいけない正本 |
| --- | --- | --- | --- |
| SV-AUTX-00 | [#259](https://github.com/scarecrow173/synvelis/issues/259) | 本契約文書群（統合・Owner/DAG・境界） | 実装コード全般 |
| SV-AUTX-01 | [#260](https://github.com/scarecrow173/synvelis/issues/260) | Action = CapabilityRef/Descriptor の Presentation 投影・Action Metadata・catalog | 別 Action Registry/Executor・schema/effect/permission の複写 |
| SV-AUTX-02 | [#261](https://github.com/scarecrow173/synvelis/issues/261) | CLI local issuer（trusted local session の attest・`AdapterSeal` 付き信頼境界） | `PrincipalKind::Human` の偽造・TTY/`--yes`/JSON からの Human 昇格 |
| SV-AUTX-03 | [#262](https://github.com/scarecrow173/synvelis/issues/262) | Headless CLI adapter（`action run --file`/`workflow run --file`・機械可読 Outcome/Error） | Native/WebView 依存・第二 Gateway/認可/Job 経路・`script run`/`--script` |
| SV-AUTX-04 | [#263](https://github.com/scarecrow173/synvelis/issues/263) | Atomic Macro: 既存 `synvelis.operation.batch`→`ProjectOp::Composite`→GW-05/06 Commit（1 CAS/1 Undo）・scratch-ID seam | 新 Transaction Engine・第二 CommitGate・部分 commit |
| SV-AUTX-05 | [#264](https://github.com/scarecrow173/synvelis/issues/264) | Workflow Definition/Plan Compiler（型付き Plan・step kind・plan 署名/revision/idempotency） | Gateway 内への Plan 実行混入・第二 Registry |
| SV-AUTX-06 | [#265](https://github.com/scarecrow173/synvelis/issues/265) | Workflow Coordinator（Run/Step 状態・journal・resume cursor・cancel・recovery） | GW-02 Grant・GW-05 Approval・GW-06 ProjectWriter/Journal・GW-07 Provider Job Truth の再実装 |
| SV-AUTX-07 | [#266](https://github.com/scarecrow173/synvelis/issues/266) | GPUI UX（Action catalog・Workflow 可視化・Approval/結果 surface） | Mock/Fixture を越えた実機主張・GUI 独自の実行経路 |
| SV-AUTX-08 | [#267](https://github.com/scarecrow173/synvelis/issues/267) | E2E 受入（MCP/CLI/GPUI の同一 Capability・独立 Policy/Commit oracle） | 他 lane の検証責務の代行 |
| SV-SCRIPT-01 | [#271](https://github.com/scarecrow173/synvelis/issues/271) | Script 定義 schema・`ScriptActionDefinition`・backend registry | authority の script 内同梱・第二 Registry |
| SV-SCRIPT-02 | [#272](https://github.com/scarecrow173/synvelis/issues/272) | Typed Plan 生成（Action recipe / WorkflowDefinition lower） | 直接 Project mutation・VM 内部型の公開 |
| SV-SCRIPT-03 | [#273](https://github.com/scarecrow173/synvelis/issues/273) | 言語 backend（YAML/Luau/Rhai typed port・共通 Plan 収束） | VM 固有意味論の Plan への漏出・Lua 5.4 互換保証 |
| SV-SCRIPT-04 | [#274](https://github.com/scarecrow173/synvelis/issues/274) | CLI `--file` 面・Analyzer/Runtime 独立境界 | `script run`/`--script`・Analyzer 成功=Grant の混同 |
| Ruviform RF#553 | [ruviform#553](https://github.com/scarecrow173/ruviform/issues/553) | 共通化の設計評価（PoC） | Gateway/DAW 型の Ruviform への移植（評価のみ） |
| Ruviform RF#558 | [ruviform#558](https://github.com/scarecrow173/ruviform/issues/558) | Luau+Rhai 同一バイナリ共存 PoC | 製品正式サポートの確約 |
| Ruviform RF#559 | [ruviform#559](https://github.com/scarecrow173/ruviform/issues/559) | Luau/Rhai 共存 PoC（backend 要件の根拠） | Rhai 正式サポート宣言 |
| Ruviform RF#560 | [ruviform#560](https://github.com/scarecrow173/ruviform/issues/560) | native Luau Analyzer/FFI（静的検査） | Analyzer 成功=実行許可の混同・core 契約への混入 |

## 2. 依存 DAG（execution-plan-design.md AUTX lane の再掲＋統合）

```
SV-AUTX-00(#259) 本契約
   │ start
   ├→ SV-AUTX-01(#260 Action)       ──┐
   ├→ SV-AUTX-02(#261 CLI issuer)   ──┤ 並行
   └→ SV-AUTX-04(#263 Atomic Macro) ──┘（GW-04/05/06 merge 済み・fixture で並行可）
                                     │
   SV-AUTX-05(#264 Workflow Plan) ──→start── SV-AUTX-06(#265 Coordinator)   ← 唯一の真の Start Blocker
        ▲                                                            │
        │ 契約参照                                                   │ start（#264 契約確定後）
   SV-SCRIPT-01〜04(#271〜#274) ──────────────────────────────────────┤
        （YAML/Luau/Rhai → typed Plan → Coordinator/Gateway 収束）      │
                                                                    ▼
                                          SV-AUTX-03(#262 mutating CLI) ←acc─ SV-AUTX-02(#261 auth 受入)
                                                                    │
                                          SV-AUTX-07(#266 GPUI UX) ───┤ Mock/Fixture から並行
                                          SV-AUTX-08(#267 E2E)  ──────┘
```

| Edge | 型 | 根拠 |
| --- | --- | --- |
| AUTX-00(#259) →start {AUTX-01(#260), AUTX-02(#261)} | start | 契約確定後、Action 投影と CLI issuer を並行着手 |
| AUTX-02(#261) →acc AUTX-03(#262) | acceptance | **mutating CLI の公開は #261 auth 受入まで禁止**。それまでは read-only/fixture 面のみ |
| AUTX-05(#264) →start AUTX-06(#265) | start | **#264 が真の Start Blocker**（Plan が無いと Coordinator は実行対象を持たない） |
| AUTX-07(#266)・AUTX-08(#267) | — | Mock Contract/E2E Fixture から並行着手可。統合受入は #267 が他 lane の oracle を束ねる |
| SV-SCRIPT-01〜04(#271〜#274) | integration | typed Plan が AUTX-05(#264) の Plan 契約に収束。backend は Ruviform RF#558/#559 PoC に依存 |
| RF#553/#558/#559/#560 | integration/評価 | Ruviform 側 PoC・評価。Synvelis core 契約へは typed port 経由でのみ接続 |

補足:
- AUTX-03(#262)・AUTX-04(#263)・AUTX-07(#266) は #261/#264 の契約が
  確定すれば fixture/mock を使って並行着手可能（Issue 本文の「統合受入は
  deferred でも最小 slice は開始可」の方針に従う）。
- AUTX-04(#263) の scratch-ID seam は GW-04 prepare engine の拡張であり、
  #264/#265 の Plan 契約とは独立に進められる。

## 3. 境界非重複の照合表（GW-R-13 の統合）

| 概念 | 唯一の正本（Owner） | AUTX/Script/CLI 側の禁止 |
| --- | --- | --- |
| Capability 識別・版・入出力 schema・effect/permission/data-access 上限 | `synvelis-gateway` `CapabilityRegistry`/`SchemaRegistry`（GW-01） | Action Registry・CLI 独自 catalog・Script 独自 registry・第二 Executor |
| 実行認可・Grant・Approval | GW-02 `PolicyEvaluator` + GW-05/06（Grant/Approval/Apply） | CLI 独自 auth・Script 内 Grant・Workflow 独自 policy・引数による Human 昇格 |
| Project 変更・revision/CAS・Undo・journal | GW-06 `CommitGate`/ProjectController（単一 writer） | Macro/Workflow/CLI/Script の直接 mutate・第二 Transaction Engine・部分 commit |
| Proposal/Approval/Apply 状態機械 | GW-05/06 proposal store（`ProposalStatus`/`ApprovalStatus`/`ApplyStatus`） | Workflow journal への状態複写・第二 approval store |
| Job 実行状態・Cancel 確定 | provider/coordinator（`JobPort` 先。`JobPhase`/`CancelLink`） | adapter/CLI/Workflow の job truth 複写・request のみでの `Cancelled` 確定 |
| Workflow 定義→Plan→Run 実行 | AUTX-05(#264) Plan Compiler + AUTX-06(#265) Coordinator | Gateway への Plan 実行混入・第二 workflow engine・VM 内部状態の共有 |
| Script→typed 呼出/plan 生成 | SV-SCRIPT-01〜04(#271〜#274) | script 内 authority・直接 mutation・`script run` 独立面 |
| GUI/CLI/MCP の outcome 表現 | GW-01 `GatewayOutcome`/`ErrorEnvelope`/Receipt | adapter 独自 error 語彙・成功の推測昇格 |

**確認**: 第二 Gateway・第二 Job Executor・第二 Project Writer・第二
Action Registry・第二 Grant/Approval 系の作成は 0 件でなければならない。

## 4. Case ID レジストリ（Action→Macro→Workflow→CLI→GUI/MCP）

契約試験・E2E で共有する case ID の命名規則と初期セット。
各 Owner は自分の Issue の受入に対応する case を実装・検証する。

命名: `AUTX-<面>-<領域>-<連番>`（例: `AUTX-CLI-TRUST-01`）。
面 = `ACT`(Action) / `MAC`(Atomic Macro) / `WFL`(Workflow) /
`CLI`(Headless CLI) / `GUI`(GPUI) / `MCP` / `SCR`(Script) / `TRU`(Trust)。

| Case ID | 面 | 検証内容 | 主担当 | 関連契約 |
| --- | --- | --- | --- | --- |
| AUTX-ACT-CAP-01 | ACT | Action 一覧が Registry 投影のみ・第二 registry なし | #260 | design-capability-action §1/§6 |
| AUTX-ACT-AV-01 | ACT | `Disabled/Planned/Retired` が `Unavailable` で拒否 | #260 | design-capability-action §3 |
| AUTX-MAC-BND-01 | MAC | 64 member 受理 / 65 拒否 / nested 拒否 / cross-project 拒否 | #263 | design-atomic-macro §2 |
| AUTX-MAC-CAS-01 | MAC | 単一 batch→1 proposal→1 approval→1 commit→1 undo | #263 | design-atomic-macro §2 |
| AUTX-MAC-SID-01 | MAC | create-then-reference: seam 有=対応 / 無=typed Unsupported→Workflow 分解 | #263 | design-atomic-macro §3 |
| AUTX-WFL-STEP-01 | WFL | 7 step kind の plan lower と順序実行 | #264 | design-atomic-macro §4 |
| AUTX-WFL-RUN-01 | WFL | Run/Step 状態遷移表の全合法遷移・非法遷移拒否 | #265 | design-workflow-run §3 |
| AUTX-WFL-FLT-01 | WFL | F-1〜F-10 fault 注入で二重副作用 0・Indeterminate reconcile | #265/#267 | design-workflow-run §5 |
| AUTX-WFL-CYC-01 | WFL | cycle/再帰 trigger・無制限 loop の plan 拒否 | #264 | design-workflow-run §2 |
| AUTX-TRU-HUM-01 | TRU | `--as human`/`--yes`/TTY/env のみで Human が生成されない | #261 | design-trust-boundary §3 |
| AUTX-TRU-AGT-01 | TRU | 無人/CI session が Agent principal・ApprovalRequired を受領 | #261/#262 | design-trust-boundary §2/§4 |
| AUTX-CLI-FILE-01 | CLI | `action run --file`/`workflow run --file` のみ存在・`script run` 非存在 | #262/#274 | design-script §5 |
| AUTX-SCR-DEF-01 | SCR | `ScriptActionDefinition` 全 field・source/version drift 拒否 | #271 | design-script §2 |
| AUTX-SCR-TYP-01 | SCR | YAML/Luau/Rhai→同一 typed Plan→同一 Gateway 経路 | #272/#273 | design-script §3/§4 |
| AUTX-SCR-ANA-01 | SCR | Analyzer 成功のみでは Grant/実行許可が変わらない | #274/RF#560 | design-script §4 |
| AUTX-SCR-REF-01 | SCR | 異種言語参照が Plan 上 digest pin・VM 状態非共有 | #273 | design-script §4 |
| AUTX-E2E-SAME-01 | E2E | GUI/MCP/CLI が同一 CapabilityRef で同一 Outcome/Error | #267 | design-capability-action §5 |
| AUTX-E2E-ORACLE-01 | E2E | 独立 Policy/Commit oracle（他 lane 結果を Pass にしない） | #267 | spec §5 |

case ID は registry であり、**未実装 case を Pass と記録しない**
（POLICIES.md: 実測と推測の区別、NotRun/Unsupported の正直な記録）。

## 5. 本契約で確定しない事項（Deferred）

- 各 Owner Issue の具体的 Rust API・crate 配置の最終確定 → 各 Issue の設計
- Default Budget/Dialog 操作性・第三者 Plugin AI の制御保証限界 →
  使用感 PoC / AUTX 系 Issue（GW spec §deferred 準拠）
- 実 OS/GUI/CLI/provider の統合証拠 → AUTX-08 (#267) 以降の受入 lane
