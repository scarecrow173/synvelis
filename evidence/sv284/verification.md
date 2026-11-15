# Verification — synvelis#284 CB-07 Arrangement headless スライス

Date: 2026-11-15. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust 1.99.1
（repo pin 1.99.0 系、edition 2024 / MSRV 1.89）。

## 実行結果（実測）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0、`cargo fmt --all -- --check`） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | Pass（exit 0 — 全 test suite green。`cb07_arrangement.rs` 17 件を含む。cb01〜cb06 系の回帰なし） |
| `cargo test -p synvelis-project --test cb07_arrangement` | Windows 11 | Pass（17/17 — review 対応で split note gate・PPQN scope 変換・adapter/scope 結束・boundary 構造検証を追加） |
| `mise run test-rt` | Windows 11 | Pass（exit 0 — workspace `rt_` contract tests） |
| `mise run check-headless-deps` | Windows 11 | Pass（exit 0 — 12 headless crates に GUI framework 依存なし） |

## 補助証拠（canonical = mise）

- lint で検出・修正した点: `trivially_copy_pass_by_ref`
  （`require_arrangement_mask` を `ArrangementDomainMask`
  by-value 化 — 同型は Copy で 2 byte）、test 側
  `similar_names`（fixture の lane/clip local を role 名
  `lane_main`/`lane_spare`/`lane_b`/`lane_out`/`clip_on_b`
  へ）、`too_many_lines`（cb03 先例の `#[allow]` を
  `cb_arr_01`/`cb_arr_08` の lifecycle fixture に付与）、
  `let_and_return`、`needless_lifetimes`、`iter_kv_map`。
- 実装上の要点: `adapter.rs` に `is_arrangement_op`
  predicate（受理 10 op family）・`check_arrangement_op_
  targets`（entity 対象は `require_scope`+mask 照合、
  `AddClip`/`RemoveClipsInRange` は scoped lane + `time`
  内包）・`Arrangement` scope dispatch・`ScopeRemovalCheck`
  seam（`edit_scoped_project_op_with_removal` — 既定は
  従来の fail-closed を維持）を追加。`variant/
  arrangement.rs` の `ArrangementAdapter` が `validate_
  scope`/`fork_candidate`/`edit_candidate`（op validate +
  `clip_edit_guard` + arrangement removal policy）/
  `preflight_adopt`（scoped track 全 lane の recording
  gate）/`capabilities`/`boundary_dependencies`（
  `OutsideRange`/`AbsoluteAnchor` typed inventory）を
  実装。ownership 導出は CB-01 の `scope_objects` を
  再利用し adapter 側で再実装しない。
- fixture は `arrangement_fixture()` — 2 scoped track
  （track_a: 2 lane、track_b: 1 lane）+ foreign track、
  shared source、各 lane に in-range/edge/out-of-range/
  absolute clip、clip-local note 2 件（scoped/foreign）。
  recording 注入 controller は `CounterIdGenerator::new
  (10_000)`（CB-04/05/06 先例と同規約）。
- `cb_arr_11` が固定する deferred 契約: `RemoveClip`/
  `RemoveNote` の削除候補は fork/edit/diff/save-reload
  可能だが adopt は `VariantError::Unsupported`、render
  解決は `RenderInputError::Unsupported` — `plan_adopt_
  writes` の slice 横断 no-removal 不変条件により部分
  commit は構造的に発生しない。
- 独立 review（cooperative + adversarial の 2 系統）で
  確認・修正した blocking 級の指摘:
  - `SetProjectPpqn` の再射影 walk が
    `VariantScope::Arrangement.time` を変換していなかった
    — `arrangement_scope` site を追加し、preview/mutation
    の site 列挙を共有（`cb_arr_14`）。
  - `edit_branch_op` が adapter と scope を独立に受け取る
    ため arrangement 専用 removal policy が非 Arrangement
    scope へ漏れ得た — `edit_candidate` に scope kind guard
    を追加（`cb_arr_15`）。
  - `SplitClip` が `clips`-only mask で scope 非所有の
    clip-local note を新 clip へ再親付けし candidate/adopt
    が乖離し得た — `check_split_note_migration` で
    `Unsupported`（`cb_arr_13`）。
  - mask 未宣言 family の判定を target confinement に先行
    させ capability report と enforcement を一致
    （`cb_arr_08` の verdict を更新）。
  - `boundary_dependencies` に `validate_scope_structure`
    を先行し malformed scope を typed error 化、capability
    doc の非対称表現を修正。
  - 残存する P2/P3 候補（scope 導出の overflow 取り扱い、
    dead lane の typed adopt 失敗順序、empty-lane sweep
    no-op 等）は fail-closed 方向であり blocking ではない
    と判定 — exp/IR に記録。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク
  指定により対象外（daily CI の責務）。
- 実 OS provider・実 Plugin host・外部 Plugin GUI・実 RT・
  実 Render pipeline・実デバイス入力 pipeline・workspace/
  GPUI projection・実 timeline GUI（#114）— headless
  fixture のみ。Fake/Domain Oracle の PASS を実機 PASS に
  昇格しない（後続 Issue #285/#286/#287/#288 系で記録）。
- CB-08 比較 orchestration・semantic merge・blend・
  `Unaligned`/`Unknown` verdict — #285 Deferred。
- `doc-driven-dev-graph` route — spec/design/plan/task
  相当の内容はリース済み `docs/impl/ir/cb-07-
  arrangement-adapter.md`・`docs/test-specs/cb-07-
  arrangement-adapter.md` に集約（リース外 docs への
  書き込みは行っていない）。
