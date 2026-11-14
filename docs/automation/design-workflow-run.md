# Workflow Plan/Run 状態遷移・Fault/Recovery 契約（AUTX-05/AUTX-06 境界）

一次正本: `crates/synvelis-gateway/src/job.rs`（`JobPhase`/`CancelLink`/
`CancelResponse`/`JobPort`）、`proposal.rs`（Proposal/Approval/Apply 状態機械）、
`commit.rs`（CommitGate journal/recovery）、`outcome.rs`（`GatewayOutcome`/
`GatewayError::Indeterminate`）。Workflow Coordinator はこれらを**参照**し、
第二正本を持たない。

## 1. Workflow が所有するもの / 所有しないもの

| Workflow が所有 | Workflow が所有しない（参照のみ） |
| --- | --- |
| `WorkflowDefinition` → 型付き `Plan` への lower | Project の revision/CAS/Undo（CommitGate） |
| Run/Step の状態機械と resume cursor | Job の実行 phase（provider/`JobPort` が正本） |
| journal（step 境界の記録）と recovery 手順 | Grant/Approval の状態（GW-02/05/06 が正本） |
| cancellation request の発行と追跡 | Proposal/Apply の状態（proposal store が正本） |
| plan の署名・revision・idempotency key | Capability Registry・Schema Registry |

Workflow は Gateway・CommitFence・Project mutex を**保持も再帰呼出もしない**。
各 step は step 境界で逐次的に Gateway 呼出を行い、戻り値
（`GatewayOutcome`/`ErrorEnvelope`/`JobStatus`/`ProposalRef` 等）を journal
に記録する。Project/Job/Grant/Approval の状態を journal に**複写して
正本化しない**（参照 id と観測値のみ）。

## 2. Plan step kind（確定）

[design-atomic-macro.md](design-atomic-macro.md) §4 の 7 kind:
`Query` / `AtomicBatch` / `JobSubmit` / `Await` / `Result` / `If` /
`BoundedLoop`。`BoundedLoop` は反復上限を plan に必須記述し、
無制限 loop・再帰 trigger を型/検証で拒否する。cycle（step が自分自身・
祖先を trigger する定義）は plan 検証で拒否する。

## 3. Run / Step 状態機械

### Run 状態

```
Created → Ready → Running ⇄ Waiting → Approval? → Completed
                │          │          │
                │          ├→ CancelRequested → Completed|Failed
                │          ├→ Failed
                │          └→ Indeterminate →（reconcile）→ Running|Failed
                └→ Failed（plan/検証失敗）
```

| 状態 | 意味 | 終端性 |
| --- | --- | --- |
| `Created` | Run record 作成済み。plan は検証済みだが未開始 | 非終端 |
| `Ready` | 開始条件（grant/snapshot/job slot）確認済み | 非終端 |
| `Running` | 少なくとも 1 step が実行中または直列進行中 | 非終端 |
| `Waiting` | `Await`/外部待ち。job/project は外部正本が進行 | 非終端 |
| `Approval` | `ApprovalRequired` outcome を受け trusted approval 待ち | 非終端 |
| `CancelRequested` | cancel を発行済み。provider/commit の確認待ち | 非終端 |
| `Completed` | 全 step が証拠付きで完了 | **終端** |
| `Failed` | typed error で中断（retryable は plan が判断） | **終端** |
| `Indeterminate` | 永続化境界を跨ぎ結果不明。reconcile 必須 | **非終端**（reconcile 後に遷移） |

遷移規則:
- `Indeterminate` は**証拠なしに `Completed`/`Failed` へ遷移しない**。
  reconcile（正本の再照会）でのみ抜ける。
- `CancelRequested` から `Completed` への遷移は「cancel が間に合わず
  job/commit が完了した」場合のみ許し、その場合は実際の正本状態を
  結果として記録する（`CancelResponse::Settled` と同じ正直さ）。
- `Approval` は期限切れ・revoke・proposal invalidation で `Failed` または
  `Waiting`（再 approval 要求）へ遷移しうる。

### Step 状態

`Pending → Ready → Running → {Completed | Failed | Waiting | Approval |
CancelRequested | Indeterminate}`。Step の `Completed` は「その step が発行
した GatewayOutcome/JobStatus/ProposalRef が journal に記録された」ことを
意味し、下位の project commit・job 完了・media verify・engine apply・save
が個別に完了したことを**含意しない**（それらは各正本の状態）。

## 4. journal と resume cursor

- journal は step 境界で append-only に記録する: `{run_id, step_id, seq,
  plan_revision, capability_ref, outcome_kind, ref_ids, digest}`。
- resume cursor は「最後に journal へ確定記録された step」。再開時は
  cursor の次の step から再実行し、**既に正本へ反映済みの step を再送しない**。
- 冪等性の三層:
  1. **Plan 冪等**: `plan_revision` + `run_id` + `idempotency_key` で
     同一 plan の重複 Run 起動を `DuplicateRequest`/`IdempotencyConflict`
     で拒否。
  2. **Step 冪等**: 各 Gateway 呼出に `IdempotencyKey` を付与し、
     crash-retry での二重 commit/job submit を防ぐ。
  3. **正本照合**: journal に結果が無いが正本に副作用がある可能性がある
     場合は `Indeterminate` とし、reconcile で照合してから進む。

## 5. Fault / Recovery Matrix

| # | 障害 | 検出 | 契約上の回復 | 禁止 |
| --- | --- | --- | --- | --- |
| F-1 | journal 記録前に Project commit が成功 | commit 後・journal 前に crash | resume 時に CommitGate の journal/recovery が `Indeterminate`/`Committed` を報告 → Run を `Indeterminate` → reconcile で `Completed` 確定 | commit の再送（二重 commit 禁止） |
| F-2 | job 完了後・結果 record 書込前に crash | `Await` の再照会で provider が `Completed` を返す | provider status を正本として journal に記録し先へ進む | job の再 submit |
| F-3 | network timeout（adapter↔provider/coordinator） | 応答なし | `CancelLink::Unconfirmed` / `JobPhase` 不明のまま `Indeterminate`。retry は `ReconcileState` | timeout を失敗/成功と推測 |
| F-4 | cancel 不確実（provider 無応答） | `CancelResponse::Unreachable` → `CancelLink::Unconfirmed` | cancel request を再送せず status 再照会を継続。`JobPhase::Cancelled` は provider 確認後のみ | request だけで `Cancelled` と記す |
| F-5 | provider/coordinator 再起動 | job record 喪失または phase 不明 | `JobPort` が job を `Indeterminate` 相当で報告 → Run `Indeterminate` → provider journal/receipt と照合 | 存在しない job を `Failed` と断定 |
| F-6 | Grant revoke / expiry / PolicyEpoch 変更 | 各 step・commit 境界での再評価 | `Denied`/`ApprovalInvalid`/`Consistency` 系 typed error → Run `Failed`（retryable なら plan 判断） | stale grant での継続・commit |
| F-7 | Proposal/Approval の期限切れ・invalidated | `ProposalUnavailable`/`ApprovalInvalid` | 当該 step を `Failed` または再 prepare/approval へ。commit 済みなら影響なし | dead proposal の apply 再試行 |
| F-8 | ExpectedRevision 競合 | `RevisionConflict` | `AfterCorrection`: 最新 revision で plan を再 compile/再 prepare | 強制上書き commit |
| F-9 | 外部副作用の非原子性 | job/provider 側で完了したが project commit 未了（または逆） | 各正本の状態を個別に報告。Run は `Indeterminate` → 両側照合 → 人間/plan 判断 | 「全部成功/全部失敗」の推測 |
| F-10 | plan 自体の改竄・版ずれ | `plan_revision`/署名/digest 不一致 | Run 開始拒否（`Validation`/`Consistency`）。実行中は journal の `plan_revision` と照合 | 旧 plan の黙った継続 |

原則: **未知・不確定は証拠なしに成功へ昇格しない。** resume/reconcile は
常に正本（CommitGate journal・JobPort・Proposal store）への照会を伴い、
副作用（commit・job submit・外部呼出）の重複を 0 にする。

## 6. 接続寿命との独立

- GUI close・MCP disconnect・CLI process 終了は **Run/Job/Proposal の
  所有権を変更しない**。Run は host 内の coordinator が継続または
  durable journal から resume 可能である。
- 再接続後は `run_id` で状態再照会（`ReadOperationResult` 相当）でき、
  `CancelRequested` の発行・結果取得・承認要求の提示が可能。
- CLI の `--detach`/`--follow` の別はあっても、**実行の正本は接続ではなく
  coordinator + Gateway + provider** である。

## 7. 非 RT 制約

- Workflow/Script/CLI の実行は**非 RT**。RT callback（audio thread）で
  plan 実行・VM 実行・I/O・allocation を行わない。
- `RuntimeControl` domain の capability は transport 等の RT 隣接制御で
  あり、Workflow step としては「制御要求の発行」まで。RT 内部状態を
  Workflow が直接触らない。

## 8. 検証可能な受入（AUTX-05/AUTX-06 向け）

- F-1〜F-10 の各 fault を fake adapter/coordinator で注入し、Run が
  `Indeterminate` を経て証拠付きで終端へ到達すること（二重副作用 0）。
- resume cursor が「journal 確定済み step の再送」をしないこと。
- `BoundedLoop` 上限超過・cycle 定義が plan 検証で拒否されること。
- 接続切断後に `run_id` で状態再照会・cancel・結果取得ができること
  （fake/fixture で契約試験。実 OS/GUI 証拠は AUTX-08 (#267) の責務）。
