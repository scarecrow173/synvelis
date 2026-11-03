---
id: "IMPL-0Gw05ChangeSetApprov01"
type: "impl"
status: "completed"
title: "GW-05 Immutable Prepared ChangeSet・Semantic Preview / Approval Receipt State Machines"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/57"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/proposal.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/proposal_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/proposal_policy.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/proposal.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/proposal_regressions.rs"
      - type: "file"
        path: "docs/impl/ir/gw-05-immutable-prepared-changeset-semantic-preview-approval-receipt-state-machines.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-gateway/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-gateway/src/gateway.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/outcome.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/rig/mod.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/57"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "docs/impl/ir/gw-02-authorization-policy.md"
    - "docs/impl/ir/gw-03-context-projection.md"
    - "docs/impl/ir/gw-04-semantic-operation-preparation-effect-evaluation-linked-midi-scope.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/57"
  derived-by: []
  verifies:
    - "https://github.com/scarecrow173/synvelis/issues/57"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# GW-05 Immutable Prepared ChangeSet・Semantic Preview / Approval Receipt State Machines 実装記録

Issue #57。#45 GTC-007・#46 GW-05 に基づく「変更案・承認・Apply結果は別の状態として管理し、
Digestに結び付いた承認だけをGW-06へ引き渡す」境界を実装した。GW-04 の `PreparedRecord` を
凍結した `PreparedChangeSet`(決定的 digest 付き)を正本とし、preview・approve・apply の
3 capability と proposal/approval/apply の独立した3状態機械を `ProposalStore` 単一権威で
管理する。Project 本体・Undo・commit には一切触れず、GW-06 へは再検証依存情報と
consumption/commit 整合の seam だけを引き渡す。

## 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `proposal.rs`(新規) | `PreparedChangeSet` — canonical 化(JSON object key ソート)した semantic 内容の SHA-256 `ContentDigest`。digest 入力は schema/schema_version・base{project,revision,tempo_map_revision}・commands・preconditions・effects・unsupported のみ — proposal/operation/request ref・caller・task/grant・snapshot ref・時刻・保持順序は**意味的同一性に入れない**(GT-007)。`freeze`/`verify`/`schema`/`schema_version`。3状態機械 — `ProposalStatus{Prepared,Invalidated,Expired}`・`ApprovalStatus{Requested,Granted,Declined,Expired,Revoked,Invalidated,Consumed}`・`ApplyStatus{AwaitingApproval,CommitRequested,Committed,Rejected,Conflict,Indeterminate}`、遷移表は `transit` データ一元化。`ApprovalReceipt` — approved_by/for_principal/project/grant/task/proposal/digest/expected_revision/expiry を proposal の**記録済み** identity から bind(自己申告値は使わない、GT-008)。`ProposalStore` — 単一 lock 権威: `insert`/`resolve`(owner+human のみ可視、SEC-10)/`changeset`/`record`/`view`/`evict`/`request_approval`/`grant_approval`/`transit_approval`/`approval_verdict`/`consume_approval`/`accept_apply`/`mark_apply_awaiting`/`fail_apply`/`transit_apply`。`refresh` の lazy expiry は `AwaitingApproval` のみ Reject し、受理済み apply は決して書き換えない(GW-06 所有)。`accept_apply` は liveness・retry 参照・receipt consume・apply record 作成を同一 lock で atomic に実施。`consume_in_entry` は Granted→Consumed を atomic にし、消費した attempt 自身に限り verify として冪等再確認を許す。bounded 保持は terminal entry を優先退避し、全 entry が live のときは `insert` が `LimitExceeded` の backpressure を返す(live entry は決して黙って捨てない)。`consume_approval` は受理済み `CommitRequested` apply の存在を要求し、消費を `apply.approval` に記録して attempt と結び付ける(accept なしの消費は永久 dead-end のため拒否)。`preview_value` — DerivedEffect 集合から entity before/after と MIDI note 対(added/removed/modified)を射影する review document — 第二の diff engine ではなく GW-04 観測契約の再利用(GT-010)。`ProposalEvent::ALL`/`ApprovalEvent::ALL`/`ApplyEvent::ALL` が遷移表テストの正本 |
| `proposal_engine.rs`(新規) | `ProposalEngine` — `synvelis.proposal.preview`(Proposal domain、ReadContext、metadata floor + emission boundary で emitted class 検査)/`synvelis.proposal.approve`(trusted user のみ — agent は capability 保有でも denial、GT-008)/`synvelis.proposal.apply`(ProjectMutation domain、expected-revision 必須、`AsyncAccept`)。apply 経路: 存在盲腸 resolve → digest assertion(SEC-09)→ accepted-or-later apply の既存結果参照(retry は second consume/commit しない)→ Prepared 検査 → request expected==base(ACG-002)→ live revision fence(SEC-11)→ `accept_apply` → `Accepted{operation}`。commit 成功は決して主張しない(ACG-004)。GW-06 seam: `changeset`/`prepared_record`/`approval_record`/`apply_record`/`try_consume_approval`(verify-冪等)/`settle_apply`(Commit/Reject/Conflict/Indeterminate のみ — `Accept` は gate の遷移)/`decline_approval`/`revoke_approval` |
| `proposal_policy.rs`(新規) | `ProposalGate` — `PolicyEvaluator` decorator。`RequestApply`+APPLY_ID 以外は inner(GW-02 `GrantPolicy`)へ委譲。inner `Deny`/`PermitForStage` はそのまま(revoked grant・Suggest mode は receipt 以前に deny、SEC-12)。`ApprovalRequired` のみ `gate_probe` で receipt verdict を照会: Granted/OpenApply/NotAProposal → Permit、Pending → `request_approval`+`mark_apply_awaiting` 記録して `ApprovalRequired`、Invalid → `Deny` |
| `prepare_engine.rs` | `PrepareEngine::new` に `ProposalStore` を追加 — prepare 成功時に `insert` で凍結し `{proposal, digest, expires_at}` を結果に含める |
| `prepare.rs` | serialization helper(`ref_tagged`/`attrs_json`/`target_json`/`effect_kind_name`)を `pub(crate)` 化 — preview・digest canonical 化で共有 |
| `gateway.rs` | `approval_ref_for(operation_id)` を `pub(crate)` 化 — `ApprovalRequired` の wire correlation ref |
| `outcome.rs` | `GatewayError::ProposalUnavailable`/`DigestMismatch`/`ApprovalInvalid` 追加 — proposal 不在・digest 不一致・receipt 非消費可の型付き taxonomy |
| `lib.rs` | `proposal`/`proposal_engine`/`proposal_policy` module + public 型 export |
| `Cargo.toml`/`Cargo.lock`/crate `Cargo.toml` | `sha2` 依存追加(digest) |
| `tests/proposal.rs`(新規) | 30 テスト — GT-007..010・SEC-09..12・ACG-002/004・GTC-007 の受入 matrix(下表) |
| `tests/proposal_regressions.rs`(新規) | 16 テスト — review-protocol で確定した各指摘の恒久検証(下表) |
| `tests/rig/mod.rs` | GW-05 wire builder(`proposal_req`/`preview_req`/`approve_req`/`apply_req`/`apply_grant`/`prepare_mute(_agent)`/`preview_value_of`)を shared rig へ移動、`fixture_rig_with_cap` 追加 |

## 設計上の主要判断

| 判断 | 理由 |
| --- | --- |
| digest 入力から ref・時刻・保持順序を除外 | 「同じ意味的 ChangeSet」は誰がいつ prepare しても同一 digest — expiry・snapshot ref は lifecycle/依存参照であって意味内容ではない。base{project,revision,tempo_map_revision} は依存 identity として入れる |
| preview は第二 diff engine でなく DerivedEffect 射影 | GW-04 が導出した effect/detail(notes map)が semantic source of truth — diff を再実装すると digest が bind する内容と preview 内容が乖離し得る |
| receipt は proposal の記録済み identity に bind | wire の自己申告 grant/task/principal に bind すると credential 偽装が通る — store が prepare 時に記録した request の次元を bind(GT-008/SEC-09/10) |
| consume は受理と同一 lock で atomic | gate verdict(read-only)と consume の間で proposal 死亡・racing apply が入ると「受理したのに消費していない/二度消費」が起き得る — `accept_apply` で verify+spend+record を一括 |
| wire `approval` arg は correlation であり権威ではない | `ApprovalRequired` が返す `approval-for-<operation>` は提示用の参照 — 権威は receipt binding。`Requested` 中のみ record ref を last-told で追従、Granted 以降は凍結(superseded ref は deny) |
| 受理済み apply は proposal 死亡から独立 | `AwaitingApproval` は proposal と共に Reject するが `CommitRequested` は GW-06 の settlement 対象 — expiry/evict が `Rejected` へ書き換えると consumption/commit 整合が壊れる(review で確定した P1) |
| 終端 apply の後だけ承認 lifecycle を再開可 | `Rejected`/`Conflict`/`Indeterminate` で終わった receipt は `Consumed` のまま据え置き(二度消費不可)だが、live proposal の承認 request は再開して retry を詰まらせない |
| `settle_apply` は commit 側 event のみ受理 | `Accept` は apply gate の遷移 — GW-06 が受理を報告することはあり得ない。同一 status の再報告は冪等(報告済みを証明できない fence のため) |

## 受入条件との対応

| AC | 対応 |
| --- | --- |
| 同一 semantic ChangeSet は決定的 digest | `canonicalize` で key ソート→SHA-256。`identical_prepares_digest_identically` で別 proposal ref/時刻でも同一、`semantic_drift_changes_the_digest` で target/args/base revision 各々の変化を検証、`changeset_verify_detects_tampering` が effects/preconditions を含む全 digest 入力の改竄を検出 |
| Proposal 不変・Preview 参照・MIDI Notes before/after | `PreparedChangeSet::verify` が改竄を検出(`changeset_verify_detects_tampering`)、lookup は ProposalRef+Digest(`lookup_requires_proposal_and_digest`)、preview の `diff.midi_notes` が added/modified/removed・`diff.entities` が modified/created/deleted を報告(`preview_reports_midi_notes_before_and_after`/`preview_reports_modified_and_removed_note_pairs`/`preview_follows_the_emitted_classes`)、`preview_value` は DerivedEffect detail 由来(第二 diff なし) |
| 他 Principal/Project への流用・改竄・期限切れ・RevisionConflict で不可 | `binding_mismatch` で principal/project/grant/task/digest/expected_revision の全次元を検査(`receipt_binding_rejects_wrong_grant`/`receipt_binding_rejects_wrong_task` 等)、`assert_digest` で tamper `DigestMismatch`、expiry は `refresh` で `Expired`+unavailable(`expired_proposal_is_unavailable`/`expired_approval_reopens_as_fresh_request`)、`expected != base`/`live != base` は `RevisionConflict`(`expected_revision_mismatch_is_a_conflict`/`revision_conflict_rejects_apply`) |
| 3状態機械の全許可/禁止遷移を表形式で検証 | `proposal_transition_table_is_exact`/`approval_transition_table_is_exact`/`apply_transition_table_is_exact` — 全 status×全 event の許可 edge 集合を `transit` と照合、表に無い edge は全て `None` |
| GW-06 契約: 二重消費・retry 既存結果参照 | `consume_in_entry` は Granted→Consumed を lock 下で atomic — `double_consumption_can_never_succeed`(別 operation は二度消費不可)。accepted-or-later apply は record を参照(`commit_outcome_is_gw06s_to_report`/`committed_apply_references_itself_after_expiry`)。消費した attempt 自身の verify は冪等(`consume_verify_reconfirms_only_the_spending_attempt`) |
| GT-007〜010・SEC-09〜12・G2/G3 | 下記 matrix 参照 |

## SEC / GT matrix(#46 Failure Matrix 対応・GW-05 該当分)

| Case | 内容 | 判定 | 証拠 |
| --- | --- | --- | --- |
| GT-007 | digest は semantic identity — 同一変更は同一、drift は変化 | Pass | `identical_prepares_digest_identically`/`semantic_drift_changes_the_digest` |
| GT-008 | receipt は trusted user 発行・agent 自己申告不可・記録済み identity に bind | Pass | `approve_is_a_trusted_user_only_act`(agent capability 保有でも deny)、receipt フィールドは `record.request`/`entry.changeset` 由来 |
| GT-009 | approval は proposal digest・revision 結合で1回限り消費 | Pass | `approve_to_apply_flows_through_request_grant_consume`/`double_consumption_can_never_succeed`/`seam_consume_requires_the_accepted_attempt`/`binding_mismatch` 全次元 |
| GT-010 | preview は semantic before/after(MIDI notes 含む)を返す | Pass | `preview_reports_midi_notes_before_and_after`/`preview_reports_modified_and_removed_note_pairs` |
| SEC-09 | digest assertion・tamper・ref 偽装で apply 不可 | Pass | `assert_digest`/`changeset_verify_detects_tampering`/`receipt_for_another_proposal_denies`/`a_superseded_request_ref_is_denied` |
| SEC-10 | proposal/approval の存在盲腸・binding 次元 | Pass | `preview_hides_foreign_proposals`/`apply_hides_foreign_principals_proposals`(foreign agent は preview・apply 共に `unknown proposal`)/`resolve` owner+human のみ |
| SEC-11 | revision 結合 — expected!=base、live drift、receipt revision | Pass | `expected_revision_mismatch_is_a_conflict`/`revision_conflict_rejects_apply`/`a_live_apply_survives_another_attempts_failure` |
| SEC-12 | Suggest mode・revoked grant は receipt 以前に deny | Pass | `suggest_mode_never_applies`/`apply_denies_without_the_capability_in_grant`/`revoked_approval_denies_apply` |
| G2 | unknown/unsupported の丸め込みなし | Pass | `ProposalUnavailable`/`DigestMismatch`/`ApprovalInvalid` の型付き taxonomy — foreign/unknown/dead は同一 verdict で盲腸化 |
| G3 | commit 成功の虚偽申告なし | Pass | `Accepted{operation}` は accept のみ主張 — `commit_outcome_is_gw06s_to_report` で Committed は `settle_apply` 経由のみ |

## 実行した検証(証拠)

- 環境: Windows、rust 1.99.0(mise pin)、GW-01〜04 merge 済み main(`ddfb6cc`)+ 本変更
- `cargo fmt --all -- --check` — Pass
- `cargo check --workspace --all-targets` — Pass
- `cargo clippy --workspace --all-targets -- -D warnings` — Pass
- `cargo test --workspace --locked` — Pass、workspace 全 test binary 緑・0 failure。
  `tests/proposal.rs` 30 テスト + `tests/proposal_regressions.rs` 16 テスト pass、
  既存 gateway/recording/render/project/workspace/e2e 全 suite pass

## Review-protocol 実施結果

独立 reviewer 2 系(A: 受入条件トレース / B: adversarial・failure)を投入し、全 candidate を
コード実測で個別検証(Validator)。確定した P0/P1 は全て修正し、`tests/proposal_regressions.rs`
に恒久検証を追加。最終判定(Judge): 全 blocking 解消・客観検証全緑で **ACCEPT**。

| 指摘 | 判定 | 対応 |
| --- | --- | --- |
| lazy expiry/eviction が受理済み `CommitRequested` apply を `Rejected` へ書き換え、GW-06 の settlement 不能・consumption/commit 整合破壊 | 確定(B) P1 | `refresh`/`evict` の cascade を `AwaitingApproval` のみに限定 — commit 経路の record は GW-06 所有として保持。`accepted_apply_survives_proposal_expiry`/`accepted_apply_survives_invalidation`/`waiting_apply_is_refused_with_the_proposal` |
| liveness 検査と apply record 作成の間で proposal 死亡・racing apply が入ると非受理なのに消費/記録される競合 | 確定(B) P1 | `accept_apply` 新設 — liveness・retry 参照・receipt consume・record 作成を store lock 下で atomic 化 |
| dead proposal に approval request・apply record が生える(lifecycle 偽造) | 確定(B) P1 | `request_approval`/`mark_apply_awaiting`/`fail_apply` は `Prepared` でない entry に記録を作らない。`dead_proposal_grows_no_lifecycle_records` |
| receipt の re-consumption/verify 冪等性が未定義 — GW-06 fence が消費実績を再確認できない | 確定(B) P1 | `consume_in_entry` 集約 — `CommitRequested`+同一 operation+同一 receipt に限り冪等 verify、他は `Consumed` で `ApprovalInvalid`。`consume_verify_reconfirms_only_the_spending_attempt` |
| 終端 apply の後も Consumed receipt が恒久的に提案を詰まらせる / 受理済み apply 中の再承認が receipt を奪う | 確定(B) P1 | apply terminal のみ request を再開(`rejection_reopens_the_approval_lifecycle`)、live apply の receipt は seal のまま |
| bounded FIFO が live proposal を dead tombstone より先に退避 | 確定(B) P1 | 容量超過時は terminal entry を優先 victim に。`dead_entries_evict_before_live_ones` |
| `settle_apply` が `Accept` を受理 — commit seam から受理遷移を偽造可能 | 確定(B) P1 | commit 側 event のみ受理、同一 status 再報告は冪等。`settle_apply_reports_only_commit_outcomes` |
| preview descriptor の data_access floor が emitted class より過剰に広い | 確定(B) P1 | floor を `MetadataRead` に下げ、実 bound は `emitted_classes` を emission boundary で検査。`preview_follows_the_emitted_classes` |
| `Requested` 中に複数 attempt が走ると wire-told ref が supersede され旧 ref が黙って有効に見える | 非 blocking(A/B) | last-told-wins を明文化し superseded ref は `approval does not match` で deny — `a_superseded_request_ref_is_denied` で文書化 |
| 失敗 attempt が live apply record を上書きする | 確定(B) P1 | `fail_apply` は live record がある限り記録しない — 失敗証拠は attempt 自身の error。`a_live_apply_survives_another_attempts_failure` |
| changeset の schema identity が読めない | 非 blocking(A) | `schema()`/`schema_version()`/`verify()` を public 化。`changeset_exposes_its_schema_identity` |
| `try_consume_approval`/`consume_approval` が受理済み apply 不在でも `Granted` receipt を消費 — `Consumed`+apply なしの永久 dead-end を生成し得た | 確定(A) P2→修正 | `consume_approval` は同一 operation の `CommitRequested` record を要求し `apply.approval` へ消費を記録。`seam_consume_requires_the_accepted_attempt` |
| bounded FIFO が全 entry live のとき live proposal を黙って退避 | 確定(A) P2→修正 | `insert` を `Result` 化 — terminal victim が無ければ `LimitExceeded{proposals}` の backpressure。`capacity_never_drops_a_live_proposal` |
| digest drift 証拠が effects/preconditions を mutate していない(対象/args/base のみ) | 確定(A) P2 | `changeset_verify_detects_tampering` に effects 消去・preconditions 改竄の verify 失敗を追加 |
| preview テストが added note のみ — modified/removed note・created/deleted entity 未検証 | 確定(A) P2 | `preview_reports_modified_and_removed_note_pairs`(velocity 変更=modified、note 削除=removed、clip 削除=deleted entity)、`preview_follows_the_emitted_classes` に created entity/added pair の断言を追加 |
| receipt の task 次元・apply 経路の foreign-principal 盲腸が未検証 | 確定(A) P2 | `receipt_binding_rejects_wrong_task`(同一 PersistentProject grant・別 task で isolate)、`apply_hides_foreign_principals_proposals`(独自権限の foreign agent は `ProposalUnavailable`) |
| `ProposalGate` の host wiring が未定義のまま | 確定(A) 非 blocking | `ProposalEngine::install` doc に `ProposalGate::new(proposals, inner)` で base policy を wrap する必須手順を明記 — rig は同構成で全テスト緑 |
| `PreparedChangeSet` の clone 境界が不明瞭 | 確定(A) 非 blocking | store 境界では immutable・read surface は clone・`verify()` で digest 検証である旨を doc 明記 |
| 遷移表テストが event 列挙を手動管理 — 将来 variant が検証漏れし得る | 確定(A) 非 blocking | `ProposalEvent::ALL`/`ApprovalEvent::ALL`/`ApplyEvent::ALL` を正本化し表テストが全 variant を iterate |

## Risks

- `ProposalStore` は in-memory・bounded — restart で全 proposal/approval/apply 記録が消える。
  durable journal は #57 非対象(standalone persistent Journal は別 issue 範囲)
- `approval-for-<operation>` wire ref は proposal 単位ではなく attempt 単位の correlation —
  `Requested` 中の再要求で last-told に追従するため、古い ref を保持する wire peer は
  `approval does not match` の denial を受ける(意図的 — correlation は権威ではない)
- `view.record`(`PreparedRecord`+candidate `Arc<Project>`)は `Prepared` のみ保持 —
  expiry/evict で drop されるため GW-06 は `Prepared` 中に再検証入力を取得する必要がある
- human による proactive approve は wire が見ない `approval-<counter>` ref を mint する —
  record 自身の ref で fund するため wire 提示不要だが、response の `approval` 値を
  提示しても同じく受理される(`the_approve_result_ref_stays_presentable`)

## Follow-ups

- GW-06: `PreparedRecord{request, op, prepared{candidate}}` + `changeset` を
  `ProjectController::apply` commit path へ。`try_consume_approval` は fence 内で
  verify-冪等として呼び、`settle_apply` で Commit/Reject/Conflict/Indeterminate を
  報告する — approval consumption と commit result の atomic 契約は GW-06 が所有
- GW-09: `preview_value` の review document を Approval UI の確認画面へ接続
- `midi.note.*` 系の preview は `StructuredContentRead` を emission boundary で要求 —
  metadata-only の read grant は entity-level diff のみ許可する方針と整合するか GW-09 で再考
