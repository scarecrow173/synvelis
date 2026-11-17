---
id: "DESIGN-sv511cD4eF6gH8iJ0kL1mN3oP"
type: "design"
status: "approved"
title: "Issue #511 設計 — CB-07 materialization 残件の方式決定"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "docs/specs/issue-511-cb07-materialization.md"
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
    - "docs/designs/variant-sets-vcs-restore.md"
    - "docs/impl/ir/cb-07-arrangement-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-511-cb07-materialization.md"
  derived-by:
    - "docs/plans/issue-511-cb07-materialization.md"
    - "docs/adr/issue-511-cb07-materialization.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-511-cb07-materialization.md"
  references:
    - "docs/impl/ir/cb-07-arrangement-adapter.md"
  defers: []
  deferred-by: []
---

# Issue #511 設計 — CB-07 materialization 残件

## 残件1: Removed write materialization（adopt + render）

### 判定構造

`plan_adopt_writes`（`variant/engine.rs`）の removal 検出を
「一律 Unsupported」から 3 分岐へ変更する:

- `origin ∩ live_scope − cand_scope` の entity が candidate に
  **実在** → scope から外れただけ（move-out）→
  `VariantError::Unsupported`（adapter の edit-time 判定と同じ
  verdict、defense in depth）。
- entity が candidate に**不在** → 真の削除 → kind が
  `entity::removal_supported_kind`（現行 `Clip | Note`）なら
  `ChangeKind::Removed` write（payload = `Value::Null` — after
  entity は存在しない）を計画。他 kind → `Unsupported`。

### write 経路

- `apply/entity.rs` `write_entity` 冒頭で `Removed` を
  `remove_entity` へ分岐。`remove_entity` は `ProjectOp::RemoveClip`/
  `RemoveNote` と同一の参照整合を mirror する:
  - Clip: `project.clips` から除去 + 全 lane の `clip_ids` から
    除去 + `note.clip == clip` の notes を cascade 除去
    （clips-only mask でも note は親を失えない — 参照整合であり
    scope 拡大ではない）。
  - Note: `project.notes` から除去（Clip cascade が先に消した
    場合に備え冪等）。
- `removal_supported_kind` は `Clip | Note` のみ。control-closure
  kind（ParameterTarget 等）は clip 削除が `validate_references`
  で既に candidate 段階から拒否されるため到達不能 — 将来 kind を
  増やす場合は cascade 設計と共に再検討する fail-closed 述語。
- VCS `apply_selected`（`plan.rs` `change_supported`）は全 kind
  の `Removed` を従来どおり `Unsupported` で拒否 — 共有 surface
  は変更しない。`preflight_*`/`introduced_digests` は `Removed`
  write を自然にスキップする（Added 限定・kind 限定のため
  `Null` payload でも安全）— plan.rs 無変更で整合。

### render 解決

`render_input.rs` `resolve_render_project` は `plan_adopt_writes`
+ `entity::write_all` をそのまま再利用するため、同一 plan で
自動追従し materialize される。`composed.validate_references()`
が最終 oracle — cascade 漏れがあれば typed error で失敗し、
半 materialize された render は構造的に生成されない。

### atomicity

`VariantOp::Adopt::apply` → `entity::write_all` は 1
`ProjectController::apply`（1 CAS = 1 undo step）内で完結。
plan/preflight の全失敗は CAS 前に typed reject — 部分 commit
なし。`AdoptReceipt.objects` は `Removed` write を含む writes
総数をそのまま報告する。

## 残件2: SetProjectPpqn × candidate payload — typed 拒否

### 判定

`ppqn.rs` `reproject_ppqn` の先頭（identity no-op 判定の直後）で、
いずれかの variant set の branch が `BranchContentRef::Captured`
を保持する場合 `Error::Validation` で拒否する。

- `validate`（ops.rs dry-run）・`apply`・`preview_ppqn` の 3 経路が
  同一関数を通るため判定は一箇所 — authoritative boundary。
- `Captured` は編集中 candidate と adopt 時に凍結された旧 adopted
  branch の双方を含む。frozen payload も再 adopt 可能で同じ
  stale-grid リスクを持つため、区別せず全 `Captured` を対象とする
  保守的契約。
- payload 再射影（decode→変換→再 encode→新 digest→`Captured`
  ref/scope_digest 更新）は store/engine の大改修であり、
  frozen 履歴も対象になるため本 slice の範囲外 — ADR に記録。

### 帰結（正直な記録）

adopt 履歴のある variant set は frozen `Captured` を保持し続ける
ため、実質 PPQN 変更は全 branch が `Live` の set に限られる。
これは旧 grid transplant を構造的に防ぐ意図的な制約であり、
variant set 解消（discard/将来の set 撤去）後にのみ PPQN を
変更できる。`preview_ppqn` は `applies: false` + rejection を
正直に報告する。

## 残件3: absolute anchor tempo-map policy — `AbsoluteAnchor` 継続

### 判定

musical scope 所有拡張は行わず、`absolute_anchor:
Unsupported`・`ArrangementBoundaryReason::AbsoluteAnchor` の
fail-closed 契約を維持する。

- absolute anchor/extent と musical range の相互変換は tempo map
  の versioned 解釈を要し、clip extent の musical 換算は tempo
  曲線全域に依存する。partial overlap の帰属・tempo 変化を跨ぐ
  clip の policy が未確立のまま所有に含めると、fingerprint/
  conflict の正本性を損なう推定になる（E3 の却下理由を踏襲）。
- `scope.rs`/`arrangement.rs` は無変更 — 既存 pin
  （cb_arr_04 boundary inventory・cb_arr_05 render pin identity）
  が契約を固定し続ける。

## 影響面

- `compare.rs`（sv#285）: removal candidate の render 解決が
  成功するようになるため compare は削除後の composed view を
  fingerprint する — 従来は `RenderInputError` で失敗していた
  経路が正常系になるだけで verdict 規則の変更はない。
- 他 domain adapter: `removal_supported_kind` が `Clip | Note`
  のみのため、comp/midi/routing 等の scope-owned kind の削除は
  従来どおり `Unsupported`。edit-time の各 adapter 契約は不変。
- `evidence/sv284/acceptance-ledger.md` CB-ARR-06 行の
  「adopt/render は Unsupported で拒否」は本 slice で契約が
  変わる — ledger は sv284 時点の歴史記録として据え置き、
  新契約は `evidence/sv511/` に記録する。
