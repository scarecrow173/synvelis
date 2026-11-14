---
id: "EXP-cb05-fx-range-adapter"
title: "CB-05 FX Processing Range Adapter 設計実験メモ (synvelis#282)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: CB-05 FX Processing Range headless スライス（synvelis#282）

## E1 任意 mid-chain `end` を初期受理するか

**仮説**: `ProcessingRangeRef` は schema 上任意の
`start`/`end` `ProcessorId` を保持できる。Issue は
「End=Chain末尾。任意EndはSchema表現可能だが本Issueでは
初期受入不要」と規定するため、mid-chain end をそのまま
受理するか検討が必要。

- 調査: `range_processors` は chain 内位置で `start..=end`
  を解決するため、mid-chain end でも構造的には解決可能。
  しかし end が tail でない range は「range の下流に同一
  chain の processor が残る」状態を生む — 境界を跨ぐ
  main-path 接続の扱い・downstream processor との関係が
  初期 slice の confinement 保証を複雑化する。
- 案 A（mid-chain end を受理）: Issue が初期受入不要と
  明示。境界跨ぎ main connection の安全な扱いを別途
  設計する必要があり、本 slice の fail-closed 契約を
  超える。却下。
- 案 B（`end` = chain tail を adapter が強制）:
  `ProcessingRangeRef` の schema 表現力は維持しつつ、
  `validate_scope` が tail 以外を `Unsupported` で型付き
  拒否。silent widen も暗黙の tail 延長もしない。

**観察**: 案 B を採用。`cb_fx_01_boundary_validation_
fail_closed` で mid-chain end → `Unsupported`、reversed
order → `Project(Validation)`、未知 processor → `NotFound`
を検証。

## E2 chain 構造 op（insert/remove/reorder）をどう confinement するか

**仮説**: FX range の候補編集に processor の追加・削除・
並べ替えを許すべきか。許すなら chain membership と
scope 境界の整合をどう保証するか。

- 調査: `InsertProcessor` は processor+port+instance を
  mint し chain に挿入するが connection は作らない
  （配線は明示 op）。`RemoveProcessor` は port/chain/
  instance を除去するが外部 connection の参照整合は
  oracle 依存で、scope 境界を跨ぐ参照を安全に扱えない。
  `ReorderProcessor` は chain 順序を変えるが、順序は
  scope objects に含まれない共有 context — 候補側での
  reorder は「どの processor が range 内か」を post-edit
  再導出に委ねるしかない。
- 案 A（insert/remove/reorder を全て受理）: remove/reorder
  は外部配線・境界 membership を不確定にし、confinement
  の post-edit 検証だけでは「触ってはいけない共有状態」
  への影響を排除できない。却下。
- 案 B（`InsertProcessor` のみ受理・post-edit scope 再導出）:
  insert は「range の chain」かつ「適用後の scope 導出で
  `start..=end` 内に収まる」場合のみ受理 — 新規 processor
  が範囲内なら scope-owned、外なら `ScopeViolation`。
  remove/reorder/全 wiring op は `Unsupported`。

**観察**: 案 B を採用。`rich_candidate_edits` の index=2
insert が範囲内に着地し scope-owned になること、
foreign strip への insert が `ScopeViolation`、remove/
reorder が `Unsupported` であることを検証。採用後は
live chain に insert 位置が正しく materialize される
（`cb_fx_05_adopt_range_only_one_cas_undo_save_reload`）。

## E3 processor-owned control confinement を CB-04 から共有化するか複製するか

**仮説**: CB-04 は `InstrumentSlot` 1 processor の
control closure を confinement した。CB-05 は range 内
全 processor が対象 — 同一ロジックの複製か共有化か。

- 調査: `adapter.rs` の instrument 専用 checker は
  `ControlOwner::Processor` の target が scope 内
  processor に属するかを検査していた。range では
  「scope 内 processor 集合」が複数になるだけで規則は
  同一 — `scope_objects` から `ObjectKind::Processor`
  を抽出した `scoped_processor_ids` で所有権を一元導出
  できる。
- 案 A（range 用に複製）: 同一規則の二重実装 — 将来の
  op 追加時に drift する危険。却下。
- 案 B（`check_processor_control_op_targets` として共有化）:
  `InstrumentSlot`/`ProcessingRange` 双方が同じ checker
  に dispatch。所有 processor 集合は scope kind に応じて
  `scope_objects` から導出 — confinement 規則の authoritative
  実装は 1 箇所。

**観察**: 案 B を採用。`cb04_instrument_slot.rs` 全件が
無回帰で green（共有化による CB-04 挙動変更なし）。
`cb_fx_04_external_wiring_ops_reject_typed` で upstream
`pre` processor の param 操作が `ScopeViolation`、range
param の removal が `Unsupported` であることを検証。

## E4 境界を跨ぐ connection/send/sidechain を scope object にするか

**仮説**: sidechain feed や send が range 境界を跨ぐ場合、
Issue は「Preflight closureで検証。初期に安全に包め
なければUnsupported/Reject」を要求。これらを scope
objects に含めるべきか。

- 調査: `processing_range_objects` は「両端 port が scope
  内」の connection のみを所有する — 境界跨ぎ connection
  は定義上 scope 外（shared）。scope object に含めると
  adopt が外部 port に触れる経路が生まれ fail-closed
  契約と矛盾する。
- 案 A（境界跨ぎ connection を scope object 化）: 両端の
  片方が scope 外 port — adopt transplant が scope 外
  entity を変更することになり契約違反。却下。
- 案 B（typed inventory + 配線 op 全 reject）:
  `boundary_dependencies` が跨ぎ connection を
  `{connection, kind, source_in_scope}` で列挙するが
  scope objects には含めない。候補が配線を変更する全
  `ProjectOp`（`AddConnection`/`RemoveConnection`/
  `SetConnectionEnabled`/`SetConnectionChannelMap`/
  `AddSend`/`RemoveSend`/`SetSendTap` 等）は
  `Unsupported` — 外部配線は共有のまま動かない。

**観察**: 案 B を採用。`cb_fx_04_boundary_dependency_
detected_and_shared` で sidechain feed がちょうど 1 件
inventory され、無効化しても依存として残り
shared-context digest が動く（scope digest は不変）
ことを検証。`cb_fx_04_external_wiring_ops_reject_typed`
で境界 connection への 3 op + 新規外部配線 3 op +
foreign chain 構造 3 op + strip mute が全て型付き拒否
されることを検証。

## E5 recording gate の lane 写像を range にどう適用するか

**仮説**: CB-04 は 1 processor → 所属 strip → track lane
で recording-open を検査した。range は複数 processor・
chain 単位 — lane 写像をどう一般化するか。

- 調査: range の chain は 1 strip に属し、strip は track
  または bus owner を持つ。track strip → 所有 track の
  lane、bus strip → 帰属 lane を一意に決められないため
  全 project lane を検査する fail-closed 写像が安全。
- 案 A（range 内各 processor の strip を個別解決）: 全
  processor は同一 chain = 同一 strip なので冗長。却下。
- 案 B（chain の strip で 1 回判定・bus は全 lane）:
  `strip_recording_gate_lanes` を `pub(crate)` 共有 helper
  として audio.rs に置き、instrument.rs の局所実装を
  置換。range adapter は chain の strip に対して同 helper
  を呼ぶ — 判定規則の authoritative 実装は 1 箇所。

**観察**: 案 B を採用。`cb_fx_05_adopt_while_recording_
rejects_typed` で owning lane の recording session Open
が `RecordingActive` で adopt を先行拒否し live/revision
不変であることを検証。CB-04 回帰なし。
