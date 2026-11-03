---
id: "IMPL-0Gw07JobMediaProv01"
type: "impl"
status: "completed"
title: "GW-07 NonRT Job・Verified Media・Generation Provenance Gateway Adapter"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/61"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/job.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/job_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/media.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/provenance.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/stub.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/job.rs"
      - type: "file"
        path: "docs/impl/ir/gw-07-nonrt-job-verified-media-generation-provenance-adapter.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/id.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/grant.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/mcp_tools.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/rig/mod.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/61"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "docs/impl/ir/gw-02-authorization-policy.md"
    - "docs/impl/ir/gw-03-context-projection.md"
    - "docs/impl/ir/gw-04-semantic-operation-preparation-effect-evaluation-linked-midi-scope.md"
    - "docs/impl/ir/gw-05-immutable-prepared-changeset-semantic-preview-approval-receipt-state-machines.md"
    - "docs/impl/ir/gw-06-authorized-commit-gate-revoke-linearization-idempotency-crash-recovery.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/21"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  derived-by: []
  verifies:
    - "https://github.com/scarecrow173/synvelis/issues/61"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    - "ProviderStub / MediaStub による deterministic contract proof(実 AI provider なし)"
---

# GW-07 NonRT Job・Verified Media・Generation Provenance Gateway Adapter

非 RT Job / verified media / generation provenance を既存の
GW-01..06 契約(registry・grant/policy・snapshot・prepare/proposal・
commit)の上に adapter として実装する。並行 authorization scheme は
作らず、Job 状態の権威は adapter 側 book(Project/Application
所有)に置き、MCP session には寄せない(GIA-001..003、issue #45 の
GW-07〜09 v0.6 節)。

## 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `job.rs`(新規) | Job ドメイン型: `JobPhase`(Pending/Running/Completed/Failed/Cancelled — Cancelled は provider 確認時のみ)、`JobStatus`、`JobKind`/`OutputKind`、`DeclaredOutput`(digest/format/len は claim)、`InputSnapshot`/`InputPin`(admission 時に committed base を pin)、`JobSpec`(prompt は job input であり metadata ではない)、`CancelLink`(NotRequested/Requested/Confirmed/Unconfirmed — CancelPhase とは別軸)、`CancelResponse`(Confirmed/Pending/Unreachable/Settled{phase} — settle race は実 phase を正直に返す)、`JobPort`(coordinator への seam)、`OutputReport`/`JobResult`/`JobRecord`(principal/project/task/grant の owner scope)、`JobBook`/`SharedJobBook`(bounded・same-scope eviction・existence-blind scoped lookup・grant 別の budget 計数/revocation sweep) |
| `job_engine.rs`(新規) | `synvelis.job.submit`/`job.get`/`job.cancel` capability。submit は `StartAsyncOperation` stage で `evaluate_effects`(input scope + `ProviderTransfer` external transfer)→ input snapshot → `limits.job_budget` admission spend → `JobPort::submit` → `Accepted{operation}`。get は blind owner-scope → `ReadOperationResult` + disclosure 評価(`MetadataRead`、external caller には `ExternalDisclosure` 追加)→ status → 初回のみ materialize(verify + provenance)。cancel は provider 応答を confirmed/requested/unconfirmed/not-cancellable+実 phase で正直に返す。`note_grant_revoked` は host 向け revocation sweep — grant 失効の authorization 効果は denial であり、job への影響は別途 provider への cancel request(嘘の Cancelled を立てない) |
| `media.rs`(新規) | `MediaLifecycle`(Staged/Validating/Verified/Rejected/Unavailable — forward evidence)、`VerifiedMedia`(再導出 digest/format/len)、`MediaVerdict`、`MediaPort`(staging/hash/format/length 検証 seam)、`MediaRecord`/`MediaBook`(MediaRef 鍵・owner scoped・判決は一度書き immutable)、`MediaGate`(verified かつ caller/project 所有のみ解決 — unknown/foreign/rejected/未検証は同一 `None` で blind)。verified でも commit 無しには project を動かさず、verified record は orphan protection として残る |
| `provenance.rs`(新規) | `EvidenceLevel`(Confirmed/Declared/Unknown)、`ProvenanceField`、`ProviderClaim`(attested フィールド集合つき)、`GenerationProvenance`(Producer/Model/Rights/Transformation + 束縛 input + input revision + media)。claim 無しは `Unknown`、attested は `Confirmed`、その他の申告は `Declared`。`disclosed()` は bounded — prompt/token/grant credentials を含まない |
| `stub.rs`(新規) | `StubContent`(deterministic byte store、sha256 digest)、`CancelMode`(Confirm/Pending/Unreachable)、`DriveMode`(Manual/OnPoll)、`StubOutputSpec`(`tampered()` で偽造 digest fixture)、`ProviderStub`(`JobPort` — `job-N` mint、test-driven phase、submissions evidence、provider claim)、`MediaStub`(`MediaPort` — staged bytes から digest/format/len を再導出し tamper を決定的に reject) |
| `id.rs` | `JobRef`/`MediaRef`(string ref 型、`OperationId`/`RequestId`/`ProposalRef`/`SourceId` とは別名空間) |
| `grant.rs` | `GrantLimits::job_budget`(GW-07 execution budget、admission 時に book 側で 1 unit 消費・返却無し) |
| `prepare.rs`/`prepare_engine.rs` | `synvelis.media.register` を semantic op table に登録(`Create{parent: Asset, creates: Source}`、input は `"media"` ref のみ)。`prepare::lower_for` の直 lowering は fail-closed;`PrepareEngine::with_media(...)` が `Arc<dyn MediaGate>` を受け、`lower_media` が caller principal+project+verified をゲートに問い、`VerifiedMedia.content_digest` で `ProjectOp::RegisterSource` を引く(caller supplied digest は一切受け付けない)。gate 未配線・unknown/foreign/未検証は一律 `Denied`。`lower_named`/`run`/batch lowering は束縛済み `OperationRequest` を lowering へ渡す |
| `mcp_tools.rs` | `GrantLimits` に `job_budget: None` を追加(MCP tool surface は job budget を発行しない) |
| `lib.rs` | `job`/`job_engine`/`media`/`provenance`/`stub` module export |
| `tests/rig/mod.rs` | `JobRig`/`job_rig`/`job_grant`/`job_req`/`submit_req` — job engine を実 registry/policy/snapshot に配線、media book を `with_media` gate として共用 |
| `tests/job.rs`(新規) | deterministic 17 テスト(下記 evidence) |

## Acceptance evidence(`tests/job.rs`)

| 基準 | テスト |
| --- | --- |
| JM-01 / JOB-01,02 | `ids_stay_distinct_and_jobs_survive_disconnect` — OperationId≠JobRef≠RequestId、book は adapter 所有で同一 principal の新 `CallerIdentity`(MCP reconnect 相当)が job を解決 |
| JM-02 / GIA-002 | `input_snapshot_pins_committed_revision` — admission 時に committed revision+digest+state を pin |
| JOB-03 | `revocation_denies_new_calls_but_only_requests_cancel` — revoke で新規 call は `Denied`、sweep は CancelLink::Requested までで Cancelled を捏造しない |
| JOB-04 | `job_budget_bounds_admissions` / `expired_grant_admits_no_new_job` — budget 超過は `LimitExceeded`、期限切れ grant は新規 protected op を開始させない |
| JOB-05 / G5 | `failed_verification_never_registers` — completed でも verify 失敗なら `Rejected` で register 不可・revision 不変;`unknown_or_foreign_media_denies_identically`、`register_fails_closed_without_a_gate` |
| JOB-06 / JM-04 / G5 | `verified_media_lowers_to_register_source_with_gate_digest`(digest は gate 由来、prepare 単体では revision 不変)、`verified_media_commits_through_gate`(prepare→apply→`CommitGate::commit` → rev2、committed source の digest = verified evidence) |
| JOB-07 | `result_disclosure_requires_the_disclosure_class` — `ExternalDisclosure` 無しは `ReadOperationResult` で `Denied`(Data Read / External Transfer / Result Disclosure / Commit は別 grant boundary) |
| JOB-08 | `foreign_and_unknown_operations_deny_identically` — owner-scope 外の job は存在しない job と同一 verdict |
| JOB-09 / JM-03 | `cancel_verdicts_are_honest`(Confirmed→cancelled / Pending→requested / Unreachable→unconfirmed / settled→not-cancellable+実 phase)、`progress_polls_report_provider_truth` |
| JM-05 / GIA-003 | `provenance_levels_and_secret_bound` — declared/unknown の evidence 写像、prompt は result・provenance・media いずれにも出ない |
| admission 境界 | `submit_denies_out_of_scope_inputs_and_missing_transfer_class`(scope 外 input は provider 未到達で deny、ProviderTransfer 無しも deny)、`unknown_provider_denies_admission` |

## 設計上の主要判断

| 判断 | 理由 |
| --- | --- |
| Job 状態の権威は adapter の `JobBook`、provider truth は `JobPort` のみ | MCP disconnect/window close は Job を消さない(JOB-01/02)。MCP op book は session 相関であって Job の owner ではない |
| `synvelis.media.register` は GW-04 semantic op として登録し lowering は `MediaGate` 必須 | verified media の Project 配置は通常の RegisterSource をそのまま流すが、digest は caller input ではなく gate の verified record 由来。並行 mutation 経路を増やさず GW-02 effect 評価・GW-05 proposal・GW-06 commit fence をそのまま通る |
| `CancelLink`(request 側)と `JobPhase::Cancelled`(provider 確認)を別軸にする | cancel request≠cancel 確認。Unreachable は `Unconfirmed` であり `Cancelled` を名乗らない(JOB-09)。grant revocation protection も同じ request 経路を通るだけで implicit cancel ではない |
| `evaluate_effects`/`decide`/`authorize_disclosure` を再利用 | job admission・read scope・external transfer・result disclosure は GW-02 の同じ policy evaluator に通す — 並行 authorization を作らない |
| media record は verdict 一度書き・owner scoped・existence-blind | tampered 再提出で `Rejected→Verified` に書き換わらない、verified でも commit までは project revision を動かさない(G5) |
| provenance は bounded field 写像 | prompt・token・grant credentials は Project metadata/provenance/disclosure に出さない(GIA-003)。unknown model/rights は `Unknown` |

## 検証

- `mise run fmt-check` / `check` / `lint` / `test`:全 green
- `cargo test -p synvelis-gateway --test job`:23/23 pass
- workspace test 全 pass(GW-01..06,08 の既存 suite 含む非退行)

## レビュー反映(cooperative + adversarial)

独立レビューの指摘を以下のとおり修正した。

- `JobBook::insert` が foreign-scope 飽和時にレコードを静かに落とし、
  `Accepted` だけ返る ghost job(budget バイパス・revocation sweep 対象外・
  lookup 不能)になり得た問題 → `insert` を `bool` 化し、失敗時は
  provider へ `request_cancel` を送ったうえで `LimitExceeded` を返す。
  operation id 衝突も同じく拒否し alias しない。
- budget チェック→provider submit→insert に跨る TOCTOU → spend を
  `JobBook::try_spend` の atomic な check-and-spend に移し、provider
  admission が起きなかった submit error のみ `unspend` で返却。
  spend は eviction/terminal で減算されない累積カウンタ。
- `job.get` が book ロックを保持したまま media verify / provider claim
  を呼んでいた問題 → ロック内は link 昇格・phase 前進のみ、
  materialization はロック外で構築してから冪等に書き戻す。
- provider の古い status が terminal phase を巻き戻す経路 → terminal は
  terminal 相手にのみ前進するよう guard。
- prompt が idempotency ledger fingerprint / `Debug` derive に残る問題
  → fingerprint を SHA-256 化、`JobSpec` の手書き Debug で `<redacted>`。
- provider 由来の自由テキスト(failure/rejection reason)と outputs 数・
  media book の非有限性 → 256 char bound・`MAX_JOB_OUTPUTS`・
  `MEDIA_BOOK_CAP` で境界化。飽和時は Unavailable として fail closed。
- provider 発行 media ref の衝突 → book は first-writer を保ち、他 job
  の record に衝突した出力は "media ref collision" として正直に
  Unavailable 報告(登録も fail closed)。
- `OutputKind::Midi` 宣言が media gate を通り得た構造上の穴 →
  `MediaRecord.kind` で media lane のみ resolve。
- cancel 直後の eviction で `phase: null` になり得た見え方 → record
  不在時は `"unknown"` を正直に返す。
- media gate 未配線を `Denied` としていた taxonomy → host 設定不備は
  `Internal`。
- cancel linkage 更新を `JobRecord::note_cancel` に集約
  (`Settled{Cancelled}` は Confirmed へ昇格 — provider truth が確認)。
- 追加テスト:book 容量/spend、Pending→Confirmed 昇格、foreign
  project の gate 拒否、batch 経由 `media.register`、materialization
  の冪等性。

## 既知の境界(レビュー指摘・要裁定)

- `synvelis.source.register` + `synvelis.source.set-state` は既存
  contract として caller 申告 digest で `Verified` 状態に到達でき、
  GW-07 の media gate とは別経路。本 issue の境界は「job 出力は
  gate 経由」であり、`Verified` source 一般を「verified evidence が
  必須」とするかどうかは既存 source contract の変更を伴うため
  別 issue で裁定する。
- `note_grant_revoked` の呼び出し wiring(grant revoke → job sweep)
  は adapter の公開 API として用意済みだが、呼び出し側(store /
  mcp_server)の配線は統合側の責務として残る。
- `JobEngine::record` は introspection 用に全 record(spec 含む)を
  返す — in-process admin API であり、wire には出さない。

## 既知の限界 / 後続

- `JobPort`/`MediaPort`/`MediaGate` は trait seam。実 AI provider・codec・
  staging stack との接続は後続 issue(本実装は deterministic stub で
  contract を固定)。
- `MediaGate` の production 実装(issue #17 の staging/verification
  lane への配線)は adapter 側の責務として残る。
- MIDI 出力 lane(`OutputKind::Midi`)は capability contract 上は
  宣言済みだが、`synvelis.media.register` は media lane のみを扱う。
