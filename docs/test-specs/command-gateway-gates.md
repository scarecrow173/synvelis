---
id: "TSPEC-034X9hRV8Ud0Ty0IZllwy5"
type: "test-spec"
status: "approved"
title: "Capability / Command Gateway G0-G6 受入照合 検証仕様 (synvelis#46)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/46"
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
    - "docs/designs/command-gateway-design.md"
    - "docs/specs/command-gateway.md"
    - "docs/test-specs/command-gateway.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/command-gateway-design.md"
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
  verified-by: []
  references:
    - "evidence/sv46/command-gateway-gates-ledger.md"
    - "evidence/sv45/command-gateway-case-ledger.md"
    - "evidence/sv45/command-gateway-verification.md"
    - "docs/impl/ir/command-gateway-gates.md"
    - "docs/impl/exp/command-gateway-gates.md"
  defers: []
  deferred-by: []
---

# Capability / Command Gateway G0-G6 受入照合 検証仕様 (synvelis#46)

## Purpose

Issue #46 の G0〜G6 各 Gate について、「何を・既存のどの
実装/fixture/設計正本へ・どの Revision・どのコマンドで照合し、
どの status で記録するか」を固定する。本 slice は受入照合・
証拠台帳のみであり、新規実装・Fixture 追加・別 Gate の
PASS 宣言は行わない。G0〜G6 の headless/mock/harness/PoC
real transport レベルの受入結果と、`Product Accepted`
（実 GUI・第三者 MCP Host・実 Provider・実 OS lane・電源断・
大規模負荷・Consumer 回帰）が未達である残件を、別状態として
正直に分離して記録する契約を規定する。

## Feature

Capability / Command Gateway の G0〜G6 + GTV 各 Gate の
受入条件が、main 上の既存実装・既存 fixture・#44/#45 設計
正本へ一意に照合され、対象 Revision・実行コマンド・
`Pass`/`Fail`/`Blocked`/`NotRun`/`Unsupported` と
`Headless Verified`/`Product Accepted` の 2 軸 status を
持つ受入台帳が `evidence/sv46/` に存在し、要約が
test-spec/impl-ir/impl-exp の 3 文書に記録されること。

## Rules

### GWGT-T-R-01 Gate 網羅

- G0（Contract）・G1（Read & Policy）・G2（Proposal）・
  G3（Authorized Commit）・G4（MCP E2E）・G5（Async &
  Generated Media）・G6（Integration & Hardening）・
  GTV（harness 自己検証）の全 Gate が台帳に存在し、Issue
  #46 の Gate 判定条件（GW10-A/B/C/D/E/F/G/H 系）と
  受入条件の各項目が照合行を持つ。
- 各 Gate 判定は `mise run gw10` + `gw10_report -- --strict`
  の集約 verdict を正本とし、case 単位の fixture/oracle/
  Owner/mode/layer/status は `evidence/sv45/
  command-gateway-case-ledger.md` を正本として参照する
  （複写しない・陳腐化させない）。

### GWGT-T-R-02 二軸 Status

- 各 Gate・各受入項目は `Headless Verified` と
  `Product Accepted` を別状態で記録する。
- `Headless Verified` = headless/mock/harness/PoC real
  transport の既存証拠で契約レベル受入が実測 Pass
  （`mode`/`layer` を明記）。`Product Accepted` = 実製品
  経路（実 Window/GUI 承認 UX・第三者 MCP Client・実
  Provider・実 OS lane・実機 RT）での受入。
- 要求状態（Required/Deferred/NotApplicable、POLICIES.md
  §3）と実行結果は別軸で管理し、混同しない。

### GWGT-T-R-03 Evidence Honesty

- headless/mock/PoC real transport の Pass は mode/layer を
  明記し、実機/GUI/Provider/第三者 MCP Host 受入に繰り上げ
  ない。設計合意・Issue 起票・mock 成功・PoC transport 成功
  を `Product Accepted` と記録しない。
- `real_only` matrix row は `mode=real` record のみで
  満たされる（`gw10_report` の集約規則どおり）。
- 未実装・未検証領域（実 GUI・SYS-02-ux/03-ux・SYS-05-
  powerloss・SYS-07-rt・CONSUMER-RUVIFORM/AUDIOLENS・
  大規模負荷・第三者 Host）は `NotRun`/`Unsupported`/
  `Deferred` として理由と後続 Owner 付きで残す。
- `Pass`/`Fail`/`NotRun`/`Unsupported` は実際に実行した
  検証のみを反映する。推測と実測を区別する。

### GWGT-T-R-04 Issue 固有要求の照合

- G3 の 3 Mode（Suggest/Approve-to-Apply/Delegated Apply）・
  ProjectController Single Writer・Undo・Grant 失効競合・
  Approval 再利用防止・Idempotency/Crash Recovery が、既存
  実装のどこで実証されるかを file/test 関数単位で一意に
  照合する。Mode が Grant の Approval Policy から導出され
  Invocation 引数・caller 自己申告で昇格しないことを記録
  する。
- G4 の外部 MCP E2E・G5 の非 RT Job/Media/Provenance・
  G6 の再起動/永続 Grant/負荷/RT 非干渉/監査は、headless
  で実測可能な範囲と実機/実 Provider/実 OS として `NotRun`
  の範囲を分離して記録する。
- 切断→再接続→Operation 照合/Idempotent Retry、MCP Tool
  Discovery/Schema が権限の代替でないこと、AI Runtime と
  External MCP Access の分離、Transport 選択（Local stdio
  Helper 優先候補・未確定）を台帳に明記する。
- AUTX Consumer（#259〜267）の接合点は GW への新 Consumer
  機能として記録し、独立 Macro Engine/別 Commit Gate を
  Gate 判定に含めない。`synvelis.operation.batch` と
  `ProjectOp::Composite` は既存 GW-04 の一部として扱う。

### GWGT-T-R-05 検証コマンドと記録

- 台帳は対象 Revision・OS/toolchain・実行コマンド・exit
  code・結果を記録する。正規コマンドは `mise run
  fmt-check`・`mise run gw10`・`mise run gw10-report`・
  `mise run test synvelis-gateway`（タスク名・引数は
  `mise.toml` が正本）。MSRV 系タスクは実行しない。
- 文書・台帳のみの変更であり、コード検証は変更範囲に
  比例した軽量チェック + 当該 crate の正規 test で行う。

### GWGT-T-R-06 文書鎖整合

- test-spec→impl-ir→impl-exp→evidence の依存が front
  matter relations で一貫する。`docs/*/README.md` index は
  doc-maintenance の再生成のみで手編集しない。
- doc-status lint・impl audit・route graph が実行・記録
  され、blocker が残らない。

## Case Map（概要 — Gate 別の詳細照合は evidence/sv46/command-gateway-gates-ledger.md）

| Gate | 主な照合先 | Headless Verified | Product Accepted |
|---|---|---|---|
| G0 Contract | `contract.rs`/`gw10_contract.rs`/`schema_compat.rs`・GT-001〜003・SYS-08 | Pass | NotRun（#114/実機 lane） |
| G1 Read & Policy | `authorization.rs`/`gw10_authorization.rs`(SEC/DATA)・context.rs | Pass | NotRun |
| G2 Proposal | `prepare*.rs`/`proposal*.rs`/`gw10_project.rs`(GT-005〜009) | Pass | NotRun（#114） |
| G3 Authorized Commit | `commit.rs`/`commit_gate.rs`/`gw10_{project,concurrency,persistence}.rs`・REC/AC・3 Mode（mock+PoC real/L4） | Pass | NotRun（#114） |
| G4 MCP E2E | `mcp*.rs`/`gw10_mcp.rs`/`mcp-poc/tests/gw10_transport.rs`(E2E-2-* real/L4) | Pass（PoC real・第三者 Host ではない） | **NotRun**（第三者 MCP Host・Transport 未確定） |
| G5 Job/Media | `job*.rs`/`media.rs`/`provenance.rs`/`gw10_job.rs`(JOB/JM/E2E-3) | Pass | NotRun（実 Provider 別設計） |
| G6 Integration | `gw10_system.rs`/`gw10_persistence.rs`・E2E-1・SYS/REC/AC | Pass（required case） | NotRun（実 GUI/OS lane/電源断/大規模負荷/CONSUMER-*） |
| GTV | `gw10_harness.rs`/`evidence.rs`/`oracle.rs`/`schedule.rs`/`trace.rs` | Pass | — |

## Examples

### EX-1: G4 第三者 MCP Host 未受入（NotRun）

- **Given**: `mcp-poc/tests/gw10_transport.rs::gw10f_*_real`
  4 件が real stdio + 認証 loopback transport・real
  `DirectoryBundleStore`+`commit.journal`+`FilePermitStore`
  で PASS。
- **When**: G4 の Product Accepted を評価。
- **Then**: real transport の client は本 repo の `mcp-poc`
  test harness であり第三者 MCP Host（実 Claude/Cursor 等）
  ではないため `NotRun` とし、Host 互換・実認証・Transport
  最終決定は #46 G4 拡張・#114 系後続へ送る。
- **Status**: Headless Verified = Pass（PoC real/L4）、
  Product Accepted = NotRun。

### EX-2: SYS-07-rt budget 未実測（Unsupported）

- **Given**: `gw10_system.rs::sys07_rt_boundary_is_recorded_
  not_claimed` は RT boundary を記録するのみで budget を
  実測しない。
- **When**: `gw10_report -- --strict` を実行。
- **Then**: SYS-07-rt は `Unsupported`（required=false）
  として報告され G6=PASS を保持。実機 RT budget 実測は
  実機 lane の後続であり、headless 記録を Pass にしない。
- **Status**: Unsupported/NotRun — required=false。

### EX-3: G3 3 Mode の PoC real 実証 vs Product 未受入

- **Given**: Suggest/Approve-to-Apply/Delegated Apply が
  in-process mock（`proposal.rs`/`authorization.rs` 系）と
  PoC real transport（`gw10f_three_modes_over_real_
  transports`・`gw10f_delegated_apply_commits_and_survives_
  restart_real`）の両方で実動作。
- **When**: G3 の各 Mode を照合。
- **Then**: Mode は Grant の Approval Policy から導出され
  wire/自己申告で昇格しないことを併記し、実 Window の human
  承認 UX は #114 の `NotRun` として分離する。
- **Status**: Headless Verified = Pass（mock/L2-L3 +
  real/L4）、Product Accepted = NotRun。

### EX-4: AUTX Consumer は Gate 判定に含めない

- **Given**: AUTX #259〜267 は GW への新 Consumer 接合点
  （`synvelis.operation.batch`/`ProjectOp::Composite` は
  既存 GW-04 の一部）として記録する。
- **When**: G0〜G6 の Gate 判定を評価。
- **Then**: 独立 Macro Engine・別 Commit Gate・第二 Writer
  を AUTX 側に要求せず、#263 Atomic Macro の ID Binding の
  み GW-04 既存変更として特定済みであることを記録する。
  AUTX 自体の受入は AUTX Issue の scope であり本台帳の
  Gate 判定対象外。
- **Status**: 記録のみ — Gate 判定に含めない。

## Guarantees

- G0〜G6・GTV の全 Gate が台帳に存在し、Issue #46 の受入
  条件各項目が照合行を持つ。
- `Headless Verified`/`Product Accepted` が別状態で記録
  され、headless/mock/PoC real の Pass が製品受入に
  繰り上げられない。
- 未達残件（実 GUI・第三者 Host・実 Provider・実 OS lane・
  電源断・大規模負荷・Consumer 回帰・Transport 未確定）が
  理由と後続 Owner 付きで残る。
- AUTX 接合点・Transport 未確定・Discovery≠権限・AI
  Runtime/External MCP 分離が明記される。

## Non-goals

- 実装コードの作成・変更（`crates/synvelis-gateway/`・
  `synvelis-project/`・`synvelis-app/`・`mcp-poc/` は
  参照のみ）。
- 新規実装・Fixture 追加・別 Gate の PASS 宣言。
- `docs/specs/command-gateway.md` 等の #45 正本 docs の
  編集（参照のみ）。
- 実 GUI・第三者 MCP Host・実 Provider・実 OS・電源断・
  大規模負荷・Consumer 回帰での受入。
- AUTX 文書の作成・編集、CLI mutating invocation の公開。

## Risk

- **R-GT-1 status の誤昇格**: PoC real transport の Pass を
  `Product Accepted` や第三者 Host 受入と記録すると実環境
  受入と混同される。→ 2 軸 status と mode/layer/required/
  real_only を明記し、未達は後続 Issue へ送る。
- **R-GT-2 case ledger との乖離**: Gate 照合が sv45 case
  ledger と矛盾する。→ case 単位の正本は sv45 ledger と
  し、本台帳は Gate 受入条件への照合に限定して参照する。
- **R-GT-3 Transport/Host の確定扱い**: Local stdio
  Helper+認証 IPC を「決定済み」または第三者 Host 実測済み
  と記録すると Issue #46 の「まだ最終決定しない」および
  G4 拡張を歪める。→ 「第一候補・未確定」「PoC real・
  第三者 Host ではない」を固定語彙で記録する。
- **R-GT-4 AUTX の Gate 混入**: 独立 Macro Engine/別 Commit
  Gate を G 判定に含めると AUTX scope と GW 受入が混線
  する。→ 接合点の記録に限定し Gate 判定対象外と明記する。
