---
id: "TSPEC-034XNnfbnwSCTXarhURIQx"
type: "test-spec"
status: "draft"
title: "CB-11 Creative Branch 7領域 cross-gate — headless coverage inventory・evidence ledger・runnable subset"
created: "2026-11-15"
updated: "2026-11-17"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/288"
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
    - "evidence/sv288/coverage.md"
    - "evidence/sv288/external-gates.md"
    - "evidence/sv288/verification.md"
    - "docs/impl/ir/cb-11-cross-gate.md"
    - "docs/impl/exp/cb-11-cross-gate.md"
    - "crates/synvelis-project/tests/cb11_cross_gate.rs"
    - "crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs"
    - "docs/test-specs/cb-01-variant-set.md"
    - "docs/test-specs/cb-02-audio-clip-adapter.md"
    - "docs/test-specs/cb-03-midi-clip-adapter.md"
    - "docs/test-specs/cb-04-instrument-adapter.md"
    - "docs/test-specs/cb-05-fx-range-adapter.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/test-specs/vcs-02-semantic-diff-fixtures.md"
    - "docs/test-specs/sf06-cross-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CB-11 Creative Branch 7領域 cross-gate — headless coverage inventory・evidence ledger・runnable subset

## Purpose

Issue #288（CB-11 長期 tracker）の case-ID 群
（`CB-E2E-01`〜`CB-E2E-09` + `CB-X-RCP-01`）を、merge 済みの
CB-01..07 domain adapter・CB-08 orchestrated compare・
CB-10 gateway variant actions・VCS-02 semantic diff・
RCP branch render-pin 解決・RCP-07 case ledger の上で
headless に
棚卸しし、被覆済み fixture・runnable subset・外部/未 merge
gate を正直に分類することを保証する。この spec は「どの
case が今 headless で証明でき、どれが前提 Issue・実機
gate 待ちか」の判定意図を固定する。headless Pass を
GUI-L2/EXT-L3・実 plugin・実信号・consumer 受入へ換算
しないことがこの spec の中核であり、全 case の最終受入は
tracker 後続 slice の責務 — 本 spec が退役できるのは
CB-01..10 全 integration と外部 gate 完結後のみ。

## Feature

Creative Branch 7 領域 × PCM/RT/Project cross-gate の
headless coverage inventory・evidence ledger・runnable subset

## Rules

- Rule: case-ID の正本語彙は Issue #288 の
  `CB-E2E-01`〜`CB-E2E-09`・`CB-X-RCP-01`。cb-01..06 既存
  fixture が被覆する部分は「既存 fixture 被覆」列で写像し、
  重複 fixture を新設しない。
- Rule: 各 case の oracle 種別（独立 oracle / assessment
  レベル / 外部 oracle 必須）と runnability
  （Runnable headless now / Blocked on unmerged Issue /
  NotRun external gate / Deferred）を分離して記録する。
  `POLICIES.md` の `Pass`/`Fail`/`NotRun`/`Unsupported` と
  `Required`/`Deferred`/`NotApplicable` の語彙を混ぜない。
- Rule: runnable subset は `synvelis_project` の公開 API のみを
  使い、`src/` 内部・private helper を参照しない
  （cb-01..06・sf06 先例の Rule）。production コード・既存
  test・canonical doc は変更しない。
- Rule: pinned invariant は (a) adopt = 1 CAS = 1 undo unit
  で undo が pre-adopt state へ完全復帰すること、(b) fork/
  edit/adopt/undo/save/reload を通して persisted raw
  source・asset・content digest が不変であること、(c)
  `AdoptReceipt` 系 provenance（`BranchContentRef` digest・
  `last_adopt` lineage）が candidate と verbatim で
  save/reload 後も一致すること。
- Rule: `resolve_render_project` の `BranchRenderPin` で
  scope-owned 編集は `scope_digest` のみを、shared-context
  編集は全 branch の `shared_context_digest` を動かすこと
  （CB-X-RCP-01 の headless 実行可能部・CB-E2E-04 の pin
  レベル）を pin する。recipe cache/invalidation 本体・
  RCP-GATE は #296/RCP-07 側の所有で再実装しない。
- Rule: stale reject（CB-E2E-05 の cross-gate 半分）は
  「ある domain の adopt 拒否が live・revision・他 domain
  candidate を汚染しない」ことを pin する。stale
  revision/ID collision/missing plugin/queue-full/recording-
  active の個別網羅は cb-01..06 既存 fixture の被覆を参照し、
  重複 pin しない。
- Rule: CB-E2E-03 の arrangement composite は CB-07 merge
  済み `ArrangementAdapter` 経由で pin し、2+ Track / 4+
  Clip の単一 composite adopt・1 undo/redo・scope 外不変を
  検証する（`cb07_arrangement.rs` 先例の fixture 慣習）。
- Rule: CB-E2E-04 の orchestration 軸は CB-08 merge 済み
  `ComparisonOrchestrator` の session/freshness/evidence
  契約を通し、stale session の evidence が current proof を
  装わないことを pin する（`cb08_compare_orchestrator.rs`
  先例）。実 RT publish/plugin の実挙動は gate のまま
  NotRun とする。
- Rule: CB-E2E-07 の AI/VCS OFF 半分は CB-10 merge 済み
  gateway 契約で pin する — agent principal の grantless
  op が prepare/apply 両段で fail-closed し、VCS/durable
  backend 非搭載の `VariantPorts::standard(None)` rig で
  variant lifecycle が独立動作すること。実 AI 統合・実
  VCS backend・consumer Gate A は外部 gate の責務。
- Rule: CB-X-RCP-01 の RCP-GATE 紐付けは merge 済み
  RCP-07 headless case ledger（`evidence/sv296/`・
  `crates/synvelis-render/tests/rcp07_gate.rs` — 参照のみ、
  sv#518 占有で改変不可）の case-ID へ写像し、scope×recipe
  identity は synvelis-project 層の pin fixture で被覆する。
  recipe cache/invalidation 本体は RCP-07/render 側の所有で
  再実装しない。
- Rule: 新規 CI workflow を作らない。検証は既存
  `mise.toml` task と daily/full CI へ委譲し、実機・consumer
  gate は `evidence/sv288/external-gates.md` で NotRun/Deferred
  として記録する。

## Examples

- Example: `gate_fixture` が 1 project に 5 つの非重複 scope
  （AudioClip・MidiClip・InstrumentSlot・ProcessingRange・
  StripFields）を構築し、各 domain adapter で candidate を
  fork/edit しても live の shared raw（asset・media source・
  recording registry）と sibling candidate が不変である
  （`cb11_e2e_01_fork_edit_isolation_across_five_domains`）。
- Example: 5 candidate を逐次 adopt すると各 receipt が
  `committed`・`undo_steps == 1`・`after == before + 1` を
  報告し、undo×5 で pre-adopt `Project` が bitwise 復帰、
  redo×5 で post-adopt が復帰、save/reload で variant set・
  adopted pointer・`BranchContentRef` が verbatim に残る
  （`cb11_e2e_01_adopt_single_cas_undo_save_reload_across_domains`）。
- Example: TakeUsage scope（comp region の candidate 差替）を
  `BranchEditOp::Comp` で編集・adopt・undo・save/reload しても
  `recording` registry（take・segment・stream・session）が
  bitwise 不変（`cb11_e2e_01_take_comp_adopt_undo_save_reload_raw_immutable`）。
- Example: candidate 編集後の render pin は adopted branch と
  `scope_digest` が異なり `shared_context_digest` が一致、
  その後 live で scope 外 entity（strip1 fader target）を
  編集すると全 set の両 branch の `shared_context_digest`
  のみが動き `scope_digest` と candidate payload digest は
  不変（`cb11_x_rcp_01_render_pin_scope_vs_shared_context`）。
- Example: adopt 前後の live snapshot を `compare_snapshots`
  で比較すると `Different` で scoped semantic change
  （Clip/MidiNote/ParameterTarget の `Modified`）のみが出て、
  Asset/MediaSource/Take/Recording*/ClockEpoch/SmfDocument の
  kind は一切出ない
  （`cb11_e2e_04_semantic_diff_reports_scoped_changes_only`）。
- Example: StripFields の adopt intent を pin 後に live で
  scoped 内容を変更すると adopt は `StaleScope` で typed
  reject され、live `Project`・revision・sibling domain
  candidate が無変更のまま、sibling domain の adopt は
  成功する（`cb11_e2e_05_stale_reject_is_atomic_and_domain_isolated`）。
- Example: 2 Track / 4 Clip の arrangement scope（out-of-
  range・absolute-anchored witness 付き）で mute+move+note
  remove の composite candidate edit が単一 adopt=1 CAS=
  1 undo unit に materialize され、undo/redo が bitwise
  復帰し、scope 外 clip・他 domain・raw material が不変
  （`cb11_e2e_03_arrangement_composite_adopt_undo_scope_isolation`）。
- Example: 3 domain の `ComparisonOrchestrator` session を
  open すると scoped candidate edit は自 session のみ
  Right-side stale dims + `ProjectRevision` で stale 化し
  sibling session は `ProjectRevision` のみ、shared-context
  edit は全 session を `SharedContext` stale にする —
  stale session の再評価は pin-scoped semantic evidence を
  保ちつつ aux layer を stale flag し、`Unaligned` policy・
  opaque plugin state・非有限 metric・probe loss が
  `EvidenceReason`/`Unknown`/`Partial` で誠実に降格する
  （`cb11_e2e_04_orchestrated_compare_*`）。
- Example: gateway の agent principal が task/grant 未提示で
  variant fork を prepare しても `Denied`("no applicable
  authorization") で proposal は残らず、human prepare 済み
  proposal も agent apply で `Denied` — human は同 proposal
  を apply/commit できる。`VariantPorts::standard(None)`
  rig で fork→edit→adopt（ApproveToApply + human receipt）
  →query→discard が durable/VCS backend なしに完走し、
  arrangement scope も配線済み `ArrangementAdapter` 経由で
  composite adopt（fork→`clip.mute`+`clip.move`→adopt、
  単一 CAS=1 undo・scope 外 bitwise 不変）が完走する
  （`crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs` —
  synvelis#539 で配線・旧 fail-closed pin を positive
  検証へ flip）。

## Guarantees

- 7 merged domain adapter（AudioClip・TakeUsage・MidiClip・
  InstrumentSlot・ProcessingRange・StripFields・Arrangement —
  CB-07 merge 済み）で fork→edit→adopt→undo→save→reload が
  headless で完走し、single-CAS undo・raw immutability・
  provenance verbatim の 3 invariant が pin される。
- `compare_snapshots`（VCS-02）が adopt の materialize 結果を
  scoped semantic change として報告し、raw-material kind を
  誤報しないことが pin される。
- `ComparisonOrchestrator`（CB-08）の session freshness・
  evidence honesty 契約が pin され、stale session の
  evidence が current proof を装わないこと、provider/
  alignment/plugin/probe の各 honesty 経路が typed に
  降格することが証明される。
- `resolve_render_project`（RCP-01 pin）の scope/shared
  context digest 分離が pin され、CB-X-RCP-01 の
  invalidation 契約の headless 実行可能部が証明され、
  RCP-07 case-ID（RCP-GATE-01..07）への mapping が
  ledger に記録される。
- stale reject の atomicity と domain 間非汚染が pin される。
- gateway variant ops（CB-10）が grantless agent に対し
  prepare/apply 両段で fail-closed し、VCS/durable backend
  非搭載でも lifecycle が独立動作することが pin される。
- `evidence/sv288/` の ledger が全 case-ID の runnability・
  oracle 種別・所有 Issue・状態語彙を記録し、Blocked/
  NotRun/Deferred を推測で Pass にしない。

## Non-goals

- GPUI Difference Explorer（CB-09・synvelis#286）の GUI
  fixture — 実 window・実 plugin は外部 gate。CB-10 の
  gateway variant ops は headless 契約として pin 済み。
  `VariantPorts::standard` の `ArrangementAdapter` 未配線
  gap は synvelis#539 で解消済み（adapter 登録 + fixture
  positive flip — `evidence/sv539/`）。
- 実 PCM/render 実測・実 RT publish/noalloc/PDC・実 OS
  GUI-L2・実 plugin EXT-L3・AudioLens/Ruviform consumer・
  3OS L1 — 全て外部 gate（`evidence/sv288/external-gates.md`）。
- 実 AI 統合・実 VCS/durable backend・通常 DAW Gate A
  （Record/Playback/Mixer/Save/Export）の実機非回帰 —
  CB-E2E-07 の headless 半分（grantless fail-closed・
  VCS-off lifecycle）のみ pin。実機証拠は #23/#114 系
  gate の責務で、headless suite green を実機 Pass に
  換算しない。
- RCP recipe cache/invalidation・RCP-GATE Case 本体 —
  #296/RCP-07 側の所有（case-ID 参照と scope×recipe
  identity pin のみ実施）。
- production コード・既存 test・canonical doc の変更 —
  本 slice は fixture/ledger のみ。

## Risk

- headless green を実機/consumer Pass に換算する誤読 —
  ledger の Lane 定義と状態語彙で防ぐが、review 時に
  `evidence/sv288/` の各行が正しい lane に分類されているか
  再確認が必要。
- 未 merge domain（arrangement 等）の細部を推測で固定して
  しまう — test-spec/ledger は「前提 Issue merge 後に詳細化」
  と明記し、case-ID の存在のみを inventory 化する。
- 既存 cb-01..06 fixture との被覆重複 — ledger の「既存
  fixture 被覆」列で写像し、新規 fixture は cross-gate
  invariant（single-CAS undo・raw immutability・provenance
  verbatim・pin 分離・semantic diff scoped-only・stale
  非汚染）に限定する。
