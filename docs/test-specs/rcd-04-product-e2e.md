---
id: "TSPEC-034VmO7ak9xboyiIKIf80v"
type: "test-spec"
status: "approved"
title: "RCD-04 product E2E test spec"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
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
    - "docs/specs/rcd-04-product-e2e.md"
    - "docs/designs/rcd-04-product-e2e-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-04-product-e2e.md"
    - "docs/designs/rcd-04-product-e2e-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 product E2E test spec

## Purpose

Issue #118 app 層 slice の製品 E2E 契約を固定する。
crate 層 fixture（PR #219）が domain 意味論を固定した
上で、「製品が実際に呼ぶ `syn.*` 文字列 dispatch 経路
（`UiRuntime::dispatch_op` → `Service::call` →
`AppService` → `SynvelisApp`）を通したとき、Comp
Transition / MIDI Phrase / Recovery Review / Stale +
read-only の契約が wire JSON として観測できる」ことを
headless で検証する。実 Window/OS GUI/device/OS FS/
RT sample/AudioLens f64 は NotRun/Unsupported であり、
fake fixture の成功を実機 Pass に昇格しない。

## Feature

RCD-04 product E2E — headless `syn.*` op dispatch による
Comp Transition・MIDI Phrase・Recovery Review・Stale/
read-only の製品レベル証拠。

## Rules

- R1: 全ての製品操作は `syn.*` 文字列 op として
  `UiRuntime::dispatch_op` を通す。test 専用の第二
  dispatch・第二 project/recording/comp モデルを
  作らない。fixture 構築に必要な domain op のみ
  `ProjectSession::apply(SynvelisOp, expected_revision)`
  の canonical 経路を使う（これは製品 undo/redo/CAS
  が通る同一 mutation 経路であり、検証対象の
  `syn.*` dispatch とは別層）。
- R2: Comp Transition — TakeGroup/Comp/CompRegion/
  CompTransition が `syn.*` 経路で構築され、Group
  Comp 全体の stale expected-revision が
  `StaleRevision` で原子的に拒否され、undo/redo
  ×7・save/reload で stable tagged ID が維持され、
  VCS diff（committed pre→post で各 entity が Added、
  committed post→live で entity-level 変更 0 件）に
  stable ID が現れる。comp op は take/source/asset/
  segment を変更しない。
- R3: MIDI Phrase — `compile_midi_comp` の plan が
  phrase 境界で NoteOn/NoteOff を同一 `(source, note)`
  identity で 1:1 に保ち（hanging note・controller
  leak なし）、virtual off で境界跨ぎ note を切断し、
  terminal `AllNotesOff` で終了する。undo/redo・
  save/reload 後も Note stable ID・plan が決定的に
  一致し、VCS diff に MidiNote Added stable ID が現れ、
  committed post→live diff が `equal` になる。
  MIDI comp selection は plan 評価層のみ（永続化は
  #126 deferred）。
- R4: Recovery Review — staging partial + cancel(Hold)
  + retry(Adopt) → `commit_session` が Complete
  stream のみ commit し Partial を `partial-finalize`
  recoverable に分離する。Hold は media/journal を
  byte 同一に保ち review record を永続化し project
  revision を進めない。Adopt は verified prefix のみ
  Partial finalize し冪等。commit 全体が 1 Undo で
  戻り Redo で復元され、`syn.recording.info` が
  committed entity を投影し save/reload で stable ID
  が維持される。
- R5: Recovery guards — probe 無し Discard は
  fail-closed（`Refused`）で media を保持し、
  corrupt journal は scan が `Finalized` と報告せず
  review action が fail-closed または verified
  prefix のみ finalize し、project revision・media
  を変更しない。
- R6: Stale + read-only — 2 view が同一 project
  session を共有する構成で、stale-generation
  `syn.control.dispatch` は receipt を発行せず drop
  され project revision を進めず、現行 generation
  は `applied` receipt を発行する。VCS history の
  open/refresh/select_node/pin/compare/view op 列は
  project revision を進めない read-only である。
- R7: 正直な失敗分類 — headless で実行したものだけを
  Passed とし、実 Window/OS GUI/device/OS FS/実 RT
  sample/AudioLens f64 は `NotRun`/`Unsupported` と
  して記録する。VCS diff `overall` は tri-state を
  保ち、fake asset digest による `unknown` を `equal`
  に丸めない（entity-level `changes`/`summary.total`
  を主張する）。

## Examples

- E2E-1 Comp Transition: track+2 lane+epoch+committed
  source → finalized session に verified take ×2 →
  TakeGroup/Comp/3 Region/ActiveComp/Transition（
  EqualPower/Linear fade）→ `syn.recording.info` が
  takes=2・session=finalized を投影 → stale CAS 拒否
  → `syn.project.undo`×7 で comp registry 空・
  recording 不変 → `syn.project.redo`×7 で同一 stable
  ID → `syn.project.save`/`reload` で維持 → VCS
  committed(pre)→committed(post) diff に
  TakeGroup/Comp/CompRegion/CompTransition の Added
  stable ID、committed(post)→live で変更 0 件・
  revision 不変。
- E2E-2 MIDI Phrase: track→lane→MIDI source
  （division 480、project PPQN 960 で厳密写像）→clip
  →7 note（phrase 境界 2400 を跨ぐ・直前 onset・
  境界 onset を含む）→ plan boundary oracle（on=off・
  active 空・AllNotesOff terminal・virtual offs・
  chase が宣言 (channel,kind) union を各 region で
  網羅）→ undo で最後の note が plan から消え
  redo で NoteId/plan が一致 → save/reload で source/
  note stable ID 維持 → VCS diff に MidiNote Added
  7 件の stable ID・committed(post)→live `equal`・
  comp registry 空のまま。
- E2E-3 Recovery Review: Closed session + lane 毎
  stream を project 登録 → staging stream0 Complete /
  stream1 crashed partial（checkpoint prefix +
  journaled tail + unverified media 追記）→ scan が
  2 stream を報告 → stream1 に Hold（media/journal
  byte 同一・`Held` record・revision 不変）→ Adopt
  retry（`Partial` finalize・verified prefix のみ・
  冪等 re-Adopt）→ `commit_session` が Applied・
  takes=1・clips=1・recoverable=[stream1
  `partial-finalize`] → `syn.recording.info` が
  session=finalized・take=verified を投影 → undo で
  commit 全体が戻り（pre-registered session/stream は
  残存）redo で復元 → save/reload で session id 維持。
- E2E-4 Recovery guards: probe 無し Discard が
  `Refused` で media 保持・revision 不変 → journal
  corrupt で scan が `Finalized` を報告せず、
  `Unreadable` なら全 action が fail-closed・media/
  revision 不変。
- E2E-5 Stale+read-only: `syn.view.attach` で
  console-a/console-b を win-1/win-2 に attach →
  generation 0 の transport dispatch が receipt 無し
  で drop・revision 不変 → generation 1 が applied
  receipt → VCS history ops 列後も revision 不変・
  comp entity 残存。

## Guarantees

- `syn.*` dispatch 経路のみが製品操作の検証対象で
  あり、fixture 構築は canonical `ProjectSession::
  apply` 経路（undo/redo/CAS 対象の同一 mutation）
  である。
- Comp/MIDI/Recovery の stable tagged ID が
  undo/redo・save/reload・VCS diff を貫いて維持される。
- Group Comp・commit_session が all-or-nothing で、
  stale revision・corrupt/unreadable・probe 無し
  Discard が fail-closed で project/media を変更しない。
- VCS history/control dispatch が project revision を
  進めない read-only/stale-drop 契約を wire JSON 面で
  満たす。
- 実機未検証領域が Passed に見えない（NotRun/
  Unsupported の明示）。

## Non-goals

- 実 Window・実 OS GUI・実 device・実 plugin・実 OS
  filesystem durability・実 RT engine sample 一致・
  AudioLens native-f64 の検証（NotRun/Unsupported）。
- MIDI comp selection の project 永続化（#126）。
- RT PCM vs Offline PCM 差分の本格 golden（crate 層
  fixture `rcd_04_take_comp_pcm` が担う）。
- `synvelis-project`/`synvelis-recording` 等の実装変更
  （本 slice は `synvelis-app` test + docs のみ）。
- 実 Git provider の検出/binding（fake provider 注入
  のみ。実 provider は後続 slice）。

## Risk

- `SharedService` の `Rc<RefCell>` 共有が `Service::
  call` の `&mut self` を正しく満たさない場合、
  borrow panic または dispatch 不能になる → call 単位
  の短い borrow で回避（実装済み・test pass で確認）。
- fixture 構築を「test 専用経路」と誤認される risk →
  R1 で canonical apply 経路であることを明示し、
  test-spec/impl record に境界を記録する。
- VCS `overall: unknown` を `equal` と誤読する risk →
  R7 で entity-level `changes`/`summary.total` を主張し
  `overall` は `different` でないことのみ確認する。
- fake asset digest・mock staging file の成功を実機
  Pass と誤認する risk → file 冒頭・scenario・docs・
  PR に NotRun/Unsupported を明記する。
