---
id: "IMPL-0Gw10TestHarnessE2E01"
type: "impl"
status: "completed"
title: "GW-10 Gateway Contract・Security・Concurrency / Recovery E2E Test Harness"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/62"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/evidence.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/oracle.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/trace.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/schedule.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/bin/gw10_report.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_harness.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_contract.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_authorization.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_project.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_concurrency.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_persistence.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_mcp.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_job.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw10_system.rs"
      - type: "file"
        path: "crates/mcp-poc/tests/common/mod.rs"
      - type: "file"
        path: "crates/mcp-poc/tests/gw10_transport.rs"
      - type: "file"
        path: "docs/impl/ir/gw-10-gateway-e2e-test-harness-evidence-gates.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/rig/mod.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/commit_gate.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/mcp_adapter.rs"
      - type: "file"
        path: "crates/mcp-poc/tests/stdio_e2e.rs"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/62"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "docs/impl/ir/gw-02-authorization-policy.md"
    - "docs/impl/ir/gw-03-context-projection.md"
    - "docs/impl/ir/gw-04-semantic-operation-preparation-effect-evaluation-linked-midi-scope.md"
    - "docs/impl/ir/gw-05-immutable-prepared-changeset-semantic-preview-approval-receipt-state-machines.md"
    - "docs/impl/ir/gw-06-authorized-commit-gate-revoke-linearization-idempotency-crash-recovery.md"
    - "docs/impl/ir/gw-08-mcp-server-adapter-trusted-local-connection-midi-3mode-e2e.md"
    - "docs/impl/ir/gw-07-nonrt-job-verified-media-generation-provenance-adapter.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/62"
  derived-by: []
  verifies:
    - "https://github.com/scarecrow173/synvelis/issues/62"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "PoC — 期待値Oracle・Controlled Scheduler・Fault Injection・Causal Trace・Evidence JSONL・Gate 集約 report"
    rejected: []
---

# GW-10 Gateway Contract・Security・Concurrency / Recovery E2E Test Harness

## Summary

GW-01〜08 の実装契約(#45)と failure matrix(#46)を、再現可能な
executable test harness + evidence 系へ翻訳する。核心の制約は
「テストだけの第二の Policy/Project 正本を作らず、期待値 Oracle は
実装と独立」—— oracle は実装の policy 構造を共有せず、decision
class(Permit/Deny/ApprovalRequired/Conflict/Unknown)で比較するのみ。

すべての GW-10 検証 case は run 毎に `EvidenceRecord` を 1 件 emit し、
`MATRIX`(canonical case→gate/owner/required の唯一の正本)に従って
`target/gw10-evidence/*.jsonl` へ追記する。`gw10-report` が per-case /
per-gate verdict に集約し、`--strict` で required case の未実行/失敗を
CI gate として検出する。`NotRun`/`Unsupported` は first-class verdict で
あり、optional(実 UX・power-loss・実 RT・cross-repo consumer)の
欠測は gate を fail させないが、記録は残る。mock と実環境の Pass は
`layer`/`mode`/`backend`/`transport` field で常に区別される。

## Context

- Issue: #62(GW-10 本体)。検証対象の契約出典は #45(GW contract)と
  #46(failure matrix)— `implements`/`verifies` の双方に記録。
- 依存 IR(`depends-on`): GW-01〜08 の全 IR
  (`docs/impl/ir/gw-01..08` 各ファイル)。
- 位置づけ: GW-01〜08 の実装契約を executable test harness + evidence
  系へ翻訳する統合検証レイヤー。

## Implementation

### 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `src/evidence.rs`(新規) | GTV-008 evidence schema。`Verdict`(Pass/Fail/Blocked/NotRun/Unsupported)、`Gate`(G0..G6/Gtv)、`Layer`(L1..L5)、`RunMode`(Mock/Real)、`fault` 注入点 vocabulary、`EvidenceRecord`(test_id・requirements・owner・gate・layer・fixture・git_revision・platform・backend・transport・mode・fault_point・expected・actual・verdict・trace・log)、`MATRIX`(canonical case→gate/owner/required の唯一の正本:GT/SEC/DATA/AC/REC/JOB/SYS/E2E/GTV 全行 + optional 行)。`Evidence::suite` は process 単位の JSONL sink、`EvidenceCase` は builder で `verify`(expected/actual 一致を assert し emit)/`close`(Blocked・NotRun・Unsupported の明示 emit)を持つ。gate は `.meta()` の引数ではなく matrix row から導出 — record が自らの帰属 gate を偽れない |
| `src/oracle.rs`(新規) | 実装と独立の expectation model。`Expected::{Permit,Deny,ApprovalRequired,Conflict,Unknown}`、grant/approval/revision/budget を独自に状態遷移させ `expect_*` が decision class を返す。`GrantPolicy`/`ProposalStore` の内部型を一切参照しない — 「第二の Policy 正本」ではなく答え合わせ専用モデル |
| `src/trace.rs`(新規) | `CausalTrace` — actor/kind/detail 付きの単調 sequence event 列。並行 driver・commit gate・oracle の event を一列に畳み込み、verdict の causal backing として record.trace へ添付 |
| `src/schedule.rs`(新規) | `ScheduledFence` を shared src へ抽出(commit_gate.rs から)。label 付き commit section を park し、driver が選んだ順序で release — revoke-before-commit / commit-before-revoke / concurrent commit を確率的 race ではなく決定的に列挙 |
| `src/bin/gw10_report.rs`(新規) | evidence JSONL を走査し gate 別 verdict(G0..G6・GTV)と case 表を出力。supplementary(non-matrix)record と missing evidence を明示列挙、`--strict` で required case の Fail/NotRun を exit 2 で落とす |
| `src/lib.rs` | `evidence`/`oracle`/`schedule`/`trace` module export |
| `tests/rig/mod.rs` | shared rig 拡張:`boot`/`restart`/`tmpdir`/`journal_payloads`/`policy_*` helpers(commit_gate.rs から共有化)、context-query helpers、`accepted`/`verdict_of`/`verdict_of_err`/`lbl`、`oracle` builders、MIDI note prepare helper。fixture_rig が `synvelis.context.query` engine まで install するよう統一 |
| `tests/commit_gate.rs` | 重複していた persistence/fence/clock helper を削除し shared rig を利用(22 tests、挙動不変) |
| `tests/mcp_adapter.rs` | fixture_rig の context engine install 化に伴い重複 install を除去(27 tests、挙動不変) |
| `tests/gw10_harness.rs` | harness 自己検証(GTV-001〜008 の基盤):fence ordering・CrashHook・ManualClock・oracle 独立性・trace 単調性・evidence roundtrip・gate 集約 |
| `tests/gw10_contract.rs` | GW10-A / G0:capability registry・typed invocation・schema version・typed ID・error envelope — GT-001〜003 |
| `tests/gw10_authorization.rs` | GW10-B / G1:SEC-01〜14 + DATA-01〜10 + GT-004(25 tests)。principal binding・human authority 詐称・project 境界・linked source scope・selection drift・suggest apply・digest 改竄・foreign approval・revision 変更後 approval・revoke 後 approval・capability 拡張・opaque plugin・metadata/PCM 分離・derived feature・snapshot pagination・失効後 chunk・foreign operation・prompt injection・不正 path・oversized・tool list 後 revoke・external transfer |
| `tests/gw10_project.rs` | GW10-C / G2:GT-005〜009 + AC-01 + AC-04(sequential)+ REC-09(cancel)+ GT-010。proposal/digest/approval state machine・preview semantic diff・accepted identity・receipt binding/replay・undo/redo 同一 gate/writer・stale-pin revision conflict・cancelled apply の Indeterminate・note-level before/after preview |
| `tests/gw10_concurrency.rs` | GW10-D / G3(前半):AC-02〜07 + REC-01〜06 + GTV-003/005。ScheduledFence で revoke/commit の両順序を確定列挙、同一 revision 並行 commit、idempotency key+fingerprint replay/conflict、disconnect 後 reconcile、approval 消費 race、commit-stage effect revalidation |
| `tests/gw10_persistence.rs` | GW10-E / G3(後半):REC-07〜12 + AC-08〜10 + SYS-04 + GTV-002。CrashHook で authorize/apply/journal 境界の crash を注入、journal replay・torn revoke・save/HEAD 順序・verified media orphan・restart 後 stale proposal・expired proposal・delegated budget durable spend |
| `tests/gw10_mcp.rs` | GW10-F / G4(in-process 側):permit/channel/kind・blind verdict・wire authority・3 モード・op handle scope・revoke 即時反映 — GTV-006(7 records) |
| `tests/gw10_job.rs` | GW10-G / G5:JOB-01〜10 + E2E-3。operation/job identity の disconnect 耐性・revoke 後 job call の deny と cancel request・completed→verified→registered→committed の state ladder・provenance・cancel・provider failure・content integrity |
| `tests/gw10_system.rs` | GW10-H / G6:E2E-1 + SYS-01〜10 + GTV-002/007/008。AI-off 手動編集/undo/recovery・grant 編集の明示 ID・approval 中 selection 変更・expired proposal・revoke/cancel 状態区別・commit≠engine 状態・RT 非主張(honest Unsupported)・schema version 拒否・restart 後 durable state・error 内の機密非露出 |
| `crates/mcp-poc/tests/common/mod.rs` | stdio_e2e.rs から抽出した shared real-transport harness:process spawn・LineIo・permit CLI・frame read/write・control TCP |
| `crates/mcp-poc/tests/gw10_transport.rs` | G4 real-transport evidence:E2E-2-suggest/approve/delegated/restart + SYS-09。real stdio + loopback TCP 上で 3 モード・boundary fail-closed・commit→restart→ledger reconcile・permit CLI を実行(mode=real で記録) |
| `crates/mcp-poc/tests/stdio_e2e.rs` | harness を common へ移動(5 tests、挙動不変) |
| `mise.toml` | `gw10`(9 suite + real-transport + `gw10_report --strict`)と `gw10-report` task |

### 設計上の主要判断

| 判断 | 理由 |
| --- | --- |
| `MATRIX` が case→gate/owner/required の唯一の正本 | test 側の `.meta()` に gate を渡させず matrix row から導出 — evidence record が自らの帰属を偽る経路を API 境界で閉じる。ad hoc case id(GW10-C-* 等)は matrix に乗らず supplementary として報告に残るため、canonical id への寄せ替えを強制 |
| oracle は実装型を共有しない | issue の核心制約 —「第二の Policy/Project 正本を作らない」。oracle は独自状態モデルで decision class のみ比較し、実装の内部型・store を一切触らない |
| 並行は race ではなく列挙 | ScheduledFence が commit section を park、driver が両順序を明示 release。thread 完了順も join で確定させる — revoke/commit の線形化不変を flaky ではなく再現可能に証明 |
| 実 transport と in-process は crate を分ける | G4 の real evidence は mcp-poc で actual stdio/TCP process を起動、in-process adapter 試験は mode=mock として正直に記録 — mock の Pass を real coverage に見せない |
| cancel/undo 等の観測を正直に記録 | cancel_apply は journal に durable verdict を残さず post-cancel commit が Indeterminate になる(REC-09 の観点)、undo は journal 経由の正規 commit として replay 数に入る等、期待を強制せず観測を分類して記録 |
| optional 行は `required=false` で NOT-RUN 記録 | 実 UX・power-loss・実 RT・cross-repo consumer は本 harness で実行不能 — 黙って欠測にせず NotRun として報告し gate も通さない(ただし required のみが gate を持つ) |
| `real_only` 行は `mode="real"` の record のみ集計 | E2E-2-*・SYS-09 は mock record を表示はするが verdict には数えない — mock の Pass が real-transport coverage にすり替わる経路を matrix 層で構造的に閉じる(GTV-003) |
| `gw10` task は report `--clean` から開始 | per-process `*-<pid>.jsonl` は run 毎に増える — 古い run の stale Pass/Fail が今回の集計を汚さないよう証拠 dir を run 境界で空にする |
| `--strict` は全 gate `Pass` のみ成功 | required case の Fail・NotRun・Blocked・Unsupported はすべて exit 2 — 「失敗していない」と「合格」を混同しない |
| GT-010 の oracle は「GW-01..05 で Revision 不変」 | #45 の criterion は preview 差分ではなく pre-commit stage が project revision を動かさないこと — prepare→preview→apply-accept 後に revision が pinned のままであることを computed actual で verify |
| `.owner()` / suite-level fixture は API から削除 | gate と owner は matrix 由来に固定し、fixture は case 毎の必須設定 — record が帰属・fixture を偽る経路を残さない |

## Related Experiments

`metadata.experiments.adopted` に記録された PoC を採用した:
期待値 Oracle・Controlled Scheduler・Fault Injection・Causal Trace・
Evidence JSONL・Gate 集約 report。これらは `src/oracle.rs`・
`src/schedule.rs`・`src/trace.rs`・`src/evidence.rs`・
`src/bin/gw10_report.rs` および `tests/gw10_harness.rs` の基盤検証
(GTV-001〜008)として実装された。`docs/impl/exp/` の実験ファイルは
存在せず、棄却実験もない。

## Validation

### 検証エビデンス

| 分類 | エビデンス |
| --- | --- |
| gate verdict | `mise run gw10` — 9 gateway suite + 1 real-transport suite + `gw10_report --strict` が exit 0。**G0..G6 + GTV の 8 gate すべて PASS**。required 全 case に evidence、missing は optional 5 行(SYS-02-ux・SYS-03-ux・SYS-05-powerloss・CONSUMER-RUVIFORM・CONSUMER-AUDIOLENS)のみで NOT-RUN、SYS-07-rt は record 付き UNSUPPORTED |
| GW-10 suite | `cargo test -p synvelis-gateway --test gw10_*` — 82 tests pass(harness 8・contract 3・authorization 25・project 9・concurrency 7・persistence 7・mcp 7・job 9・system 7) |
| real transport | `cargo test -p mcp-poc --test gw10_transport` — 4 tests pass(real stdio/TCP・process restart・permit CLI)。`stdio_e2e` 5 tests は common harness 共有後も pass |
| 全体回帰 | `mise run fmt-check`/`check`/`lint`(-D warnings, all-targets)/`test`(workspace 63 test binary)すべて green |

注記: SYS-07-rt(実 RT 負荷測定)・SYS-05-powerloss(OS 電源断)・
CONSUMER-*(cross-repo consumer 回帰)は optional 行として NOT-RUN を
正直に記録しており、gate は required 行のみで PASS 判定されている。
mock 層の PASS を実機証拠に読み替えない — layer/mode field が常に
evidence の出所を示す。

## Risks

- optional matrix 行は `required=false` で NOT-RUN/UNSUPPORTED を正直に
  記録するが、gate verdict は required 行のみで PASS 判定される —
  実 UX(SYS-02-ux/SYS-03-ux)・OS 電源断(SYS-05-powerloss)・実 RT 負荷測定
  (SYS-07-rt)・cross-repo consumer 回帰(CONSUMER-RUVIFORM/
  CONSUMER-AUDIOLENS)の実環境証拠は本 harness の範囲外であり、
  「gate PASS」はこれらを含まない。
- mock 層(in-process suite / `mode=mock`)の PASS は real transport・
  real device の証拠ではない — `layer`/`mode`/`backend`/`transport`
  field で常に区別される設計だが、evidence JSONL の読み手がこの区別を
  無視すれば mock の Pass を実機 coverage に見せかける運用リスクがある。

## Follow-ups

- SYS-07-rt(実 RT 負荷測定)・SYS-05-powerloss(OS 電源断)・
  CONSUMER-RUVIFORM/CONSUMER-AUDIOLENS(cross-repo consumer 回帰)は
  optional 行として NOT-RUN/UNSUPPORTED — 実環境での evidence 取得は
  別途必要。
- SYS-02-ux/SYS-03-ux(実 UX 系)も optional 行で NOT-RUN — real GUI
  環境での evidence 取得は別 harness/manual gate の範囲。
