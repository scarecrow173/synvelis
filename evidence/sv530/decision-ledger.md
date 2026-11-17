# sv530 decision-ledger — stateless PluginStateDecl 契約

Issue: https://github.com/scarecrow173/synvelis/issues/530
Date: 2026-11-19
Worktree: `G:\orca\workspaces\synvelis\sv530-plugin-state-decl`
Branch: `feature/530-plugin-state-decl`（base: `origin/main` `07b67f5b`）

## 判定事項

### D1: stateless/capture-applicability の契約表現（本 Issue の中核）

**決定**: 既存語彙の意味論確定（案 b）。`PluginStateDecl` に新
field を追加せず、`StateRestoreSupport`/`PluginVerdict` に variant
を追加しない。

- decl 側: `PluginStateDecl::stateless()` predicate を新設。
  stateless-by-design = `digest: None` +
  `quality: StateCaptureQuality::Unsupported` +
  `status != OpaqueStateStatus::Known`。
- env 側: `StateRestoreSupport::Unsupported` を dual-use。
  「plugin が state restore を実装しない」環境側事実として扱う
  （doc は既にこの意味を持ち、供給経路
  `HostError::Unsupported → PluginStateCaptureError::Unsupported`
  が実在）。
- `verify_plugin` の `Unsupported` arm のみ変更:
  `decl.state.stateless()` → `Live`、それ以外 →
  `StateUnsupported`（従来どおり）。

**根拠**:

1. AC2「`digest: None` の型上区別」は閉じた enum
   `StateCaptureQuality` の variant で既に担保される。
   `Unsupported` は TIM-04 の「plugin は state save/restore を
   実装しない」語彙であり「capture 忘れ」の意味を持たない。
   `digest: None` + capture 可能 quality は「capture 必要だが未
   capture」として `stateless() == false` → fail-closed。
2. field 追加案は構造体 literal 3 点（`rcp07_gate.rs`:947-951 は
   lease 外）を compile 破損させ、scope 拡張 ask が必須。
   得られるのは `quality == Unsupported` と論理同値の情報のみ。
3. variant 追加案（`StateRestoreSupport::NotApplicable`・
   `PluginVerdict` 新 variant）は `render_ui.rs` の `_` arm なし
   exhaustive match を破損させ、識別力の増分はゼロ。
4. 未 capture・env/decl drift・env=Unknown は全て fail-closed を
   維持（verdict matrix oracle で pin）。

**帰属**: sv#514 EXP-4 で「workspace 側 lever なし → player 契約
変更が必要」と確定した follow-up を本 Issue で消化。
`evidence/sv514/decision-ledger.md` AC4 の deferred residual を解消。

### D2: `status != Known` を stateless profile に含める理由

`OpaqueStateStatus::Known` は「検証済み opaque state が存在する」
主張であり、stateless 宣言と論理矛盾する。`Unknown`/`Missing` は
検証済み state を主張しないため両立する。`Missing`（binding 未解決）
を stateless として許容するのは、plugin が state 非対応を宣言して
いる以上 state slot の解決失敗は stateless 性と矛盾しないため。

### D3: env=`Unsupported` + decl 非 stateless の verdict

`StateUnsupported` を維持。state-bearing decl（digest Some）や
未 capture decl（capture 可能 quality + digest None）は「host が
宣言された state 証拠を再現できない」ことを意味し、Live への
fail-open はしない。

## AC 対応表

| AC | 対応 | 証拠 |
| --- | --- | --- |
| stateless plugin が Live 判定可能 | env=`Unsupported` + `stateless()` decl → `Live` → selection `Exact` | `ex03_stateless_plugin_selects_live`・`rcp_ex01_stateless_plugin_evidence_is_live` |
| `digest: None` の型上区別 | `PluginStateDecl::stateless()` predicate + `StateCaptureQuality` enum | `ex03_stateless_profile_is_typed_and_distinct_from_uncaptured` |
| 未 capture は fail-closed | `stateless() == false` → `StateUnsupported`/`StateUnknown` | verdict matrix・`ex03_stateless_decl_env_drift_and_uncaptured_stay_blocked`・integration 第 2 部 |
| 既存 oracle pass | `rcp_04` 30/30・`rcp_selection_integration` 11/11 | `verification.md` |
| mise lane pass | fmt-check・check/lint/test player・check/lint/test workspace | `verification.md` |

## review-protocol self-review 記録

- 実施: 1 cycle（objective checks → cooperative/adversarial lens
  の分離パス → 候補検証 → 判定）。subagent 起動なし（worker spec
  が helper 禁止のため単独で分離パスとして実施）。
- 確認した論点:
  1. `Unsupported` arm のみの変更で Verified/Unknown arm の
     fail-closed 不変条件が維持されるか → 維持（コード差分と
     matrix oracle で確認）。
  2. `stateless()` profile の `status != Known` 条件の妥当性 →
     D2 に記録。`Known` は検証済み state 主張で矛盾のため除外正当。
  3. evidence 偽装リスク（stateful plugin を Unsupported と偽って
     Live 化）→ evidence producer は trusted internal caller で、
     digest/ports 等の既存 decl field と同一の trust model。
     honest-input 契約の範囲内。
  4. lease 外波及 → variant/field 追加なしのため
     `render_ui.rs`・`rcp07_gate.rs`・`rcp06_render_ui.rs` は
     無変更。`mise run check synvelis-render`（all-targets）pass
     で実証。
  5. `compute_structure_digest` への影響 → struct 形状不変のため
     digest 値への影響なし（doc comment は Debug derive に含まれ
     ない）。
- 結果: **blocker 0**。P0/P1 指摘なし。fmt 指摘 1 件
  （`assert_is_empty` clippy）は修正済み。

## 未対応・後続

- docs index（`docs/**/README.md`）再生成 → Coordinator が統合時に
  `doc-maintenance` で実施。
- `evidence/sv293/rcp-04-fault-matrix.md` L148-152 の deferred
  residual 行の状態更新 → lease 外のため本 slice では未実施。
  本 Issue の完了をもって当該 residual は解消済みと読める旨を
  PR 本文に記録（更新自体は Coordinator 判断）。
- 実 plugin/実機受入 → sv#296 cross-gate・#109 に委譲（NotRun）。
