---
id: "TSPEC-034X9G0UGtaLhkU5NlUIjh"
type: "test-spec"
status: "approved"
title: "VCS Integration 統合契約の保証事項 (synvelis#7)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/vcs-04-native-mount.md"
    - "docs/test-specs/vcs-05-apply-restore-fixtures.md"
    - "docs/specs/vcs-06-fixture-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-integration.md"
    - "docs/designs/vcs-integration-design.md"
  derived-by: []
  verifies:
    - "docs/specs/vcs-integration.md"
    - "docs/designs/vcs-integration-design.md"
  verified-by: []
  references:
    - "evidence/sv7/vcs-integration-case-ledger.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
  defers: []
  deferred-by: []
---

# VCS Integration 統合契約の保証事項 (synvelis#7)

## Purpose

Issue #7 の統合契約が「文書として正しく、既存 slice 実装と矛盾せず、
受入 Case が誠実に被覆されている」ことを保証する test-spec。
本スライスは docs-only であり、ここの Rules/Examples は文書監査
（front matter 整合・Case ID 被覆・verdict 誠実性・既存 slice との
矛盾不在）に対する保証を記述する。実装挙動の保証は各 slice の
test-spec（`vcs-04-*`、`vcs-05-*`、`vcs-06-*` 等）が担う。
統合契約の意味が変わり Case 被覆の再定義が必要になった時点で
本書は改訂または supersede される。

## Feature

VCS Integration 統合契約文書チェーンと受入 Case 被覆台帳の正しさ。

## Rules

- **Rule**: 7 文書（spec/design/plan/task/test-spec/impl-ir/impl-exp）
  は front matter の `id`/`type`/`status`/`relations` が相互に解決
  可能であり、spec → design → plan → task → test-spec → impl の
  依存方向が一方向である。
- **Rule**: `evidence/sv7/vcs-integration-case-ledger.md` は基底 Case ID
  （VCS-SNAP-01〜06、VCS-DIFF-01〜08、VCS-GIT-01〜06、VCS-UI-01〜05、
  VCS-APPLY-01〜06、VCS-RESTORE-01〜06、VCS-COMPAT-01〜04）を
  漏れなく列挙し、ledger 上の拡張行（VCS-DIFF-09/10、
  VCS-APPLY-07/08、VCS-RESTORE-07/08/09、VCS-GIT-REAL-*）を
  拡張として区別して記録する。
- **Rule**: 各 Case 行は owner（責務 Issue）・依存・fixture/test
  または実装リンク・execution mode・verification layer・
  requirement state（Required/Deferred/NotApplicable）・
  result state（Pass/Pass*/Fail/NotRun/Unsupported/Blocked）・
  evidence path を持つ。
- **Rule**: verdict は `evidence/reports/vcs-06-fixture-matrix-ledger.md`
  の実測値を転記し、headless/fixture/CI の Pass を実 GUI・実 OS・
  実 provider・plugin host・RT・consumer の Pass に換算しない。
  未実行は NotRun、真に不可能な能力は Unsupported、未実装は
  Blocked/NotRun と理由付きで記録する。
- **Rule**: design doc の slice 責務対応表は既存 vcs-01..06 spec の
  正本関係と矛盾しない（各 slice が所有する責務を本書が再定義・
  二重実装しない）。
- **Rule**: spec の 14 受入条件それぞれに、対応する Case ID 群・
  証拠・または設計記録（8 view 採否・Creative Branch 分離等）への
  対応が文書上で追跡可能である。

## Examples

- **Example**: VCS 非接続環境で project new/open/save・Live vs
  Saved diff が動作し `syn.vcs.*` が `NoBackend` を返す —
  `vcs_backend_wiring` fixture で実測済み（ledger の VCS-UI/GIT
  系行に反映）。本 test-spec はこの契約が spec/design/ledger の
  3 文書で一貫して記述されていることを保証する。
- **Example**: merge 親を持つ commit の history が全 parent を保持し
  A/B pin で任意 2 点比較できる — VCS-03 provider 契約
  （`RevisionRelations.parents`）と VCS-04 `chosen_parent`
  metadata の組合せ。本 test-spec は ledger の対応行がこの 2 つの
  正本を参照していることを保証する。
- **Example**: Asset 欠落 snapshot 同士の比較が `Unknown(
  MissingAsset)` を返し Equal にならない — VCS-02 tri-state
  契約。本 test-spec は ledger の VCS-DIFF Unknown 系行が
  Pass/Pass* で記録され「変更なし」誤表示がないことを保証する。
- **Example**: mockup HTML 未収録のため 8 view の採否が Issue 本文
  + view mode 決定（SideBySide/ChangeFocus 採用、Overlay/Evolution
  Map 延期、Plugin State/Audio は条件付き・延期）を根拠に記録され、
  pixel レビュー未実施が Deferred として明示される。

## Guarantees

- 正しい文書チェーンは、spec の全受入条件について「どの Case ID・
  どの fixture/証拠・どの検証層で判定するか」が一意に追跡できる。
- 正しい ledger は、実装済みで未検証の領域（実 GUI・実 provider・
  RT・consumer）を Pass と誤記しない。
- 正しい design は、VCS slice 間の責務境界を変えずに Issue #7 の
  統合不変条件を満たす対応を提供する。
- 本契約が維持される限り、将来の slice 追加・Case 追加は ledger
  への行追加で吸収でき、spec/design の再定義を要しない。

## Non-goals

- 実装コードの正しさ（`crates/` の test fixture が担保 — 本書は
  docs のみ）。
- 実 GUI/OS/provider/RT/consumer の受入判定（#114・#97 REAL lanes）。
- VCS Writer・3-way merge・Creative Branch の内部設計検証。
- mockup HTML の pixel/実機レビュー（本体未収録・別作業）。

## Risk

- 台帳の verdict を鵜呑みにすると、未検証領域を「完了」と誤認して
  #97 の統合受入が形骸化する。
- Case ID の対応漏れがあると、Issue #7 の受入条件の一部が永遠に
  未検証のまま残る。
- slice spec との矛盾を見逃すと、実装 Owner が親契約と異なる前提で
  作業を進める。
