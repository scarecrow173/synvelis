---
id: "EXP-cb11-cross-gate"
title: "CB-11 cross-gate headless fixture 実験メモ (synvelis#288)"
created: "2026-11-15"
updated: "2026-11-17"
---

# 実験記録: CB-11 Creative Branch cross-gate headless slice（synvelis#288）

## E1 5 domain disjoint scope を単一 Project に同居させられるか

**仮説**: AudioClip・MidiClip・InstrumentSlot・
ProcessingRange・StripFields の 5 scope が互いに scope object
を共有しない fixture を構成すれば、1 つの candidate 内で
全 domain の edit→adopt→undo→save/reload を検証できる。

- 調査: cb01..06 の既存 fixture は各 domain を単独 Project
  で検証する。cross-gate の価値は「別 domain の候補・共有
  context が adopt で汚染されない」ことの同時観測にある。
- 案 A（domain ごとに Project を分ける）: cross-domain
  contamination を観測できない。却下。
- 案 B（disjoint scope を 1 Project に同居）: Audio clip と
  MIDI clip は別 track/lane、Instrument slot は独立
  processor、ProcessingRange は clip 外の安定境界、
  StripFields は第 2 strip の channel target に配置。
  shared context witness として strip channel target を
  scope 外 edit の対象にした。

**観察**: 案 B を採用。`cb11_e2e_01_adopt_single_cas_
undo_save_reload_across_domains` で 5 domain の candidate
edit が 1 adopt=1 CAS=1 undo unit に収まり、undo で
pre-adopt Project と完全一致、save/reload 後も variant
set・lineage・content ref・digest が verbatim であることを
確認。`cb11_e2e_05_stale_reject_is_atomic_and_domain_
isolated` で stale adopt が live revision・sibling
candidate を汚染しないことを確認。

## E2 raw material witness に MIDI source を含めるべきか

**仮説**: 全 content digest を raw material witness として
serialize 比較すれば raw immutability を一括検証できる。

- 調査: 初回実行で take fixture が comp validation error、
  5-domain fixture が raw witness 不一致で失敗。
- 観察: `midi_sources` は MidiClip scope の scope-owned
  content であり、candidate edit が意図的に変更する対象。
  raw material witness に含めると「scope-owned content の
  変更」を「raw churn」と誤認する。除外して audio source・
  asset・recording registry・take 実体の digest のみを
  witness とした。また take fixture の `source_offset` は
  絶対 source frame であり、take1 coverage `[40000,
  70000)` を満たすよう修正した。

**観察**: 両方とも fixture 側の構成ミスであり src defect
ではない。修正後 `cargo test -p synvelis-project --test
cb11_cross_gate` で 6 test 全件 green。

## E3 adopted branch の content ref 検証対象

**仮説**: adopt 後の provenance 検証は adopted branch の
`BranchContentRef` を見ればよい。

- 調査: `adopt_branch_adapted` は candidate payload を live
  へ適用し、採用側 branch は `BranchContentRef::Live` を
  指し、旧 branch は `Captured` に凍結される。
- 観察: receipt の `last_adopt` が指す live 側を検証対象に
  しないと、凍結済みの旧 branch を見て provenance を
  誤判定する。`VariantSet::last_adopt` 経由で live branch
  の content ref・payload digest を candidate と verbatim
  比較する形に修正。

## E4 render pin の scope/shared-context digest 分離

**仮説**: `resolve_render_project` の `BranchRenderPin` は
scope-owned edit で `scope_digest` のみ、shared-context
edit で `shared_context_digest` のみを動かし、scope digest
が shared context に依存しない。

**観察**: `cb11_x_rcp_01_render_pin_scope_vs_shared_context`
で両方向を確認。scope edit は `scope_digest` のみ変化、
strip channel target（scope 外 shared context）への edit
は `shared_context_digest` のみ変化し、全 branch の pin が
共有 context 変更を観測する。CB-X-RCP-01 の runnable 部分
として採用。recipe cache/invalidation 本体は #296/RCP-07
の所有として本 slice では再実装しない。

## E5 stale-scope adopt の atomicity

**仮説**: fork 後に live project を変更して scope digest を
stale にした adopt は `VariantError::StaleScope` で
fail-closed し、live state・revision・sibling candidate
を汚染しない。

**観察**: `cb11_e2e_05_stale_reject_is_atomic_and_domain_
isolated` で確認。reject 後の `Project` は pre-attempt
snapshot と一致、revision は単調増加のみ、sibling
candidate の content digest は不変。CB-E2E-05 の
cross-domain 組合せ部分として採用。recording active・
missing plugin・queue full の個別 gate は cb01..06 既存
fixture の被覆を ledger で参照。

## E6 orchestrated compare session の scoped-edit 分離は
   `Current` ではなく `Stale{[ProjectRevision]}` か

**仮説**: 異なる variant set の sibling session は scoped
candidate edit の影響を受けず `Current` のままである。

- 調査: `ComparisonOrchestrator::open_session` は open 時の
  `project_revision` を `pinned_context` に pin し、
  `freshness` は pinned vs live を全次元で比較する。
  candidate edit は live project revision を進めるため、
  scoped 内容が無関係でも `ProjectRevision` 次元は全
  session で stale になる。
- 案 A（sibling は `Current`）: `cb11_e2e_04_orchestrated_
  compare_stale_session_never_poses_as_current` 初回実行で
  `Stale{[ProjectRevision{pinned:32, actual:36}]}` を観測し
  棄却。
- 案 B（sibling は `Stale{[ProjectRevision]}` のみ）:
  scope content・branch revision・payload・shared context・
  tempo/plugin/source/time-mapping のいずれの次元も
  cross-domain で drift しないことを assert。

**観察**: 案 B を採用。domain 分離の正直な契約は
「scope 固有次元は隔離されるが project revision tick は
全 session に及ぶ」であり、stale session の再評価が
pin-scoped semantic evidence を保ち aux layer（signal/
perceptual）を stale flag することを確認。実 RT/plugin
signal は `NotRun` のまま（`evidence/sv288/external-gates.md`）。

## E7 probe loss は asset を参照しない scope では
   `unresolved`/`Partial` に効かないか

**仮説**: `BlobProbe` が `Absent` を返せば任意の compare
session で `unresolved` issues + `Partial` completeness に
なる。

- 調査: `BlobProbe` は snapshot の asset availability
  manifest 経由でのみ参照される。TakeUsage fixture の
  asset digest は compare 対象 content から到達しない
  ため、`AbsentProbe` でも `Complete` のままだった。
- 案 A（take scope で assert）: `cb11_e2e_04_orchestrated_
  compare_unaligned_probe_loss_provider_honesty` 初回実行で
  `Complete != Partial` を観測し棄却。
- 案 B（asset 保持の audio scope で assert）: media
  source→asset 参照を持つ gate fixture の audio clip
  session で `Partial`・`unresolved` 非空・`Unknown` を
  確認。

**観察**: 案 B を採用。probe loss の honesty は「比較対象
content が実際に参照する asset の不在」のみを `unresolved`
として報告し、無関係 asset の probe 結果で equality を
毀損しない — これも誠実な契約として採用。

## E8 gateway `VariantPorts::standard` の Arrangement
   未配線は意図的か defect か

**仮説**: CB-07 merge により `ArrangementAdapter` が
`synvelis_project::variant` に存在するため、gateway の
`VariantPorts::standard` も配線済みのはず。

- 調査: `crates/synvelis-gateway/src/variant.rs` は
  `Arrangement` を `VariantScopeKind` に列挙するが
  `adapter_for` が `adapters` map を引き、
  `VariantPorts::standard` は 6 adapter のみ insert —
  コメントに「Arrangement has no adapter in this slice」
  とあり CB-10 slice の意図的な未配線。CB-07 merge 後に
  gateway 側が更新されていない。
- 案 A（fixture で fail-closed を pin）: arrangement scope
  の fork prepare が `variant-unsupported: no domain
  adapter wired for arrangement scopes` で拒否されること
  を `cb11_e2e_07_arrangement_scope_fails_closed_until_
  gateway_wiring` で確認。
- 案 B（src 修正で adapter 登録）: `crates/**/src/**` は
  本 slice の編集範囲外 — 修正せず ledger/PR に gap と
  して記録。

**観察**: 案 A を採用。fail-closed は安全側の挙動であり、
CB-E2E-03 の project 軸 coverage（`cb11_e2e_03_*`）は
独立に成立する。gateway 経路の arrangement 対応は
`VariantPorts::standard` への adapter 登録を要する
follow-up として記録。

**追記（synvelis#539）**: 後続 slice で案 B 相当の src
修正が実施された — `VariantPorts::standard` の adapters
map に `ArrangementAdapter` を登録し、陳腐化した
「no adapter yet」系 doc comment を更新。当該 fixture は
`cb11_e2e_07_arrangement_scope_composite_adopt_undo_scope_isolation`
へ positive flip され、gateway 経路の CB-E2E-03 composite
adopt（単一 CAS=1 undo・scope 外 bitwise 不変）は
headless Pass（`evidence/sv539/`）。`adapter_for` の
fail-closed 機構は未配線 kind 向けに維持。
