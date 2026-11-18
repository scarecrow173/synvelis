# sv559 — stored candidate payload の観測不能 entity 削除 adopt 再現 evidence

Issue: `scarecrow173/synvelis#559` / branch
`bugfix/559-adopt-unobservable-removal`（base: `origin/main`
`848ebb73` — sv#557 PR `synvelis#560` 着地済み HEAD）
OS: Windows（mise managed toolchain: rustc 1.99.1 /
cargo 1.99.1）。cargo 直接実行は補助証拠、canonical = mise。

## AC1 — 再現対象

pre-553 相当の stored candidate payload（send-owned
`ModulationSource`/`ModulationAssignment` 削除を含む）に
対する adopt が、fail-closed 検査なしに live へ物質化する
ことの実測。現行 wire（sv#553 以降）では当該 payload を
新規生成不能なため、再現は domain 層
`edit_branch`/`edit_branch_op` 経路（gateway facts gate を
持たない — wire 側 deny を受けず domain cascade が不可視
削除入り payload を生成する。pre-553 wire 生産物と同型）
で行った。

## 再現手順（pre-fix HEAD `848ebb73` + test 追加のみ）

`crates/synvelis-project/tests/cb06_strip_fields.rs` 末尾に
一時 test `cb_strip_559_repro_adopt_materializes_
unobservable_removal` を追加（production code 非変更）:

1. `strip_fixture` — track strip + return bus + key strip +
   strip→bus-strip `Send`（gain_target 未バインド）。
2. `send_modulated_closure` — fork 前に live へ send-owned
   modulation closure を構築:
   - `live_lfo_source(c, ControlOwner::Send(ids.send))`
     → `ModulationSource`（owner = Send）
   - `AddParameterTarget`（owner = Send, role = SendLevel）
     → send-owned `ParameterTarget`
   - `live_assignment(source, lvl)` → `ModulationAssignment`
     （source → send-owned target）
3. `Strip::fork(scope_of(&ids, &[StripField::Sends]))` —
   `{Sends}` mask の scope は send-owned source/target/
   assignment を scope object として捕捉する
   （`variant/scope.rs` `strip_routing_objects` +
   `strip_assignment_objects`）。
4. `Strip::edit(set, candidate, ProjectOp::RemoveSend
   { send_id })` — domain cascade `remove_control_owned`
   （`ops.rs`）が send・edge・SendLevel target・send-owned
   `ModulationSource`・`ModulationAssignment` を candidate
   から削除。`check_strip_fields_scope_removals` は
   `removal_supported_kind`（両 kind を含む）で受理する
   ため edit は commit される。
5. `Strip::adopt`（`adopt_branch_adapted` 経由）—
   `adopt_guard` → `verified_candidate`（payload decode +
   scope digest 再検証 — 自己整合的なため通過）→
   `preflight_adopt` → `plan_adopt_writes` が
   `ModulationSource`/`ModulationAssignment` を `Removed`
   write として計画・適用。

## 実測結果（pre-fix）

``
$ mise run test synvelis-project --test cb06_strip_fields cb_strip_559
running 1 test
test cb_strip_559_repro_adopt_materializes_unobservable_removal ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out
```

exit code 0 — test が Pass = **物質化を実測**:

- `AdoptReceipt.outcome == AdoptOutcome::Applied`
- adopt 後の live から `routing.sends` の send・
  `control.sources` の send-owned `ModulationSource`・
  `control.targets` の send-owned `SendLevel` target・
  `control.assignments` の `ModulationAssignment` が
  全て除去された（assertion で確認）。
- `derive_effects`/`authorize_effects` はこれらの kind を
  観測しない（`ResourceKind` 非含有 — SEC-14 境界）
  ため post-simulation gate を素通りした。

## 備考

- 本 test は修正実装前の再現記録として追加したもので、
  fix 適用後は同一 scenario が typed rejection
  （`VariantError::UnverifiableRemoval`）になることを
  別 test で pin する（本 test は fix commit で rejection
  pin に置き換える）。
- stored-bytes 注入経路（`InMemoryBranchContentStore` /
  `DirectoryBranchContentStore` + `BranchContentRef::
  Captured` 差し替え）でも同じ payload が
  `decode_candidate`（`variant/store.rs` —
  `schema_version == 15` + payload digest）と
  `verified_candidate`（scope digest 再検証）を通過する
  ことをコード上確認済み（`SCHEMA_VERSION` は sv#550→
  sv#553 窓で不変・現行 15 — `store.rs`）。
- 実 PCM/実 plugin/実 GUI lane は対象外（POLICIES.md §5 —
  headless のみ）。

## 関連リンク

- `synvelis#559`（本 Issue）/ `synvelis#557`（起票根拠の
  監査）/ `synvelis#553` / `synvelis#550` / `synvelis#561`
  （companion — wire 側 honest-deny・別 slice）/
  `synvelis#283`（CB-06 tracker）
- `docs/discovery/issue-559-adopt-unobservable-removal.md`
- `crates/synvelis-project/src/variant/engine.rs`
  （`plan_adopt_writes`）
- `crates/synvelis-project/src/apply/entity.rs`
  （`removal_supported_kind`）
- `crates/synvelis-project/src/ops.rs`
  （`remove_control_owned`/`release_send_level_target`）
- `crates/synvelis-project/tests/cb06_strip_fields.rs`
- `POLICIES.md`
- `AGENTS.md`
