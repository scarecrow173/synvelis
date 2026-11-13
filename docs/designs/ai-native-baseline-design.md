---
id: "DESIGN-AjvrOoTZDaCcUxKLXeoxA3"
type: "design"
status: "approved"
title: "AI-Native Hybrid DAW Baseline — Grant/Principal/Scope/Approval/ChangeSet・AI Off 既定・3 Mode 責務設計 (synvelis#44)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/44"
  changes:
    added:
      - type: "file"
        path: "docs/specs/ai-native-baseline.md"
      - type: "file"
        path: "docs/designs/ai-native-baseline-design.md"
      - type: "file"
        path: "docs/plans/ai-native-baseline.md"
      - type: "file"
        path: "docs/tasks/ai-native-baseline-task.md"
      - type: "file"
        path: "docs/test-specs/ai-native-baseline.md"
      - type: "file"
        path: "docs/impl/ir/ai-native-baseline.md"
      - type: "file"
        path: "docs/impl/exp/ai-native-baseline.md"
      - type: "file"
        path: "evidence/sv44/ai-native-baseline-case-ledger.md"
      - type: "file"
        path: "evidence/sv44/ai-native-baseline-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/specs/ai-native-baseline.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ai-native-baseline.md"
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/designs/midi-source-note-editing-design.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/product-value-core.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/workspace-model.md"
    - "docs/specs/media-render.md"
  refines:
    - "docs/specs/ai-native-baseline.md"
  refined-by: []
  derives-from:
    - "docs/specs/ai-native-baseline.md"
    - "https://github.com/scarecrow173/synvelis/issues/44"
  derived-by:
    - "docs/plans/ai-native-baseline.md"
    - "docs/test-specs/ai-native-baseline.md"
  verifies: []
  verified-by:
    - "docs/test-specs/ai-native-baseline.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
---

# AI-Native Hybrid DAW Baseline — 設計契約 (synvelis#44)

Issue #44 の Approved baseline（AIB-001〜007 / AHG-001〜011）を、
実装可能な設計契約へ落とす。**本設計は docs/ 契約層であり、
Command Gateway の型・実装詳細は #45、MCP/Permission UX/RT/
AI OFF の製品受入は #46/#114 が Owner**。GW-01〜10 の headless
実装（`crates/synvelis-gateway`・`crates/mcp-poc`）が存在する
事実と、本設計が固定する不変条件、および未実装・未検証の
lane を混同しない。

## Context

- Issue #44 は「AI-Native Hybrid DAW Baseline v0.1」として、
  Manual/AI の対等共存・Intentional Usage・Rights/Consent/
  Privacy・Shared Project・Agent Direction の製品・セキュリティ
  不変条件を Approved baseline として保持する。
- `docs/designs/gw-01-gateway-contract-design.md`（Capability
  Registry・Typed Invocation・Versioned Result/Error）と
  `docs/designs/gw-02-authorization-policy-design.md`
  （Principal/Execution Grant/Authorization Policy・Fixed/
  Dynamic Scope）が、#44 の不変条件を継承する実装側の先行
  設計として存在する。本設計はそれらを**再定義せず**、#44 の
  製品・権限・状態契約として上位に位置づける。
- `mise run gw10`（`crates/synvelis-gateway` の
  `gw10_{authorization,concurrency,project,persistence,mcp,
  job,system,contract,harness}.rs` + `crates/mcp-poc` の
  `gw10_transport.rs`）は headless の認可・3 Mode・Revoke/
  Commit 線形化・冪等・Crash Recovery・AI Off 人間 pipeline
  の証拠を持つ。本設計の各不変条件はこれらの fixture へ写像
  するが、headless PASS を製品受入・実機受入に繰り上げない。

## Scope

### In Scope

- Principal/Caller・Grant・Scope・Invocation Target・
  Approval Receipt・ChangeSet の論理契約と状態機械。
- Suggest / Approve-to-Apply / Delegated Apply / Proactive の
  責務分離と、Apply Mode が Grant から来て Request から来ない
  ことの固定。
- AI Off 既定と、AI/MCP 無効時の通常 DAW 独立経路。
- 固定 Scope・制約付き動的 Scope の評価原則（親子無条件継承
  禁止・作成/移動/未知効果の安全判定）。
- Revoke/Expiry/Revision Drift/ChangeSet Tamper/Idempotent
  Replay/Crash の失敗モードと回復契約。
- Data Access・外部送信・Model Rights・Cost・Provenance の
  Unknown 明示と保証境界。
- RT/nonRT 境界と ProjectController Single Writer の維持。
- AIB/AHG → 機構 → fixture/Oracle → status の写像。

### Out of Scope

- Command Gateway の具体 Rust 型・API・wire Schema・永続化
  形式・crate 配置（#45 が Owner。本設計は論理契約のみ）。
- MCP Tool Schema・Transport・接続認証の実装・GUI の権限
  UX・実外部 Provider 接続（#46/#114 が Owner）。
- Ruviform 汎用 Gateway/認証 Provider の実装・Synvelis 専用
  としての重複実装。
- Agent Reasoning Runtime（LLM/Prompt/Tool-Calling）の選定・
  実装 — 交換可能な責務として境界のみ固定。
- 実装コード・`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI の変更。

## Owner 分離（責務境界の固定）

| 領域 | Owner | 本設計での扱い |
|---|---|---|
| DAW 意味論・Semantic Project 正本・ProjectController Single Writer・Undo/Revision・非破壊編集 | Synvelis（#17/#10/#11/#38） | Gateway/Agent は同一検証済み編集契約へ収束。別 Writer/別 Undo 基盤を作らない（AHG-005） |
| AI 利用ポリシー・Grant/Approval/Activation の権威・Context 公開範囲・AI Off 既定・Rights/Provenance の製品意味 | Synvelis（本 Issue #44 → #45/#46） | 本設計が論理契約を固定。具体型は #45 |
| Capability/Command Gateway の型付き契約・Authorization 評価・Commit Gate | Synvelis `synvelis-gateway`（#45/#46、GW-01〜10 実装済み headless） | 本設計の不変条件を継承。実装詳細は #45 |
| MCP Server Adapter（外部 Agent 接続） | Synvelis `mcp-poc`/`synvelis-gateway` adapter 層（#46） | 内部契約の正本ではなく公開 Adapter（AHG-002） |
| Agent Reasoning Runtime（推論・計画・Tool-Calling 反復） | 交換可能・Synvelis 非所有 | 固定 Framework を必須にしない（AHG-001）。DAW Job 所有・結果検証・登録・Project 整合は Synvelis |
| Generic Graph/Time/Audio/Plugin/Media/Offline/Job/Observability | Ruviform | そのまま利用。MCP/Agent 意味論・DAW 固有 Scope/Grant を Ruviform へ移さない（AHG-005） |
| GUI/OS/実機・実外部 Provider・承認 UI・Privacy の製品受入 | #46/#114（別 Gate） | headless 証拠を製品受入に繰り上げない |

## Canonical Concepts（論理契約）

### Principal / Caller / Grant

- **Principal**: 信頼境界で確認された検証済み identity。外部
  Agent が自己申告する Principal/Task/「承認済み」は信頼しない
  （AHG-007）。Gateway wire 上の authority field は Permit
  Record から流れ、client claim を authority にしない
  （`sec02_wire_authority_fields_cannot_self_declare`・
  `gw10f_session_binds_permit_record_never_client_claims`）。
- **Caller**: 認証状態を持つ呼出し主体。未認証 Caller・Grant
  なし・他 Principal の Grant 借用は Default Deny
  （`sec01_unauthenticated_request_denies`・
  `sec01_agent_without_any_authority_denied`・
  `sec01_grant_ref_alone_is_not_proof`・
  `sec03_foreign_principal_cannot_spend_the_grant`）。
  人間の通常 DAW 操作は AI 用 Grant を要求しない
  （`sec01_human_needs_no_grant`・`e2e1_ai_off_human_pipeline_
  commits_undoes_and_recovers`）。
- **Execution Grant**: 誰が（Principal Binding）・何を
  （明示的 Capability 集合）・どの対象（Resource Scope）・
  どの情報（Data Access）・どの Activation/Approval 条件・
  いつまで（Validity/Revocation）・どの上限（Limits）で許される
  かを束ねる論理契約。Grant ID・Schema/Policy Revision・Issued
  By・Project ID を持つ。Execution Grant・Activation
  Authority・Approval Policy・Authentication は**別契約**で
  あり、一つの「AI 許可」に束ねない（AHG-007）。新 Capability
  は既存 Grant で暗黙に許可しない（`gt003_new_capability_not_
  implicitly_granted`）。
- **AI Off 既定**: Synvelis 管理下の AI は初期状態で無効。
  「利用可能」「UI を開いた」状態と「実行許可」を区別し、
  パネル表示・対象選択・通常 DAW 操作を暗黙の推論実行許可と
  解釈しない（AIB-002）。AI 無効時も既存 Project の手動利用・
  再生・保存・書き出しを妨げない。

### Scope（固定 / 制約付き動的）

- **固定 Scope**: Exact ID 集合、または発行時 Snapshot に基づく
  固定集合。評価は列挙一致であり、Snapshot 固定は Exact と
  同じ振る舞いをする（`sec09_exact_scope_permits_listed_
  member`・`sec09_snapshot_scope_behaves_like_exact`）。
- **制約付き動的 Scope**: 明示条件（Resource Kind・Owner・
  時間範囲・Member 上限・対象 Revision の扱い）で評価する。
  条件を外れた対象・kind/owner/time-domain 不一致・上限超過・
  未解決/将来 member は拒否し、新たに追加された対象を暗黙に
  無制限許可しない（`sec10_dynamic_*` 系 10 件・
  `sec06_dynamic_scope_escape_denies`）。
- **継承禁止**: Project→Track→Clip→Note 等の親子関係から権限を
  無条件継承しない。共有 Source への書込は Source 側 Scope を
  要し、Clip 経由の関連参照は権限を連鎖しない
  （`sec05_shared_source_write_needs_source_scope`・
  `sec05_shared_source_not_inherited_through_clip`・
  `sec05_read_only_capability_does_not_follow_related`）。
- **作成/移動/未知効果**: 新規リソース作成は許可された親・
  Resource Kind・配置軸を検証し、既存対象編集と区別する
  （`sec11_create_*` 系）。移動は変更前後の所属・時間範囲を
  評価し、範囲外移動・検証不能引数を拒否する
  （`sec06_move_*` 系）。Unknown/検証不能な副作用は Delegated
  Apply でも自動許可しない（`sec14_unknown_effect_is_never_
  auto_permitted`・`sec14_unverifiable_effects_deny`・
  `data_effect_disclosure_and_transfer_checked`）。

### Invocation Target / Explicit Resolution

- 実行対象は **Project ID + Typed Stable Resource ID + 時間範囲
  + Expected Project Revision** に Command 発行前に解決する。
  Selection・Inspection Focus・Active Window/View・現在選択中
  等の可変 UI 状態から暗黙推測しない（AHG-006）。
- UI の選択が変わっても既存 Invocation Target/Grant の対象は
  変わらない（`sec07_selection_changes_do_not_retarget_the_
  grant`・`sys03_approval_binds_proposal_not_ambient_selection`）。
- Context 取得（読取専用 Snapshot/許可された Projection）は
  Apply 権限ではなく、別権限の明示的 Context API として扱う
  （`gt004_context_query_projects_pinned_snapshot`・
  `data01`〜`data10` 系）。

### Approval Receipt / ChangeSet

- **ChangeSet**: immutable な準備済み変更集合。digest で固定
  され、改竄は digest check で拒否される
  （`sec09_changeset_tamper_denies_at_digest_check`）。
  Proposal/Validation/Preview/Approval/Apply の状態を混同
  せず、Prepare/Approve/Preview は Revision を変更しない
  （`gt005`〜`gt010`・`gw10c_*`）。
- **Approval Receipt**: 特定の ChangeSet・Project・Principal・
  Expected Revision・期限に結び付いた承認。receipt は proposal
  に結び付き ambient selection に結び付かず、他 Principal の
  receipt は拒否、revision drift は conflict、revoke は未使用
  receipt を殺す（`sec10_foreign_approval_receipt_denies`・
  `sec11_revision_drift_after_approval_conflicts`・
  `sec12_revoked_grant_kills_the_unused_approval`・
  `gtv005_revoke_before_receipt_consumption_denies`・
  `sys04_expired_proposal_rejects_not_applies`）。
- **Apply Mode は Grant 由来**: Request が mode を自己申告
  しても Grant の mode が優先される（`ahg009_apply_mode_
  comes_from_grant_not_request`）。

### Apply Modes / Proactive 分離

| Mode | 契約 | 権限源 |
|---|---|---|
| Suggest | Proposal/Preview まで。Apply は別のユーザー操作 | Grant は Apply を許可しない（`ahg009_suggest_mode_denies_apply_but_permits_prestages`・`gw10f_suggest_mode_denies_apply`・`suggest_mode_prepares_but_denies_apply`） |
| Approve-to-Apply | 特定 ChangeSet の承認 receipt を消費して適用 | Approval Policy + receipt（`ahg009_approve_to_apply_requires_approval`・`approve_to_apply_requires_and_consumes_the_receipt`・`invoke_approve_to_apply_returns_approval_outcome`） |
| Delegated Apply | Grant の Scope/上限内で個別承認なし適用 | Execution Grant のみ（`ahg009_delegated_apply_permits_inside_budget`・`gw10f_delegated_apply_commits_through_the_host`・`delegated_apply_commits_through_the_seam`・`invoke_delegated_apply_runs_handler`） |
| Proactive | 事前明示許可された起動条件で AI が自発動作 | **Activation Authority** — Delegated Apply から導出しない（`ahg007_delegated_grant_is_not_proactive_authority`・`ahg007_live_proactive_permit_activates`・`ahg007_revoked/expired_proactive_permit_denies`・`ahg007_permit_for_other_principal_denies`・`ahg007_session_binding_*`・`ahg007_persistent_project_binding_permits`） |

- 3 Mode すべてで Validation・Stable ID/Revision 確認・Undo・
  Semantic Diff・Provenance・権限失効確認・RT 安全性を省略
  しない。権限上 Permit でも Semantic Validation 失敗時は
  Commit しない。Agent は自分の Grant を増権できず、複数
  Grant を無条件合成しない。

## State Machines

### Grant Lifecycle

```text
Issued → Active → Expired | Revoked | Superseded(policy epoch)
```

- Issued: 信頼境界で Principal Binding・Project・Scope・Data
  Access・Activation・Approval・Validity/Limits を確定して発行。
- Active: 有効期限内・未失効・Policy Revision が現行。
- Expired/Revoked: 以後の全評価で拒否。失効は永続化され、
  torn tail では確認済み失効を巻き戻さない
  （`rec08_confirmed_revoke_survives_torn_tail_does_not`・
  `gw10f_revocation_denies_the_next_call`・`revoked_grant_
  denies_the_very_next_call`）。
- Superseded: Policy epoch 更新で旧 Grant を失効させるが、
  ceiling 未設定の Grant は policy update を生き残る場合が
  ある（`sec12_policy_epoch_supersedes_grant`・`sec12_grant_
  without_ceiling_survives_policy_update`）。再発行は失効・
  消費済み budget を保持する（`ahg010_reissue_preserves_
  revocation_and_spend`）。

### Proposal / ChangeSet Lifecycle

```text
Prepared → Validated → Previewed → Approved(receipt) → Applied
                    ↘ Rejected | Expired | Conflicted | Cancelled
```

- Prepare/Approve/Preview は pinned base を変更しない
  （`gt005_prepare_leaves_the_pinned_base_untouched`・
  `gt010_prepare_approve_preview_leave_revision_untouched`・
  `gw10c_cancel_apply_leaves_base_unchanged`）。
- 状態を混同しない: Proposal 状態・Receipt・Commit・Save・
  Engine Applied・VCS Commit は別（`gt007_proposal_states_do_
  not_conflate`・`gtv002_observed_states_stay_distinct_across_
  the_lifecycle`・`sys05_sys06_state_nonconflation_is_
  observable`）。
- Apply は operation identity を持ち、commit receipt は
  proposal に結び付き replay を処理する（`gt008_accepted_
  apply_carries_an_operation_identity`・`gt009_commit_receipt_
  binds_proposal_and_replays`・`rec05_rec06_keyed_retry_replays_
  and_conflicts`）。
- Revision drift・stale proposal・expired proposal は
  conflict/reject であり暗黙適用しない（`gw10c_conflict_after_
  revision_drift`・`rec12_stale_proposal_never_commits_after_
  restart`・`sys04_expired_proposal_rejects_not_applies`）。

### Invocation / Commit Gate

```text
Request → Parse/Bind → Authorize → (Deny | Permit)
       → Prepare(proposal) → [Approval?] → Commit Gate
       → Revalidate(fresh state) → Single Writer Apply → Receipt
```

- Authorize は Capability・Resource Scope・Grant・Approval
  Policy・AI 実行/外部送信許可を検証し、Deny は fail-closed。
- Commit Gate は失効確定と Project Commit 最終認可の**確定順序**
  を持つ: revoke-first は commit を拒否し、commit-first は
  失効だけで自動 Undo しない（`rec01_revoke_first_denies_
  commit`・`rec02_commit_first_stands`・`gtv005_both_orderings_
  enumerated_deterministically`・`acg003_commit_revalidates_
  fresh_state`・`acg003_commit_before_revoke_stands`）。
- 同一 base への 2 commit は一方のみ成功（`rec03_two_commits_
  one_base_one_conflicts`）。Keyed retry は replay または
  conflict（`rec05_rec06`）。Crash point は committed または
  indeterminate に回復し torn state を残さない（`rec07`〜
  `rec12`）。
- Out-of-scope write は commit 段階でも拒否（`ac05_commit_
  stage_evaluation_denies_out_of_scope_writes`）。

## Failure Taxonomy（横断）

| 分類 | 代表ケース | 契約 |
|---|---|---|
| 認証/Grant 欠落 | `sec01_*`・`sec02_*`・`sec04_*` | Default Deny・存在 leak なし（`sec04_no_existence_leak_between_deny_reasons`） |
| Scope 外/継承違反 | `sec05_*`・`sec06_*`・`sec09_*`・`sec10_dynamic_*`・`sec11_*`・`sec14_*` | 固定/動的 Scope の定義評価・作成/移動/未知効果の安全判定 |
| Mode/Approval 違反 | `sec08_*`・`sec10_foreign_*`・`sec11_revision_*`・`sec12_revoked_*`・`ahg009_*` | mode は Grant 由来・receipt は proposal/ChangeSet/Revision に結合・改竄/再利用/他者 receipt 拒否 |
| Capability/Version | `sec13_*`・`gt003_*`・`sys08_unsupported_versions_refuse_typed` | Grant の Capability 集合・version 完全一致・新 Capability の暗黙許可なし |
| Budget/Limits | `ahg010_*`・`data08_bounded_budgets_reject_oversized_requests` | apply budget は commit に結び付き枯渇で拒否 |
| Data Access/外部送信 | `data01`〜`data10`・`data_effect_disclosure_and_transfer_checked` | class subset 強制・外部送信は独自 class・pagination は revision を混ぜない・失効後は新規開示なし・foreign operation は結果を読めない・prompt injection は authority を変えない・任意 URI は resource を漏らさない・tool discovery は Grant を拡張しない |
| Revoke/競合/冪等/Crash | `rec01`〜`rec12`・`gtv005_*`・`ac05_*` | 線形化順序・one-winner・keyed retry・crash recovery・torn tail |
| MCP 境界 | `gw10f_*`・`mcp_adapter.rs` 系 | session は permit record に結合・client claim を信頼しない・dead/revoked permit は blind fail・unknown tool/malformed call は typed refuse・disabled adapter は surface を deny |
| AI Off/人間経路 | `e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`・`sec01_human_needs_no_grant` | Grant なしで human pipeline が commit/undo/recover |

## Data Access / Rights / Provenance / Cost

- **Data Access class**: metadata・structured content・derived
  feature・external transfer を別 class として扱い、Grant の
  class subset を強制する（`data_access_class_subset_enforced`・
  `data_access_full_set_permits`・`data01_metadata_grant_cannot_
  read_structured_content`・`data02_derived_feature_disclosure_
  needs_its_class`・`data10_external_transfer_needs_its_class`）。
  未許可 Media Read・外部送信は拒否（AIB-AC-01）。
- **Unknown 明示**: Model Rights・Provider/モデル/バージョン・
  生成経緯・ライセンス/学習データ・コストは、把握できる範囲で
  記録し、不明は Unknown/Unverified/Unavailable として扱う。
  観測事実・申告・推定・未確認を混同せず、根拠なく「AI 不使用」
  「権利クリア」と保証しない（AIB-005）。
- **保証境界**: 外部 Agent へ情報を渡した後のモデル利用・保持・
  第三者 Plugin/外部素材の内部 AI 利用は Synvelis が完全には
  検証できない。「Synvelis 管理下で AI 処理を実行しない保証」
  と「AI 不使用保証」を区別する（AIB-006）。
- **監査最小化**: Principal/Grant/Task/ChangeSet/Commit 結果等の
  最小必要情報を記録し、Prompt/素材全文の無制限保存を前提に
  しない（AHG-010）。
- **Secret 取り扱い**: permit/secret は file store が restart
  後も漏洩せず、tampered secret hash は fail-closed
  （`file_store_survives_restart_and_never_leaks_secrets`・
  `tampered_secret_hash_fails_closed`）。

## RT / nonRT Boundary

- モデル実行・ネットワーク・外部サービス・生成 Asset の検証/
  登録・Agent Job は **nonRT** で扱い、Audio RT Thread で
  無期限待機・I/O・推論を行わない（AIB-006）。
- headless 証拠は RT boundary が**記録される**ことを示すが、
  実 RT thread での無待機を実測したものではない
  （`sys07_rt_boundary_is_recorded_not_claimed` — 記録と主張を
  区別）。実機 RT lane は #46/#114 の別 Gate。
- 生成物は既存 Verified Media/Project Commit の境界に従い、
  未検証生成物を正規 Source として無断公開しない
  （`rec11_crash_after_apply_leaves_verified_source_and_no_clip`）。

## Alternatives and Rejection Rationale

| 代替案 | 却下理由 |
|---|---|
| AI が UI 疑似クリック/Project Store 直接書換えで編集 | AIB-004: 手動編集と共通の typed Semantic Command/Controller・Stable ID/Expected Revision 検証・Undo/Semantic Diff を迂回するため却下 |
| UI Selection/Focus を暗黙の実行対象・権限根拠にする | AHG-006: 複数 Window/View で一意でなく、選択変更が権限を再解釈するため却下。明示 ID/Revision 解決へ収束 |
| Track 権限を Clip/Asset へ無条件継承 | AHG-008: 共有参照・Plugin/Routing は単純ツリーでなく、権限昇格を許すため却下 |
| 単一の「AI 許可」権限に Grant/Activation/Approval を束ねる | AHG-007: 情報開示/モデル実行/外部送信/Project 編集/開始権限/個別承認は別契約であり、束ねると最小権限と Delegated≠Proactive を壊すため却下 |
| Delegated Apply を Proactive 起動権限として扱う | AHG-004/AHG-007: 自動適用は自発的作業開始を意味しないため却下。Activation Authority を分離 |
| MCP を内部 Project Model/Command 契約の正本にする | AHG-002: MCP は公開 Adapter であり、内部契約・権威は Synvelis Gateway が保持するため却下 |
| 新 Capability を既存 Grant で暗黙許可 | AHG-007 Default Deny/Least Privilege: Capability 追加が既存権限を拡大しないため却下（`gt003`） |
| Ruviform 汎用 Gateway/認証を Synvelis 専用に重複実装 | Issue 範囲: Ruviform は再利用基盤であり、DAW 固有意味論を持ち込まない。共通化は実需検証後に別途判断 |
| 事前 Check＋後段 Check のみで Commit 原子性を主張 | AHG-010 TOCTOU: 失効確定と最終認可に確定順序が必要なため却下（`rec01`/`rec02`/`gtv005`） |
| headless fixture PASS を製品/実機受入と同一視 | POLICIES.md: OS-L1/GUI-L2/EXT-L3 と Required/Deferred/NotApplicable・Pass/Fail/NotRun/Unsupported を分離。外部 Provider/実 GUI/実機は NotRun/Unsupported のまま |

## Requirement → Mechanism → Evidence 写像

| 要件 | 設計機構 | 代表 fixture（headless） | 現在 status |
|---|---|---|---|
| AIB-001 AI なしで完結 | AI Off 既定・human は Grant 不要・同一 Semantic Project/Controller | `e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`・`sec01_human_needs_no_grant` | Pass(部分) — headless。製品 UX/実機は #46/#114 |
| AIB-002 Intentional Usage | 既定 OFF・実行許可と利用可能の区別・無許可 fallback 禁止・確認/制御 | `disabled_adapter_denies_the_surface`・`e2e1_*`・AI Off 系 | Pass(部分) — UI/Provider 接続は未検証 |
| AIB-003 明示指示/Proactive 分離 | Activation Authority を Grant/Apply Mode と別契約化 | `ahg007_*` 系 | Pass(部分) — headless。Proactive UI は未実装 |
| AIB-004 共通 Command/履歴 | typed Semantic Command・Stable ID/Expected Revision・Preview/Undo/Diff・状態分離 | `gt004`〜`gt010`・`gw10c_*`・`sys05_sys06`・`gtv002` | Pass(部分) |
| AIB-005 Rights/来歴/透明性 | Unknown 明示・class 分離・最小監査・secret fail-closed | `data01`〜`data10`・`data_effect_*`・`file_store_*`・`tampered_secret_hash_fails_closed` | Pass(部分) — provenance schema/権利 UI は未決定 |
| AIB-006 技術的強制/RT | 共通許可境界・nonRT Job・未検証生成物の非公開・保証境界明示 | `sys07_rt_boundary_is_recorded_not_claimed`・`rec11_*`・`sec14_*` | Pass(部分) — 実 RT thread 未実測 |
| AIB-007 モデル非依存 | Provider 抽象は未決定・製品原則として固定のみ | （設計上の非対象 — 実装 fixture なし） | NotRun — Provider 契約は #45/#46 |
| AHG-001 Pluggable Hybrid | Synvelis 権威・共通意味契約・交換可能 Runtime・Job 所有非委譲 | `gw10_*` 全体・`e2e1_*` | Pass(部分) |
| AHG-002 MCP 初期連携 | Adapter 位置づけ・明示有効化・信頼境界・限定 Projection/Job Handle | `gw10f_*`・`mcp_adapter.rs` 系 | Pass(部分) — 実外部 MCP Client は NotRun |
| AHG-003 Gateway 論理契約 | Capability/Invocation/Authorization/Context/Mutation/Execution | `gw10_contract.rs`・`gw10_authorization.rs` | Pass(部分) — 型詳細は #45 |
| AHG-004/AHG-009 3 Mode | mode は Grant 由来・receipt 結合・budget・各境界認可 | `ahg009_*`・`sec08_*`・`gw10f_*`・`mcp_adapter` 3 mode 系・`gw10_transport` 3 mode E2E | Pass(部分) |
| AHG-005 既存整合 | ProjectController Single Writer・別 Writer/Undo 禁止・Ruviform 無変更 | `e2e1_*`・`gw10c_*`・`rec10_saves_publish_head_in_commit_order` | Pass(部分) |
| AHG-006 Explicit Target | UI 状態非依存・明示 ID/Revision 解決・選択変更で対象不変 | `sec07_selection_changes_do_not_retarget_the_grant`・`sys03_*`・`gt004_*` | Pass(部分) |
| AHG-007 Grant/Policy 論理型 | Principal/Grant/Activation/Approval 分離・Default Deny・Least Privilege | `sec01`〜`sec04`・`sec12_*`・`sec13_*`・`ahg007_*`・`ahg010_*` | Pass(部分) — 具体 Rust 型は #45 |
| AHG-008 Scope/効果評価 | 固定/動的 Scope・継承禁止・作成/移動/未知効果 | `sec05_*`〜`sec11_*`・`sec14_*`・`ac05_*` | Pass(部分) |
| AHG-010 Validity/Revoke/Commit | 線形化・TOCTOU・冪等・crash recovery・最小監査 | `rec01`〜`rec12`・`gtv005_*`・`sys02_*`・`sys04_*` | Pass(部分) — power-loss/実機は NotRun |
| AHG-011 受入/変更管理 | 8 受入条件・不変条件の非緩和・見直しは明示変更 | 本 evidence 台帳・`evidence/sv44/` | Pass(部分) — 設計レビュー済み・製品受入は別 Gate |
| AI-GATE 独立第四軸 | Gate A/B/C と独立・AI Off でも A を壊さない・逆も同様 | `e2e1_*`・`sys01`〜`sys10` 系・`gw10_system.rs` | Pass(部分) — 製品 Gate 判定は #23/#46 |

## Risks

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | headless fixture PASS を製品/実機受入と誤認 | med | high | evidence 台帳で `Pass(部分)`/`NotRun`/`Unsupported` を分離。実 GUI/OS/Provider/実機 lane は #46/#114 の別 Gate として未検証のまま記録 |
| 2 | #45/#46 が本設計の不変条件を継承せず詳細契約が矛盾 | med | high | relations で #45/#46 を references/related に固定し、AIB/AHG → fixture 写像を台帳化。矛盾発見時は本 spec/design を更新 |
| 3 | 未決定事項を受入 Gate 化してしまい実装を過剰拘束 | med | med | spec の Deferred Design Concerns で「独立した受入 Gate ではない」と明示。合意済み AIB/AHG のみを受入対象に限定 |
| 4 | AI Off 既定が通常 DAW 経路へ影響し Gate A を壊す | low | high | `e2e1_ai_off_human_pipeline_commits_undoes_and_recovers` で human pipeline の commit/undo/recover を headless 確認。製品 Gate A 判定は #23/#1 |
| 5 | Grant/Scope/Approval の論理契約が #45 の具体型で実現不能 | med | med | 本設計は論理契約に限定し、具体 Rust 型・永続化・線形化手法は #45/#46 の未確定事項として明示 |
| 6 | Ruviform 汎用基盤へ DAW 固有意味論を混入 | low | med | Owner 分離表で Ruviform の責務を固定し、Synvelis 固有の Grant/Scope/MCP 意味論を Ruviform へ移さないことを明示 |

## Deferred Design Concerns

- Execution Grant の具体 Rust 型・認証方式・Principal
  Binding の永続化表現 | #45 | 型契約が本設計の論理属性を
  正確に写像すること。
- Approval Policy の意味・ChangeSet との結合・承認有効期限・
  再利用防止・一括処理・操作中の Policy 変更 | #45/#46 |
  receipt の単一使用・期限・結合対象が本設計どおりか。
- MCP Client 認証・ローカル接続安全性・外部情報流出境界・
  Read/Media/AI/External Transfer の権限分離の実装 | #46 |
  `data01`〜`data10` の class 分離が実 transport で維持されるか。
- 実行中の権限取消・競合・再試行・タイムアウト・Cancel・
  Project Controller との二重検証を含む TOCTOU の実装手法 |
  #46 | `rec01`/`rec02`/`gtv005` の順序保証が実装で維持されるか。
- ChangeSet/Context/Job の具体 Schema・Snapshot/Revision 整合・
  永続化/監査ログ・プライバシー設計 | #45/#46 | 状態分離・
  digest 固定・最小監査が維持されるか。
- 限定 MIDI 編集を対象にした 3 Mode E2E と拒否・失効・重複
  適用・外部 Agent 切断の検証計画の製品受入 | #46/#114 |
  headless 証拠を製品受入に正しく格上げ/格下げするか。
- Grant 設定 UI・同意 UI・権限失効 UI・Provenance/権利確認
  UI・Privacy/Retention | #46/#114 | UI が権限を暗黙拡大しないか。
- AI 使用許可の粒度・期限・失効の既定値・Agent 自律性
  レベル・実行予算・中断・人間介入 | 未決定（Issue #44 継続
  議論） | 既定値を実測なしに確定しない。
- Model Provider/Capability 契約・ローカル/クラウド切替・
  コスト・障害時挙動 | 未決定 | Provider 接続を通常 DAW へ
  強制しない。
- 最初の最小 AI E2E PoC/失敗系テスト/他 Consumer 互換テストと
  既存実行計画への接続 | #23/#46 | F0/E2E 計画を暗黙変更しない。

## Verification Notes

- 本設計の検証は `docs/test-specs/ai-native-baseline.md` と
  `evidence/sv44/` 台帳に従う。headless fixture の PASS は
  `Pass(部分)` として記録し、実機/GUI/OS/外部 Provider/実
  MCP Client の受入は `NotRun`/`Unsupported` のまま分離する。
- `mise run gw10`（G0〜G6/GTV）・`mise run gw10-report`・
  `mise run test synvelis-gateway`・`mise run fmt-check`・
  doc-status lint/audit・index 再生成・impl-doc audit・
  review-protocol を実行し、結果を
  `evidence/sv44/ai-native-baseline-verification.md` に記録
  する。
- 実行結果は `evidence/sv44/ai-native-baseline-case-ledger.md`
  で AIB/AHG/受入条件 → fixture → Oracle → status → 証拠 →
  後続に写像する。
