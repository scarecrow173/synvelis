---
id: "IMPL-0Gw03ContextProjecti01"
type: "impl"
status: "completed"
title: "GW-03 Scoped Context Projection / Snapshot Pagination / Read-Disclosure Guard"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/55"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/context.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/context_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/snapshot.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/context.rs"
      - type: "file"
        path: "docs/impl/ir/gw-03-context-projection.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/capability.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/id.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/outcome.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/policy.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/55"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  implemented-by: []
  depends-on:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "docs/impl/ir/gw-02-authorization-policy.md"
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
    - "https://github.com/scarecrow173/synvelis/issues/55"
  derived-by: []
  verifies:
    - "https://github.com/scarecrow173/synvelis/issues/55"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# GW-03 Scoped Context Projection / Snapshot Pagination / Read-Disclosure Guard 実装記録

Issue #55。#45 GTC-004・#46 GW-03 に基づく read-only context projection を、
GW-01 gateway 契約の `synvelis.context.query` capability として、GW-02
認可ポリシー上に構築した。Project Snapshot の第二の正本は作らず、
`SnapshotSource` seam 経由でコミット済み Project 状態を pin して射影する。
Prepared Operation 連携の実配線は GW-04、approval receipt・commit fence は
GW-05/06 へ defer。

## 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `capability.rs` | `DataAccessClass` に `StructuredContentRead`・`WorkspaceSelectionRead` を追加(既存5 variant と合わせて metadata/media/structured/derived/selection/disclosure/transfer を区別)。`DataAccessSet` に構築・部分集合判定 API |
| `id.rs` | `CursorRef`・`SnapshotRef`・`SelectionViewRef` を追加 |
| `outcome.rs` | `GatewayError` に `SnapshotExpired`・`CursorInvalid`・`CursorExpired`・`BudgetExceeded` を追加。category/retry/`safe_message` を external 向けに分類 — cursor/snapshot の内部 detail は external envelope に出さない |
| `context.rs`(新規) | 型付き query model:`ContextQuery`(selector/fields/range/consistency/page/selection_view)、`ResourceSelector`(All/Kind/Resources — 厳格 literal、`{"resources":[]}` や `{all:false}` は SchemaViolation)、`FieldSelection`/`FieldGroup`(metadata/structured-content/media/derived/selection — 各 group が要求する `DataAccessClass` を宣言)、`PageRequest`(First{limit}/Continuation{cursor}、limit と cursor は排他)、`TypedRange` の semantic len≥1 guard(schema floor に加えて parse 層でも検証)。出力 model:`ContextProjection`/`ProjectedItem`/`SelectionProjection`/`Completeness`(complete/unknown/unavailable)。seam:`SelectionSource`(host の selection view 供給、`SelectionFacts`)、`ContextAuthorizer`(scope admission + disclosure authorization の2境界 trait)、`ContextBudget`(page items/total items/selector ids/range window/pinned snapshots/cursors/page count/cursor TTL) |
| `snapshot.rs`(新規) | `SnapshotSource` trait(host seam — commit 済み Project 状態を revision で pin)。`PinnedSnapshot`(SnapshotRef+ProjectId+ProjectRevision+`Arc<Project>` — 第二の store ではなく共有不変参照)。`MemorySnapshotSource`(決定的 headless 実装、commit で latest 更新)。`ProjectOracle` — pin 済み `Arc<Project>` 上の `ResourceOracle` 実装で scope facts を data と同じ revision に揃える。`project_item` — kind 別 field-group 射影:metadata/structured(MIDI note は source-native tick・division・digest を保存)/media(descriptor のみ、PCM bytes は emit しない)/derived(source 不在 → unavailable)。media-domain の `clip_window`/`item_window` は per-value time base を持つ値を base なし `TypedRange` に混在させないよう `None`(正直な欠落) |
| `context_engine.rs`(新規) | `ContextProjectionEngine` — descriptor(`OperationDomain::Query`/`EffectKind::Read`/`TargetContract::None`、required `ReadContext`+`DiscloseContext`、descriptor-level data_access は `MetadataRead` のみ — field group 要求 class は per-request disclosure 境界で検査し metadata-only grant が metadata 問合せ可能)。`install` で input/output schema + handler を `Gateway` に登録。`pin_for` は Latest/ReadAt/FixedSnapshot を解決(Fixed が evict/foreign なら `SnapshotExpired`、Latest への silent fallback なし)。`pin_state` は source.pin を lock 外で呼び re-check で競合 dedupe。`candidates` は selector のみの列挙、`range_filter` は scope admission 後に適用(out-of-scope 存在/配置の range probe 遮断)。cursor record は project/principal/adapter/task/grant/resources/fields/external/range/limit/position/page count/絶対期限を pin — continuation は lock 内で authorize+consume+rotate を原子化し、純粋な `project_item` は lock 解放後に実行。budget は page/total/ids/range/page count/retention を強制、max=0 設定は 1 に clamp |
| `policy.rs` | `authorize_agent` を `pub(crate)` 化し `ContextAuthorizer` を `GrantPolicy` に実装 — preflight read(task/grant/capability/permission/scope)と disclosure(emit する data-access class + ExternalDisclosure)を同一 binding 連鎖で2回評価。両 method は descriptor-vs-request の id+version 一致を `evaluate_with` と同様に検証。`OUTSIDE_SCOPE` を `pub(crate)` 単一定数に集約し engine と共有 |
| `lib.rs` | `context`/`context_engine`/`snapshot` module + public 型 export、crate doc に GW-03 節 |
| `tests/context.rs`(新規) | 43 テスト — DATA-01〜05/08〜10・GT-004・SEC-03/04/05/07 該当分 + pagination/cursor/budget/selection/error hygiene。下記 matrix 参照 |

## 受入条件との対応

| AC | 対応 |
| --- | --- |
| 明示 Project ID・型付き resource selector・field selection・revision consistency・page request | `ContextQuery` が project/selector/fields/consistency/page を型化 — `ResourceSelector`(all/kind/resources の厳格 literal)、`FieldSelection` は group 名で宣言、schema+parse の2層検証(`wire_args_are_schema_checked`、`selector_literals_are_strict`) |
| Snapshot-revision-fixed pagination・expiring cursor | 全 page が単一 `PinnedSnapshot` を読む。cursor は TTL 付き・rotate・caller/adapter/task/grant/snapshot/fields 結合(`pagination_chain_is_revision_stable_across_project_change`、`cursor_expires_after_ttl`、`cursor_rotates_and_rejects_replay`) |
| Content/media/derived classification・completeness/unknown/unavailable | `FieldGroup`→`DataAccessClass` 対応、`Completeness` 3値を item/selection/page で正直に報告(`structured_content_preserves_source_native_time`、`derived_group_reports_unavailable_honestly`、`*_reports_unknown`/`unavailable` 群) |
| preflight read と disclosure の分離 | `ContextAuthorizer` 2 method — `admitted_scope`(read)と `authorize_disclosure`(emit class)。descriptor-level は MetadataRead のみで field group class は emit 境界で検査 |
| metadata-only caller は PCM/derived を得られない | `metadata_only_grant_reads_metadata_never_content`・`media_group_is_descriptor_never_bytes`・`derived_group_reports_unavailable_honestly` — media は descriptor のみで bytes 非 emit、derived source 不在は `unavailable` |
| 変更後も pagination は revision 混在しない | chain が pin を保持 — project commit 後も同一 revision を serve(`pagination_chain_is_revision_stable_across_project_change`) |
| 失効 snapshot は Latest へ silent switch しない | `SnapshotExpired` を返す(`unservable_revision_reports_expired`、`evicted_snapshot_reports_expired_never_rebased`、`continuation_after_pin_eviction_reports_expired`) |
| Resource ID/Job Handle のみでは外部開示不可 | `task_grant_refs_alone_grant_nothing` — wire の ref 所有は authority でなく、全 stage で binding 再検証 |
| UI Selection が request/grant target を変異させない | selection は named `selection_view` + `WorkspaceSelectionRead` 経由のみ。GW-01 層が selection/focus 系 field を `ImplicitTargetRejected` で拒否(`selection_needs_named_view_and_class`) |
| 数量・page 数・時間窓 budget | `page_limit_is_bounded`・`page_count_is_bounded`・`total_items_are_bounded`・`selector_id_count_is_bounded`・`range_window_is_bounded`・`pin_retention_evicts_oldest`・`cursor_retention_evicts_oldest` |
| grant 失効後の追加 page/chunk 拒否 | continuation は毎回 fresh binding 検証(`expired_grant_denies_the_next_page`・`revoked_grant_denies_the_next_page`・`cursor_expires_after_ttl`) |
| Query→Snapshot→Prepared Operation の headless 整合 | `query_snapshot_prepared_operation_consistency` — Query が pin した `SnapshotRef`/revision と project revision が一致することを fixture で検証(GW-04 が参照する seam を確定) |
| source-native timing・content hash/version 保持 | `structured_content_preserves_source_native_time` — MIDI division/tick/note window と stored digest を source そのまま emit |
| Source↔Clip の false sharing なし | 双方向に検証(`clip_scope_never_implies_source_scope`・`source_scope_never_implies_clip_scope`) |

## SEC / DATA matrix(#46 Failure Matrix 対応・GW-03 該当分)

| Case | 内容 | 判定 | 証拠 |
| --- | --- | --- | --- |
| SEC-03 | 範囲外 resource の存在漏えい | Pass | `explicit_selector_denies_uncovered_and_absent_identically`・`explicit_selector_denies_pin_absent_blind` — uncovered/absent/never-existed を同一 deny reason に統一。range filter は admission 後で存在 probe 不可(`range_cannot_probe_out_of_scope_resources`) |
| SEC-04 | 別 Project の snapshot/cursor 提示 | Pass | `foreign_project_snapshot_is_existence_blind`・`foreign_caller_cursor_is_indistinguishable_from_unknown` |
| SEC-05 | Clip→Source・Source→Clip の scope 非継承 | Pass | `clip_scope_never_implies_source_scope`・`source_scope_never_implies_clip_scope`(双方向) |
| SEC-07 | UI Selection が target/grant を変異させない | Pass | selection は明示 named view 経由のみ・implicit field は GW-01 が拒否(`selection_needs_named_view_and_class`) |
| DATA-01 | Metadata 権限のみで PCM 取得 → Deny | Pass | `metadata_only_grant_reads_metadata_never_content`・`media_group_is_descriptor_never_bytes` |
| DATA-02 | Derived Feature への開示権限なし → Deny | Pass | `derived_group_reports_unavailable_honestly` — field group の `DerivedDataRead` を disclosure 境界で照合 |
| DATA-03 | paging 中の Revision 混在 | Pass | `pagination_chain_is_revision_stable_across_project_change`・`continuation_requires_the_chain_snapshot`・`continuation_with_mismatched_snapshot_is_cursor_invalid` |
| DATA-04 | Grant 失効後の追加 Chunk → Deny | Pass | `expired_grant_denies_the_next_page`・`revoked_grant_denies_the_next_page` |
| DATA-05 | 別 Client が既知 cursor/ID で取得 → Deny | Pass | `foreign_caller_cursor_is_indistinguishable_from_unknown`・`task_grant_refs_alone_grant_nothing` |
| DATA-08 | 過大要求 → bounded 拒否 | Pass | `page_limit_is_bounded`・`page_count_is_bounded`・`total_items_are_bounded`・`selector_id_count_is_bounded`・`range_window_is_bounded` + pin/cursor retention test |
| DATA-09 | 取得後の Grant 失効 → 実行時再認可 Deny | Pass | `revoked_grant_denies_the_next_page` — continuation は fresh store read で再評価 |
| DATA-10 | External Transfer/Disclosure 未許可 | Pass | `external_envelope_strips_diagnostics` — external caller には disclosure 境界+診断除去 |
| GT-004 | Query→Snapshot→Prepared Operation 整合 | Pass | `query_snapshot_prepared_operation_consistency` — headless fixture で参照一貫性を検証 |

## 実行した検証(証拠)

- 環境: Windows、rust 1.99.0(mise pin)、GW-02 merge 済み base + 本変更
- `mise run fmt-check` — Pass
- `mise run check` — Pass(workspace 全 target)
- `mise run lint`(`cargo clippy --workspace --all-targets -- -D warnings`) — Pass
- `mise run test`(`cargo test --workspace --locked`) — Pass、workspace 全 43 テスト
  target 緑。`tests/context.rs` は 43 テスト全 pass

## Review-protocol 実施結果

独立 reviewer 2 系(A: 受入条件トレース / B: adversarial security)を投入し、
全 candidate をコード実測で個別検証。blocking(P1)を修正してから再検証まで実施。

| 指摘 | 判定 | 対応 |
| --- | --- | --- |
| B1 range filter が scope admission より先に走り explicit selector+range で out-of-scope 存在/配置 probe が可能 | Confirmed P1 | `candidates`(selector のみ)と `range_filter` を分離し admission 後に適用 — `range_cannot_probe_out_of_scope_resources` |
| B2/A5 scope admission が live oracle facts で評価され data は pin から読む — false admit/deny・存在 leak | Confirmed P1 | `ProjectOracle::new(pinned.state)` を `admitted_scope` に渡し facts と data を同一 revision に揃える。pin-absent covered member は pin から serve(`deleted_member_is_served_from_its_pin`)、pin-absent は explicit selector で一律 deny(`explicit_selector_denies_pin_absent_blind`) |
| A1/B3 `project_selection` が `pin_for` を再呼出し — Latest で item と selection が別 revision を混在しうる(page 0) | Confirmed P1 | pin 済み `Arc<Project>`+oracle を `project_selection` へ受け渡し、2度目の pin を構造的に除去 |
| B5 continuation で `project_item` が engine lock 下で走る | Confirmed P2 | consume/authorize/rotate は lock 内原子、純粋射影は lock 解放後に移動 |
| B7/A8 `SnapshotSource::pin` が lock 下で呼ばれる(host I/O で全 cursor traffic を塞ぐ/再入 deadlock) | Confirmed P2 | lock 外で fetch + lock 再取得後の競合 re-check(`existing_pin`)。`ContextAuthorizer` に no-reentry 契約を明記 |
| A3 pin-absent selection member が無言で drop され selection が `complete` を報告 | Confirmed P2 | completeness は filter 前 member 集合で判定 — `selection_pin_absent_member_reports_unavailable` |
| A9 `SelectionFacts::Unknown` が `unavailable` に潰れる | Confirmed P3 | `Completeness::Unknown` に写像 — `selection_unknown_view_reports_unknown` |
| A2 media-domain `TypedRange` が per-value time base を持つ値の `value()` を混在 — 単位不定の範囲算術 | Confirmed P2 | `clip_window`/`item_window` は base を伴う media window で `None` を返し正直に欠落。`TypedRange` に base field がなく canonical base 決定は上游契約が必要(残課題) |
| B13 負の `max_range_len` が unsigned cast で wrap | Confirmed P3 | `u64::try_from(..).unwrap_or(0)` |
| A6/B9 degenerate selector literal(`{"resources":[]}`/`{"project":false}`/`{"all":false}`)受理 | Confirmed P3 | parse で拒否 → SchemaViolation — `selector_literals_are_strict` |
| A7 `parse_range` に semantic len≥1 guard なし(schema floor のみ依存) | Confirmed P3 | `parse_page` と同型の guard を追加 |
| A10 `OUTSIDE_SCOPE` が2箇所に複写 | Confirmed P3 | `policy.rs` の `pub(crate)` 単一定数に集約し engine が import |
| A13 `ContextAuthorizer` が descriptor-vs-request の id+version 一致を検証しない | Confirmed P3 | `admitted_scope`/`authorize_disclosure` に `evaluate_with` 同型の検証を追加 |
| B10/A4 `max_pinned_snapshots`/`max_cursors` の max=0 off-by-one + retention 未検証 | Confirmed P2 | constructor で ≥1 clamp、`pin_retention_evicts_oldest`/`cursor_retention_evicts_oldest`/`continuation_after_pin_eviction_reports_expired` |
| A12/B14 SEC-05 逆向き・eviction の coverage 欠落 | Confirmed | `source_scope_never_implies_clip_scope`・`continuation_with_mismatched_snapshot_is_cursor_invalid` 追加 |
| B4 pin/cursor retention が principal 横断で共有 — 他 principal が pin を evict しうる | Recorded | per-principal quota 設計が必要 — GW-08 session/adapter 契約と併せて残課題 |
| B6 `pin_for` が pub — 外部から任意 revision pin が可能 | Rejected | trusted host seam(GW-04 seam)であり host は state 所有者 — capability 経路では到達不能 |
| B8 deny verdict の順序主張 | Rejected | 設計意図 — verdict 順は契約として固定 |
| B11/B12 emit shape/deny message の代替案 | Rejected | 出力契約は意図的な選択 |
| A11 `MidiDivision` emit が `#[non_exhaustive]` にも関わらず `"ppqn"` 固定 | Recorded | latent — 将来 variant 追加時に tagged division への移行が必要 |
| A14 total-items budget より先に全 candidate を列挙 — 列挙コストは project size 比例 | Recorded | 文書化 limit — emit 件数は cap 済み、lazy enumeration は残課題 |

blocking 指摘は全て修正済み、修正込みで `mise run fmt-check / check / lint /
test` 再実行・全緑を確認。Final verdict: **ACCEPT WITH NON-BLOCKING NOTES**。

## 未実施の検証

- `mise run test-msrv`(rust 1.89 toolchain 未導入のため未実行)
- GW-04 との実配線結合(Prepared Operation が SnapshotRef を消費する経路) —
  Issue スコープ外、seam のみ確定

## 残課題

- pin/cursor retention は engine 全体の共有 quota — principal 別 quota と
  cross-principal eviction 遮断は GW-08 session/adapter 契約と併せて設計。
- `TypedRange` は time-base field を持たない — media-domain range は per-value
  base 解決の canonical 契約が決まるまで honest `None`(混在算術しない)。
- candidate 列挙は budget 判定より先に O(project) — lazy/early-exit 化は
  大規模 project 要求が出た時点で。
- `MidiDivision` が非 metrical variant を得た場合、division emit を tagged 表現
  へ移行する必要がある。
- Prepared Operation 連携・approval receipt binding・commit fence/durable
  recovery は GW-04/05/06。`pin_for` が GW-04 用の pin seam。
- Selection view の供給元は host `SelectionSource` seam — adapter/UI 側の
  named view 管理は GW-08 と併せて。
