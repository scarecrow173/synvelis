---
id: "IMPL-0Gw06CommitGateRevok01"
type: "impl"
status: "completed"
title: "GW-06 Authorized Commit Gate・Revoke Linearization・Idempotency / Crash Recovery PoC"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/58"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/journal.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/durable_policy.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/commit_gate.rs"
      - type: "file"
        path: "docs/impl/ir/gw-06-authorized-commit-gate-revoke-linearization-idempotency-crash-recovery.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/grant.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/id.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/proposal.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/request.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/58"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "https://github.com/scarecrow173/synvelis/issues/17"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "docs/impl/ir/gw-02-authorization-policy.md"
    - "docs/impl/ir/gw-04-semantic-operation-preparation-effect-evaluation-linked-midi-scope.md"
    - "docs/impl/ir/gw-05-immutable-prepared-changeset-semantic-preview-approval-receipt-state-machines.md"
    - "docs/impl/ir/vs-c-staging-writer-journal-crash-recovery.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "https://github.com/scarecrow173/synvelis/issues/17"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/59"
    - "https://github.com/scarecrow173/synvelis/issues/62"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/58"
  derived-by: []
  verifies:
    - "https://github.com/scarecrow173/synvelis/issues/58"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "PoC A — 単一 ordered commit+policy journal(recovery ordering は seq で完全復元)"
      - "PoC B — independently durable FilePolicyStore(policy log と commit journal の cross-LSN 記録)"
    rejected: []
---

# GW-06 Authorized Commit Gate・Revoke Linearization・Idempotency / Crash Recovery PoC

## Summary

GW-05 の prepared changeset + approval receipt を消費し、commit 時点の
最終認可(commit-stage `evaluate_effects`・`ApproveToApply` receipt 再検証・
delegated apply budget の atomic spend)を経て、既存 `ProjectController::apply`
単一 writer で mutation を commit する `CommitGate` を実装する。
「Gateway に第二 Writer を作らない」制約の下、revoke・commit・save・
undo/redo を同一 `CommitFence` で直列化し、durable journal(FileJournal)に
outcome / policy lifecycle / saved-head を記録して crash recovery を
冪等 ledger 付きで検証した。

## Context

- Issue: #58(GW-06 本体)。要件の出典は #45・#46(failure matrix)・#17。
- 依存 IR(`depends-on`): GW-01 gateway contract・GW-02 authorization policy・
  GW-04 semantic operation preparation・GW-05 prepared changeset / approval
  receipt(`docs/impl/ir/gw-01..05` 各 IR)・VS-C staging writer journal
  crash recovery(`docs/impl/ir/vs-c-staging-writer-journal-crash-recovery.md`)。
- 後続: `blocks` issue #59(MCP adapter)・#62(GW-10 E2E harness)が本実装の
  `CommitGate`/journal を commit boundary・durable verdict として利用する。

## Implementation

### 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `journal.rs`(新規) | `FileJournal` — magic `0x4A4A`(`"JJ"`)+version+seq+len+payload+CRC32 の 16B header フレーム。append は `sync_data` で永続化、open は replay scan で torn tail / bad magic / checksum 破損 / seq gap を検出し intact prefix を保持した上で破損末尾を切詰める(VS-C 確立済み codec の GW 移植)。`StopReason`/`OpenReport` で修復詳細を返す |
| `commit.rs`(新規) | `CommitGate` — `commit`/`commit_hooked`(fault injection)/`commit_manual`(human のみ)/`undo`/`redo`/`save`/policy admin(`issue_*`/`revoke_*`/`advance_epoch`)/`open`(recovery)/`reconcile`。`CommitFence`+`MutexFence` — commit・revoke・save・undo/redo を同一 section で直列化。`CommitHooks`+`GateCrash` — authorize後/apply後/journal後の crash fault injection(commit は `&mut dyn` で受ける)。`Ledger` — `(principal,project,IdempotencyKey)` で fingerprint→outcome を記録、fingerprint mismatch は `IdempotencyConflict`、同一は durable outcome を replay。journal payload — `Committed`(snapshot envelope・観測した `policy_lsn` 内蔵)/`Rejected`/`Conflicted`/`Indeterminate`/`Policy`/`Saved`。recovery は最新 intact `Committed` の snapshot を `ProjectController` へ restore し、`Committed` を終端扱いする replay(後続 `Indeterminate` で上書きしない)、policy lifecycle・delegated consumption・idempotency ledger を再生し `RecoveryReport` に stop reason/truncated bytes/各 replay 数/saved head/restored revision/journal由来可否を報告。crash 意味論 — authorize 後・apply 後・journal 前は `Indeterminate`、durable `Committed` が在れば restart で committed 復元 |
| `durable_policy.rs`(新規) | `FilePolicyStore` — PoC B。independently durable policy journal に grant/task/permit/epoch lifecycle・consumption・`PolicyLogEvent::Fence{commit_seq, policy_lsn}` marker を記録し restart で完全 replay。commit 側は `CommitRecord.policy_lsn` に fence 内で観測した policy seq を記録し、A(単一 ordered journal)と B(独立 log+cross-LSN)の recovery ordering を比較可能にする。durable admin 配下では gate replay が policy lifecycle/consumption を再適用しない(独自 log が権威 — 二重消費しない) |
| `grant.rs`/`store.rs`/`request.rs`/`id.rs`/`project id.rs` | journal replay 用の `Deserialize` 広範追加(`Principal` は bounded ctor 経由で validation 保持、u128 ID は `Deserialize` 実装)、`PolicyAdmin` 拡張 trait(`InMemoryPolicyStore`/`FilePolicyStore` に実装、gate の fenced admin が呼ぶ)、`ApplyRecord.request`(settle 経路が operation request を再構成せず報告できるよう保持)、`CommitId` |
| `lib.rs` | `commit`/`journal`/`durable_policy` module + public 型 export |
| `tests/commit_gate.rs`(新規) | 22 テスト — AC-01..10・REC-01..12 matrix。`ScheduledFence`(thread-local label で section を park/resume、revoke↔commit 順序を決定的に列挙)、`CommitHooks` crash injection、real `FileJournal`/`DirectoryBundleStore`/`FilePolicyStore` への fault injection(torn tail・kill→reopen) |

### 設計上の主要判断

| 判断 | 理由 |
| --- | --- |
| 最終認可 + `ProjectController::apply` は同一 fence section | fence 外で認可してから apply すると revoke が割り込み「認可したのに revoked grant で commit」が起き得る — commit-stage 再評価は fence 内・現行 store で実施 |
| revoke と commit は fence 進入順で全順序 | revoke-first → commit は revoked grant を観測して deny。commit-first → mutation は残り revoke は undo しない(両ケースを ScheduledFence で決定的に検証) |
| `ApproveToApply` は commit 段階で `ApprovalRequired` を返す | `GrantPolicy` の契約通り — gate は consumed receipt を fence 内で verify-冪等再確認して通す(二度消費しない・受理 attempt と結合) |
| idempotency identity は `(principal, project, key)`+fingerprint | request/operation ID は retry 毎に変わるため fingerprint から除外 — capability/project/target/args/consistency/task/grant が同一なら durable outcome を replay、異なれば `IdempotencyConflict` |
| transport 復帰は commit 確認にならない — journal persistence が権威 | `Committed` record が sync_data で永続化されるまで成功を返さない。apply後 journal 前 crash は `Indeterminate` で caller は `reconcile` で実 outcome を引く |
| policy admin の durable record は mutation 前に append | journal が確認した revoke は restart を生き延びる — record があって mutation に落ちなかった場合は replay で policy lifecycle を再生して辻褄を合わせる |
| undo/redo も gate+fence+journal 経由 | history 遷移自体が新 revision の commit — revoke との直列化を逃れると undo が revoked-commit を「再 commit」してしまう。post-state を `Committed` record として記録 |
| `Committed` は replay で終端 | 同 operation に後続 `Indeterminate` があっても durable commit を覆さない(after_journal crash が残す2レコード列の解釈) |
| HEAD/snapshot だけから commit を推測しない | journal に outcome の無い apply は `Indeterminate` — project 側の実 state と outcome 権威を分離(REC-09/11) |
| PoC A vs B の LSN 記録を commit record へ埋め込む | `CommitRecord.policy_lsn`(観測した policy 位置)+ policy log 側 `PolicyLogEvent::Fence{commit_seq,policy_lsn}` — B(独立 log)でも crash 点の ordering を事後比較可能 |
| `IdGenerator: Send` 追加 | gate は mutex 背後の単一 writer を複数 worker thread から share する必要がある — `Box<dyn IdGenerator>` が非 Send では `CommitGate` が thread を跨げない。workspace 唯一実装 `CounterIdGenerator` は既に Send で無害 |

## Related Experiments

`metadata.experiments.adopted` に記録された 2 つの PoC を実施し、いずれも
採用した(棄却なし):

- **PoC A — 単一 ordered commit+policy journal**: recovery ordering を
  journal seq で完全復元する構成。`FileJournal` + `CommitGate` として実装。
- **PoC B — independently durable FilePolicyStore**: policy log と commit
  journal の cross-LSN 記録(`CommitRecord.policy_lsn` +
  `PolicyLogEvent::Fence`)で A/B の recovery ordering を比較可能にした構成。
  `durable_policy.rs` として実装し、durable admin 配下では独自 log が権威。

これらは `docs/impl/exp/` の実験ファイルではなく本 IR に inline 記録された
PoC であり、比較結果は「設計上の主要判断」「Review-protocol 実施結果」の
表に反映されている。

## Validation

### 受入条件との対応

| AC | 対応 |
| --- | --- |
| AC-01 authorized apply→commit→undo が既存単一 writer を通る | `CommitGate::commit` は `ProjectController::apply` のみを呼び、undo/redo も gate+fence 経由 — 第二 writer/store/undo engine なし。`delegated_apply_commits_spends_once_and_undoes`/`manual_commit_is_human_only_and_journaled` |
| AC-02 revoke-before-commit は deny | fence 順 = journal seq。`revoke_confirmed_before_commit_denies` が durable record 順(RevokeGrant→Rejected)まで検証 |
| AC-03 commit-before-revoke は commit 成立(revoke は undo しない) | `commit_confirmed_before_revoke_stands` — commit seq < revoke seq、mutation 残留、後続 undo は別 commit として可能 |
| AC-04 concurrent commit は単一 writer conflict | `concurrent_commits_single_writer_conflict` — 並列 apply/manual commit、一方 `Committed`・他方 `RevisionConflict`→`Conflicted` outcome、`reconcile` で各自 durable verdict 取得 |
| AC-05 commit-stage revalidation(scope/effects) | `commit_stage_evaluation_denies_out_of_scope_effects` — fence 内 `evaluate_effects` が scoped 外 write を deny。linked-source 派生証拠は GW-04 |
| AC-06 idempotency retry は durable outcome を replay | `keyed_commit_retry_replays_the_receipt` — 同一 fingerprint で同一 receipt/outcome、project は一度だけ mutate |
| AC-07 key 再利用・別 payload は conflict | `wire_level_keyed_conflict_rejects_different_payload` — `IdempotencyConflict` |
| AC-08 durable commit record は crash 後も復元 | `crash_after_durable_record_recovers_committed`/`restart_recovers_committed_state_and_verdict` — snapshot から state・verdict・ledger を再生 |
| AC-09 awaiting approval / expired proposal は deny | `apply_awaiting_approval_is_denied_not_indeterminate`(AwaitingApproval は settled 扱いしない)/`expired_proposal_rejects_at_commit`/`stale_proposal_can_never_commit_after_restart` |
| AC-10 delegated apply budget は commit 認可で atomic spend | `delegated_apply_commits_spends_once_and_undoes` — fence 内 commit-stage で1回のみ消費、commit record が消費権威で replay 復元 |

### REC matrix(#46 Recovery 条件・GW-06 該当分)

| Case | 内容 | 判定 | 証拠 |
| --- | --- | --- | --- |
| REC-01 | revoke は durable に確認される | Pass | `durable_revoke_survives_restart` — restart で revoked のまま |
| REC-02 | revoke↔commit ordering は durable order | Pass | `revoke_confirmed_before_commit_denies`/`commit_confirmed_before_revoke_stands` が journal seq で検証 |
| REC-03 | torn revoke tail は revoke を確認しない | Pass | `torn_revoke_tail_never_confirms_a_revocation` — truncate 後 grant は復活 |
| REC-04 | authorize 後 crash は commit なし | Pass | `crash_after_authorize_leaves_nothing_committed` |
| REC-05 | apply 後 journal 前 crash は Indeterminate | Pass | `crash_after_apply_reports_indeterminate_and_no_partial_state` |
| REC-06 | durable record 後 crash は committed 復元 | Pass | `crash_after_durable_record_recovers_committed` |
| REC-07 | delegated consumption は replay 復元・二重消費なし | Pass | `restart_recovers_committed_state_and_verdict`(restart で consumed==1)/`crash_after_authorize_leaves_nothing_committed`(journal 未達なら復元しない)/`durable_policy_admin_never_double_counts_consumption`(PoC B: 独自 log 権威で二重消費しない) |
| REC-08 | policy lifecycle は restart 越しに replay | Pass | `file_policy_store_replays_lifecycle`/`durable_revoke_survives_restart` |
| REC-09 | outcome 権威は journal(HEAD 推測しない) | Pass | `restart_recovers_committed_state_and_verdict` — journal 由来の verdict のみ reconcile 可能 |
| REC-10 | save は commit 順に HEAD publish | Pass | `saves_publish_head_in_commit_order` — fenced save、HEAD は古い commit で巻戻らない |
| REC-11 | Indeterminate は reconcile 対象を残す | Pass | `crash_after_apply_reports_indeterminate_and_no_partial_state`/`stale_proposal_can_never_commit_after_restart` |
| REC-12 | journal repair 詳細を報告 | Pass | `RecoveryReport{stop,truncated_bytes,unreadable,replayed_*}`、torn tail テストが検証 |

### サポート対象外(明示)

- fsync が嘘をつく storage、検出不能な silent bit rot は**サポート対象外** — CRC32 + seq gap + torn-tail 検出の範囲を `journal.rs` doc と本レコードで明記。bit rot が header/payload を破壊すれば replay はそこで stop するが、破壊が検出不能な形(内容改竄し CRC まで再計算)は脅威モデル外
- journal の GC/checkpoint・複数 project の multiplex は PoC 範囲外

### 実行した検証(証拠)

- 環境: Windows、rust 1.99.0(mise pin)、GW-01〜05 merge 済み main + 本変更
- `mise run fmt` / `mise run fmt-check` — Pass
- `mise run check` — Pass
- `mise run lint`(`cargo clippy --workspace --all-targets -- -D warnings`)— Pass
- `mise run test` — Pass、workspace 全 test binary 緑・0 failure。
  `tests/commit_gate.rs` 22 テスト pass、gateway lib 単体(journal codec/fingerprint/append-rollback)5 テスト pass、
  既存 authorization/proposal/recording/workspace/render/project/e2e 全 suite pass

### Review-protocol 実施結果

独立 reviewer 2 系(A: 受入条件トレース / B: adversarial・failure)を投入し、指摘は
コード実測で検証した。実装中に確定し修正済みの主要 findings:

| 指摘 | 判定 | 対応 |
| --- | --- | --- |
| commit 段階で `ApprovalRequired` を無条件 reject していた — ApproveToApply の全正当 commit が死ぬ | 確定(B) P0 | `receipt_verified` 追跡 — fence 内で consumed receipt を verify-冪等再確認した場合のみ Proceed。`approve_to_apply_commits_only_with_verified_receipt` |
| `AwaitingApproval` の apply が settled として `Indeterminate` 経路に乗る | 確定(B) P1 | commit 前に `Denied`(「apply awaits an approval decision」)を返す。`apply_awaiting_approval_is_denied_not_indeterminate` |
| replay で後続 `Indeterminate` が durable `Committed` を上書き | 確定(B) P1 | `replay_noncommit` は `Committed` を終端扱いで保持。`crash_after_durable_record_recovers_committed` |
| `FilePolicyStore::mutate` の閉包が no-op で inner store が更新されない | 確定(A) P1 | mutation event で inner を実更新 + durable record。`file_policy_store_replays_lifecycle` |
| `Box<dyn IdGenerator>` 非 Send で `CommitGate` が worker thread を跨げない | 確定(B) P1 | `IdGenerator: Send` 追加(単一 writer を mutex 背後で share する構造要件)。scheduled テストが multi-thread fence で検証 |
| ScheduledFence が unlabeled section も park して main thread を永久 block | 確定(A) P1 | label 付き section のみ park、unlabeled は本番同様即実行 |
| `ApproveToApply` commit path が wire store と durable store を分離し grant が wire 側に無い | 確定(A) P2 | PoC-B テストは両 store に install — wire 経路と commit 経路が同一 policy 観測を共有 |

| `OperationId` 衝突 — manual/undo/redo commit が revision 由来の synthetic id を journal し、replay の `by_operation` が真の apply op と衝突し得る(終端 Committed guard で Conflicted が誤って drop) | 確定(A) P1 | `CommitRecord.operation`/`CommitReceipt.operation` を `Option<OperationId>` 化 — 非 apply commit は `None` で ledger 不登録。wire op の reconcile と混線しない |
| PoC-B durable admin で gate replay が consumption/lifecycle を再適用 — `note_consumption` が再記録して二重消費 | 確定(A) P1 | `admin_is_durable` で durable store は独自 log が権威として replay スキップ。`durable_policy_admin_never_double_counts_consumption` で回帰検証 |
| fingerprint 単体テストが stub(`payload_fingerprint` を未実行) | 確定(A) P2 | 実 request を構築し operation/request id 盲腸・args drift conflict を検証 |
| 非 commit outcome が journal append 前に ledger settle(同 process retry は durable 未確認の outcome を見得る) | 確定(A) P2 | journal restart 権威の下で動作一貫 — Risks に明記 |
| テストコメントの AC/REC numbering drift | 確定(A) P2 | canonical numbering(#46/#58)へ realign |
| `CommitHooks` doc が crash 後の Indeterminate record を「nothing recorded」と過剰記述 | 確定(A) P2 | best-effort append の存在と replay 終端性の目的を明記 |
| `FileJournal::append` が write/sync 失敗時に partial bytes を残置 — 後続の成功 append が torn bytes の後ろに積まれ replay が確認済み record まで失う | 確定(B) P0 | append 開始 offset を記録し失敗時 `set_len(start)`+ seek-to-end で rollback。`append_fail_after` fail-injection seam + `failed_append_rolls_back_so_the_next_record_stays_replayable` で回帰検証 |
| commit-stage 再検証が prepared capability floor/after-state を捨て `apply` cap のみ再構成 — apply grant が inner op capability・write scope を未包含でも commit 可能(SEC-13/06 迂回) | 確定(B) P1 | fence 内で `authorize_agent` 再認可し `effects.capabilities ⊆ grant.capabilities` と `GrantPolicy::coverage`(after-state 全域)を検証 |
| durable policy mutation (`issue_*`/`revoke_*`/`advance_epoch`/`note_consumption`) が `let _ =` で IO 失敗を黙殺 | 確定(B) P1 | `PolicyAdmin` を `io::Result` 返しへ変更し全経路で伝播。`FilePolicyStore::try_consume_apply` は consume record 失敗時 store を poison(再起動時 refund を許さない)。`advance_epoch` は失敗時 epoch 0 を返さず propagate |
| project apply 成功後に durable commit record 失敗 → durable outcome なしの生きた mutation が残り後続 mutation も継続 | 確定(B) P1 | `CommitGate::diverged` flag — post-apply record 失敗(commit/manual/history 全経路)で poison、`open` re-baseline まで全 project mutation を `Indeterminate` で拒否 |
| `commit`/`undo`/`redo`/`save` が caller 未認証 — manual/history/HEAD publish を unauthenticated caller も起動可能 | 確定(B) P1 | 全経路に `CallerIdentity` 引数 + human-authority 検査。history record は実 caller を journal |
| commit replay の ledger key が `(tag|principal|project|key)`、noncommit replay が `(tag|principal|key)` — restart 後 keyed retry が outcome 種別で挙動不一致 | 確定(B) P1 | key format を 4-segment に統一 |
| `controller.load` の失敗を `let _ =` で黙殺 — 破損 HEAD が fresh project と誤認され得る | 確定(B) P1 | `store.head()` で存在判定してから load — HEAD あり破損は propagate、HEAD なしのみ fresh と扱う |
| journal header の reserved byte を未検証 | 確定(B) P2 | `StopReason::BadReserved` 追加(CRC 範囲内だが明示検証) |
| commit fingerprint が `input_schema` を含まず gateway wire fingerprint と scope 不一致 | 確定(B) P2 | `schema` を fingerprint map に追加 |
| replay 済み `Rejected` が wire 上 `Denied{Commit}` に正規化され元の error variant を失う | 確定(B) P2 | outcome(reason)は保持、wire-variant 忠実度は Risks に明記 |

## Risks

- `FileJournal`/`FilePolicyStore`/`DirectoryBundleStore` は real storage への fault injection を
  torn tail・kill→reopen で行うが、bit rot・lying fsync は脅威モデル外(上記サポート対象外)
- `ProjectController::save` 経路の saved-HEAD は `Saved` record で publish 順を記録するが、
  gate を通さない直接 store 書き込みは順序保証の対象外(単一 writer は gate 経路)
- proposal/apply の in-memory record は bounded — restart で lifecycle 記録は journal replay で
  verdict/消費は復元されるが、approval lifecycle そのものは再 request 前提
- gate 内 `Ledger` scope は `(principal-tag, project, key)` で GW-01 wire ledger の `(adapter, project, principal, key)` より狭い(adapter 次元なし) — restart 後の gate ledger が dedup backstop となる構成で両層の scope が一致しない。多 adapter 化の際は同一 fingerprint scope へ揃える必要がある
- 非 commit outcome は ledger settle→best-effort journal append の順(同 process retry は durable 未確認の outcome を返し得る — restart 権威は常に journal)
- replay 済み `Rejected`/`Indeterminate` outcome は wire 上 `Denied{Commit}`/`Indeterminate` に正規化される — durable reason 文字列は保持するが、元の `GatewayError` variant 忠実度は失われる(retry の成否判定は outcome であって variant ではない、という設計判断)
- `CommitGate::open` の restore は divergent saved-HEAD を journal 権威で上書きする — `saved_head`/`restored_revision`/`restored_from_journal` が report 上の evidence(overwrite 自体の confirm UI は GW-09/#60 側)
- `ProjectController::load` は in-memory undo/redo stack をクリアする — restart を跨いだ undo 継続は現行で非対応(history record の post-state は journal から復元されるが「あの commit を undo する」操作自体は restart 後の stack に存在しない)
- host precondition: controller の `apply` 内で実行される resource oracle / op 実装は controller mutex 保持中に gate へ再入してはならない(deadlock)。同様に `PolicyStore`/`ProposalStore`/`ProjectStore` の生 mutator は公開されたまま — gate の順序・権限保証は caller が fence 経路を通る前提であり、bypass mutator への直接アクセス制御は adapter 層(#59)の責務
- `ScheduledFence` は production `MutexFence` と同一 ordering 意味論だが、real timing race は
  完全に列挙しない — ordering の決定的列挙を目的とし timing fuzz は別 issue(#62)

## Follow-ups

- GW-07: context/pagination 経路を経る export/dispatch への gate 適用拡張
- #62: timing-based concurrency fuzz・journal GC・multi-project ordering
- #59 MCP adapter: `CommitGate` を adapter 経路の commit boundary として接続
- `RecoveryReport` の saved-HEAD と journal-restore 判定は UI/診断向け surface —
  operator-facing recovery report の整形は GW-09/#60 側で検討
