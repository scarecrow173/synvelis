---
id: "SPEC-iciMnP7AaIMP2qKQPCoAId"
type: "spec"
status: "approved"
title: "VS-D Recording Commit / E2E Vertical Slice — Idempotency・Undo・Reload"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/synvelis/issues/23"
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
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/project-asset-persistence-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/13"
    - "https://github.com/scarecrow173/synvelis/issues/14"
    - "https://github.com/scarecrow173/synvelis/issues/15"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/21"
    - "https://github.com/scarecrow173/synvelis/issues/22"
    - "https://github.com/scarecrow173/synvelis/issues/26"
    - "https://github.com/scarecrow173/ruviform/issues/187"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/192"
  defers: []
  deferred-by: []
---

# VS-D Recording Commit / End-to-End Vertical Slice — Idempotency・Undo・Reload

## What Should Be Built

Recording Vertical Slice **VS-D**（Synvelis Issue #16・実行計画 #23 の
**E2E-R 完了 Gate**）として、VS-A〜VS-C を統合し、Mock Input →
sample-accurate Gate → bounded RT handoff → Staging Writer/Journal →
Finalize → **Project Controller への冪等 Commit** → Save/Reload/
Undo/Redo までを End-to-End で実証する。

- `CommitRecordingCommand`（`RecordingOp::CommitRecording`）が
  `RecordingCommitIdentity`・Expected Project Revision・Finalized
  Sources/Takes/Placement を検証し、**単一 Project Mutation** として
  登録する。PCM を Project Command へコピーしない（Media Byte は
  staging file が所有、Command は参照と digest のみを運ぶ）。
- AssetId/SourceId/TakeId/ClipId（および commit が新規作成する
  Pass/Segment の Stable ID）は **Commit 前に Staging の
  `commit-manifest.json` へ確保・保存**し、Restart/Retry で再採番しない。
- Commit 判定は Project 内の Applied Commit Registry（
  `recording.applied_commits`、永続化）が担う。同一 CommitIdentity +
  同一 Content は `AlreadyApplied`（再適用なし・新 revision なし）、
  同一 Identity + 異内容は Integrity Conflict、Expected Revision
  競合は Recoverable Conflict（`StaleRevision`）として分類する。
- Session の Normal/Loop/Punch 結果に対し、Stream/Segment/Take/Pass/
  Source/Clip の参照・Project 配置・Clock Evidence を整合させる。
  初回空 Lane は正常 Capture 後に自動 Clip するが、既存 Comp
  （clip 保有 Lane）を暗黙に置き換えない。
- 原則 **1 Recording Session = 1 Project Undo 単位**。Undo は
  Project 参照を外すが Staging/Finalized Media を直ちに削除しない。
  Redo は同一 Stable ID と Source Mapping を復元する。
- Journal（commit marker）と Project Commit の前後どちらで Crash
  しても、CommitIdentity が二重登録を防ぐ。
- Storage Failure/Queue Overflow/Project Revision Conflict/不正
  Source Range では Source/Take/Clip の部分適用を作らない。
  回復可能 Media（Partial/missing finalize）と Project 登録済み状態を
  別構造で扱い、Recovered→Committed の自動昇格をしない。
- Headless `recording-poc` が結果比較とテストレポートを出す。
  実 DSP/GUI が動作するように偽装しない。

## Why Now

- VS-A/B/C が main に統合され、F0 基盤（Stable ID/Command/Save-Reload）
  ・RT Capture 契約・Staging/Journal/Recovery が確定した。E2E-R の
  完了判定は「Finalized Media → 冪等 Project Commit → Undo/Reload」
  の実測を要求する（#16 Execution v1.0・#23）。
- #17 §8 の Orphan 保護・Applied Commit Identity Registry・Media
  Recovery と Project Recovery の分離は VS-D が初の実装対象。
  `FinalizedSourceRef` の引き渡し境界は VS-C で確定済み。
- E2E-X（#26）は E2E-L（#25）と本 Gate の両合格後にのみ開始するため、
  ここで証拠を確定させなければ全体ゲートが進まない。

## Users and Value

- **レビューア / Coordinator:** VS-001〜015 を `cargo test --workspace
  --locked`（mise 経由）と headless `recording-poc` で決定的に再現
  できる。Crash 前後・Revision Conflict・Undo/Reload の各状態を
  実測レポートで追跡可能。
- **後続の Playback/Comp/Export（#26・v0.7.1）:** Project 内の
  Source/Take/Clip/Placement・Clock Evidence の正本参照を再利用
  できる（Replay/Export 実装は #26 が所有）。
- **#17 設計:** Applied Commit Registry・Orphan 保護・Undo≠物理
  削除の初の実装検証。

## Scope

### In Scope

- `synvelis-project`: `RecordingRegistry` への
  `applied_commits` 追加（永続化・`#[serde(default)]` 後方互換）、
  `ProjectController` の外部 ID 確保入口（manifest 用 stable ID
  発行・watermark 整合）、`CommitRecording` op が利用する検証 API。
- `synvelis-recording` 新規 `commit` モジュール:
  `RecordingCommitManifest`（commit-manifest.json・ID/Content Hash
  確定済み payload）・`CommitRecording` op（validate 全検査 →
  apply 全登録 + registry 記録 + generator reseed）・commit driver
  （finalized ref 収集 → manifest 確保 → status 分類 → 単一
  mutation 適用 → commit marker 公開 → 結果 report）。
- 回復可能 Media（`FinalizedOutcome::Partial`/未 finalize/
  faulted stream）の分離構造（commit 対象外として report する）。
- `recording-poc` の `vsd` シナリオ（record → finalize → commit →
  save → reload → undo/redo → retry）と決定的レポート出力。
- Acceptance Matrix VS-001〜VS-015 の実測カバレッジ（`tests/vs_d.rs`
  と既存 vs_b/vs_c 試験の再実行含む）。

### Out of Scope

- 実 Device Input/Duplex/Latency の Backend 精度（ruviform #187/
  #188）、Timed MIDI、Plugin Host、Audio Comp DSP/UI。
- 正式な Media GC/Retention（Undo 後の物理削除は行わないが、
  本 Gate は GC 実装ではない）、公開 Project 互換形式。
- Recording Source の Playback/Export 統合（E2E-X #26 が所有）。
- 全 OS/電源断での durability 保証（capability 記録のみ）。
- Journal Checkpoint と一般 Project Store の連携（#17 §15 未確定）。

## Requirements

- Commit は `synvelis-recording` の `RecordingOp`（`Operation` 実装）
  として `ProjectController` に単一適用される。`validate` は全ての
  参照・ID 衝突・範囲・Clock Evidence・セッション状態を **副作用
  なしに全検査**し、`apply` は全 Entity 登録・Session Finalized 化・
  registry 記録を一括で行う（部分適用なし）。
- `RecordingCommitIdentity` は `RecordingSessionId` + payload 内容
  Hash（canonical serialization の CRC32 fold）で構成する。
  registry key は session、比較は content hash で行う。
- commit-manifest.json は staging session dir に tmp→sync→rename で
  原子公開し、既存なら payload/identity が再計算と一致するか照合
  （不一致は Integrity Conflict）。Crash 前後で manifest が生きる
  限り ID は再採番されない。`apply` 内部で generator を manifest 内
  最大 raw ID 以後へ reseed し、watermark との不整合による再発行を
  防ぐ。
- Undo/Redo は Project 参照のみを対象とし、staging media/journal/
  finalized/commit marker の物理削除を伴わない。
- RT パスで Allocation/Blocking/危険 Drop を行わない（VS-B 契約を
  不変とし、commit/driver は非 RT で完結する）。
- Clock Epoch 断絶に対して偽の連続録音を作らない（engine の
  FAULT_EPOCH_DISCONTINUITY fail-stop を維持し、faulted/partial
  stream は commit 対象外）。
- Missing/Partial Media は Recovered と Committed に自動昇格しない。
  Commit 対象は `FinalizedOutcome::Complete` の stream のみ。
- 検証は `mise run <task>` 経由、commit は Conventional Commits・
  論理単位。

## Acceptance Criteria

- [ ] VS-001: 1 Track Normal で全 Frame/Channel/Checksum 一致（staging
  media の bit 検算と committed segment/source range の一致）。
- [ ] VS-002: 2ch・2Track で同 Clock の Frame 整合性を維持。
- [ ] VS-003: Block 途中 Punch で保存長と正確な Placement 一致。
- [ ] VS-004: 3 Pass Loop で Pass/Take・Session Frame 単調増加と
  Project Loop Mapping 一致。
- [ ] VS-005: Punch+Loop の複数 Segment が正しく Source へ対応。
- [ ] VS-006: Queue Overflow が RT 非 Blocking/欠落を明示し、Heap
  Drop が RT 上で発生しない。当該 stream は commit 対象外。
- [ ] VS-007: Writer Error でも確認済 Media を Partial Recovery へ
  渡せる（committed へ自動昇格しない）。
- [ ] VS-008: Journal Truncation は Last Valid Checkpoint まで復旧。
- [ ] VS-009: Project Commit 直前 Crash 後の再試行に成功（manifest
  の同一 ID・同一 Content で適用される）。
- [ ] VS-010: Project Commit 直後（marker 未記録）Crash からの再試行
  が重複生成しない（AlreadyApplied）。
- [ ] VS-011: Undo/Redo で Project 参照と Stable ID を復元（media
  非削除）。
- [ ] VS-012: Save/Reload で Recording Session/Source/Take/Clip/
  Placement 整合（registry も永続化）。
- [ ] VS-013: Clock Epoch 断絶に対して偽の連続録音を作らない
  （当該 stream は Recovered 扱い）。
- [ ] VS-014: Project Revision Conflict で不正な部分適用がない
  （StaleRevision → 部分適用なし → retry 成功）。
- [ ] VS-015: RT Allocation/Blocking/危険 Drop 計測の要件を満たす
  （vs_b 計測試験の維持 + commit 経路の非 RT 性）。

## Deferred Design Concerns

- 正式 Media GC/Retention、複数 process の同時 commit、Project Lock、
  Autosave/Recovery Root 連携（#17 §15）。
- `CommitIdentity` の session 超えた再録音（同一 session への追記）は
  現行「1 session = 1 commit」モデルの外。後続 Issue で検討。
- Partial media の明示的 commit（現行は Complete のみ対象）。
- E2E-X (#26) に向けた Source/Clip の Playback/Export 参照解決。

## Sources and Relations

- 正本 Issue: [Synvelis #16](https://github.com/scarecrow173/synvelis/issues/16)
  （実装範囲・Acceptance Matrix VS-001〜015・非対象・完了定義）。
- 実行計画: [Synvelis #23](https://github.com/scarecrow173/synvelis/issues/23)
  （E2E-R 完了 Owner・追加 Gates・証拠要件）。
- 設計正本: [Synvelis #12](https://github.com/scarecrow173/synvelis/issues/12)・
  `docs/designs/project-asset-persistence-architecture.md`（#17・
  CommitIdentity/Stable IDs/Asset Manifest/Orphan 保護/Crash Matrix）。
- 依存 spec: `docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md`・
  `docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md`・
  `docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md`。
- 関連合意: [Synvelis #20](https://github.com/scarecrow173/synvelis/issues/20)・
  [#21](https://github.com/scarecrow173/synvelis/issues/21)・
  [#22](https://github.com/scarecrow173/synvelis/issues/22)・
  [#26](https://github.com/scarecrow173/synvelis/issues/26)。
- 非依存を明記: Ruviform #187/#188（実 Device）・#191（WAV Writer）。
