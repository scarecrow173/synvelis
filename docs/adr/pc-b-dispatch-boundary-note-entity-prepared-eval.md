---
id: "ADR-034PCB02Lm9wQr3tYuVbCd"
type: "adr"
status: "accepted"
title: "PC-B: dispatch 境界・note entity・prepared eval の配置決定"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/43"
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
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
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
  defers: []
  deferred-by: []
---

# PC-B: dispatch 境界・note entity・prepared eval の配置決定

## Status

approved

## Context

PC-A は dispatch を `lower_event`(`TimedParamEvent` の産出)までに留め、
bridge/plugin adapter への実配送、note-local scope、RT alloc-free
証拠、offline render 一致性を残件とした(issue #43)。配置を誤ると
依存方向(project→workspace→player/render)や永続化境界を壊す。

## Considered Options

* Option 1(採用): dispatch オーケストレーションを `synvelis-player`
  に置き、note entity を `Project.notes` として永続化し、RT 評価は
  `EvalPlan` prepared eval に分離する。offline 一致性は実
  `render()` 経路で検証する。
* Option 2: dispatch/session 経路を `synvelis-workspace` に集約する
  — session crate が plugin bridge/host の重い依存を持ち、実行系と
  session 状態機械の責務境界を崩す。
* Option 3: note entity を作らず `ControlScope::Note` の解決を
  clip+key の検索で代替する — persisted note identity がなく、
  `RemoveClip` cascade・reference validation・note-relative 記録点
  の不変条件を保てない。
* Option 4: offline 一致性を fixture 経路の比較で代替する —
  PC-A と同じ「fixture までの正直な実装」で止まり、受入条件の
  実 bridge 経路検証を満たさない。

## Decision

1. **dispatch オーケストレーションは `synvelis-player` に置く。**
   `ParamDispatch` は `ParamTargetTable` + `ParamDispatchLedger` +
   `PluginNodeControl` + `TransportPlanBuilder<EventKind>` を束ねる
   non-RT 制御面。player は既に graph/RtRuntime を所有し、
   `ruviform-plugin-host` + `ruviform-audio-plugin-bridge` を新規
   依存として追加する。workspace(session 状態機械)には重い
   bridge 依存を持ち込まない — session は evaluate/lower までを
   担い、配送は engine 側が行う。fixture callback でなく実
   `PluginHost`→`attach`→`PluginNode` 経路を使う(実 VST3 は
   rf#253 で scope 外)。
2. **note entity は `Project.notes: BTreeMap<NoteId, Note>` とする。**
   `Note { id, clip: ClipId, key: u8, start: BeatPoint,
   len: BeatPoint }`、start/len は clip-relative beats。
   `ControlScope::Note` の解決は project pos → clip-relative →
   note-relative の合成。clip の musical anchor に追随するため
   `MoveClip` で domain も移動する。schema version は 3→4。
   ops は `AddNote`/`RemoveNote` のみ(MoveNote 等は YAGNI)。
3. **記録点は scope 座標に正規化してから transaction に入れる。**
   `record_points` が project pos を `resolve_scope_pos` で scope
   座標へ変換する。domain 外の点(note 範囲外等)は skip する —
   transport 継続を止めず、scope 外の書き込みを黙って混ぜない。
   clip scope の記録点が project beats のままだった PC-A の潜在的
   不整合もこの変更で整合する。
4. **RT 評価は prepared eval に分離する。**
   `EvalPlan`(compile, non-RT)が target ごとの merged series・
   解決済み assignment・topo 順を保持し、`eval_rt` が caller 提供
   scratch のみで allocation-free に host_normalized を計算する。
   `evaluate_control`(breakdown 付き豊富な経路)は non-RT のまま
   残る。`lower_event` 成功パスは既に alloc-free で、rt_ test が
   その契約を測定する。
5. **offline render 一致性は「同一イベント列→同一絶対 frame 配送」
   で検証する。** `render()`(rf#197)と RtRuntime block drive の双方
   に同一 plan を install し、endpoint 観測の (param, absolute
   frame, value) 列を比較。呼び出し partition 差は契約上許容。

## Consequences

- `synvelis-player` が plugin-host/bridge 依存を持つ(実行系 crate
  として正当)。
- project schema v4: `notes` フィールド追加(`#[serde(default)]`
  で v3 body は deserialize 可能だが、`load()` の厳格な version
  gate により v3 snapshot は `UnsupportedSchema` で拒否 — 既存の
  no-migration ポリシーどおり)。
- session API の signature 変更(`gesture_end`/`record_write` が
  `&Project` を取る)— 呼び出し側は `control_with` 経路に揃う。
- `PerNote` write means は中立契約未対応のまま — note 寿命内の
  真の配送は follow-up。
