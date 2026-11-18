# sv561 reproduction — op_facts under-report (修正前再現記録)

- 対象 Issue: `synvelis#561`（OPEN 維持）
- 再現 base: `41d85021f29ef16ece8f8606a6ef03d07df817ad`（`origin/main`、
  ブランチ `bugfix/561-opfacts-unverifiable-cascade`）
- 実行環境: Windows (Git Bash)、mise 管理 toolchain、cargo test
  （補助証拠。canonical 検証は `mise run` — `verification.md` 参照）
- 日時: 本 slice 作業時点

## 再現方法

`crates/synvelis-gateway/tests/prepare.rs` に追加した pin test を
修正前コードで実行:

```sh
mise exec -- cargo test -p synvelis-gateway --test prepare sv561
```

結果: **4 failed / 2 passed**（exit 101）。denied 系 4 件は全て
`GatewayOutcome::Completed` で返り、`expect_err` が panic —
つまり prepare が提案を発行し、cascade の観測不能 entity 削除が
`unsupported` にも effects にも現れない under-report を実測した。
positive control 2 件は修正前から Pass（fixture が oracle-clean で
あること、および gate が op family ではなく unobservable entity
存在に連動する設計の裏付け）。

## 観測された under-report（proposal JSON の `unsupported` / `effects`）

### 1. `synvelis.send.set-gain-target` clear（`gain_target: null`）

- fixture: send + sealed `SendLevel` target + その target を指す
  `ModulationAssignment`（`ModulationSource` は Project owner で
  生存する独立 entity）
- 結果: `Completed`、`"unsupported": []`
- effects に現れるのは send の read/write と旧 target の
  `parameter-target` delete のみ。`release_send_level_target` が
  削除する `ModulationAssignment`（`a.target == old`）は
  `ResourceKind` を持たず diff に一切現れない。

### 2. `synvelis.send.set-gain-target` rebind（別 target へ）

- 同一 fixture、新 target は Project owner の canonical spec 付き
  adoptable target
- 結果: `Completed`、`"unsupported": []` — clear と同じく
  assignment 削除が不可視。

### 3. `synvelis.track.remove`

- fixture: lane 非保持の新規 track + `AddTrackStrip`（chain/ports/
  seeded channel targets/master route 生成）→ main route を
  `RemoveConnection` で除去 → strip owner の `Macro`
  `ModulationSource` を追加
- 結果: `Completed`、`"unsupported": []`
- effects に現れるのは track/strip/seeded `parameter-target`
  （Fader/Mute/Solo）の delete。`remove_strip_owned` →
  `remove_control_owned(ControlOwner::Strip)` が削除する
  strip-owned `ModulationSource` は不可視。

### 4. `synvelis.midi.source.remove`

- fixture: `ImportMidiSmf` で作成した clip 非参照の MIDI source
  （`smf_documents` エントリ持ち）
- 結果: `Completed`、`"unsupported": []`
- effects に現れるのは source と note の delete のみ。
  `RemoveMidiSource` apply が無条件で落とす `SmfDocument`
  （`project.smf_documents.remove(source_id)`）は `ResourceKind`
  に variant がなく不可視 — `detail_of` の Source arm も
  `sources`/`midi_sources` のみ参照する。

## 判定根拠（domain 側の発火条件 — `ops.rs` 精読済み）

- `SetSendGainTarget` apply（`ops.rs:2405` 付近）: 旧
  `send.gain_target` が `Some(old)`・新旧異なる・旧 target が
  `ControlOwner::Send(send_id)` + `ParameterRole::SendLevel` の
  とき `release_send_level_target`（`ops.rs:2648`）が発火し、
  lanes（observable）・assignments（`a.target == old ||
  a.depth_source == Some(old)`、**unobservable**）・target 本体
  （observable）を削除する。
- `RemoveTrack` apply（`ops.rs:1629`）: `strip_of_track` が
  strip を返せば `remove_strip_owned`（`ops.rs:2598`）→ strip/
  chain/ports 削除 + `remove_control_owned(ControlOwner::Strip)`
  （`ops.rs:2666`）が strip-owned targets/lanes/**sources**/
  **assignments** を削除。`monitoring.tracks` の decl 削除は
  TrackId key の strictly-1:1 companion であり track delete の
  観測可能境界に包含されるため受理内と判断（判断根拠は
  `docs/designs/issue-561-opfacts-unverifiable-cascade.md`）。
- `RemoveMidiSource` apply（`ops.rs:1727`）:
  `project.smf_documents.remove(source_id)` を無条件実行。
  `SmfDocument` は note 射影を超える provenance（tempo/meta/
  sysex/イベント列）を保持し、観測面に出ない。

## 既存の fail-closed 前例

`RemoveSend` arm（`prepare.rs` `op_facts`）は synvelis#550 で
同型の判定を確立済み: owner の control closure に
send-owned `ModulationSource` または closure に触れる
`ModulationAssignment` があれば `unsupported.push(...)` →
`provisional_gate` が `PreparationRejected { reason:
"unverifiable effects: …" }` で全 caller を fail-closed
（`cb06_strip_550_send_remove_unobservable_cascade_denied` が
pin 済み）。本 slice はこのパターンを残り 3 arm に拡張する。

## review 追加分の red 確認（2026-11-18 追記）

review-protocol の 2 名独立レビューで発見された 2 件も、
修正前に pin test を追加して red を実測した。

### 5. `synvelis.operation.batch` 内の seal→release

- fixture: send（`gain_target: None`）+ adoptable `SendLevel`
  target + その target を指す `ModulationAssignment`
- op: `synvelis.operation.batch` で
  `[set-gain-target → target, set-gain-target → null]`
- 修正前の `op_facts` は全 member を pinned base に対して
  評価するため、member 2 の `previous` は `None` →
  release 条件不成立で `unsupported` 空のまま `Completed`
  を実測。`Composite::apply` は逐次で member 1 の seal を
  member 2 が解放し assignment を削除するため、同じ
  under-report クラスが batch 経由でも成立した。

### 6. `synvelis.track.remove` の `monitoring.tracks` decl

- fixture: `SetTrackMonitor` で decl を持つ track
- 修正前の `RemoveTrack` arm は `monitoring.tracks` を
  検査しないため、decl 削除（strip 有無に関わらず apply が
  無条件実行）を `unsupported` に報告せず `Completed` を
  実測。`TrackMonitorDecl` は `ResourceKind`/`detail_of`
  双方に非存在で、`smf_documents` entry と同型の
  keyed companion — 初版の「strictly-1:1 で受理内」判断は
  `smf_documents` を unverifiable とした同一修正内の判断と
  非整合であり、レビューで指摘されて修正した。

## positive control（修正前 Pass — 過剰拒否がないことの裏付け）

- `sv561_set_gain_target_clear_without_assignment_accepted`:
  sealed target の release が観測可能 entity のみを消す場合は
  prepare が受理され、released target の delete が effects に
  現れる。
- `sv561_track_remove_plain_strip_accepted`: strip-owned
  modulation を持たない strip teardown は受理され、track/strip
  の delete が effects に現れる。

## 結論

`op_facts` の `SetSendGainTarget`・`RemoveTrack`・
`RemoveMidiSource` 3 arm が、domain apply の観測不能 cascade
（`ModulationAssignment`/`ModulationSource`/`SmfDocument` 削除）
を `unsupported` に報告しない under-report を確認。修正は
`RemoveSend` arm の unverifiable 検査パターン踏襲で
`prepare.rs` のみに閉じる（domain の削除動作は既存仕様であり
変更しない）。
