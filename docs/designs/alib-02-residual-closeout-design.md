---
id: "DESIGN-034VMQIe8i9hOn2Tb317Ue"
type: "design"
status: "approved"
title: "ALIB-02 residual closeout — 証拠照合と gate 分類の設計"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-02-residual-closeout.md"
  changes:
    added:
      - type: "file"
        path: "docs/designs/alib-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/specs/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/plans/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-residual-closeout.md"
      - type: "file"
        path: "evidence/issue-86-v5-migration/closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/plans/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/test-specs/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-02-residual-closeout.md"
  implemented-by: []
  depends-on: []
  blocks:
    - "docs/plans/alib-02-residual-closeout.md"
    - "docs/tasks/alib-02-residual-closeout.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/alib-02-streaming-verified-import-design.md"
    - "docs/designs/alib-02-v5-snapshot-compat-design.md"
    - "docs/designs/issue-86-verified-import-wiring-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/issue-86-verified-import-wiring/verification-ledger.md"
  defers: []
  deferred-by: []
---

# ALIB-02 residual closeout — 証拠照合と gate 分類の設計

## Context

ALIB-02 のプロダクション実装は 6 本の PR（#220 library slice・
#236 ALIB-07 fault matrix・#253 v5 digest/migration・#256 ALIB-07
import cancel/crash・#326 DoD(2) consumer 配線・#328 DoD(1) 実 v5
fixture）で main に統合済み。
残る作業は「Issue の受入条件/DoD 行 ↔ 実在 fixture/テスト/lane
結果」の照合、canonical lane の再実測、外部 gate の NotRun 分類、
doc status の整合という**簿記のみ**である。

照合の入力は 3 系統:

1. Issue #86 本文 — 受入条件 5 行・残実装 DoD 4 行・検証方法。
2. doc chain — spec/design/plan/task/test-spec/IR
   （`alib-02-streaming-verified-import`・`alib-02-v5-snapshot-compat`・
   `issue-86-verified-import-wiring`・`alib-02-residual-closeout`）。
3. 実測面 — `crates/*/tests/` の実在テスト、`tests/fixtures/` の
   実 v5 wire bytes、`evidence/` の ledger/README、canonical mise lane。

## Scope

### In Scope

- DoD/受入条件行ごとの証拠写像表（spec の Acceptance Criteria 節と
  IR の対応表）。
- canonical lane 7 本の実走と Pass/Fail 記録。
- doc status/relations/checkbox の整合（編集許可範囲内）。
- 外部 gate の NotRun 分類と所有 Issue の記名。

### Out of Scope

- プロダクションコード・依存定義・workflow の変更。
- 外部 gate の実施または Pass 昇格。
- Issue close・PR merge・index README の手動更新。

## Components and Boundaries

- **照合器（本設計の中核）**: Issue 行 → fixture/テスト/lane の
  対応表。判定語彙は `Pass`（実測）/`NotRun`（lane あるが未実測）/
  `Unsupported`（host に能力なし）/`Fail`（実測失敗）の 4 値で、
  `evidence/issue-86-verified-import-wiring/verification-ledger.md`
  の既存語彙を踏襲する。
- **canonical lane 実行者**: `mise run fmt-check|check|lint|test|
  build|test-rt|test-msrv` を worktree で逐次実行し、exit code を
  記録する。docs-only 変更のため cargo 系 lane は doc 差分に
  影響されないが、「実際に走らせた」ことを証拠とするため省略しない。
- **doc 整合器**: 既存 doc の status/relations/checkbox を実態に
  合わせる。編集境界は worker spec の占有リストに限定し、境界外の
  drift（例: `issue-86-verified-import-wiring` spec の status
  `approved`、`alib-02-streaming-verified-import` spec の未チェック
  acceptance 行）は変更せず IR/PR 本文に残存簿記として記録する。
- **証拠固定者**: `evidence/issue-86-v5-migration/
  closeout-verification.md` に lane 結果・再現手順・照合表の要約を
  追記し、IR に恒久記録を置く。

## Data and Control Flow

1. Issue #86 本文の受入条件/DoD 行を抽出。
2. 各行について `rg`/`grep` で実在テスト名・fixture・evidence を
   照合（存在しない証拠を推測で補わない）。
3. canonical lane を実走し結果を採取。
4. spec Acceptance Criteria・plan Task Breakdown・task Done When・
   IR Validation に同一の判定を転記。
5. 外部 gate 行は `NotRun` + 所有 Issue（synvelis#91 実 OS/FS、
   #97 統合試験、#114 実 GPUI/GUI、#331 real provider/remote LFS、
   ruviform#294 cross-consumer/AudioLens）を記名。
6. doc_status lint/audit と audit_impl_record を対象ファイルで実走し、
   新規 finding が無いことを確認（repo-wide baseline の legacy
   invalid-id 系 finding は既知・migration 対象として区別）。

## Non-Functional Constraints

- **誠実性不変条件**: NotRun/Unsupported/Mock を Pass と書かない。
  headless fixture の PASS は headless 範囲の証拠に限定する。
- **占有境界**: worker spec の編集許可リスト外は一切変更しない。
  必要な変更が境界外に及ぶ場合は Coordinator へ question する。
- **再現性**: lane 結果には実行環境（OS・toolchain・対象 commit）
  と再現コマンドを添える。
- **index 非編集**: `docs/*/README.md` は merge 時に Coordinator が
  再生成するため、本 slice では触らない（index-missing-entry は
  既知の一時状態として扱う）。

## Risks and Trade-offs

- **照合の誤判定**: テスト名の列挙だけでは受入条件の意味的充足を
  誤る可能度がある → 各行に fixture/テスト名と「何を証明するか」を
  併記し、外部 gate 由来の部分は行内で明示分離する。
- **lane の環境依存 flake**: `mise run test` で過去に webdav flake・
  LNK1102 の観測例あり → Fail 時は単独再実行で切り分け、最終結果を
  正直に記録する。
- **doc 間の status drift 再発**: 今回の整合対象外ファイル
  （wiring spec 等）に残る drift は本 PR では直せない → IR の
  Follow-ups と PR 本文に残存簿記として明記し、Coordinator の
  merge 時判断に委ねる。
- **過剰な doc 量**: 簿記タスクに spec/design/plan/task/IR の全
  chain を起こすのは重い → ただし DoD ledger の canonical な置き場と
  doc-driven-dev-graph の traceability（plan の verified-by→
  test-spec 要件等）を満たすため、各 doc は最小構成に留める。

## Deferred Design Concerns

- 実 OS/FS・実ネットワーク・実 GUI・AudioLens・cross-consumer の
  gate 実施 | 本環境（Windows headless）では原理的に不可 |
  synvelis#91・#97・#114・#331・ruviform#294 | mock PASS の
  実機昇格。
- Issue #86 の close 判定 | 外部 gate が残存 | gate 所有 Issue の
  完了後 | 未証明行を閉じたままにする誤判定。
- 編集境界外 doc の status 整合 | 占有ルールで変更不可 | Coordinator
  の merge 時または後続 bookkeeping PR | spec `approved` のまま残る
  drift。

## Verification Notes

- [x] canonical lane 7 本を `mise run` で実走し exit code を記録
  （結果は `docs/impl/ir/alib-02-residual-closeout.md` Validation
  と `evidence/issue-86-v5-migration/closeout-verification.md`）。
- [x] doc_status lint/audit・audit_impl_record を変更対象で実走し、
  新規 finding なしを確認。
- [x] spec の Acceptance Criteria 各行が実在テスト/fixture/lane に
  対応することを `rg` で照合済み。
