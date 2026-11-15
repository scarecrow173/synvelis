---
id: "IMPL-034XPMrg013WoI7EuJkO5f"
type: "impl"
status: "in-progress"
title: "CB-07 Arrangement 候補 Adapter — Track集合×MusicalRange×ArrangementDomainMask・Atomic Adopt（synvelis#284）"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/284"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/variant/arrangement.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb07_arrangement.rs"
      - type: "file"
        path: "docs/impl/ir/cb-07-arrangement-adapter.md"
      - type: "file"
        path: "docs/test-specs/cb-07-arrangement-adapter.md"
      - type: "file"
        path: "evidence/sv284/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv284/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ppqn.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb11_cross_gate.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/creative-branch.md"
    - "docs/designs/creative-branch-design.md"
    - "docs/adr/cb-00-creative-branch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
    - "docs/impl/ir/cb-03-midi-clip-adapter.md"
    - "docs/impl/ir/cb-02-audio-clip-adapter.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/cb-07-arrangement-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# CB-07 Arrangement 候補 Adapter — Track集合×MusicalRange×ArrangementDomainMask・Atomic Adopt（synvelis#284）

## Summary

`VariantScope::Arrangement { tracks, time: MusicalRange, fields:
ArrangementDomainMask }` の実ドメイン `DomainAdapter`
（`ArrangementAdapter`）を `synvelis-project` に実装する。
Track 集合の lane 上で `time`（半開区間 `[start, end)`）と
overlap する musical clip を scope 所有とし、`fields.clips`/
`fields.notes` mask が clip 構造 op と clip-local note op の
編集面を宣言する。clip-local note（`fields.notes` 宣言時）と
clip 所有 control closure は `scope_objects`（CB-01 正本）が
所有権を導出し、adapter は membership のみを参照する。
`boundary_dependencies` が scoped lane 上の非所有 clip
（範囲外 musical = `OutsideRange`、absolute anchor/extent =
`AbsoluteAnchor`）を typed inventory する。candidate 編集は
共有 confined-edit 経路を通り、in-range の clip/note 削除のみを
arrangement 固有の removal policy で受理し、range 外への
move-out・sweep 範囲超過・mask 外・foreign target は全て型付き
fail-closed（mask 未宣言 family は target 所在に関わらず
`Unsupported`、宣言済み family の foreign target は
`ScopeViolation`）。adopt は既存 plan/preflight/CAS/Undo で
宣言範囲のみを 1 CAS で materialize する。schema/永続 model
は非変更。実 OS/Plugin/RT/PCM/GUI timeline 受入は本スライス
対象外（`evidence/sv284/` で NotRun 記録）。

## Context

- Issue: synvelis#284（CB-07 Arrangement 候補 Adapter）。
  `TrackId set × MusicalRange × ArrangementDomainMask` の型付き
  領域、1 候補が複数 Track/Clip 変更を包含、`CB-ARR-01`〜
  `CB-ARR-06` + `CB-ARR-RCP-01`。
- CB-01（#278）が `VariantScope::Arrangement`/
  `ArrangementDomainMask`/`scope_objects` の所有権導出を、
  CB-02（#279）が clip op confinement・recording gate 共有を、
  CB-03（#280）が clip-local note 面と branch-local ID lineage
  を確立済み。本 adapter はそれらを再利用し、range 導出
  ownership・mask gating・boundary inventory・adopt preflight
  の recording gate を追加する。
- engine/apply の scope-removal 拒否（`plan_adopt_writes`）は
  slice 横断の不変条件であり本スライスでは変更しない —
  削除を含む candidate は fork/edit/diff/save-reload 可能だが
  adopt/render 解決は型付き `Unsupported` で deferred。

## Implementation

### Changes

- `variant/arrangement.rs`（新規）: `ArrangementAdapter` —
  `DomainAdapter` 実装。`validate_scope`（scope kind・構造・
  `scope_objects` 全導出）、`fork_candidate`/`edit_candidate`/
  `preflight_adopt`（scoped track 全 lane の recording gate）、
  `capabilities`（`ArrangementCapabilities` 報告 = enforcement
  と一致・mask 相対）、`boundary_dependencies`（scoped lane 上
  の非所有 clip の `ArrangementBoundaryDependency` +
  `ArrangementBoundaryReason` typed inventory）。
  `check_arrangement_scope_removals` — arrangement 固有の
  post-edit removal policy: `Clip`/`Note` が candidate から
  実体削除された場合のみ scope 脱退を許可し、実体が残ったまま
  range を外れた場合は `Unsupported`、その他 kind は fail-closed。
- `variant/adapter.rs`: `is_arrangement_op` predicate（受理
  10 op family）、`check_arrangement_op_targets`（entity 対象
  op は mask 照合を先に行い `require_scope` で target を
  confinement、`SplitClip` は `clips`-only mask で note 移動を
  起こす場合 `check_split_note_migration` が `Unsupported`、
  `AddClip` は scoped lane、`RemoveClipsInRange` は scoped
  lane + `time` 内包を要求）、`Arrangement` scope dispatch、
  `ScopeRemovalCheck` 型と
  `edit_scoped_project_op_with_removal`（removal policy seam —
  既存 adapter は従来の fail-closed 既定を維持）。
- `variant/arrangement.rs`: `edit_candidate` に scope kind
  guard（engine が adapter と scope を独立に受け取るため、
  arrangement 専用 removal policy が foreign scope へ漏れない
  よう非 Arrangement scope は `InvalidScope`）、
  `boundary_dependencies` に `validate_scope_structure` 先行
  （malformed scope を typed error 化）。
- `ops.rs`: `clip_beats` を `pub(crate)` 化 — split note
  migration 判定が apply と同一の tick→beat 変換を共有する
  ため（規則の drift 防止）。
- `ppqn.rs`: `reproject_ppqn_with` が
  `VariantScope::Arrangement.time` も再射影（`arrangement_scope`
  site — `preview_ppqn` と site 列挙を共有）。CB-07 が
  persisted musical-tick を持つ最初の scope payload であり、
  変換漏れは scope 宣言を旧 grid に残して所有導出を静かに
  ずらす。
- `tests/cb11_cross_gate.rs`: `adapter_for` dispatch を
  `ArrangementAdapter` に接続（#284 merge に伴う stale
  placeholder の解消。arrangement fixture 自体は #288 の
  責務のまま）。
- `variant/mod.rs`: `ArrangementAdapter`・
  `ArrangementCapabilities`・`ArrangementBoundaryDependency`・
  `ArrangementBoundaryReason` の export。
- `tests/cb07_arrangement.rs`（新規）: headless fixture —
  CB-ARR-01〜06 + CB-ARR-RCP-01 相当の 17 case（review 対応で
  split note gate・PPQN scope 変換・adapter/scope 結束・
  boundary 構造検証を追加）。
- `evidence/sv284/`（新規）: acceptance ledger・verification
  記録（OS-L1 headless、GUI-L2/EXT-L3/実 plugin/実 Render は
  NotRun）。

### Decisions During Implementation

- **ownership は `scope_objects` に一元化**: adapter は
  clip/track/range/mask の所有権を再導出しない。
  `arrangement_clips`（scope.rs 正本）の半開 overlap
  （`start < time.end() && time.start() < end`）と
  musical-only 配置条件をそのまま使い、境界 inventory も同じ
  導出結果との差分で報告する — confinement・fingerprint・
  conflict・render pin・adopt plan の全てが単一の真実を共有。
- **mask は編集面のみに効く**: `fields.notes` 宣言時は
  `scope_objects` が clip-local note と親 clip を所有する
  （notes implies clip ownership — CB-01 契約）。mask 未宣言の
  family の op は target 所在に関わらず `Unsupported`（capability
  report = enforcement の一致）、宣言済み family の scope 外
  target は `ScopeViolation` — mask 判定を target confinement
  に先行させる（review で統一。creation/sweep arm は当初から
  mask 先行だった）。
- **`SplitClip` の note 再配置は mask 外副作用として gate**:
  apply は split 点以降の clip-local note を新 clip へ無条件に
  再親付けする。`clips`-only mask では note が scope object
  でないため、candidate だけが note を書き換え adopt が
  materialize できず live と静かに乖離する（review で確認）。
  `fields.notes` 未宣言時、split 点以降に note を持つ clip の
  split を `Unsupported` で拒否 — split 点より前の note は
  kept clip に残り乖離しない。
- **`RemoveClipsInRange` は内包を要求**: sweep range が `time`
  を部分的にでもはみ出すと、object-set diff が観測できない
  範囲外 clip を削除し得るため、`time.start() <= range.start()
  && range.end() <= time.end()` を pre-edit で強制する。
  結果 clip が `time` 内に収まるかは post-edit の
  created-within-scope 検査が担保。
- **in-range 削除は受理、adopt は deferred と正直に宣言**:
  range 選択が clip を削除できなければ CB-ARR-01 の A/B 面を
  表現できないため、`RemoveClip`/`RemoveClipsInRange`/
  `SplitClip` を候補編集として受理する。ただし
  `plan_adopt_writes`/`resolve_render_project` の scope-removal
  拒否は slice 横断の不変条件（engine/apply は本 slice の
  変更境界外）なので、削除を含む candidate の adopt/render は
  型付き `Unsupported` で拒否し live/revision/branch 状態を
  一切変更しない — 部分 commit は構造的に発生しない。
- **move-out は削除ではない**: `MoveClip`/`SetClipExtent`/
  `SplitClip` で scoped clip が実体を保ったまま `time` を外れる
  と、scope object 集合からの脱退として検出される。clip/note
  実体が candidate に残る限り arrangement removal policy は
  `Unsupported` を返す — 「編集」ではなく「未追跡の live
  mutation」であり、range の再解釈による暗黙の scope 拡大を
  許さない。
- **absolute anchor/extent は所有も編集もしない**: musical
  span を安定的に導けない配置（tempo-map 変換 policy 不在）
  は `scope_objects` が所有せず、`boundary_dependencies` が
  `AbsoluteAnchor` として報告する。`OutsideRange` は同 lane 上
  の musical 非重複 clip — 隣接 shared context であり、より
  広い sweep が届き得た境界として区別する。
- **recording gate は scoped track の全 lane**: clip 編集は
  lane 上で起きるため、scope 対象 track のいずれかの lane に
  open session があれば `RecordingActive` で adopt を拒否
  （CB-ADP-03 共有契約）。foreign track の session は gate
  しない（回帰 test で両側を固定）。
- **PPQN 再射影は scope 宣言も対象**: `SetProjectPpqn` の
  walk（`reproject_ppqn_with` — preview と共有）が
  `Arrangement.time` を `arrangement_scope` site として
  変換する。scope 宣言を旧 grid に残すと clip だけが変換され
  所有導出が黙ってずれる。なお branch content store 内の
  candidate payload は project 外のため本 walk の対象外 —
  PPQN 変更後の adopt が旧 grid の clip 値を transplant
  し得る slice 横断の残存リスクであり、全 domain 共通の
  engine/apply 側論点として Follow-ups に記録する。

## Related Experiments

### Adopted

<!-- 実装中に採用した experiment log を記録する。 -->

### Rejected

<!-- 棄却した experiment log を記録する。 -->

設計分岐の検討記録は `docs/impl/exp/cb-07-arrangement-adapter.md`
に集約（candidate 側 scope-removal の adopt materialization・
absolute anchor の tempo-map 変換・sweep の部分重複受理はいずれも
棄却 — fail-closed と typed defer を優先。review で確定した
split note migration gate・mask 先行順序・PPQN scope 変換・
adapter/scope 結束も E7 に記録）。

## Validation

- `mise run fmt-check` — Pass（exit 0、`cargo fmt --all` 適用後）。
- `mise run check synvelis-project` — Pass（exit 0、
  `--locked --all-targets`）。
- `mise run lint synvelis-project` — Pass（clippy
  `-D warnings`、exit 0）。検出・修正: `trivially_copy_
  pass_by_ref`（`require_arrangement_mask` を by-value 化）、
  test 側 `similar_names`（lane/clip local を role 名へ）、
  `too_many_lines`（cb03 先例の `#[allow]`）、`let_and_return`、
  `needless_lifetimes`、`iter_kv_map`。
- `mise run test synvelis-project` — Pass（exit 0、全 suite
  green。`cb07_arrangement.rs` 17 件、cb01〜cb06/cb11 系の
  回帰なし）。
- `mise run test-rt` — workspace `rt_` contract tests。
- 詳細は `evidence/sv284/verification.md`。

## Risks

- `scope_objects`（CB-01 正本）と adapter の mask/target 判定
  が drift すると mask 外波及や range 外所有を見逃す —
  adapter は所有権を再導出せず `scope_objects` membership と
  その差分のみを参照する構造で抑止。
- removal policy の「実体削除のみ許可」判定が clip/note 以外
  の kind を見落とすと scope 脱退が黙って通る — 現在は
  `Clip`/`Note` 以外を fail-closed にしており、`scope.rs` に
  新しい scope-owned kind が追加された場合は policy の再検討
  が必要（`check_arrangement_scope_removals` の match が
  網羅点）。
- boundary inventory は scoped lane 上の clip のみを見る —
  clip 以外の shared context（source/asset/automation lane・
  send/connection）は clip-local note と control closure を
  除き inventory 対象外。CB-ARR-04 の「Automation/Tail/Loop/
  Tempo 影響域の宣言」は本 slice では clip 境界 + control
  closure 所有までであり、tempo-map 依存・tail 伝播の完全な
  影響解析は後続 slice。
- 削除 candidate の adopt/render deferred は利用者にとって
  「編集できるが採用できない」中間状態 — 型付き `Unsupported`
  で明示しており silent な部分 commit はないが、apply surface
  の `Removed` materialization が来るまで UX 側で削除候補を
  adoptable と表示してはならない。
- PPQN 変更は scope 宣言を正しく再射影するが、branch content
  store 内の candidate payload は project 外のため対象外 —
  fork 後に PPQN が変わると adopt が旧 grid の musical 値を
  transplant し得る。CB-01 以来の全 domain 共通の残存
  リスクであり、本 slice の境界外として Follow-ups に記録
  （fail-closed 化するなら engine/apply 側で variant set
  存在時の PPQN 変更拒否または payload 再射影が必要）。

## Follow-ups

- scope entity removal（clip/note/closure）の adopt/render
  materialization — `plan_adopt_writes`/`apply` の `Removed`
  対応は engine 側 slice（本 slice の変更境界外）。
- PPQN 変更と variant candidate payload の grid 整合 —
  `SetProjectPpqn` は live project の scope 宣言を再射影する
  が、content store 内の candidate は旧 grid のまま残る。
  variant set 存在時の PPQN 変更を typed 拒否するか、
  adopt 時に payload を再射影するかは engine/CB-01 側の
  設計判断（全 domain 共通）。
- tempo-map 変換を伴う absolute anchor/extent clip の
  musical scope 写像 — 変換 policy 確立後に ownership 拡張を
  検討（`AbsoluteAnchor` boundary reason の解消）。
- CB-ARR-04 の Automation/Tail/Loop/Tempo 影響域の完全な
  宣言と、CB-ARR-05 の Unaligned/Unknown diff verdict —
  CB-08 比較 orchestration（#285）・cross-gate（#288）・
  実 timeline GUI（#114）の責務。
- 実 OS/Plugin/RT/PCM/render 受入は #286/#287/#288 系の
  実機 lane（`evidence/sv284/` で NotRun 記録）。
