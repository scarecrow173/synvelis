---
id: "EXP-ZnsMGpOoIU5eJo4DXLlaVd"
type: "experiment"
status: "completed"
title: "RCD-01 Virtual L1 Oracle — emission-log vs generator-recompute 検証構造と fault outcome 解釈"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/rcd-01-virtual-l1-oracle.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-01-virtual-l1-oracle.md"
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/rcd-01-virtual-l1-oracle.md"
  defers: []
  deferred-by: []
---

# RCD-01 Virtual L1 Oracle — emission-log 検証構造と fault outcome 解釈

## 仮説

独立 oracle の「期待値」を generator 再計算で作ると、入力生成と期待値
検算が同じ関数を経由し、callback→adapter→writer→media の各段変換・
順序・channel map の独立検証にならない。device が callback buffer に
実際に書き出した `f32` bit pattern を emission log に記録し、staging
media の f32le bytes を decode して bitwise 照合する構造なら、generator
の自己撞着を避けつつ真の E2E fidelity を検証できる — ただし「emission
log が device の出力を正しく捉えているか」「sentinel `u32::MAX` が有限
oracle 出力と衝突しないか」は検証が要る。

## 方法

- `oracle_sample`（splitmix64 連鎖相当 hash + 2 回 fmix64 avalanche +
  `f64`→`[-1,1)` f32 射影）を `MockPcm` と別構造で実装し、golden const
  で bit pattern を pin。
- `OracleStreamShared.emitted` に callback buffer の実 `f32` bit を記録、
  検証側は journal `ChunkRecord.media_offset`/`spans` で index した
  staging media f32le bytes と bitwise 照合。
- `RealDeviceRig::open` の `&dyn Device` 経路で scripted device を駆動し、
  `commit_session`→save→別 `ProjectController::load` まで E2E。
- fault case は `jump_to` で position gap を注入し、fail-stop → verified
  prefix oracle bit 一致 + `verified_sha256` `sha2` 再計算 + commit 層
  recoverable 分類を commit→reload 越しに検証。

## Result

7 test 全て pass（`cargo test -p synvelis-recording --test rcd01_oracle`）。

- **emission-log 構造は有効**: staging media の f32le bytes が emission
  log の bit pattern と bitwise 一致し、channel map・multi-track 分離・
  same-clock 別 seed が全て独立に検証できた。generator 再計算に依存
  しないため、各段の変換が独立に pin される。
- **sentinel `u32::MAX` は安全**: oracle 出力は有限値 `[-1,1)` の f32
  で `u32::MAX` bit pattern（NaN）と衝突しないため、未供給 frame 検出が
  正しく機能した。
- **fault outcome の解釈を訂正**: 当初 finalized outcome を `Partial`
  と期待したが、precedent（`fake_input_drop_fails_stop_and_preserves_
  verified_prefix`）を確認し、writer outcome = durable frontier `Complete`
  + session-scoped capture-fault marker が commit 層 recoverable 分類を
  制御するのが正しい契約と判明。test を precedent に合わせて修正し、
  `Source` 非昇格の honest recoverable 分類を commit→reload 越しに
  検証できた。production 変更は不要だった。

## Follow-ups

- canonical blob publish 脚（`synvelis-player` 管轄）は本 slice 対象外。
  digest adoption 契約の範囲で raw fold digest → staging media verified
  prefix bytes を解決する方式を採用。`publish_recorded` の実機経路は
  既存証拠（`rcd02_blob_digest.rs`・`evidence/sv116/`）を参照。
- rf#628 fixture variant・rf#629 仮想 endpoint EXT-L3・physical・GUI は
  NotRun/NotRequired として matrix に記録し、Issue #115 は open 継続。
