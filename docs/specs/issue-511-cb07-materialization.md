---
id: "SPEC-sv511bC3dE5fG7hI9jK1lM3n"
type: "spec"
status: "approved"
title: "Issue #511 spec — CB-07 materialization 残件の受入仕様"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "docs/discovery/issue-511-cb07-materialization.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/adr/issue-511-cb07-materialization.md"
    - "docs/designs/issue-511-cb07-materialization.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-511-cb07-materialization.md"
  derived-by:
    - "docs/adr/issue-511-cb07-materialization.md"
    - "docs/designs/issue-511-cb07-materialization.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-511-cb07-materialization.md"
  references:
    - "evidence/sv511/verification.md"
  defers: []
  deferred-by: []
---

# Issue #511 spec — CB-07 materialization 残件

## 目的

CB-07 Arrangement Adapter（synvelis#284）の IR Follow-ups で残った
3 残件を engine 側で fail-closed に解消する。silent な部分 commit・
旧 grid transplant・所有権の推定を構造的に排除する。

## 受入条件

- AC-1: 削除を含む Arrangement candidate が `plan_adopt_writes`/`apply`
  の `Removed` write で宣言範囲のみ 1 CAS/Undo として adopt され、
  live/branch/revision の部分 commit がない。
- AC-2: scope-removal を含む candidate の render 解決が
  materialize されるか、policy 上明示的に拒否する型付き契約が
  確立される（半 materialize が構造的に発生しない）。
- AC-3: variant set 存在時の `SetProjectPpqn` が typed 拒否または
  candidate payload 再射影のいずれかで旧 grid transplant を防ぎ、
  回帰テストで固定される。
- AC-4: absolute anchor/extent clip の tempo-map 変換 policy が
  確立され、musical scope 所有拡張または `AbsoluteAnchor` 継続の
  判断が docs/テストに記録される。
- AC-5: `mise run test synvelis-project` が pass し、cb01〜cb11 系
  既存 suite に回帰がない。

## 非目標

- 実 OS/実 Plugin/実 Render pipeline・実 GUI timeline の受入
  （synvelis#286/#287/#288/#114 系の責務 — `evidence/sv511` に
  NotRun 記録）。
- CB-ARR-04 の Automation/Tail/Loop/Tempo 完全な影響域伝播、
  CB-ARR-05 の `Unaligned`/`Unknown` diff verdict（#285/#288）。
- 3-way Semantic Merge・自動 Blend。
- VCS `apply_selected` 共有 surface（`change_supported`）の再設計
  — sv#513 の契約を維持し adopt 経路限定で実装する。
- candidate payload の PPQN 再射影（本 slice は typed 拒否を選択）。
- tempo-map 変換による absolute anchor の musical scope 所有拡張
  （本 slice は `AbsoluteAnchor` 継続を選択）。

## 不変条件

- scope-removal 拒否は CB-01〜CB-11 全 domain adapter 共通の
  slice 横断不変条件 — Clip/Note 以外の kind、および他 adapter の
  既定 fail-closed 契約は変更しない。
- capability 報告 = 強制の一致を維持する。
- 1 controller CAS = 1 undo step。live/branch/revision の部分
  commit を許さない。
- fail-closed pin の無断削除・テスト無効化をしない — 新契約への
  flip は assert の書き換えとして行う。
