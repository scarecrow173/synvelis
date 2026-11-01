---
id: "SPEC-034Qdwuy2lCglk3HjqtRcs"
type: "spec"
status: "approved"
title: "VS-A Project / Recording Foundation — Stable ID・時間型・Command・Save/Reload"
created: "2026-11-01"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/13"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/ruviform/issues/189"
  defers: []
  deferred-by: []
---

# VS-A Project / Recording Foundation — Stable ID・時間型・Command・Save/Reload

## What Should Be Built

Recording Vertical Slice **VS-A**（Synvelis Issue #13・実行計画 #23 の F0）として、
実Device/GUI/Pluginなしで後続の VS-B〜VS-D・E2E-L1 が共通で参照する最小の
Project Model・Recording Domain 基盤を新設する。

- Synvelis Cargo Workspace の新設（`synvelis-project`、`synvelis-recording`、
  headless `recording-poc` を候補）と検証入口の整備。
- Project / Track / Lane / Clip / Source / Asset と
  Recording Session / Pass / Stream / Segment / Take の**型付き Stable ID**。
- `ClockEpochId`、`SessionFrame`、`SourceFrame`、Project Time を別型として分離し、
  半開区間 `[start,end)` と Checked Arithmetic を備える。
- TempoMap Snapshot/Revision を参照する最小の Mapping 値型（既定 PPQN=960）。
- `ExpectedRevision` による原子的 Project Mutation、単一 Undo 単位の
  Undo/Redo、Save/Reload、SchemaVersion を持つ `ProjectController`。
- Media Blob を Project へ直接書き込まず、Verified Source 参照を表す
  アプリ内部 Port を定義する（Project Save と Media Finalize/Engine Applied は別段階）。

## Why Now

- 実行計画 #23 は F0 を全 E2E 境界（L1/L2/R1〜R3/X）の単一 Owner として
  本Issueに割り当てた。Project Store を録音・再生のそれぞれが二重実装しない
  ための共通基盤が先に必要。
- #17（Project/Asset/Persistence）・#12（Recording §13）・#10
  （Track/Clip/Source）で設計合意は済んでおり、実装 Issue として契約を
  コードと決定的テストに落とす段階にある。
- Ruviform 依存は既定ブランチ継続追従 + Cargo.lock の resolved revision で
  再現性を担保する運用（#28）を最初の Workspace から確立する必要がある。

## Users and Value

- **後続実装 Issue（VS-B/VS-C/VS-D、E2E-L1/L2）:** 共通の Stable ID・
  時間型・ProjectController・Store Port を再利用できる。
- **レビューア / Coordinator:** F0 Gate 項目が `cargo test --workspace --locked`
  で決定的に再現できる。
- **将来の Migration / VCS 機能:** Live/Saved Revision 分離と Stable ID 維持が
  初期 Snapshot から保証される。

## Scope

### In Scope

- Cargo Workspace 新設と Rust toolchain / 検証タスク（mise）整備。
- 型付き Stable ID（Project/Track/Lane/Clip/Source/Asset/
  RecordingSession/RecordingPass/CaptureStream/CaptureSegment/Take）。
  異種IDの取り違えをコンパイル時に防ぎ、Save/Reload で同一性を維持する。
- `ClockEpochId` / `SessionFrame` / `SourceFrame` / Project Time の別型、
  Checked Arithmetic、半開区間 `[start,end)`、TempoMap Snapshot/Revision を
  参照する最小 Mapping 値型。
- Recording Session の状態遷移と Session/Pass/Stream/Segment/Take の
  参照整合性検証（Source≠Clip≠Asset、Raw Source 不変）。
- `ProjectController` による Command 検証・`ExpectedRevision` 競合拒否・
  部分適用しない原子的 Mutation・基本 Undo/Redo・Save/Reload・
  SchemaVersion。
- Verified Source 参照の共通 Port（CommitIdentity/Finalized 参照を VS-D が
  登録できる境界。実際の Commit/Recovery は VS-D 所有）。
- headless `recording-poc` による決定的検証と、初期データ形式・ID 生成・
  参照保持/GC 保留の選択記録。

### Out of Scope

- GUI、実 Input、RT Capture Queue、Media Writer、Journal 形式、
  Comp Playback、MIDI（VS-B 以降 / 他 Issue）。
- Ruviform 側の汎用 Device/MIDI API の変更（編集境界は本リポジトリのみ）。
- Take/Comp の詳細スキーマ・UI、Recording Commit 本体と Recovery 実装
  （VS-D 所有）。
- Project ファイルと Media ファイルのグローバル Atomic Commit の保証。
- 永続化形式の最終公開フォーマットとしての確定（PoC 段階の選択として記録）。

## Requirements

- Ruviform は既定ブランチ追従の git 依存とし、Cargo.lock に resolved
  revision を記録する。特定 SHA の恒久的ハードコード・vendor コピー禁止。
- MSRV / toolchain は Ruviform（toolchain pin 1.99.0 / MSRV 1.89）と照合する。
- Project Save/Load の実施順・検証・データ破損時の扱いを記録する。
- 検証は `mise run` タスク経由（cargo 直接実行は入口にしない）。
- Commit は Conventional Commits・論理単位分割。
- Live `ProjectRevision` は Command 適用で単調増加し、Undo/Redo も
  **新しい Revision へ進めて**巻き戻さない（#17 §2 の不変条件）。
- Recording Session の状態遷移は合法遷移のみ許可し、Session が
  Pass/Stream/Segment/Take の登録を受け付けない状態での登録要求は拒否する。
  Session メタデータの変更も ProjectController の検証付き Command 経由とし、
  Single Writer 原則と二重 Store 禁止を守る。
- `Clip`/`Take`/`Segment` 等の `TimeAnchor`/`SourceFrame`/`SessionFrame`
  演算は Synvelis 側の checked API のみを経由する。Ruviform `Tick`/`TickDuration`
  の `Add`/`Sub` は checked でないため直接演算しない（Ruviform 側の
  checked API 不足は別 Issue として記録・追跡）。
- Project Snapshot は Project PPQN 設定（既定 960）を保持し、Musical 位置は
  `tick + ppqn` 文脈つきで保存する。`Tick` を PPQN 非依存の正本として
  単独永続化しない（#19 方向、Ruviform #189 完了は前提にしない）。
- `SchemaVersion` が対応範囲外のデータは**明示 Reject**（警告読み込み・
  未知必須要素の黙った破棄をしない）。Read-only Degraded / Migration は
  後続 Issue として本 PoC の Reject 動作を記録する。
- 半開区間型は構築時に `end >= start` を検証する。空区間（`end == start`）は
  型レベルでは許容し、Capture Segment 等の意味的に非空を要求する領域では
  ドメイン検証で拒否する。
- `SessionFrame` / `SourceFrame` は非負とし、負値構築・減算結果の負転は
  エラーとする。`ProjectTime` の Musical/Absolute 位置は pre-roll のため
  負値を許容する（#10/#19 の契約）。
- Stable ID の型混同防止は `compile_fail` doctest で検証する
  （コンパイル失敗の表明は実行時テストでは代替できないため、検証方法を
  この要件として固定する）。
- `mise.toml` に `fmt` / `fmt-check` / `check` / `lint` / `test` / `build`
  タスクを定義し、MSRV 実測用の `test-msrv`（`mise exec rust@1.89` 経由）を
  用意する。

## Acceptance Criteria

- [ ] 新しい Cargo Workspace が `mise run check` / `mise run test` で成立し、
  Ruviform 参照は既定 branch の継続追従を妨げず、Cargo.lock の resolved
  revision で `cargo test --workspace --locked`（mise タスク経由）が再現できる。
- [ ] Project/Recording の Stable ID が型安全（異種 ID の代入・比較は
  `compile_fail` doctest でコンパイル時エラーを実証）で、Save/Reload して
  同一性が維持される。`ClockEpochId` を含む全 ID 種で検証する。
- [ ] `SourceFrame` / `SessionFrame` / `ProjectTime` を誤混同できず、
  負構築・オーバーフロー・範囲外演算で Checked Arithmetic が `Err` を返し
  panic しない（負値合法な `ProjectTime` では負位置が許容されることも検証）。
- [ ] `[start,end)` が `end < start` の構築を拒否し、隣接区間
  `[a,b)` `[b,c)` が重複を持たず、Punch/Loop 境界の分割で同一
  Sample が二重帰属しないことをテストする。
- [ ] Recording Session の状態機械が合法遷移のみ許可し、Closed/Finalized
  状態での Pass/Stream/Segment/Take 登録や順序違反を拒否する。
- [ ] Session/Pass/Stream/Segment/Take の全参照リンクを検証できる
  （Take→Session/Pass/Lane/Stream、Segment→Stream/Pass、Stream→Session/Lane/
  ClockEpoch、存在しない参照・別 Session 所属参照の拒否）。
- [ ] 不正な Command Target/Revision を拒否し、Project 状態へ部分適用しない。
- [ ] 基本 Undo/Redo と Save/Reload が決定的にテストされ、Undo/Redo は
  新しい Live Revision を発行して Saved HEAD を汚さない。
- [ ] 対応範囲外の `SchemaVersion` データは明示 Reject で失敗し、Live/Saved
  Revision を区別して未保存編集を消さない。
- [ ] Project は既定 PPQN=960 を保持し、TempoMap Snapshot/Revision を参照する
  Mapping 値型が Tick↔MediaTime を変換できる（120 BPM・4/4・48kHz・
  4小節 Loop = 384,000 Frame ↔ 15,360 Tick の #12 §13.2 数値例を含む）。
- [ ] `recording-poc` が headless で決定的シナリオ（Project 構築 → Recording
  Session 登録 → Save → Reload → Snapshot 同一性）を実行し、2 回の実行で
  同一結果を出力する。
- [ ] Verified Source 参照 Port が Media Blob を Project へ直接書き込まない
  形で定義され、参照状態が Imported ≠ Verified ≠ Committed ≠ Saved と
  区別されること、Project Save と Media Finalize が別段階であることを
  テストで示せる。
- [ ] 初期データ形式・ID 生成・Save/Load 順序と破損時の扱い・参照保持/
  GC 保留の選択が記録される（impl record / 設計書）。

## Deferred Design Concerns

- Physical Serializer の最終形式（JSON/CBOR/SQLite）・Blob Hash Algorithm・
  Directory Bundle 物理配置 — #17 で PoC 保留。本 PoC は論理 Schema を
  阻害しない最小形式を採用し、選択を記録する。
- Recording Journal / WAL と一般 Project Store の Checkpoint 連携 — VS-C/VS-D。
- Cross-restart Undo Log、Retention/GC 実装 — P0 は自動破壊的 GC を行わない
  （保護のみ）。
- TakeGroup/Comp/CompTransition の詳細スキーマ — P1 以降（ID 型の用意のみ）。

## Sources and Relations

- 正本 Issue: [Synvelis #13](https://github.com/scarecrow173/synvelis/issues/13)
  （実装範囲・完了条件・F0 Gate・lockfile 更新条件）。
- 実行計画: [Synvelis #23](https://github.com/scarecrow173/synvelis/issues/23)
  F0 の受入境界と横断 Gate。
- 設計正本: `docs/designs/project-asset-persistence-architecture.md`（#17、
  Single Writer/Revision 分離/Save プロトコル/PAP-01〜18）、
  `docs/designs/configurable-ppqn-musical-time-architecture.md`（#19、
  既定 PPQN=960・時間ドメイン分離）。
- 参照: [#12 Recording §13](https://github.com/scarecrow173/synvelis/issues/12)
  （概念スキーマ・時間分離）、[#10 Track/Clip/Source](https://github.com/scarecrow173/synvelis/issues/10)、
  [#28 Ruviform 追従運用](https://github.com/scarecrow173/synvelis/issues/28)、
  [Ruviform #189 時間契約方向](https://github.com/scarecrow173/ruviform/issues/189)
  （実装完了は待たない）。
