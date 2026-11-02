---
id: "IMPL-MDBEffectiveCompLoweringLoadRevalidationCompDispatch"
type: "impl"
status: "completed"
title: "MD-B Effective Comp lowering・load-time revalidation・CompOp session dispatch (headless)"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/md_b.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/md_b.rs"
      - type: "file"
        path: "docs/impl/ir/mdb-effective-comp-lowering-load-revalidation-comp-dispatch.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lowering.rs"
      - type: "file"
        path: "crates/synvelis-player/src/processor.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/l1_e2e.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/placement.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/controller_store.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-render/src/error.rs"
      - type: "file"
        path: "crates/synvelis-render/src/export.rs"
      - type: "file"
        path: "crates/synvelis-render/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-render/src/processor.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/l2_e2e.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/x_e2e.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/session.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/mda-takegroup-comp-effective-projection-placement.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# MD-B — Effective Comp lowering・load-time revalidation・CompOp session dispatch

## Summary

Issue #50（MD-A #42 の follow-up、親 issue #10/#12）を実装した。
`project_comp` の Effective Comp 投影を player/render の lowering
経路が実際に消費するようにし、load 時再検証（tempo map 再 compile
+ 全 clip の contract 再検査）を `Project::validate_references`
に集約し、`CompOp` を session dispatch 面に配線した。v1/v2
snapshot migration は pre-release 方針どおり honest reject と
判断し再生成ツールは作らない。MD-B 受入 9 テストを含む全
workspace が green。

## Context

- MD-A で `CompRegistry`・`CompOp`・`project_comp` 投影は model 層に
  完成していたが、player/render は依然 raw lane clip のみを
  lowering しており、採用された comp arrangement は再生・書き出し
  に反映されなかった。
- load 時の `validate_references` は comp/entity 整合のみを見て
  おり、persisted tempo map の compile 失敗や clip contract 違反
  は検出されなかった（MD-A review P3 finding）。
- `SynvelisOp::Comp` variant は存在したが `ProjectSession::apply`
  が `ProjectOp` 固定であり、comp 操作が session 面から到達不可能
  だった。

## Implementation

### Changes

- `synvelis-project/src/placement.rs`: `validate_clip_contract` を
  ops.rs から移設し `pub(crate)` 化 — clip contract の
  authoritative な実装は contract の home で所有する。Absolute
  extent は declared denominator の media frames で playback
  window を検査、Musical extent は非空 window のみ検査（tempo 依存
  の長さは consumption に委譲 — MD-A の方針どおり）。
- `synvelis-project/src/project.rs`: `validate_references` が先頭で
  `TickTimeMapping::new(&self.tempo_map)` を実行し、全 clip を
  `validate_clip_contract` で再検査する。`ProjectController::load`
  と post-apply `apply` の両経路が同じ oracle を共有するため、
  malformed snapshot は load で正直に reject され、live state は
  置き換えられない。
- `synvelis-project/src/ops.rs`: contract validator を placement
  から import するよう配線（op-time 検査と同一実装）。
- `synvelis-player/src/lowering.rs`: clip-only lowering を統一
  playback-node モデルへ置き換え。
  - `NodeContent`: `Clip(ClipId)` / `CompSegment{comp_id,
    region_id}` / `CompOverlap{comp_id, transition_id, side}` —
    各ノードの project provenance を型で区別。
  - `PlaybackBinding`: `ClipBinding` の後継。`content`・
    `source_id`・node/port・`ClipWindow`・canonical・
    `Option<FadeWindow>`。
  - `FadeWindow`/`FadeDirection`: engine-frame domain 上の gain
    envelope。`Linear`/`EqualPower` を `gain_at(engine_frame)` で
    評価 — domain は side 自身の overhang span で、left は
    boundary で unity から `overlap.end` へ 0 まで減衰、right は
    `overlap.start` で 0 から boundary へ unity まで立ち上がる。
  - `SkippedPlayback`: `SkippedClip` の後継。`Muted`/
    `RateMismatch`/`UnresolvableAnchor`/`MissingSource`/
    `UnsupportedMapping` に加え `SupersededByComp`・`CompSilence`・
    `CompProjection`・`CompSegment{reason}`・`CompOverlap{side,
    reason}`（`CompSkipReason`: `MissingSource`/`RateMismatch`/
    `Unresolvable`/`UnsupportedMapping`）— clip と同一規則を
    projection 境界に適用。
  - `Lowered.clips` → `Lowered.playbacks`: clip（project 順）→
    各 active comp の segment・transition side（take-group 順）。
  - active comp の `target_range` 解決 span と重なる owner lane 上
    の raw clip は `SupersededByComp` として explicit skip —
    double-play を防ぐ。silence region は `CompSilence`。
    `project_comp` 自体の失敗は `CompProjection`（controller 経路
    では起こり得ない — state が invariant を満たすため）。
- `synvelis-player/src/processor.rs`: `ClipPlayer` が
  `Option<FadeWindow>` を受け取り、emit 時に engine 位置で
  `gain_at` を評価して sample に乗算（checked arithmetic）。
- `synvelis-player/src/driver.rs`: `activate` が `playbacks` を
  `prepared` と順序対応させ、`stats` を `NodeContent` キー化。
  `stats_of(clip_id)` は維持、`stats_of_content(&NodeContent)` を
  追加し comp node の per-node stats も到達可能に。
- `synvelis-player/src/lib.rs`: 新 lowering 型を export。
- `synvelis-render/src/prepare.rs`/`processor.rs`/`export.rs`/
  `error.rs`: offline path が `playbacks` を消費 — `open_sources`
  は comp binding source を含み、`ClipRenderer` が同じ
  `FadeWindow`/`gain_at` で gain を適用。`SkippedPlayback` の各
  variant を skip policy に対応付け、`Muted`/`CompSilence`/
  `SupersededByComp` は意図的 silence、それ以外は
  `PreflightError::UnrenderableClip` / `UnrenderableComp{comp_id}`
  で refuse。
- `synvelis-recording/src/ops.rs`: `SynvelisOp::Comp(CompOp)` —
  validate/apply を `CompOp` へ delegate。
- `synvelis-workspace/src/session.rs`: `ProjectSession::apply` を
  `pub fn apply<O: Operation>` へ一般化 — `ProjectOp`・`CompOp`・
  `SynvelisOp` 系 op が同じ validated/revision-checked 経路を
  通る。

### Decisions During Implementation

- **Unified playback node over a parallel comp path**: comp segment
  と transition side を clip と同じ graph-node 家族として lowering
  する設計を採用。comp 専用の別経路は resolver/driver/render の
  二重実装を生む。`NodeContent` で provenance を保持し、graph spec
  自体は project 型を持たない（`Lowered` の従来契約どおり）。
- **Supersede は「owner lane ∩ target_range 重複」で判定**: active
  comp が存在する時点でその lane の target span 内 raw clip は
  演奏しない — arrangement が採用されている以上 raw clip の
  併用は double-play になる。重複判定は半開区間の overlap（
  `clip_start < zone_end && zone_start < clip_end`）。target_range
  が tempo map で解決不能な group は clip を supersede せず、
  segment 個別に resolvability を報告する。
- **Transition は「base segment + overhang side」モデル**: Effective
  Comp の契約どおり、region の base read は region 全域を unity で
  鳴らし、overlap 内では他側 region の extended read が fade で
  重なる。fade domain は side の overhang span — 全 overlap を
  domain にすると双方の gain が unity に達せず crossfade として
  成立しないため、各 side の演奏区間を domain とした。
- **skip/failure は全て explicit**: comp node を下げられない理由は
  `CompSkipReason` で型化し、export は `UnrenderableComp` で
  refuse。暗黙の drop / silent success はない。
- **検証 oracle は `validate_references` に集約**: load 時専用の
  別検査を追加せず、`ProjectController::load`（snapshot 解析後）
  と post-apply `apply` が共有する oracle に tempo compile + clip
  contract を追加した — MD-A の「single oracle + writer 境界」
  方針の自然な延長。
- **v1/v2 snapshot migration: honest reject（再生成ツールなし）**:
  `SCHEMA_VERSION` は MD-A で 3。pre-release / PoC 段階の方針は
  「旧 snapshot を migrate しない」— v1/v2 は `UnsupportedSchema`
  で明示 reject される（`unknown_schema_and_corrupt_snapshots_are_
  rejected` で検証済み）。今回の load-time revalidation は schema
  version を変えない「同一 v3 内の破損」検出であり、versioned
  migration の話ではない。saved bytes を書き換える再生成ツールは
  honest reject の代替として不要 — 必要になった時点で ADR 化する
  （MD-A record の follow-up 記述どおり）。

## Acceptance Matrix Traceability

| AC | evidence | test |
| --- | --- | --- |
| Effective Comp lowering | active comp が segment/overlap node を emit・supersede/silence が explicit | `md_b_01_comp_lowering_replaces_lane_clips` |
| comp playback | lowered node が bound source を sample-exact に再生（fade 乗算込み・superseded clip の double-play なし） | `md_b_02_comp_playback_pcm_exact` |
| inactive comp | 非 active では raw clip が従来どおり再生 | `md_b_03_inactive_comp_plays_raw_clips` |
| comp skips | source 不可解決・rate 不一致が typed skip（`CompSegment`/`CompOverlap`/`CompSkipReason`） | `md_b_04_comp_skips_are_explicit` |
| render path | export が comp projection を sample-exact に書き出し、unlowerable comp は `UnrenderableComp` で refuse | `md_b_comp_export_pcm_exact`・`md_b_comp_unlowerable_fails_export`（l2_e2e） |
| load revalidation | 破損 tempo map・破損 clip contract の snapshot を load で reject・live state 不変・valid snapshot は load 可 | `load_rejects_malformed_tempo_map`・`load_rejects_invalid_clip_contract`・`load_still_accepts_valid_snapshot`（controller_store） |
| CompOp dispatch | `ProjectSession::apply` が `CompOp` を受け revision 連鎖・検証・拒否を共有 | `comp_ops_apply_through_session_dispatch`・`comp_and_project_ops_share_one_revision_chain`（workspace md_b） |

## Validation

全て exit code 0 で実測:

- `mise run fmt` / `mise run fmt-check`: clean。
- `mise run check`（`cargo check --workspace --all-targets`）: clean。
- `mise run lint`（`cargo clippy --workspace --all-targets -- -D warnings`）: clean。
- `mise run test`（`cargo test --workspace --locked`）: 全 suite
  green / 0 fail。MD-B 追加分: player `md_b` 4 本、render l2_e2e
  2 本、project controller_store 3 本、workspace md_b 2 本。
- Toolchain: `rustc 1.99.0`（workspace `rust-version = "1.89"`）。
- Ruviform 側は未変更（git 依存・lock 記録 revision のまま）。
- `docs/impl/ir/README.md` は手動で index 行を追加（`apm_modules`
  未導入のため `doc_maintenance` は未実行 — generated format に
  一致させた同内容の編集）。

## Risks

- `Raw` take の comp candidate adoption・`SetCompRegionMapping`
  op・`AddClip` overlap policy・empty-streams commit gate は引き
  続き follow-up（MD-A record の記載どおり）。
- `Loop`/`SimpleStretch` mapping は comp/clip 双方で lowering を
  explicit skip する（1:1 graph の限界は不変 — skip 理由が型化
  され render でも refuse されるようになった）。
- tempo map が runtime で解決不能な project（op 時点では合法）
  では comp zone の `span` が `None` となり clip は supersede
  されない — 各 segment は自身の resolvability を報告する。

## Known Limitations / Report

- Transition side node は `playback` の `stats` に `NodeContent`
  キーで乗る — `stats_of(clip_id)` は clip のみ。
- EqualPower fade は `sin(t·π/2)` 形状（fade-out は鏡像）—
  本格 DSP crossfade モデルの調整は editor/GUI 段階の課題。
- `CompProjection` skip は controller 経路では到達不能（invariant
  oracle が通った state でのみ発生しない）— 防御的な明示分岐と
  して保持。

## Follow-ups

- `Raw` take adoption・`SetCompRegionMapping`・`AddClip` overlap
  policy・empty-streams commit gate の明文化。
- Loop/Stretch 用 playback node 家族の導入時に comp/clip 両面で
  skip を解消。
- v1/v2 snapshot が実運用で必要になった時点で regeneration tool +
  ADR を検討（現状は honest reject で十分との結論を本 record に
  記録）。

## Rollback

- `lowering.rs` の playback-node モデルは `ClipBinding`/
  `SkippedClip` への逆置換で戻せる（`playbacks`→`clips`、fade
  field 削除、`driver`/`processor`/render 側を同時に戻す）。
- `validate_references` 追加行（tempo compile + per-clip contract）
  を除けば load-time revalidation は消失する。
- `ProjectSession::apply` の `O: Operation` 化は `ProjectOp`
  固定へ戻すだけ。`SynvelisOp::Comp` variant 削除。
- schema version・snapshot format は不変 — rollback に snapshot
  互換性の影響はない。
