---
id: "SPEC-559AdoptUnobservableRemoval02"
type: "spec"
status: "approved"
title: "Issue #559 spec — stored candidate payload の観測不能 entity 削除 adopt fail-closed"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/559"
    - "docs/discovery/issue-559-adopt-unobservable-removal.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/discovery/issue-559-adopt-unobservable-removal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-559-adopt-unobservable-removal.md"
    - "docs/test-specs/issue-559-adopt-unobservable-removal.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
---

# Issue #559 spec — stored candidate payload の観測不能 entity 削除 adopt fail-closed

## 目的

`synvelis#553` 以前に永続化された stored candidate
payload（send-owned `ModulationSource`/
`ModulationAssignment` の削除を scope diff に含む）を
`synvelis.branch.adopt` した際、観測不能 entity の削除が
fail-closed 検査なしに live へ物質化する残存経路を塞ぐ。
adopt の materialization 境界で、gateway の観測/effect
モデルが認可できない `Removed` write を typed
rejection にする。

## 要件

### R1 — 再現記録（AC1・済）

pre-553 相当の stored candidate payload に対する adopt が
現行コードで物質化することを headless で実測し、
`evidence/sv559/reproduction.md` に記録する。

### R2 — adopt 時 fail-closed（AC2）

`plan_adopt_writes` が計画する `Removed` write のうち、
gateway の resource/effect 観測モデル（`ResourceKind` /
`snapshot::ENUMERATED_KINDS` が名付け得る kind）の外に
ある entity 削除を typed `VariantError` で拒絶する。

- 判定軸は domain（synvelis-project）側に閉じた
  `ObjectKind` 分類とし、gateway 概念を domain に
  持ち込まない。
- 対象 kind は `ModulationSource` と
  `ModulationAssignment` — `op_facts` `RemoveSend` arm
  （synvelis#553）が既に "unverifiable effects" として
  扱う集合と一致させる。
- `RoutingConnection` は対象外 — send teardown の専用
  edge であり、synvelis#550/#557 で受理済みの SEC-14
  境界（観測可能 owner 削除と不可分の internal
  structure）。これを reject すると全 send.remove
  candidate が adopt 不能になり既存契約を破壊する。
- 新 typed variant は `Unsupported` と区別する —
  removal 自体は `removal_supported_kind` で support
  済みだが、その削除効果を検証できない、という別の
  判定。

### R3 — pin test（AC3）

- domain: 観測不能 kind の `Removed` write を含む
  candidate adopt が typed rejection になり、live
  project・variant set 状態が不変であることを pin
  する。
- gateway: wire 到達可能な同型 scenario（candidate が
  観測不能 entity 削除を含む stored payload）の adopt
  が `variant-unverifiable-removal` 系の安定 reason で
  `PreparationRejected` になることを pin する。

### R4 — 非回帰（AC4）

観測可能 kind の send teardown adopt
（SendConfig/RoutingConnection/ParameterTarget/
AutomationLane の `Removed` write を含む candidate）
が従来どおり物質化すること、および synvelis#550/#553/
#557 の既存 pin が全て Pass すること。

## 受入条件（Issue 転記）

- [x] pre-553 相当の stored candidate payload
  （send-owned modulation entity 削除を含む）に対する
  adopt の挙動を headless で再現・記録する。
- [x] 「修正方針の候補」から境界を決め、選択した方針で
  fail-closed（または明示的な受理判定）を実装する。
- [ ] fail-closed の場合、観測不能 kind の `Removed`
  write を含む candidate adopt が typed rejection に
  なり、live が不変であることを test で pin する。
- [ ] `mise run check/lint/test synvelis-project`・
  `mise run check/lint/test synvelis-gateway` が
  exit 0。

## 非目標

- `op_facts`/`declared_facts` の coverage 拡張
  （edit/live-apply 経路の unverifiable cascade —
  discovery §4 で記録した `SetSendGainTarget`/
  `RemoveTrack` 経路は `synvelis#561` 領域）。
- candidate payload の schema version/provenance 世代
  管理。
- gateway 側での stored payload scope diff 再検証。
- `RoutingConnection`/`SemanticPort` 削除の reject。
- `removal_supported_kind`・`check_strip_fields_
  scope_removals` の変更。
- `VariantIntent::Adopt` の `declared_facts` 伝播修正
  （synvelis#557 で gap 無しを実証済み）。
- 実 PCM/実 plugin/実 GUI lane。

## 検証方法

Issue 指定どおり:

- `mise run check synvelis-project` /
  `mise run lint synvelis-project` /
  `mise run test synvelis-project`
- `mise run check synvelis-gateway` /
  `mise run lint synvelis-gateway` /
  `mise run test synvelis-gateway`
- `mise run fmt-check` / `mise run check-headless-deps` /
  `mise exec -- prek run --all-files`

headless 実測のみ。実機/RT/GUI lane は NotRun。

## 関連リンク

- `synvelis#559`（本 Issue）
- `docs/discovery/issue-559-adopt-unobservable-removal.md`
- `docs/designs/issue-559-adopt-unobservable-removal.md`
- `docs/test-specs/issue-559-adopt-unobservable-removal.md`
- `docs/test-specs/cb-06-track-bus-adapter.md`
- `synvelis#557` / `synvelis#553` / `synvelis#550` /
  `synvelis#561` / `synvelis#283`
- `docs/specs/issue-557-adopt-facts.md`
- `POLICIES.md`
- `AGENTS.md`
