---
id: "SPEC-034PCB01Kf8vNq2wXyZmAb"
type: "spec"
status: "approved"
title: "PC-B Parameter Control — Dispatch/NoteScope/RT/Offline headless PoC"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/43"
    - "https://github.com/scarecrow173/synvelis/issues/6"
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
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/197"
    - "https://github.com/scarecrow173/ruviform/issues/253"
  defers: []
  deferred-by: []
---

# PC-B Parameter Control — Dispatch/NoteScope/RT/Offline headless PoC

## What Should Be Built

Issue #43: PC-A(issue #40, merge `a6f74aa`)が確立した Parameter Control
model/session 状態機械の残件を、headless PoC のまま実装する。PC-A は
「fixture capability までの正直な実装」に留めたため、本 issue では
実 dispatch 経路・note-local scope・RT 検証・offline render 一致性の
4 領域を完成させる。Ruviform 側の契約(rf#186 param-write、rf#188
chase/stamps、rf#197 offline render)をそのまま消費し、契約型の
重複実装は行わない。

## Requirements

### PC-B-01: 実 dispatch 経路と拒否伝播

- Host-resolved `Value`(`lower_event` が産出する `TimedParamEvent`)を
  ruviform plugin bridge/plugin adapter 経由で実 plugin ノードへ
  書き込む実経路を持つ。fixture 専用の局所コールバックではない。
  - `ruviform-plugin-host` `PluginHost` + `ruviform-audio-plugin-bridge`
    (`PluginNodeFactory`/`attach`/`PluginNodeControl`) + `RtRuntime`
    経由で `ParamEvent` が endpoint へ届く。
  - 実 VST3 plugin は rf#253 gate でスコープ外 — bridge 側 fixture
    stub(plugin adapter 経由の実 bridge 経路)で代替する。
- `ParamWriteRejection` が上位へ正しく伝播する:
  `check_param_write` 拒否 → `Error::WriteRejected`、binding 未解決/
  defunct → 明示的な拒否 reason、plan queue 満杯 → 型付きエラー。
  拒否を成功へ変換しない。
- dispatch 観測は `ParamDispatchLedger` の stage(`Dispatched`/
  `Dropped`)で表現し、観測値は session の `feed_probe`(host 側観測)
  経路と整合する。

### PC-B-02: Note-local expression scope

- PC-A で `ScopeUnavailable` だった `ControlScope::Note(NoteId)` の
  評価を実装する。評価座標は note-relative beats(note 起点=0)。
- 最小の note entity モデルを `Project` に導入する
  (`Note { id, clip, key, start, len }` — start/len は clip-relative
  beats)。project pos → clip-relative → note-relative の合成解決。
  clip 移動で note 領域も追随する(musical anchor 前提)。
- note-local expression が独立 scope として評価・記録できる:
  - `evaluate_control` / `ControlSession::evaluate` が note-relative
    series を返す。
  - 記録経路(`record_write`/gesture/transport held write)は project
    pos を scope 座標へ変換してから格納する。domain 外の点は記録
    しない(scope 外書き込みを黙って混ぜない)。
- note 外の評価は `ScopeUnavailable` を返す(domain 外に値を捏造
  しない)。note 参照を持つ既存の `ControlScope::Note` 拒否は
  entity 導入で解消される。

### PC-B-03: RT allocation-free 検証

- control-rate 評価/dispatch が allocation-free であることを `rt_`
  prefix のテストで検証する(ruviform の RT 契約テスト手法:
  `assert_no_alloc` + 測定窓外での準備/warmup)。
- 評価側: `EvalPlan` を「compile(non-RT)」と「alloc-free 評価」に
  分け、prepare 済み series/assignment データで `eval_rt` を行う。
  測定対象は成功パス(エラーパスの `format!` は RT 契約外)。
- dispatch 側: `lower_event` の成功パス(eval → check →
  `TimedParamEvent` → 事前確保 scratch への push)が alloc-free。
- 不可能な箇所は impl record に明示する。

### PC-B-04: offline render 一致性

- 同一の評価済み `TimedParamEvent` 列を playback 系(`RtRuntime`
  block drive)と offline render 系(`ruviform-audio` `render()` /
  `PreparedRender`)の両方で plugin node へ配送し、endpoint が観測
  する (param, absolute frame, value) ストリームが一致することを
  検証する。
- callback 分割差(呼び出し partition)は契約上許容 — 絶対 frame
  座標での一致をアサートする。
- upstream `PluginNode::offline_support()` は `Unknown`(bridge
  node の offline 等価性は ruviform 未検証 — rf#197 系統)のまま。
  テストは stateless・transport 決定論的な fixture インスタンスに
  限って `Verified` を宣言するラッパを factory 経由で装着し、
  一致性アサートを宣言の検証根拠とする。upstream が bridge node
  を Verified 化したら wrapper 撤去が follow-up。
- 困難な場合は一致しない理由を契約化し follow-up を提案する。

## Invariants

- `ParamWriteRejection` は型を保ったまま上位へ伝播し、成功へ変換
  されない。
- note scope の評価座標は note-relative beats であり、project/clip
  座標と混同しない。
- RT 計測窓(成功パス)での heap alloc/dealloc は 0。
- playback / offline render は同一評価セマンティクスを共有し、
  配送イベントの絶対 frame・値は一致する。
- 記録される automation point の座標は常に scope 座標
  (note-local = note-relative beats)。

## Non-Requirements

- 実 GUI、automation lane 表示、Control Mapping/外部コントローラ。
- 実 VST3 plugin(rf#253 gate)。
- `ParamWriteMeans::Immediate`/`PerNote` の新規サポート(中立契約が
  未対応なまま。PerNote の真の note 寿命配送は follow-up)。
- フル note editor/clip-local MIDI 編集。最小の headless note entity
  のみ。

## Acceptance Criteria

- PC-B-01: bridge 経路で `ParamEvent` が endpoint に届き、bind/
  resolve/queue の拒否が型付きで上位へ伝播する headless test が
  green。
- PC-B-02: note-local target の評価が note-relative 座標で正しく、
  記録点が note-relative で格納される headless test が green。
- PC-B-03: `rt_` テストで eval_rt + lower_event 成功パスの
  alloc/dealloc=0 をアサート、green。
- PC-B-04: playback drive と `render()` での配送イベント一致テスト
  が green(不可能なら契約を impl record に記録)。
- `mise run fmt-check` / `check` / `lint` / `test` 全 green。
- doc chain 完走、docs/impl に impl record。
