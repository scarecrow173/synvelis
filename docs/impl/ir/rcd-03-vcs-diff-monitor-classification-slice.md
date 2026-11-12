---
id: "IMPL-034VDfqOMnbp5s6CThZTBe"
type: "impl"
status: "completed"
title: "RCD-03 VCS semantic diff monitor 宣言分類 slice 実装記録"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
      - type: "file"
        path: "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
      - type: "file"
        path: "docs/plans/rcd-03-vcs-diff-monitor-classification-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-03-task-vcs-diff-monitor-classification.md"
      - type: "file"
        path: "docs/test-specs/rcd-03-vcs-diff-monitor-classification.md"
      - type: "file"
        path: "docs/impl/ir/rcd-03-vcs-diff-monitor-classification-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/diff/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/adapters.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs02_diff.rs"
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
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-03-task-vcs-diff-monitor-classification.md"
    - "docs/plans/rcd-03-vcs-diff-monitor-classification-plan.md"
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-03-task-vcs-diff-monitor-classification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-03-monitor-persistence-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
    - "docs/plans/rcd-03-vcs-diff-monitor-classification-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-03-vcs-diff-monitor-classification.md"
  verified-by:
    - "docs/test-specs/rcd-03-vcs-diff-monitor-classification.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-03 VCS semantic diff monitor 宣言分類 slice 実装記録

## Summary

Issue #117（RCD-03 Monitoring / Punch / Latency）の残 headless
スライスとして、schema v12 で永続化される authored monitor 宣言
（`TrackMonitorDecl`・`PunchMarks`）を VCS-02 semantic diff の
比較対象 entity 集合へ分類追加した。persistence slice が明示的に
defer した「monitor 宣言の diff 分類」を回収する。新規
`ObjectKind::TrackMonitorDecl`（track-scoped keyed-synthetic
view）と `ObjectKind::PunchMarks`（project singleton）は
`ComparisonDomain::Recording` 配下で add/remove/modify を既存
規約どおり出力し、monitor-only 差分が `Equal` と誤報しないよう
`DIFF_POLICY_VERSION` を 1 → 2 へ bump した。
route/gate/latency の意味論・session 観測値・runtime engine は
対象外のまま変更なし。

## Context

- spec `docs/specs/rcd-03-vcs-diff-monitor-classification.md`
- design `docs/designs/rcd-03-vcs-diff-monitor-classification-design.md`
- plan `docs/plans/rcd-03-vcs-diff-monitor-classification-plan.md`
- test-spec `docs/test-specs/rcd-03-vcs-diff-monitor-classification.md`
- task `docs/tasks/rcd-03-task-vcs-diff-monitor-classification.md`

先行 slice（`rcd-03-monitor-persistence-slice.md`）が
`Project::monitoring`（`MonitoringData{tracks, punch}`）を
schema v12 として永続化・write-through 済みで、VCS semantic diff
への monitor 宣言分類を「diff は curated projection のため未列挙
でも破損しない」として follow-up に残していた。本 slice はその
残件のみを回収する。`synvelis-app`/`synvelis-recording`/
`synvelis-player`/`synvelis-workspace`/`synvelis-gateway`/`ui/` は
無変更。

## Implementation

### Changes

- `crates/synvelis-project/src/diff/types.rs`:
  `ObjectKind::TrackMonitorDecl`・`ObjectKind::PunchMarks` を
  enum 末尾に追加（既存 variant の `as u8` discriminant を保持 —
  change ID・issue dedup key が discriminants を材料にする）。
  両 kind を `ObjectKind::domain()` で
  `ComparisonDomain::Recording` へ写像。`ComparisonDomain::Recording`
  の doc comment を authored recording intent を含む旨へ更新。
  `DIFF_POLICY_VERSION` を 1 → 2 へ bump（比較対象 entity 集合が
  変わり monitor-only 差分が v1 の `Equal` から `Different` になる
  ため。version は `ComparisonKey` 内にあり新旧 policy の cache
  key は衝突しない）。
- `crates/synvelis-project/src/diff/adapters.rs`:
  `RecordingAdapter::diff` 末尾に
  `diff_map(ctx, ObjectKind::TrackMonitorDecl,
  &before.monitoring.tracks, &after.monitoring.tracks,
  no_range, false)` と `Self::diff_punch_marks(ctx)` を追加。
  `punch_declared`（`{None,None}` = 宣言なし）と `punch_range`
  （生き残った宣言側優先、`[in,out)` が有効なら
  `SemanticTimeRange::Musical`、partial/inverted は `None`）の
  helper を追加。`diff_punch_marks` は project singleton
  （stable_id = project tagged ID、`RoutingModel` singleton と
  同じ慣行）として `Added`/`Removed`/`Modified` を発行し、
  `Modified` は `punch_in`/`punch_out` の canonical JSON field
  evidence を持つ。undeclared→undeclared は早期 return で
  change なし。
- `crates/synvelis-project/src/diff/engine.rs`:
  `raw_kind_map` doc comment に `monitoring.tracks` が
  keyed-synthetic（`smf_documents`/`applied_commits` と同型 —
  `TrackId` の ID 空間を再利用）で除外される旨を明記。
  ロジック変更なし。
- `crates/synvelis-project/tests/vcs02_diff.rs`:
  `monitor_decl` helper + 7 test 追加 — decl
  add/remove（Track change を伴わないこと）、decl 4 field
  edit（`lane`/`arm`/`monitor`/`channel_map.channels` の
  field evidence 付き `Modified`）、punch
  add/modify/remove（`Added` は宣言 range、`Removed` は消えた
  宣言の covered span を報告）、partial declaration は
  range を捏造しない `Modified`、Recording domain filter
  （含めば monitor/punch が出て、外せば出ない）、同一 monitor
  状態の AA equality、monitor decl の track raw が別 kind と
  衝突したとき `AmbiguousIdentity` で未解決になる regression。
- `docs/` チェーン 5 文書 + 各 README index + 本 impl record。

### Decisions During Implementation

- `monitoring.tracks` は `diff_map` で通常 entity map として
  処理するが、`raw_kind_map` には登録しない。key が `TrackId` を
  借用する keyed-synthetic view なので、登録すると全 decl が
  `Track` との cross-kind collision で `AmbiguousIdentity` 化して
  しまう。逆に decl key の raw が `Clip` 等の別真 kind と衝突
  した場合は `diff_presence`/`diff_matched` の既存
  `raw_is_ambiguous` gate がそのまま decl を未解決化する —
  regression test で確認済み。
- `PunchMarks` は `diff_map` を通さない custom 比較。singleton
  だが `{None,None}` の全 default 状態が authored「宣言なし」の
  ため、`Project`/`RoutingModel` と違って presence 意味論を持つ。
  遷移規則: undeclared→declared = `Added`、declared→undeclared =
  `Removed`、declared→declared（mark 変更）= `Modified`。
- `punch_range` は「transition を生き残った宣言」の range を
  返す: `after` が宣言済みなら `after`、さもなければ `before`
  （`clip_range` の `after.or(before)` fallback と同型だが、
  `PunchMarks` struct は常に存在するため presence 判定を
  `punch_declared` で行う）。初版は常に `after` を見て
  `Removed` で `{None,None}` から range `None` を返す誤りが
  あった — review で発見し、`Removed` が消えた宣言の covered
  span（in=480→out=1920 の実測 assertion 追加）を報告するよう
  修正。
- monitor 宣言が generic `Track` `Modified` として二重報告され
  ないことは構造的に保証される: monitor 宣言は `Track` の
  serialize 内容に含まれず（`project.monitoring.tracks` の別
  map）、`ProjectMetaAdapter` の curated view（name/ppqn/
  tempo_map）にも含まれない。test で `Track` change が 0 件
  であることを確認。
- `monitoring` は `#[serde(default)]` で pre-v12 snapshot は
  空 `MonitoringData` に復元される。v11→v12 比較で decl が
  あれば `Added`（before が宣言を持てないのは真実） —
  routing の `pre_v6` suppression と同種の「移行投影は authored
  内容として diff しない」対象ではないため gate なし。
- `DIFF_POLICY_VERSION` bump は必須と判断した根拠:
  monitor-only 差分が policy v1 では `Equal`・v2 では `Different`
  と observable 差が生じ、version が `ComparisonKey` の材料に
  含まれるため bump しないと旧 policy で計算された
  cached/persisted result と key 衝突する。

## Related Experiments

### Adopted

<!-- 実験 log なし（既存 adapter/diff_map 規約への整合的拡張）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、Windows 11 x64・rust 1.99.0
pinned、`git merge origin/main` で最新 main 取り込み済み —
already up to date）:

- `mise run fmt-check` — Pass（`cargo fmt --all -- --check`、
  exit 0）
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`、exit 0）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`、exit 0）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  exit 0。197 test binary・`vcs02_diff.rs` 28 test を含む全
  workspace 緑）
- `mise run test-rt` — Pass（`cargo test --workspace --locked
  rt_`、exit 0）
- `mise run test-msrv` — Pass（`mise exec rust@1.89.0 -- cargo
  test --workspace --locked`、exit 0。rust 1.89 下で全 workspace
  緑）
- `mise run build` — Pass（`cargo build --workspace --locked`、
  exit 0）

補助証拠（canonical = mise、直接 cargo は補助）:
`mise exec -- cargo test -p synvelis-project --test vcs02_diff`
— 28 passed / 0 failed。

検証中に観測した事象:

- 初回 `mise run test-msrv` が
  `crate synvelis_gateway required to be available in rlib
  format` で失敗（同一 target dir へ rust 1.89 build と 1.99
  fixture test を並行実行した artifact race、本変更とは無関係）。
  並行ジョブ停止後の単独再実行で Pass を確認。
- review-protocol 1 pass 実施: CONFIRMED P1 1 件
  （`punch_range` が `Removed` で before 宣言の range を落とす
  誤り）を修正・test 強化して再検証済み。残り blocker なし。
  P3 候補（`(false,false)` dead arm、singleton entity
  accounting）は非 blocking として据え置き。

## Risks

- `PunchMarks` の declaration-presence は `{None,None}` を
  「宣言なし」と読む合成 presence — serialized 上は field が
  常に存在するため「Removed なのに before に entity が有った」
  ように見える。authored 意味（いずれかの mark が `Some` で
  初めて punch intent が存在）に忠実な分類として design で規則を
  明示済みだが、consumer が serialized shape から presence を
  推測すると誤読し得る。
- `DIFF_POLICY_VERSION` 1 → 2 により、monitor 宣言を持たない
  project でも policy_version が key digest・ChangeId 材料に
  入るため比較 key が変わる（changes/unresolved/overall の
  意味論は不変）。旧 policy の cached/persisted result とは key
  が衝突しないが、monitor-only 差分は v1 の `Equal` から v2 の
  `Different` へ変わる。
- monitor 宣言が `Recording` domain に入るため、Recording のみを
  include した比較で monitor 差も現れる — Arm/Monitor/Punch が
  recording intent の authored 宣言である以上、意図した分類
  （domain filter の既存契約）。
- `TrackMonitorDecl` の `lane` が他 track の lane を指す不整合は
  `validate_references` が load 時に拒否するため、diff は素直に
  `Modified` として報告するのみで良い（snapshot は resolve 時に
  検証済み）。
- review で残った P3 候補（`(false,false)` dead arm、singleton
  entity accounting）は非 blocking として据え置き。

## Follow-ups

- 実 device monitor/latency・MonitorAdapter RT graph 搭載・
  Loop/PunchInLoop gate・count-in UX・実 GUI は Issue #117 の
  後続 slice（本 slice 対象外）。Issue は Close しない。
