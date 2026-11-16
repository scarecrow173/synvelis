---
id: "DESIGN-034YpTJ7LGHVn4vKqeBjhO"
type: "design"
status: "approved"
title: "RCD-04 residual closeout — AC 照合と gate 分類・簿記整合の設計"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/rcd-04-residual-closeout.md"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/designs/rcd-04-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-04-residual-closeout-exp.jsonl"
      - type: "file"
        path: "docs/discovery/rcd-04-residual-closeout.md"
      - type: "file"
        path: "evidence/sv118/rcd-04-closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/tasks/rcd-04-task-midi-undo-stableid.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task-rt-engine-comp-pcm.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task1-product-e2e.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-midi-undo-stableid-slice.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-product-e2e-slice.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-04-residual-closeout.md"
  implemented-by: []
  depends-on: []
  blocks:
    - "docs/plans/rcd-04-residual-closeout.md"
    - "docs/tasks/rcd-04-residual-closeout.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/designs/rcd-04-midi-undo-stableid-design.md"
    - "docs/designs/rcd-04-product-e2e-design.md"
    - "docs/designs/rcd-04-rt-engine-comp-pcm-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-residual-closeout.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/discovery/rcd-04-residual-closeout.md"
    - "evidence/sv118/rcd-04-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-04 residual closeout — AC 照合と gate 分類・簿記整合の設計

## Context

RCD-04 のプロダクション実装は 4 系統の PR（#219 crate
fixture slice: `rcd_04_comp_cas`/`rcd_04_midi_phrase`/
`rcd_04_recovery_partial`/`rcd_04_take_comp_pcm`、#300
product E2E: `crates/synvelis-app/tests/
rcd_04_product_e2e.rs`、#319 RT engine comp PCM:
`crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs`、#374
MIDI phrase boundary / Undo / VCS stable-ID:
`rcd_04_midi_phrase_boundary`/`rcd_04_vcs_stable_id`）で
main に統合済み。残る作業は「Issue の受入条件 4 行 ↔
実在 test/merge PR 記録」の照合、canonical lane の再実測、
外部 gate の NotRun/Deferred 分類、doc status の簿記整合と
いう**簿記のみ**である。

照合の入力は 3 系統:

1. Issue #118 本文 — 受入条件 4 行・検証方法。
2. doc chain — spec/design/plan/task/test-spec/IR/exp
   （`rcd-04-comp-transition-midi-phrase-fixture`・
   `rcd-04-midi-undo-stableid`・`rcd-04-product-e2e`・
   `rcd-04-rt-engine-comp-pcm`・`rcd-04-residual-closeout`）。
3. 実測面 — `crates/*/tests/rcd_04_*.rs` の実在テスト、
   merge 済み PR 記録（#219/#300/#319/#374）、canonical
   mise lane。

## Scope

### In Scope

- 受入条件行ごとの証拠写像表（spec の Acceptance Criteria
  節と IR の対応表、`evidence/sv118/
  rcd-04-closeout-verification.md` の 1:1 表）。
- canonical lane 5 本（`mise run test synvelis-project` /
  `synvelis-recording` / `synvelis-player` /
  `synvelis-render` / `test-rt`）+ 軽量チェック
  （`fmt-check`・`prek run --all-files`）の実走と Pass/Fail
  記録。opt-in `mise run test synvelis-app` は任意再実行
  （`rcd_04_product_e2e` の所在 package）。
- doc status/relations の実態整合（編集許可範囲内）。
  docs index（`docs/**/README.md`）は Coordinator が統合時に
  再生成するため本 slice では一切触れない。
- 簿記 drift 修正: merge 済み 3 task file の status
  `in-progress`→`done`（PR #374=`4946a59`・#319=`ff33ffb`・
  #300=`a030e1f`）と stale checkbox の事実整合、merge 済み
  3 impl/ir file の status `in-progress`→`completed`。
- `docs/discovery/rcd-04-residual-closeout.md`（closeout
  対象棚卸しメモ）と `docs/impl/exp/
  rcd-04-residual-closeout-exp.jsonl`（run-trace）の作成。
- 外部 gate の NotRun/Deferred 分類と所有 Issue の記名。

### Out of Scope

- プロダクションコード・依存定義・workflow の変更。
- 外部 gate の実施または Pass 昇格。
- `*-msrv`・`*-full` 系 lane（Daily Full CI 委譲）。
- docs index の編集・再生成・`index apply`・`doc_status`
  audit（Coordinator 側の責務）。
- Issue close・PR merge・他 slug の docs・`evidence/` 他 dir。

## Components and Boundaries

- **照合器（本設計の中核）**: Issue AC 行 → 実在 test 名/
  merge PR 記録/lane 結果の対応表。判定語彙は `Pass`（実測）/
  `NotRun`（lane あるが未実測または自動化不能）/
  `Unsupported`（host に能力なし）/`Deferred`（別 Issue 所有
  の残 gate）/`Fail`（実測失敗）の 5 値で、sv#116 先例の
  語彙を踏襲する。AC 各行の root 証拠は実 test 名・merge
  済み PR/CI 記録・既存 doc への引用に限定し、新規
  fixture/test は作らない。
- **canonical lane 実行者**: `mise run test <package>` /
  `mise run test-rt` を worktree で逐次実行し、exit code・
  test 数を記録する。docs+evidence only の変更のため cargo
  系 lane は doc 差分に影響されないが、「AC 照合時点の main
  lane 実測」として証拠化するため省略しない（sv#116 先例
  どおり）。
- **簿記整合器**: merge 済み 3 task file の status を実態
  （`done`）に合わせ、stale な Done When/Verification の
  `[ ]` 項目を事実に整合させる（merge 済みで実施済みの
  項目は `[x]` + merge PR 番号注記、当時未実施の項目は
  `[ ]` のまま残すか closeout evidence への参照を付記 —
  当時の作業記録本文は書き換えない）。merge 済み 3 impl/ir
  file の status を `completed` に合わせ、merge PR との
  対応付けを注記する。
- **証拠固定者**: `evidence/sv118/
  rcd-04-closeout-verification.md` に lane 結果・照合表・
  gate 棚卸しを記録し、IR/exp/discovery に恒久記録を置く。

## Data and Control Flow

1. Issue #118 本文の受入条件 4 行を抽出。
2. 各行について `crates/*/tests/rcd_04_*.rs` の実在テスト名
   を `#[test]`/`fn` 列挙で照合し、対応する merge PR
   （#219/#300/#319/#374）と doc chain を紐付ける（存在し
   ない証拠を推測で補わない・再 derive しない）。
3. canonical lane を実走し結果を採取。
4. spec Acceptance Criteria・plan Task Breakdown・task
   Done When・IR Validation に同一の判定を転記。
5. 外部 gate 行は `NotRun`/`Deferred` + 所有 Issue（実 GUI =
   synvelis#114、AudioLens native f64 consumer =
   `scarecrow173/ruviform#294`、MIDI comp selection 永続化 =
   synvelis#126、実 device/実 RT 計測 = 実機試験、comp kind
   VCS-05 apply whitelist 拡張 = 別 slice deferred、統合 CI
   証拠 = merge 後 main SHA の CI を Coordinator が確認）を
   記名。
6. 変更ファイルが占有リスト内に限定されることを
   `git diff --stat` / `git status` で確認する（docs index
   regen・`doc_status` audit は本 slice の対象外）。

## Non-Functional Constraints

- **誠実性不変条件**: NotRun/Unsupported/Deferred/Mock を
  Pass と書かない。headless fixture の PASS は headless 範囲
  の証拠に限定する。実行していない検証は NotRun と記録
  する。
- **占有境界**: worker spec の編集許可リスト外は一切変更
  しない（`crates/` 全体・`ui/`・`Cargo.toml`・`Cargo.lock`・
  `mise.toml`・`.github/`・`POLICIES.md`・`AGENTS.md`・他
  slug docs・`docs/**/README.md` 管理 index・`evidence/` 他
  dir を含む）。
- **再現性**: lane 結果には実行環境（OS・toolchain・対象
  commit）と再現コマンドを添える。
- **index 非接触**: `docs/**/README.md` は編集・再生成・
  `index apply` のいずれも行わない（Coordinator が統合時に
  `doc-maintenance` で再生成。index 未同期は許容済み）。

## Risks and Trade-offs

- **照合の誤判定**: test 名の列挙だけでは受入条件の意味的
  充足を誤る可能度がある → 各行に test 名と「何を証明する
  か」を併記し、外部 gate 由来の部分は行内で明示分離する。
- **lane の環境依存 flake**: 過去に webdav flake・LNK1102・
  os error 10053 の観測例あり → Fail 時は単独再実行で切り
  分け、最終結果を正直に記録する。
- **AC-3 の「実 GUI＋headless」併記**: 実 GUI 分は synvelis#114
  gate であり headless のみでは行全体を Pass にできない →
  headless 分と実 GUI 分を行内で分離し、後者は Deferred と
  記名する。
- **AC-4 の横断性**: 「AudioLens native f64 非回帰」は
  Synvelis 側では raw PCM/Asset 不変の pin のみで、consumer
  側の f64 精度は `scarecrow173/ruviform#294` gate → 該当
  部分を分離して gate 記名し、Synvelis 側の責務分だけを
  Pass とする。
- **過剰な doc 量**: 簿記タスクに 7 file chain + discovery +
  exp.jsonl を起こすのは重い → ただし doc-driven-dev-graph
  の traceability を満たすため、各 doc は最小構成に留める。

## Deferred Design Concerns

- 実 GUI・実 device・実 RT 計測・consumer・MIDI 永続化・
  comp kind apply whitelist 拡張の gate 実施 | 本環境
  （Windows headless）では原理的に不可または別 Issue 所有 |
  synvelis#114・#126・`scarecrow173/ruviform#294`・実機試験・
  別 slice | mock PASS の実機昇格・NotRun の Pass 誤記。
- Issue #118 の close 判定 | 外部 gate が残存 | gate 所有
  Issue の完了後 | 未証明行を閉じたままにする誤判定。
- docs index（`docs/**/README.md`）の再生成 | 生成物は
  Coordinator が統合時に `doc-maintenance` で再生成 | 本 PR
  merge 時 | index 未同期のまま残る（許容済み・PR 本文に
  明記）。

## Verification Notes

- [x] canonical lane（`mise run test synvelis-project` /
  `synvelis-recording` / `synvelis-player` /
  `synvelis-render` / `test-rt`）+ 軽量チェック
  （`fmt-check`・`prek run --all-files`）を `mise run` で
  実走し exit code を記録（結果は
  `docs/impl/ir/rcd-04-residual-closeout.md` Validation と
  `evidence/sv118/rcd-04-closeout-verification.md`）。
- [x] spec の Acceptance Criteria 各行が実在テスト/merge
  PR 記録に対応することを `#[test]`/`fn` 列挙で照合済み。
- [x] 変更ファイルが占有リスト内に限定されることを diff で
  確認（docs index・`doc_status` audit は対象外）。
