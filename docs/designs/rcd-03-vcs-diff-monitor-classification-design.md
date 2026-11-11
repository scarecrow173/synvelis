---
id: "DESIGN-rprIAB9zoxNjZbw6rODTWf"
type: "design"
status: "approved"
title: "RCD-03 VCS semantic diff monitor 宣言分類 design"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-monitor-persistence-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-vcs-diff-monitor-classification.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 VCS semantic diff monitor 宣言分類 design

## Overview

schema v12 の `Project::monitoring`（authored monitor 宣言）を
VCS-02 semantic diff の分類対象へ追加する。diff engine は
domain adapter が既知 entity のみを列挙する curated projection
であり、未列挙の `monitoring` は現在 diff に一切現れない。
本 design は `ObjectKind` 2 variant の追加と、既存の
keyed-synthetic collection・singleton・identity 規則に沿った
最小の分類実装を定義する。

```
Project::monitoring (MonitoringData)
  ├─ tracks: BTreeMap<TrackId, TrackMonitorDecl>
  │     → ObjectKind::TrackMonitorDecl を diff_map で照合
  │       (TrackId 空間を借用する keyed-synthetic collection)
  │       Added / Removed / Modified(全 field)
  └─ punch: PunchMarks (singleton)
        → ObjectKind::PunchMarks を declaration-presence で分類
          全 mark None = 宣言なし → Added / Removed / Modified
          both Some → affected_range = Musical [in, out)
両 kind とも domain = ComparisonDomain::Recording
```

## ObjectKind / Domain

- `ObjectKind::TrackMonitorDecl`・`ObjectKind::PunchMarks` を enum
  **末尾**に追加する。`change_id` の material と
  `issue_side_key` が `object.kind as u8` を使うため、既存
  variant の discriminant を不変に保ち、既存 kind の change-id
  規則・結果を変えない。
- `ObjectKind::domain()` は両 kind で `ComparisonDomain::Recording`
  を返す。Arm/Monitor/Punch 宣言は recording session の authored
  intent であり、session 観測値を含まない monitor 状態の唯一の
  永続形である。`included_domains` で Recording を外せば monitor
  宣言も diff 対象から外れる — domain filter の既存契約に自然に
  乗る。
- `ComparisonDomain` 自体は増やさない。monitor 宣言は
  `Project::recording` registry の隣接状態（Arm/Monitor/Punch は
  recording gate/route 決定の入力宣言）であり、独立 domain を
  新設するほどの境界ではない（YAGNI）。

## TrackMonitorDecl（keyed-synthetic collection）

`monitoring.tracks` は `BTreeMap<TrackId, TrackMonitorDecl>` —
`SmfDocument`（SourceId key）・`AppliedCommit`
（RecordingSessionId key）と同じ「他 entity の ID 空間を借用する
keyed-synthetic collection」の先例そのもの。

- `diff_map(ctx, ObjectKind::TrackMonitorDecl, before, after,
  no_range, false)` で照合。`EntityKey` は `TrackId` に実装済み。
- `raw_kind_map` には登録しない。登録すると monitor 宣言を持つ
  全 track が `{Track, TrackMonitorDecl}` の内部衝突で
  `AmbiguousIdentity` を捏造する。除外コメントを
  `smf_documents`/`applied_commits` から `monitoring.tracks` を
  含む形へ更新する。
- track raw が snapshot 間で異 kind に衝突する場合（track 削除→
  clip 再発行等）は既存の `raw_is_ambiguous` が
  `AmbiguousIdentity(CrossKindId)` を出力する — decl の key 元の
  identity が曖昧なら decl も確定できない、という正しい fail。
- field 分類は `classify_recording` の `_` → `Modified` 既定に
  委ねる（`lane`/`channel_map.channels`/`arm`/`monitor`）。
  `decl.lane` は宣言の owner 参照ではなく capture 先の binding
  target — `SendConfig.connection`（`Modified` 既定）と同じ
  位置付けで、identity 規則の例外を新設しない。
- `RecordingSession`/`AppliedCommit` と同様、追加の match arm は
  置かない（未掲載 kind = 全 field `Modified` の既存慣行）。

## PunchMarks（singleton + declaration-presence）

`monitoring.punch` は project 全体の値型で、`PunchMarks` の
`{None, None}` が「punch 宣言なし」を表す（serde 既定・
`SetPunchMarks{None,None}` = 解除の authored 意味）。

- object identity は project tag を `stable_id` に使う singleton
  — `RoutingModel`（master pointer）/`Project` と同じ規約。
- 分類は declaration-presence 意味論:
  - `is_declared(m) = m.punch_in.is_some() || m.punch_out.is_some()`
  - 非宣言 → 宣言 = `Added`（`before: None`、evidence 空）
  - 宣言 → 非宣言 = `Removed`（`after: None`、evidence 空）
  - 宣言 → 宣言（内容差）= `Modified`（canonical JSON field
    diff `punch_in`/`punch_out` を evidence に）
  - 同一（非宣言同士を含む）= change なし
- `Modified` 判定は singleton カスタム diff で行い
  （`diff_matched` は `raw: u128` の ambiguity 検査を必須とし
  singleton に raw が無いため）、`RoutingModel` の master
  pointer と同じ直接 emit パターンを使う。
- `affected_range`: 比較対象の宣言（`after.or(before)`）の両
  mark が `Some` で `MusicalRange::try_new(in, out)` が成立するとき
  `SemanticTimeRange::Musical` を付す。片側のみ・`out < in` の
  宣言は `None`（cleanly expressible な場合のみの既存契約）。
- entity 予算は `ctx.tick(2)`（両側各 1 entity — `diff_map` の
  per-side 計数と同じ規約）。

## Schema-version / migration 取り扱い

`monitoring` は `#[serde(default)]` + v11→v12 の identity
migration で、pre-v12 snapshot は空の monitoring として投影
される。これは v8→v9 `smf_documents` と同じ「機能不存在＝真の
不在」の faithful 投影であり、`PartialProjection` の抑制は
行わず通常 diff とする（spec §Unknown/schema 投影領域の先例に
明記済み）。v11 以前に monitor 宣言は存在し得ないため、空投影は
情報損失ではない。

## DIFF_POLICY_VERSION

`DIFF_POLICY_VERSION` を 1 → 2 に進める。monitor 宣言が分類
対象に加わることで、同一 snapshot 対に対する `ChangeSet` の
正解が変わる（monitor 差のみの対が `Equal` → `Different`）。
`ComparisonKey` は `policy_version` を束縛するため、version
bump は旧 policy で計算・保存された結果との key 衝突を防ぐ
fail-closed の既定動作である。`ComparisonSpec::new` は常に
`DIFF_POLICY_VERSION` を使い、既存呼び出し側の変更は不要。

## Apply surface への影響

`crates/synvelis-project/src/apply/`（VCS-05）は新 kind に
**変更なし**で fail-closed のまま:

- `entity::supported_kind` は `matches!` の網羅リスト — 新 kind
  は `false` を返し `ApplyError::Unsupported`。
- `entity_value`/`write_entity`/`protected_fields`/
  `reconcile_membership`/`change_supported`/`check_singleton_
  fields`/`introduced_digests`/`preflight_*` はすべて `_` arm で
  Unsupported/無視に倒れる。
- monitor 宣言 change を含む selection の apply は計画時に
  `Unsupported` で拒否される — 未実装機能を黙って適用しない
  正しい既定。monitor 宣言の apply 対応は別 slice（transplant・
  cascade・preflight の設計が必要）。

## Compatibility / Migration Notes

- `ObjectKind` は serde `kebab-case` — `track-monitor-decl`・
  `punch-marks`。既存 variant の serde 名・並び・`as u8` は不変。
- `DIFF_POLICY_VERSION` bump により `ComparisonKey::digest` は
  同一 snapshot 対でも新旧で異なる。`ChangeSetCache` は
  in-memory の derived cache であり正本ではないため、version
  差による再計算は許容範囲（仕様どおりの invalidation）。
- monitor 宣言を持たない project の比較結果は byte 同一では
  ない（policy_version が key digest・ChangeId 材料に入る）が、
  changes/unresolved/overall の意味論は不変。

## Risks / Trade-offs

- `PunchMarks` の declaration-presence は `{None,None}` を
  「宣言なし」と読む合成 presence — serialized 上は field が
  常に存在するため「Removed なのに before に entity が有った」
  ように見える。authored 意味（いずれかの mark が Some で初めて
  punch intent が存在）に忠実な分類として採用し、design 文書で
  規則を明示する。
- monitor 宣言が `Recording` domain に入るため、Recording のみを
  include した比較で monitor 差も現れる — Arm/Monitor/Punch が
  recording intent の authored 宣言である以上、これは意図した
  分類（domain filter の既存契約）。
- `TrackMonitorDecl` の `lane` が他 track の lane を指す状態は
  `validate_references` が load 時に拒否するため、diff は
  不整合な decl 同士を素直に `Modified` として報告するのみで
  良い（snapshot は resolve 時に検証済み）。

## Open Questions / Gaps

- なし（monitor POD は先行 slice で確定済み、分類規約は
  VCS-02 spec の既存先例に全て収まる）。
