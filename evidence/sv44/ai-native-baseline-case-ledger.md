# AI-Native Hybrid DAW Baseline — AIB/AHG Case Ledger (synvelis#44)

Issue #44 の AIB-001〜007・AHG-001〜011 と受入条件
AIB-AC-01〜08 について、fixture・oracle・Owner・status・証拠を
台帳化する。**本 slice は docs/ 設計契約 + evidence のみ**で
あり、実装・GUI・実 device/plugin/OS・実外部 Provider/MCP
Client・ユーザー承認 UI 受入は対象外。

Fixture 表記は `path::test_fn`（Rust integration test の
関数名）とする。`sec*`/`data*`/`ahg*`/`rec*`/`gt*`/`sys*`/
`gw10f_*`/`gw10c_*`/`e2e1_*` は `crates/synvelis-gateway/tests/`
内の test 関数名、`mcp_adapter.rs` 系は同 crate の MCP
adapter test、`gw10_transport.rs` は `crates/mcp-poc/tests/`
内の real transport E2E。

## Status 語彙

| Status | 意味 |
|---|---|
| `Pass` | 実機/対象環境で検証が成功した（本 slice では未使用） |
| `Pass(部分)` | headless fixture で関連契約が確認できるが、当該要件全体の独立 oracle・実機/GUI/OS/Provider/MCP Client/承認 UI 受入を満たさない |
| `Fail` | 検証が失敗した |
| `Unsupported` | 当該 capability が未対応として明示されている |
| `NotRun` | 未実装または未検証 |

headless Pass を実機/GUI/OS/Provider/MCP Client/承認 UI 受入に
繰り上げない。設計レビュー Pass と実測 Pass を区別する。

## Fixture 凡例（実在 file）

- `crates/synvelis-gateway/tests/`: `authorization.rs`（74
tests）・`gw10_authorization.rs`（25）・`gw10_concurrency.rs`
（7）・`gw10_contract.rs`（3）・`gw10_harness.rs`（8）・
`gw10_job.rs`（9）・`gw10_mcp.rs`（7）・`gw10_persistence.rs`
（7）・`gw10_project.rs`（9）・`gw10_system.rs`（7）・
`mcp_adapter.rs`（27）・`prepare.rs`・`prepare_regressions.rs`・
`proposal.rs`・`proposal_regressions.rs`・`schema_compat.rs`・
`contract.rs`・`fixtures.rs`・`job.rs`
- `crates/mcp-poc/tests/`: `gw10_transport.rs`（4 real
  transport E2E）

## Case Ledger

### AIB-001 Manual/AI の対等な共存、AI なしで完結する DAW（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | 録音・再生・通常編集・MIDI・Mix・Export・Save/Load 等の通常 DAW ワークフローは Synvelis 管理下の AI 機能なしに成立。手動編集と AI 編集は同一 Semantic Project 正本に作用。AI 生成物も通常編集可能な制作対象 |
| Owner | #44（本 slice）・#17（ProjectController）・#23（実行計画） |
| Fixture | `crates/synvelis-gateway/tests/gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`・`authorization.rs::sec01_human_needs_no_grant`・`gw10_project.rs::gw10c_commit_advances_revision_and_undo_rewinds` |
| Oracle | human principal が Grant なしで commit/undo/recover・AI 無効でも Project 正本・Revision・Undo が通常 DAW 契約どおり動作 |
| Status | **Pass(部分)** — headless。実 GUI/OS での Save/Playback/Export 継続・製品 Gate A 判定は #23/#114 の別 Gate |
| 証拠 | `docs/specs/ai-native-baseline.md` AIB-001、`docs/designs/ai-native-baseline-design.md` AI Off/state machines |
| 後続 | #46（AI OFF 製品受入）・#114（実機）・#23（Gate A 判定） |

### AIB-002 Intentional AI Usage（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Synvelis 管理下の AI は初期状態で無効。明示的利用意思・許可なしに推論/生成/AI 解析/AI 起因の外部データ送信を開始しない。「利用可能」と「実行許可」を区別。無許可 AI 自動フォールバック禁止。処理内容・対象範囲・Provider/モデル・ローカル/外部実行・外部送信を確認・制御。AI 無効時の既存 Project 手動利用を妨げない |
| Owner | #44（本 slice）・#46（製品受入） |
| Fixture | `crates/synvelis-gateway/tests/mcp_adapter.rs::disabled_adapter_denies_the_surface`・`gw10_system.rs::e2e1_ai_off_human_pipeline_*`・`authorization.rs::sec01_agent_without_any_authority_denied`・`gw10_mcp.rs::gw10f_dead_permits_fail_blind` |
| Oracle | disabled adapter は surface を deny・authority なし agent は拒否・dead/revoked permit は blind fail・AI 無効でも human pipeline は継続 |
| Status | **Pass(部分)** — headless。実 UI の「利用可能/実行許可」区別・Provider 接続・外部送信の実測は未検証 |
| 証拠 | `docs/specs/ai-native-baseline.md` AIB-002、`docs/designs/ai-native-baseline-design.md` AI Off 既定 |
| 後続 | #46（AI OFF 製品受入・権限 UX）・#114（実機） |

### AIB-003 明示指示先行・将来の自発性は許可範囲に限定（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | 初期の AI 実行はユーザーの明示指示を起点。将来の proactive/agent 動作は事前の明示的有効化と権限指定を要し停止・取消可能。「自発的に提案できる」と「自発的に Project を変更・外部送信できる」は別権限 |
| Owner | #44（本 slice）・#46（製品受入） |
| Fixture | `crates/synvelis-gateway/tests/authorization.rs::ahg007_delegated_grant_is_not_proactive_authority`・`ahg007_live_proactive_permit_activates`・`ahg007_revoked_proactive_permit_denies`・`ahg007_expired_proactive_permit_denies`・`ahg007_permit_for_other_principal_denies`・`ahg007_session_binding_*`・`ahg007_persistent_project_binding_permits` |
| Oracle | Delegated Apply は Proactive 起動権限を含まない・live permit のみ activate・revoked/expired/他 Principal の permit は拒否・session/persistent binding は adapter/Project に一致 |
| Status | **Pass(部分)** — headless。実 Proactive UI・自律性レベル・実行予算は未決定/#46 |
| 証拠 | `docs/specs/ai-native-baseline.md` AIB-003、`docs/designs/ai-native-baseline-design.md` Apply Modes/Proactive 分離 |
| 後続 | #46（Proactive/自律性 UX）・#45（Activation Authority 型） |

### AIB-004 共通 Command/検証/変更履歴（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | AI は UI 疑似クリック/Project Store 直接書換えを標準経路とせず、手動編集と共通の typed Semantic Command/Controller 契約を利用。Stable Target ID・Expected Project Revision で対象・競合を検証。Preview/Validation/Apply/Undo/Redo/Semantic Diff/失敗時保護。適用済み状態と提案・生成のみの状態を区別 |
| Owner | #44（本 slice）・#17（ProjectController）・#45（型契約） |
| Fixture | `crates/synvelis-gateway/tests/gw10_project.rs::gt005_prepare_leaves_the_pinned_base_untouched`・`gt006_effects_derive_from_real_before_after_attributes`・`gt007_proposal_states_do_not_conflate`・`gt008_accepted_apply_carries_an_operation_identity`・`gt009_commit_receipt_binds_proposal_and_replays`・`gt010_prepare_approve_preview_leave_revision_untouched`・`gw10c_commit_advances_revision_and_undo_rewinds`・`gw10c_conflict_after_revision_drift`・`gw10c_cancel_apply_leaves_base_unchanged`・`gw10_system.rs::sys05_sys06_state_nonconflation_is_observable`・`gtv002_observed_states_stay_distinct_across_the_lifecycle` |
| Oracle | Prepare/Approve/Preview は pinned base を変更しない・proposal 状態と commit/save/engine/vcs を混同しない・apply は operation identity を持ち receipt は proposal に結合・revision drift は conflict・cancel は base 不変 |
| Status | **Pass(部分)** — headless。Semantic Diff・Undo の製品 UX・大規模変更の分離 Snapshot 比較は #45/#46 |
| 証拠 | `docs/specs/ai-native-baseline.md` AIB-004、`docs/designs/ai-native-baseline-design.md` Proposal/ChangeSet state machines |
| 後続 | #45（ChangeSet 型）・#46（Preview/Apply UX）・#7（Semantic Diff） |

### AIB-005 権利・来歴・透明性（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | AI 使用許可・生成物の利用条件/権利関係・著作権上の適法性は別概念。把握可能範囲で AI 利用有無・生成/変換種別・Provider/モデル/バージョン・入力参照・生成物 ID・適用履歴・条件/制限・外部送信範囲を追跡。不明は Unknown/Unverified/Unavailable。観測事実/申告/推定/未確認を混同しない。AI 関連記録自体の秘匿設計 |
| Owner | #44（本 slice）・#46（Provenance/権利 UI） |
| Fixture | `crates/synvelis-gateway/tests/gw10_authorization.rs::data01_metadata_grant_cannot_read_structured_content`・`data02_derived_feature_disclosure_needs_its_class`・`data03_pagination_never_mixes_revisions`・`data04_revoked_grant_discloses_nothing_new`・`data05_foreign_operation_id_cannot_fetch_results`・`data06_prompt_injection_in_metadata_changes_no_authority`・`data07_arbitrary_uri_never_leaks_a_resource`・`data08_bounded_budgets_reject_oversized_requests`・`data09_tool_discovery_does_not_extend_the_grant`・`data10_external_transfer_needs_its_class`・`authorization.rs::data_access_class_subset_enforced`・`data_access_full_set_permits`・`data_effect_disclosure_and_transfer_checked`・`mcp_adapter.rs::file_store_survives_restart_and_never_leaks_secrets`・`tampered_secret_hash_fails_closed` |
| Oracle | Data Access class subset を強制・外部送信は独自 class・pagination は revision を混ぜない・失効後は新規開示なし・foreign operation は結果を読めない・prompt injection は authority を変えない・任意 URI は resource を漏らさない・tool discovery は Grant を拡張しない・secret は restart 後も漏洩せず tamper は fail-closed |
| Status | **Pass(部分)** — headless。生成 Asset の Provenance Schema・モデル/ライセンス Unknown 表現・権利確認 UI・Prompt/素材保存範囲は未決定/#46 |
| 証拠 | `docs/specs/ai-native-baseline.md` AIB-005、`docs/designs/ai-native-baseline-design.md` Data Access/Rights/Provenance/Cost |
| 後続 | #45（Provenance Schema）・#46（権利/Privacy UI）・#114（実機） |

### AIB-006 AI 境界の技術的強制と RT 安全性（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | AI 利用制御を UI 表示だけに依存させず共通の許可/ポリシー境界。モデル実行・ネットワーク・外部サービス・生成 Asset 検証/登録・Agent Job は非 RT。RT Thread で無期限待機・I/O・推論を行わない。未検証生成物を正規 Source として無断公開しない。保証範囲を限定 |
| Owner | #44（本 slice）・#46（RT/AI OFF Gate）・#114（実機） |
| Fixture | `crates/synvelis-gateway/tests/gw10_system.rs::sys07_rt_boundary_is_recorded_not_claimed`・`gw10_persistence.rs::rec11_crash_after_apply_leaves_verified_source_and_no_clip`・`gw10_authorization.rs::sec14_unknown_effect_is_never_auto_permitted`・`sec14_unverifiable_effects_deny`・`authorization.rs::sec14_*` |
| Oracle | RT boundary が記録される（実測ではなく記録）・crash after apply は verified source を残し clip を残さない・unknown/unverifiable effect は auto-permit しない |
| Status | **Pass(部分)** — headless。実 RT thread での無待機・実機 RT lane・実 plugin・実 OS は未実測（`sys07` は記録のみで実測を主張しない） |
| 証拠 | `docs/specs/ai-native-baseline.md` AIB-006、`docs/designs/ai-native-baseline-design.md` RT/nonRT Boundary |
| 後続 | #46（RT/AI OFF Gate）・#114（実機 RT）・#23（通常 DAW RT 契約） |

### AIB-007 モデル非依存・Provider 交換可能性（NotRun）

| 項目 | 内容 |
|---|---|
| 要求 | ローカル/クラウド・生成/解析・テキスト/MIDI/Audio の能力差を扱える設計。特定モデル/事業者への永続依存を製品原則にしない。モデル選択や外部接続が通常 DAW 制作を暗黙に変質させない |
| Owner | #44（本 slice・設計原則として固定）・#45（Provider/Capability 契約）・#46（実装/製品受入） |
| Fixture | （設計上の非対象 — Provider 抽象・機能差・権限差・コストの扱いは未決定。実装 fixture なし） |
| Oracle | Provider 接続なしでも通常 DAW が動作（AIB-001/AC-08 で被覆）。Provider 固有の能力差・切替・障害時挙動は未実装 |
| Status | **NotRun** — Provider 契約・Capability 抽象・コスト/障害時挙動は未決定/#45/#46 |
| 証拠 | `docs/specs/ai-native-baseline.md` AIB-007・Deferred Design Concerns |
| 後続 | #45（Provider/Capability 契約）・#46（Provider 接続・製品受入） |

### AHG-001 Pluggable Hybrid（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Synvelis は DAW 正本・Capability/Command Gateway・AI 利用許可・Context 公開範囲・ChangeSet 検証/適用の権威を保持。手動 UI/外部 MCP Agent/将来の内蔵 Agent は共通の意味的操作契約。手動 DAW 操作を AI 有効化に依存させない。Agent Reasoning Runtime は交換可能。DAW Job 所有・実行許可・結果検証・登録・Project 状態整合は外部 Agent 任せにしない |
| Owner | #44（本 slice）・#45/#46（実装/受入） |
| Fixture | `crates/synvelis-gateway/tests/gw10_*.rs` 全体・`gw10_system.rs::e2e1_ai_off_human_pipeline_*`・`gw10_job.rs` 系・`mcp_adapter.rs` 系 |
| Oracle | Synvelis が権威を保持・MCP は adapter・human は Grant 不要・job は Synvelis が所有・結果検証/登録/Project 整合は Synvelis |
| Status | **Pass(部分)** — headless。内蔵 Agent・交換可能 Runtime の実装・製品受入は未実装 |
| 証拠 | `docs/designs/ai-native-baseline-design.md` Owner 分離表、`docs/specs/ai-native-baseline.md` AHG-001 |
| 後続 | #45（型契約）・#46（MCP/内蔵 Agent 製品受入） |

### AHG-002 MCP Server 初期連携（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | 初期連携方式に MCP Server を採用し内部型付き Gateway の Adapter として扱う。MCP を内部 Project Model/Command 契約の正本としない。明示有効化・権限確認。接続/Tool 一覧/推論・Project 読取/適用は別概念。信頼境界で接続主体を特定。Context の限定的 Projection・権限付き操作・独立 Job Handle |
| Owner | #44（本 slice）・#46（MCP 製品受入） |
| Fixture | `crates/mcp-poc/tests/gw10_transport.rs::gw10f_permit_cli_lifecycle_real`・`gw10f_delegated_apply_commits_and_survives_restart_real`・`gw10f_boundary_attacks_fail_closed_real`・`gw10f_three_modes_over_real_transports`・`crates/synvelis-gateway/tests/gw10_mcp.rs::gw10f_*`・`mcp_adapter.rs` 系 |
| Oracle | real transport で 3 mode E2E・permit lifecycle・boundary attack fail-closed・session は permit record に結合・client claim を信頼しない・disabled adapter は surface を deny・unknown tool/malformed call は typed refuse |
| Status | **Pass(部分)** — headless real transport。実外部 MCP Client/Provider・ユーザー承認 UI・実 OS transport は NotRun |
| 証拠 | `docs/specs/ai-native-baseline.md` AHG-002、`docs/designs/ai-native-baseline-design.md` MCP Adapter 位置づけ |
| 後続 | #46（実外部 MCP Client/Provider・承認 UI）・#114（実機） |

### AHG-003 Gateway 論理契約（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Capability（操作 ID・Schema・対象種別・効果・必要権限・実行方式/利用可否）・Invocation（要求 ID・Project/Stable Target ID・Expected Revision・引数・冪等性キー・信頼境界で付与する Principal/Origin/Grant）・Authorization（Capability・Scope・Grant・Approval Policy・AI 実行/外部送信許可）・Context（読取専用 Snapshot/Projection・Revision・観測/推定/不明）・Mutation（Proposal→Validation/Preview→Approval→ProjectController Apply）・Execution（nonRT Job・進捗/取消/結果/Receipt）の 6 要素 |
| Owner | #45（型契約）・#46（実装/受入）・GW-01〜10（headless 実装） |
| Fixture | `crates/synvelis-gateway/tests/gw10_contract.rs`・`gw10_authorization.rs`・`contract.rs`・`schema_compat.rs`・`fixtures.rs` |
| Oracle | Capability registry・typed invocation・versioned result/error・authorization stage・pinned snapshot context・proposal/commit state が headless で動作 |
| Status | **Pass(部分)** — headless。具体 Rust 型・wire Schema・永続化形式は #45 が Owner |
| 証拠 | `docs/designs/gw-01-gateway-contract-design.md`・`docs/designs/ai-native-baseline-design.md` AHG-003 |
| 後続 | #45（型契約確定）・#46（実装/受入） |

### AHG-004/AHG-009 3 Apply Mode/Approval 評価（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Suggest（提案のみ・Apply 権限なし）/ Approve-to-Apply（特定 ChangeSet の承認に結合）/ Delegated Apply（Grant 範囲内の個別承認なし適用）の 3 mode を初期 MCP 連携から実際に動作。Execution Grant と Approval Policy を別契約。Delegated Apply ≠ Proactive。3 mode すべてで Validation/Revision/Undo/Diff/Provenance/失効確認/RT 安全性を省略しない。Agent は自分の Grant を増権できない。複数 Grant を無条件合成しない |
| Owner | #44（本 slice）・#45（型契約）・#46（製品受入） |
| Fixture | `crates/synvelis-gateway/tests/authorization.rs::ahg009_suggest_mode_denies_apply_but_permits_prestages`・`ahg009_approve_to_apply_requires_approval`・`ahg009_delegated_apply_permits_inside_budget`・`ahg009_apply_mode_comes_from_grant_not_request`・`ahg010_apply_budget_exhausts`・`ahg010_budget_binds_at_commit`・`ahg010_commit_without_prior_apply_enforces_budget`・`ahg010_reissue_preserves_revocation_and_spend`・`gw10_authorization.rs::sec08_suggest_mode_apply_denies_without_revision_change`・`gw10_mcp.rs::gw10f_suggest_mode_denies_apply`・`gw10f_delegated_apply_commits_through_the_host`・`mcp_adapter.rs::suggest_mode_prepares_but_denies_apply`・`delegated_apply_commits_through_the_seam`・`approve_to_apply_requires_and_consumes_the_receipt`・`proposal.rs::suggest_mode_never_applies`・`delegated_apply_needs_no_receipt`・`approve_to_apply_flows_through_request_grant_consume`・`double_consumption_can_never_succeed`・`crates/mcp-poc/tests/gw10_transport.rs::gw10f_three_modes_over_real_transports` |
| Oracle | Suggest は Apply を拒否し prestage のみ・Approve-to-Apply は承認 receipt を要求・Delegated Apply は Grant の Scope/budget 内のみ・mode は Request でなく Grant 由来・budget は commit に結合・receipt の二重消費は成功しない・3 mode E2E が real transport で動作 |
| Status | **Pass(部分)** — headless real transport。実外部 MCP Client/Provider・ユーザー承認 UI・実 OS は NotRun |
| 証拠 | `docs/specs/ai-native-baseline.md` AHG-004/AHG-009、`docs/designs/ai-native-baseline-design.md` Apply Modes |
| 後続 | #45（Approval Policy 型）・#46（承認 UI・3 mode 製品受入） |

### AHG-005 既存設計との整合と実装境界（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Single Writer は Synvelis ProjectController。Gateway は別 Project Writer・別 Undo 基盤を作らない。通常 UI Command/Selection/Focus を維持し Agent 経由は同じ検証済み編集契約へ収束。Ruviform Generic Runtime はそのまま利用。既存 F0/E2E-L/R/X を変更しない |
| Owner | #44（本 slice）・#17（ProjectController）・#45/#46（実装） |
| Fixture | `crates/synvelis-gateway/tests/gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`・`gw10_project.rs::gw10c_*`・`gw10_persistence.rs::rec10_saves_publish_head_in_commit_order`・`prepare.rs::prepared_op_commits_through_controller_and_undoes` |
| Oracle | human/agent とも ProjectController 経由で commit/undo・revision/save が commit 順序で publish・prepared op は controller を通り undo できる |
| Status | **Pass(部分)** — headless。製品 UX・実機での Single Writer 維持は #46/#114 |
| 証拠 | `docs/designs/ai-native-baseline-design.md` Owner 分離表、`docs/specs/ai-native-baseline.md` AHG-005 |
| 後続 | #17（ProjectController 契約維持）・#45/#46（実装/受入） |

### AHG-006 Explicit Target Resolution / Gateway は UI 状態非依存（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Selection/Inspection Focus/Active Window/View/現在選択中からの暗黙対象推測禁止。Command 発行前に Project ID・Typed Stable Resource ID・時間範囲/Expected Revision へ解決。MCP/内蔵 Agent/CLI/Test も同じ明示的対象指定契約へ収束。Grant 対象は発行時明示 ID/Scope に結合。UI 選択変更で Grant 対象は自動変更されない。Context 取得は Apply 権限ではない |
| Owner | #44（本 slice）・#2/#8（UI Selection/Command）・#45（型契約） |
| Fixture | `crates/synvelis-gateway/tests/gw10_authorization.rs::sec07_selection_changes_do_not_retarget_the_grant`・`gw10_system.rs::sys03_approval_binds_proposal_not_ambient_selection`・`gw10_authorization.rs::gt004_context_query_projects_pinned_snapshot`・`authorization.rs::sec07_expired_grant_denied`（expired は別評価） |
| Oracle | UI selection 変更で既存 Invocation Target/Grant 対象が変わらない・approval は proposal に結合し ambient selection に結合しない・context query は pinned snapshot を投影 |
| Status | **Pass(部分)** — headless。実 GUI の複数 Window/View・Context API UI は #46/#114 |
| 証拠 | `docs/specs/ai-native-baseline.md` AHG-006、`docs/designs/ai-native-baseline-design.md` Invocation Target/Explicit Resolution |
| 後続 | #2/#8（UI 契約維持）・#45（Target 型）・#46/#114（Context API UI） |

### AHG-007 Execution Grant/Policy の論理型と責務（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Authentication/Identity・Execution Grant・Activation Authority・Approval Policy を別契約。論理 Grant 属性（Grant ID・Schema/Policy Revision・Issued By・Principal Binding・Project ID・Capability 集合・Resource Scope・Data Access・Activation・Approval・Validity/Revocation・Limits）。MCP 認証済み≠Project アクセス許可。外部 Agent の自己申告を信頼しない。Default Deny/Least Privilege。Grant 許可と AI 処理開始許可は独立。Delegated Apply は Proactive 起動の許可ではない。情報開示/モデル実行/外部送信/Project 編集を同一権限として束ねない |
| Owner | #44（本 slice）・#45（型契約）・GW-02（headless 実装） |
| Fixture | `crates/synvelis-gateway/tests/authorization.rs::sec01_agent_without_any_authority_denied`・`sec01_human_needs_no_grant`・`sec01_grant_ref_alone_is_not_proof`・`sec02_authority_issued_for_another_principal_denied`・`sec02_wire_authority_fields_cannot_self_declare`・`sec03_authority_for_another_project_denied`・`sec04_unknown_grant_ref_denied`・`sec04_unknown_task_ref_denied`・`sec04_no_existence_leak_between_deny_reasons`・`sec04_binding_mismatch_denied`・`sec12_policy_epoch_supersedes_grant`・`sec12_grant_without_ceiling_survives_policy_update`・`sec13_capability_outside_grant_set_denied`・`sec13_capability_version_is_exact`・`sec13_capability_descriptor_mismatch_denied`・`sec13_required_permissions_outside_grant_denied`・`ahg007_*`・`ahg010_*`・`gt003_new_capability_not_implicitly_granted`・`gw10_authorization.rs::sec01`〜`sec04`・`sec12`/`sec13` |
| Oracle | 未認証/Grant なし/他 Principal/他 Project/unknown ref/binding mismatch は拒否・wire authority は client claim でなく permit record・policy epoch は grant を supersede・capability/version/permission は完全一致・新 capability は暗黙許可しない・budget は commit に結合 |
| Status | **Pass(部分)** — headless。具体 Rust 型・認証方式・Principal Binding 永続化は #45 |
| 証拠 | `docs/specs/ai-native-baseline.md` AHG-007、`docs/designs/ai-native-baseline-design.md` Principal/Caller/Grant・Apply Modes/Proactive 分離 |
| 後続 | #45（Grant/Policy 型）・#46（認証 UX） |

### AHG-008 Resource Scope/操作効果の評価原則（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Typed Stable ID 基準・親子関係からの権限無条件継承禁止・固定 Scope（Exact ID/Snapshot 固定）と制約付き動的 Scope（明示条件評価・新規対象の暗黙無制限許可禁止）・新規作成と既存編集の区別・移動の変更前後所属/時間範囲評価・Project 複製での Grant 自動移譲禁止・半開時間範囲と Musical/Native Time 区別・Validator 導出の構造的効果と音響的波及の区別・不明/検証不能副作用の Delegated Apply 自動許可禁止・Plugin 不透明 State/未検証 Media の Unsupported/Unknown 表現 |
| Owner | #44（本 slice）・#45（Scope 型）・#10/#11/#38（domain） |
| Fixture | `crates/synvelis-gateway/tests/authorization.rs::sec05_shared_source_not_inherited_through_clip`・`sec05_read_only_capability_does_not_follow_related`・`sec05_shared_source_write_needs_source_scope`（gw10_authorization）・`sec06_move_inside_scope_permitted`・`sec06_move_outside_scope_denied`・`sec06_move_range_outside_bound_denied`・`sec06_move_args_unverifiable_after_state_denied`・`sec06_dynamic_scope_escape_denies`（gw10_authorization）・`sec09_exact_scope_permits_listed_member`・`sec09_exact_scope_denies_unlisted_and_unresolvable_alike`・`sec09_snapshot_scope_behaves_like_exact`・`sec10_dynamic_member_covered`・`sec10_dynamic_wrong_kind_denied`・`sec10_dynamic_wrong_owner_denied`・`sec10_dynamic_ownerless_denied`・`sec10_dynamic_range_outside_bound_denied`・`sec10_dynamic_wrong_time_domain_denied`・`sec10_dynamic_future_member_requires_inclusion`・`sec10_dynamic_max_members_caps`・`sec10_dynamic_max_members_spans_target_and_effects`・`sec10_dynamic_unknown_revision_treated_as_future`・`sec11_create_without_axis_denied`・`sec11_create_permitted_parent_and_kind`・`sec11_create_wrong_parent_denied`・`sec11_create_wrong_kind_denied`・`sec11_create_under_unresolvable_parent_denied`・`sec14_unverifiable_effects_deny`・`sec14_write_outside_scope_denied`・`sec14_create_effect_checks_axis`・`gw10_concurrency.rs::ac05_commit_stage_evaluation_denies_out_of_scope_writes`・`prepare.rs` 系・`prepare_regressions.rs` 系 |
| Oracle | 共有 Source への書込は Source 側 Scope を要し Clip 経由で継承しない・read-only capability は related を辿らない・固定/動的 Scope は定義どおり評価・動的 Scope の kind/owner/time-domain/range/member 上限/将来 member は拒否・作成は親/kind/軸を検証・移動は変更前後を評価・検証不能引数は拒否・unknown/unverifiable effect は auto-permit しない・out-of-scope write は commit 段階でも拒否 |
| Status | **Pass(部分)** — headless。動的 Scope の評価効率・Project 複製での Grant 移譲・音響的波及の扱いの製品 UX は #45/#46 |
| 証拠 | `docs/specs/ai-native-baseline.md` AHG-008、`docs/designs/ai-native-baseline-design.md` Scope/効果評価 |
| 後続 | #45（Scope 型・評価効率）・#10/#11/#38（domain 契約維持） |

### AHG-010 Validity/Revocation/Commit の順序保証（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Task Scoped 初期基本・Session/Persistent Project Scoped の表現対象・無期限無制限の既定禁止・Persistent Grant の自動移譲禁止・ユーザー失効・Agent 自己増権/無許可再委譲禁止・失効/期限切れ/対象移動削除/Policy 変更の評価・失効確定と Project Commit 最終認可の確定順序（TOCTOU）・Job 取消/中断要求と完了済み外部送信/生成/Commit 取消の区別・Request ID/Idempotency Key による二重適用防止・最小必要監査 |
| Owner | #44（本 slice）・#45（型契約）・#46（実装/受入） |
| Fixture | `crates/synvelis-gateway/tests/gw10_concurrency.rs::rec01_revoke_first_denies_commit`・`rec02_commit_first_stands`・`gtv005_both_orderings_enumerated_deterministically`・`rec03_two_commits_one_base_one_conflicts`・`rec05_rec06_keyed_retry_replays_and_conflicts`・`gtv005_revoke_before_receipt_consumption_denies`・`ac05_commit_stage_evaluation_denies_out_of_scope_writes`・`gw10_persistence.rs::rec07_crash_after_authorize_preserves_spend_consistency`・`rec08_confirmed_revoke_survives_torn_tail_does_not`・`rec09_crash_points_recover_committed_or_indeterminate_never_torn`・`rec10_saves_publish_head_in_commit_order`・`rec11_crash_after_apply_leaves_verified_source_and_no_clip`・`rec12_stale_proposal_never_commits_after_restart`・`sys04_expired_proposal_rejects_not_applies`・`gw10_system.rs::sys02_revoking_one_grant_never_touches_another`・`authorization.rs::sec07_expired_grant_denied`・`sec07_expired_task_denied`・`sec08_revoked_grant_denied`・`sec08_revoked_task_denied`・`sec08_revoked_task_denies_like_missing_task`・`sec12_policy_epoch_supersedes_grant`・`sec12_grant_without_ceiling_survives_policy_update`・`ahg010_*` |
| Oracle | revoke-first は commit を拒否・commit-first は失効だけで自動 Undo しない・両順序が deterministic に列挙・one-winner・keyed retry は replay/conflict・crash は committed/indeterminate に回復・torn tail は confirmed revoke を巻き戻さない・stale/expired proposal は commit しない・expired/revoked grant/task は拒否・policy epoch は supersede・budget/spend は reissue で保持 |
| Status | **Pass(部分)** — headless。実 power-loss・実機 durability・実行中取消の実 transport は NotRun |
| 証拠 | `docs/specs/ai-native-baseline.md` AHG-010、`docs/designs/ai-native-baseline-design.md` Grant Lifecycle/Invocation Commit Gate・Failure Taxonomy |
| 後続 | #45（線形化手法）・#46（取消/再試行 UX）・#114（実機 durability） |

### AHG-011 受入基準と実装後の変更管理（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | 初期 MCP 連携の最小対象は限定 MIDI 編集でよいが初期から 3 mode を実際に提供。最低限 8 受入条件を自動試験/E2E 対象。実装後の見直しは Issue/設計変更として明示。意図しない AI 実行・外部送信禁止・UI 状態非依存・本人確認/最小権限・Delegated≠Proactive・3 mode 初期提供・Grant 取消と Commit 順序・ProjectController 単一 Writer・非破壊性/履歴/RT 安全性・通常 DAW 経路独立・全 Consumer 互換性は暗黙に緩和しない |
| Owner | #44（本 slice）・#45/#46（実装/受入） |
| Fixture | `evidence/sv44/ai-native-baseline-case-ledger.md`（本台帳）・`docs/specs/ai-native-baseline.md` AIB-AC-01〜08・`docs/test-specs/ai-native-baseline.md` AIB-T-R-01〜06 |
| Oracle | 8 受入条件が case ledger に fixture/oracle/Owner/status/証拠付きで写像・合意済み/未決定が分離・不変条件が非緩和として明示 |
| Status | **Pass(部分)** — 設計レビュー + headless 写像。製品受入・実機受入は #46/#114 の別 Gate |
| 証拠 | `evidence/sv44/` 台帳全体 |
| 後続 | #45/#46（実装/製品受入）・#114（実機） |

### AI-GATE 独立第四軸（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | AI Gateway は独立した第四軸。A の利用に AI 有効化・外部接続・Grant/Approval を要求しない。既存の AI Off 既定・明示指示・Execution Grant・ProjectController Single Writer・MCP 初期 3 Mode・RT 安全性を維持。GW-01〜10 headless 実装済み事実と GUI 利用・MCP 実運用/Provider/権限 UX の製品受入を区別。AI Gate 未完了でも A の制作・Playback/Recording/Undo/Export を壊さない。A の状態と独立に AI Gate を成功判定する場合も必要な ProjectController/Permission/Approval/Recovery/Security 受入を省略しない |
| Owner | #44（本 slice）・#1/#8/#23（製品 Gate）・#46（AI Gate） |
| Fixture | `crates/synvelis-gateway/tests/gw10_system.rs`（`e2e1_*`・`sys02_*`・`sys03_*`・`sys05_sys06_*`・`sys07_*`・`sys08_*`・`gtv002_*`）・`gw10_*.rs` 全体 |
| Oracle | AI Off で human pipeline が commit/undo/recover・grant revocation は他 grant に影響しない・state non-conflation・RT boundary 記録・unsupported version typed refuse・AI/MCP 無効でも通常 DAW 操作が継続 |
| Status | **Pass(部分)** — headless。製品 Gate A/B/C 判定・AI Gate 製品受入は #23/#46/#1 の別 Gate |
| 証拠 | `docs/specs/ai-native-baseline.md` AI-GATE・`docs/specs/product-value-core.md`（Gate A/B/C） |
| 後続 | #23（実行計画）・#46（AI Gate 製品受入）・#1/#8（Gate A/B/C 判定） |

## 受入条件 AIB-AC-01〜08 写像

Issue #44 の受入条件 8 件を個別ケースとして写像する。各
AIB-AC は対応する AIB/AHG 要件の fixture を参照する。

### AIB-AC-01 未認証/Grant なし/他 Project・Scope 外/未許可 Media Read・外部送信が拒否される（Pass(部分)）

| 項目 | 内容 |
|---|---|
| Fixture | `gw10_authorization.rs::sec01_unauthenticated_request_denies`・`sec02_wire_authority_fields_cannot_self_declare`・`sec03_foreign_principal_cannot_spend_the_grant`・`sec04_foreign_project_boundary_denies`・`sec04_no_existence_leak_between_deny_reasons`・`sec05_shared_source_write_needs_source_scope`・`sec06_dynamic_scope_escape_denies`・`sec13_capability_outside_the_grant_denies`・`sec14_unknown_effect_is_never_auto_permitted`・`data01`〜`data10`・`gw10_mcp.rs::gw10f_dead_permits_fail_blind`・`gw10f_foreign_principal_cannot_read_the_operation`・`authorization.rs::sec01_*`〜`sec04_*`・`sec13_*`・`sec14_*` |
| Oracle | 未認証・Grant なし・他 Principal・他 Project・Scope 外・未許可 Media Read・外部送信（独自 class なし）・unknown effect・capability 外・version 不一致は全て拒否。存在 leak なし |
| Status | **Pass(部分)** — headless。実外部 Provider/実 MCP Client/実 OS での拒否は NotRun |
| 後続 | #45/#46（実 transport/Provider での拒否確認）・#114（実機） |

### AIB-AC-02 UI Selection の変更で既存 Invocation Target/Grant の対象が変わらない（Pass(部分)）

| 項目 | 内容 |
|---|---|
| Fixture | `gw10_authorization.rs::sec07_selection_changes_do_not_retarget_the_grant`・`gw10_system.rs::sys03_approval_binds_proposal_not_ambient_selection`・`gw10_authorization.rs::gt004_context_query_projects_pinned_snapshot` |
| Oracle | UI selection 変更で既存 Invocation Target/Grant 対象が変わらない・approval は proposal に結合・context query は pinned snapshot を投影 |
| Status | **Pass(部分)** — headless。実 GUI の複数 Window/View・Context API UI は #46/#114 |
| 後続 | #2/#8（UI 契約維持）・#45（Target 型）・#46/#114（Context API UI） |

### AIB-AC-03 Track の権限が Clip・Asset へ無条件継承されず、固定 Scope・制約付き動的 Scope が定義どおり評価される（Pass(部分)）

| 項目 | 内容 |
|---|---|
| Fixture | `authorization.rs::sec05_shared_source_not_inherited_through_clip`・`sec05_read_only_capability_does_not_follow_related`・`sec09_exact_scope_*`・`sec09_snapshot_scope_*`・`sec10_dynamic_*`（10 件）・`gw10_authorization.rs::sec05_shared_source_write_needs_source_scope`・`sec06_dynamic_scope_escape_denies` |
| Oracle | 共有 Source/Clip/Asset へ無条件継承しない・固定 Scope は Exact/Snapshot 一致・動的 Scope は kind/owner/time-domain/range/member 上限/将来 member を明示条件で評価し暗黙無制限許可しない |
| Status | **Pass(部分)** — headless。動的 Scope の評価効率・製品 UX は #45/#46 |
| 後続 | #45（Scope 型・評価効率）・#10/#11/#38（domain 契約維持） |

### AIB-AC-04 Grant 範囲外への Note 移動・新規 Clip 作成・Resource 所属変更・Unknown Effect が安全に判定される（Pass(部分)）

| 項目 | 内容 |
|---|---|
| Fixture | `authorization.rs::sec06_move_*`（4 件）・`sec11_create_*`（5 件）・`sec14_*`（4 件）・`gw10_concurrency.rs::ac05_commit_stage_evaluation_denies_out_of_scope_writes`・`prepare.rs`・`prepare_regressions.rs` 系 |
| Oracle | 範囲外 Note 移動・新規 Clip 作成（親/kind/軸未検証）・Resource 所属変更・Unknown/unverifiable effect は拒否。commit 段階でも out-of-scope write を拒否 |
| Status | **Pass(部分)** — headless。実 MIDI 編集の製品 E2E は #46/#23 |
| 後続 | #45（効果評価型）・#46（限定 MIDI E2E）・#23（実行計画） |

### AIB-AC-05 Suggest で Agent Apply 不可、Approve-to-Apply は該当 ChangeSet 承認のみ、Delegated Apply は Grant の内側のみ自動適用される（Pass(部分)）

| 項目 | 内容 |
|---|---|
| Fixture | `authorization.rs::ahg009_*`（4 件）・`ahg010_*`（4 件）・`gw10_authorization.rs::sec08_suggest_mode_apply_denies_without_revision_change`・`sec10_foreign_approval_receipt_denies`・`sec11_revision_drift_after_approval_conflicts`・`sec12_revoked_grant_kills_the_unused_approval`・`gw10_mcp.rs::gw10f_suggest_mode_denies_apply`・`gw10f_delegated_apply_commits_through_the_host`・`mcp_adapter.rs::suggest_mode_prepares_but_denies_apply`・`delegated_apply_commits_through_the_seam`・`approve_to_apply_requires_and_consumes_the_receipt`・`proposal.rs` 系・`crates/mcp-poc/tests/gw10_transport.rs::gw10f_three_modes_over_real_transports` |
| Oracle | Suggest は Apply を拒否し prestage のみ・Approve-to-Apply は承認 receipt を要求し receipt は proposal/ChangeSet/Revision に結合・Delegated Apply は Grant の Scope/budget 内のみ・mode は Grant 由来・receipt 二重消費は成功しない・3 mode E2E が real transport で動作 |
| Status | **Pass(部分)** — headless real transport。実外部 MCP Client/Provider・ユーザー承認 UI は NotRun |
| 後続 | #45（Approval Policy 型）・#46（承認 UI・3 mode 製品受入）・#114（実機） |

### AIB-AC-06 Delegated Apply のみで Proactive 起動できない（Pass(部分)）

| 項目 | 内容 |
|---|---|
| Fixture | `authorization.rs::ahg007_delegated_grant_is_not_proactive_authority`・`ahg007_live_proactive_permit_activates`・`ahg007_revoked_proactive_permit_denies`・`ahg007_expired_proactive_permit_denies`・`ahg007_permit_for_other_principal_denies`・`ahg007_session_binding_*`・`ahg007_persistent_project_binding_permits` |
| Oracle | Delegated Apply は Proactive 起動権限を含まない・live permit のみ activate・revoked/expired/他 Principal の permit は拒否・session/persistent binding は adapter/Project に一致 |
| Status | **Pass(部分)** — headless。実 Proactive UI・自律性レベルは未決定/#46 |
| 後続 | #46（Proactive/自律性 UX）・#45（Activation Authority 型） |

### AIB-AC-07 Grant 失効/Project Revision 競合/承認済み ChangeSet 改竄/同一 Request 再送が安全に処理され、意図しない二重適用や権限外 Commit がない（Pass(部分)）

| 項目 | 内容 |
|---|---|
| Fixture | `gw10_concurrency.rs::rec01`〜`rec03`・`rec05_rec06`・`gtv005_*`・`ac05_*`・`gw10_persistence.rs::rec07`〜`rec12`・`sys04_*`・`gw10_project.rs::gw10c_conflict_after_revision_drift`・`gw10_authorization.rs::sec09_changeset_tamper_denies_at_digest_check`・`sec10_foreign_approval_receipt_denies`・`sec11_revision_drift_after_approval_conflicts`・`sec12_revoked_grant_kills_the_unused_approval`・`authorization.rs::sec07_expired_*`・`sec08_revoked_*`・`sec12_*`・`proposal.rs::changeset_verify_detects_tampering`・`semantic_drift_changes_the_digest`・`receipt_for_another_proposal_denies`・`double_consumption_can_never_succeed`・`revoked_approval_denies_apply` |
| Oracle | revoke-first は commit を拒否・commit-first は失効だけで自動 Undo しない・revision drift は conflict・ChangeSet 改竄は digest check で拒否・receipt は proposal/ChangeSet/Revision に結合・keyed retry は replay/conflict・crash は committed/indeterminate に回復・二重適用・権限外 Commit なし |
| Status | **Pass(部分)** — headless。実 power-loss・実機 durability・実 transport での競合は NotRun |
| 後続 | #45（線形化手法）・#46（取消/再試行 UX）・#114（実機 durability） |

### AIB-AC-08 AI/MCP 無効時にも通常 DAW 操作、既存 Project Save/Playback/Export が継続できる（Pass(部分)）

| 項目 | 内容 |
|---|---|
| Fixture | `gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`・`authorization.rs::sec01_human_needs_no_grant`・`mcp_adapter.rs::disabled_adapter_denies_the_surface`・`gw10_project.rs::gw10c_commit_advances_revision_and_undo_rewinds`・`gw10_persistence.rs::rec10_saves_publish_head_in_commit_order` |
| Oracle | AI/MCP 無効でも human が Grant なしで commit/undo/recover・disabled adapter は surface を deny するが通常 DAW 経路は継続・save は commit 順序で publish |
| Status | **Pass(部分)** — headless。実 GUI/OS での Save/Playback/Export 継続・製品 Gate A 判定は #114/#23/#1 の別 Gate |
| 後続 | #46（AI OFF 製品受入）・#114（実機）・#23（Gate A 判定）・#1（製品価値 Gate） |

## NotRun / Unsupported（本 slice 範囲外）

| 項目 | Status | 理由 |
|---|---|---|
| 実外部 MCP Client/Provider | NotRun | 未接続。#46 の製品受入で実施 |
| ユーザー承認 UI・権限 UX・Provenance/権利 UI | NotRun | GUI 未実装。#46/#114 の別 Gate |
| GPUI/OS/実機・実 device/plugin | NotRun | headless fixture の Pass(部分) を実機受入に繰り上げない。#114 の別 Gate |
| 実 RT thread での無待機・実機 RT lane | NotRun | `sys07` は boundary 記録のみで実測を主張しない。#46/#114 |
| 実 power-loss・実機 durability | NotRun | `rec07`〜`rec12` は headless crash point。実機 durability は #114 |
| Provider/Capability 契約・コスト・障害時挙動 | NotRun | 未決定。#45/#46 |
| Proactive/自律性レベル・実行予算・中断・人間介入 | NotRun | 未決定。#46 |
| 内蔵 Agent・交換可能 Agent Reasoning Runtime | NotRun | 未実装。AHG-001 の境界のみ固定 |
| 実 OS 別 transport・実 network・実 credential | NotRun | `gw10_transport` は real transport だが外部 Provider ではない。#46/#114 |
| macOS/Linux lane | NotRun | Windows OS-L1 のみ。POLICIES.md に従い他 OS は Deferred |

## Evidence Honesty

- headless fixture の Pass は `Pass(部分)` として記録し、実機/
  GUI/OS/Provider/MCP Client/承認 UI 受入に繰り上げない。
- 未実装・未検証（実外部 Provider・実 GUI・実機 RT・実 plugin・
  実 OS lane・Proactive UI・Provider 契約）は `NotRun`。
- `sys07_rt_boundary_is_recorded_not_claimed` は RT boundary が
  **記録される**ことを示すが、実 RT thread での無待機を実測
  したものではない。
- 設計レビュー Pass（本 slice）と実測 Pass（実装 Issue）を
  区別する。
- `Pass`/`Fail`/`Unsupported`/`NotRun` は実際に実行した検証
  のみを反映する。
