---
id: "TSPEC-034XNqMsVEAK46YlF4nP3X"
type: "test-spec"
status: "approved"
title: "RCP-07 cross-gate — case ledger・headless fixture tests"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/296"
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
    - "evidence/sv296/acceptance-ledger.md"
    - "docs/impl/ir/rcp-07-cross-gate.md"
    - "docs/test-specs/rcp-01-render-recipe.md"
    - "docs/test-specs/rcp-02-artifact-manifest.md"
    - "docs/test-specs/rcp-03-render-policy.md"
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
    - "docs/test-specs/rcp-06-render-gui.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCP-07 cross-gate — case ledger・headless fixture tests

## Purpose

synvelis#296（RCP-GATE-01..12 cross-gate tracker）の fixture/ledger
スライスが保証すべき振る舞いを固定する。merge 済み RCP-01..05
基盤をまたぐ接合部（recipe→cache identity・closure→invalidation・
verified→fault/GC・policy→job・branch→pin・store→resolver）を
`crates/synvelis-render/tests/rcp07_gate.rs` の headless fixture
で検証し、`evidence/sv296/` の ledger が全 case-ID の
runnability（Runnable headless now / Blocked / NotRun /
Deferred / Unsupported）を正直に記録することを保証する。
headless/Fake の Pass を実 Plugin・実 OS・実 GUI・RT・
AudioLens Consumer の Pass に昇格しないこと、および実測のない
Pass を記録しないことが本 spec の中核不変条件である。

## Feature

- `RCP-GATE-01..12` の case inventory: 各 case の oracle 種別
  （独立 oracle / assessment / 外部 oracle 必須）と runnability
  分類を `evidence/sv296/acceptance-ledger.md` が保持する。
- `crates/synvelis-render/tests/rcp07_gate.rs` が merge 済み
  RCP-01（canonical recipe/closure/invalidation/branch pin）・
  RCP-02（verified publish/CAS/GC/fault）・RCP-03
  （policy trigger/job orchestration）・RCP-04 plumbing
  （`synvelis-player::rcp` resolver）をまたぐ 7 つの cross-gate
  fixture を提供する。
- `evidence/sv296/verification.md` が実行コマンド・OS・
  toolchain・SHA・結果を記録し、Case→Fixture→Expected/Actual→
  Receipt→Issue リンクの追跡可能性（RCP-GATE-12）を満たす。

## Rules

- **RCP-GATE-XG-01（recipe→cache identity）**: 同一 canonical
  recipe key は同一 artifact index 経路を指し、recipe record の
  verify/evaluate は独立再解決に対してのみ Match を返す。
  hash 衝突注入・別 key・未解決 lookup は Match/EligibleExact
  を返さない（false-valid = 0）。
- **RCP-GATE-XG-02（closure→invalidation）**: 依存 closure 内の
  変更のみが dirty interval/full invalidation を生じ、closure 外
  object・既知 interval は過小近似せず、interval 不明・temporal
  変更は fail-closed で full invalidation になる
  （under-approximation = 0）。
- **RCP-GATE-XG-03（verified→fault/GC）**: verified publish 後の
  publish-path fault は旧 Verified artifact を乱さず、GC は
  Active reference（LiveBranch 等）を保持し、release→grace→
  collect で manifest/blob/index が一貫して消え、quarantine・
  収集後 lookup は Exact-eligible を返さない。artifact GC は
  project raw source に触れない。
- **RCP-GATE-XG-04（policy→job）**: OnDemand proposal は authorizer
  の明示承認なしに Denied で終わり job/executor/artifact を生ま
  ない。重複 manual event は coalesce し、queued cancel は
  artifact を publish しない（無許可副作用 = 0）。
- **RCP-GATE-XG-05（branch→pin）**: branch 内 acoustic edit は
  recipe key を動かし cache を分離し、adopt は単一 forward CAS で
  live content を candidate acoustics に一致させ（独立 oracle
  との key 一致で検証）、`RecipeBranchPin` は hashed identity
  として live/branch key を分離し、branch 参照 artifact は adopt
  後も verified/reachable を保つ。
- **RCP-GATE-XG-06（store→resolver）**: `lookup` が返す
  `CandidateReport.eligibility` をそのまま `RenderCandidate` に
  載せたとき、Missing Plugin + EligibleExact artifact は Exact
  selection + 名前付き substitution を生じ、store 側 integrity
  fault（quarantine）後は substitution なし・non-Exact verdict に
  fail-closed する。
- **RCP-GATE-XG-07（job key 分離）**: 異なる project 内容は異なる
  job key・recipe key・artifact を生み、旧 recipe key の verified
  artifact は新 recipe の render 後も自身の key で Exact-eligible
  を保つ（stale の誤 activation = 0、headless contract level）。
- **RCP-GATE-XG-08（ledger 正直性）**: ledger の status 語彙は
  Pass/Fail/NotRun/Unsupported/Deferred/Blocked を区別し、
  実測（コマンド・SHA・toolchain・OS）のない Pass を記録しない。
  headless Pass を実機/consumer Pass に換算しない。
- **RCP-GATE-XG-09（公開 API のみ）**: fixture は `src/` 内部・
  private helper を参照せず、既存 rcp01..06 fixture の helper を
  再実装で共有しない（contract の独立性）。test fn は
  `rcp07_*` prefix で `RCP-GATE-NN` を doc comment/名に含む。

## Examples

- GATE-01: `rcp07_gate01_recipe_key_is_cache_identity_and_false_
  valid_zero` — 同一 project の再解決で key 一致、recipe_record
  round-trip Match、tempo 変更で Stale、canonical bytes 改竄で
  Unknown、空 store で Missing。
- GATE-02: `rcp07_gate02_closure_invalidation_is_sound_and_
  complete` — closure 内 clip 変更で dirty interval、foreign
  object で空 plan、interval None/temporal=true で full。
- GATE-03: `rcp07_gate03_verified_artifact_survives_faults_and_
  gc_roots` — publish→EligibleExact、CommitIndex fault で旧
  artifact 無傷、LiveBranch ref で sweep keep、release→collect、
  raw source digest 不変、quarantine/収集後 non-Exact。
- GATE-04: `rcp07_gate04_ondemand_denied_manual_idempotent_
  cancel_clean` — OnDemand Denied で job/executor 0、manual
  重複 coalesce、queued cancel で state=Cancelled・artifacts
  空・render 呼出 1 回のみ。
- GATE-05: `rcp07_gate05_branch_edit_moves_key_adopt_pins_live`
  — AudioClip scope fork→mute edit で key 分離、candidate key
  配下の verified artifact、adopt=単一 forward CAS、独立 muted
  live project との key 一致、LiveBranch artifact が adopt 後も
  reachable。
- GATE-06/07 headless: `rcp07_gate06_store_eligibility_drives_
  resolver_selection`（lookup verdict→RenderCandidate→Exact
  substitution、quarantine→non-Exact）と
  `rcp07_gate07_job_keys_separate_stale_and_current_recipes`
  （R1/R2 別 job key・別 artifact・R1 key で引き続き
  EligibleExact）。

## Guarantees

- `mise run test synvelis-render` で `rcp07_gate` 7 case が
  green であること（実測は `evidence/sv296/verification.md`）。
- 全 fixture が公開 API のみを使い、src 内部参照がないこと。
- ledger が `RCP-GATE-01..12` 全件の status・所有 gate・
  fixture 対応を保持し、実機/consumer lane を NotRun/Deferred/
  Blocked/Unsupported として区別すること。

## Non-goals

- 実 Plugin ABI（VST3/CLAP/AU）・実 OS・実 GPUI/Window・
  実 signal/PCM clock・RT no-alloc/lock/IO/PDC 実測・
  AudioLens Consumer 受入 — 全て外部 gate（`evidence/sv296/
  acceptance-ledger.md` の NotRun/Deferred 表）。
- `synvelis-app` 側 presentation-layer cross-gate fixture:
  `synvelis-app` は `synvelis-render` 型を名指せない境界制約が
  あり、resolver→projection の両端は RCP-04（`rcp_04.rs`）と
  RCP-06（`rcp06_render.rs`/`rcp06_render_ui.rs`）が既に pin
  済み。store→resolver 接合部は依存方向上 `synvelis-render`
  test に置くのが正しく、本 slice では app test を追加しない
  （判断は PR 本文に記録）。
- RCP-05（#294）未整備部（late attach タイミング契約・old
  commit pin の完全 fixture 語彙）— merge 済み plumbing で
  pin 可能な範囲のみ本 fixture が被覆し、残りは #294 完了待ち
  として ledger に記録。
- 実 PCM bit-exact/readback 比較・実 signal clock — headless
  では manifest/digest 検証レベルまで（外部 gate）。

## Risk

- fixture が既存 contract を超えて推測で case を「Pass」に
  しないことが最大のリスク。ledger の判定規則（実測なし
  Pass=0・headless≠実機）と fixture の独立 oracle（同一
  project の独立再構成・lookup verdict の直接転記）で抑止する。
- `synvelis-app` test を置かない判断は coverage gap になり
  得るが、app の依存境界（render 型を名指せない）と既存
  RCP-04/RCP-06 fixture の被覆を根拠に proportionate と判断。
  将来 resolver→ViewSpec の直接接合を pin する必要が出た
  場合は別 slice で app/workspace test を追加する。
