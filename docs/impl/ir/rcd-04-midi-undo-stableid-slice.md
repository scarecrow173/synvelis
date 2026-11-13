---
id: "IMPL-034WEVyS44eEOELiBCYBIG"
type: "impl"
status: "in-progress"
title: "RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID slice 実装記録"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs"
      - type: "file"
        path: "docs/specs/rcd-04-midi-undo-stableid.md"
      - type: "file"
        path: "docs/designs/rcd-04-midi-undo-stableid-design.md"
      - type: "file"
        path: "docs/plans/rcd-04-midi-undo-stableid-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task-midi-undo-stableid.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-midi-undo-stableid.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-midi-undo-stableid-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_comp_cas.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-04-task-midi-undo-stableid.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/rcd-04-task-midi-undo-stableid.md"
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

# RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID slice 実装記録

## Summary

Issue #118 残件のうち headless 検証可能な 3 項目 — MIDI phrase
境界・comp 切替 Undo/Redo・VCS stable-ID — を test fixture
で pin した。test-only slice で src 側の変更はゼロ。player に
境界 fixture（wire 列完全一致 + HeadlessDriver 配送 ledger
不変条件）、project に comp 切替 CAS/Undo/Redo + raw 不変
テスト 1 件、VCS stable-ID 横断 fixture 4 テストを追加した。

## Context

`docs/tasks/rcd-04-task-midi-undo-stableid.md` /
`docs/plans/rcd-04-midi-undo-stableid-plan.md` /
`docs/specs/rcd-04-midi-undo-stableid.md` に従う。既存カバ
レッジ（PR#219 `compile_midi_comp` plan oracle・PR#300 app
E2E・PR#319 PCM 3 経路 bit-exact）との差分だけを実装した。

## Implementation

### Changes

- `crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs`
  （新規、2 tests）: `tim02_midi_playback.rs` の fixture 群を
  踏襲し、`rcd_04_midi_phrase.rs` と同じ canonical 2 region
  selection（division 480 → PPQN 960、region A `[0,3840)`・
  B `[3840,6720)`、同一 key64 の境界 cut→再発音、5 pair の
  controller chase union、ch1 混在）を `compile_midi_comp` →
  `compile_midi_events` → `ParamDispatch::dispatch_midi` →
  `HeadlessDriver`/fixture plugin に通す。wire 列の手導出
  期待値との完全一致に加え、配送列への ledger 走査
  （全 note-on が一意の note-off と対・終端 active 空・
  重複発音なし）を独立検査する。
- `crates/synvelis-project/tests/rcd_04_comp_cas.rs`（拡張、
  +1 test → 計 6 tests）: `SetActiveComp` による comp 切替が
  1 apply = 1 revision で行われ、comp registry JSON が切替前
  後で byte 同一、単一 `undo()`（+1 revision — undo は新
  revision を発行する既存契約）で完全復元・`redo()` で同一
  ID 再適用、raw take/source/asset が全操作で不変であること
  を pin。
- `crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs`
  （新規、4 tests）: VCS harness を踏襲し、実 sha256 digest
  の asset seed + `build_comp(with_transition)` で構成。
  save → saved-HEAD snapshot → `comp_key_sets` 一致・
  `compare_snapshots` が `Equal`、transition 有/無の diff が
  `CompTransition` object の `Added`/`Removed` のみを
  `comp-transition-<32hex>` stable ID で表現、comp kind の
  `apply_selected`/`target_fingerprint` が
  `ApplyError::Unsupported` で fail-closed（live 不変）、
  `restore_as_copy` が receipt の `stable_ids.counts` で
  comp 系 4 kind を計上し発行 snapshot の comp 4 map key
  集合が live と一致することを pin。
- doc-chain 6 文書（`rcd-04-midi-undo-stableid*`）+ 管理
  index 再生成。

### Decisions During Implementation

- **期待 wire 列は手導出**: `compile_events` の出力をそのま
  ま期待値にせず、`midi_comp.rs` の境界契約（virtual off →
  chase → note-on の同一 frame 順序・chase 値の latest/
  default 規則・terminal CC123 の発音 channel 限定）から
  導出した。project 側 `rcd_04_midi_phrase.rs` が plan 契約を
  独立 pin するため、wire 層 drift は本 fixture が検出する
  構造。
- **canonical wire 値**: Ruviform の `MidiValue` は canonical
  wire domain（CC は u32 スケール、pitch bend は符号付き
  canonical）を使うため、期待値 builder も同じ変換を経由
  させた（初回実行で raw 7-bit 値を期待して失敗 → canonical
  値に修正して pass）。
- **undo revision 契約**: `undo()`/`redo()` は履歴を戻すの
  ではなく新 revision を発行する既存契約のため、「revision
  番号の復元」ではなく「+1 step・comp registry 内容の完全
  一致」で pin した。
- **apply fail-closed の固定**: comp kind は `entity.rs` の
  `supported_kind` に含まれず `ApplyError::Unsupported` を
  返す現行契約を、そのまま回帰 gate として pin した。
  whitelist 追加は別 slice の設計事項。
- **diff の独立 identity**: `Comp.transition_ids` は親の
  ordered-list membership 差分として Skip 分類されるため、
  transition 増減は `CompTransition` object 自身の change
  のみで表現される — これを双方向（有→無・無→有）で固定。

## Related Experiments

### Adopted

- なし（新規実験なし — 既存 fixture パターンの踏襲のみ）。

### Rejected

- **comp kind の apply whitelist 追加**: 受入条件は stable-ID
  保存の検証であり apply ではない。fail-closed を pin する方
  が現行契約を正直に固定できる。
- **`MidiCompSelection` の永続化 fixture**: #126 の
  native-tick/chase 意味論確定待ちで明示的範囲外。
- **semantic/render 経路の MIDI parity**: その経路に MIDI
  event が存在せず比較対象がない。

## Validation

実行済み（cargo 直接実行は補助証拠、canonical = mise）:

- `cargo test -p synvelis-player --test
  rcd_04_midi_phrase_boundary` → 2 passed, 0 failed
- `cargo test -p synvelis-project --test rcd_04_comp_cas` →
  6 passed, 0 failed
- `cargo test -p synvelis-project --test
  rcd_04_vcs_stable_id` → 4 passed, 0 failed
- `cargo check --workspace --all-targets`（synvelis-project
  関連範囲）→ 成功

canonical lane（Windows、`mise run`、最終コード状態で実行;
test-msrv は `mise exec rust@1.89.0`）:

- `mise run fmt-check` → exit 0
- `mise run check` → exit 0
- `mise run lint` → exit 0
- `mise run test`（`cargo test --workspace --locked`）→ exit 0
- `mise run test-rt` → exit 0
- `mise run check-app-native` → exit 0
- `mise run lint-app-native` → exit 0
- `mise run test-msrv` → exit 0（rust 1.89.0; 変更した 2 test
  binary は最終コードで targeted 再実行して pass）
- `mise run build` → exit 0

review-protocol: 独立 reviewer 2 名（cooperative/adversarial）
によるレビュー実施。P0–P2 なし。P3 指摘のうち実害のあるもの
（vacuous assert・take_groups 未比較・redo 片側 assert・
unresolved 未検査）を修正し対象 test を再実行して pass。
残る P3（ledger 走査の冗長性・intra-chase 順序の厳密 pin）は
意図的な契約固定として維持。

## Risks

- 期待 wire 列の手導出ミスの残存リスク（design の Risks 節
  参照）。
- restore fixture は headless の published-layout 検証であり
  FS 耐久性認定ではない。

## Follow-ups

- Issue #118 残件: 実 device/OS GUI・AudioLens f64・app GUI
  E2E・MIDI comp 永続化（#126 待ち）は本 slice 範囲外 —
  Issue は Open 継続（PR 本文は `Refs #118`）。
- comp kind の VCS-05 apply whitelist 拡張を行う場合は、
  typed entity extraction/transplant/membership reconcile を
  含む別 slice で本 fixture の fail-closed 期待を更新する。
