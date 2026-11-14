---
id: "SPEC-034X6inBx3ortD0y20pxqk"
type: "spec"
status: "approved"
title: "Project Model — Track/Lane/Clip/Source/Take/Time, non-destructive editing, stable ID, save/reload contract"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/10"
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
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/vcs-05-apply-restore-headless.md"
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/issue-86-verified-import-wiring.md"
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/specs/rcd-04-midi-undo-stableid.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/ed-a-editing-model-design.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
    - "docs/designs/mda-comp-effective-projection-placement-design.md"
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
  refines:
    - "docs/designs/project-asset-persistence-architecture.md"
  refined-by: []
  derives-from:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/17"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/65"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/277"
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "https://github.com/scarecrow173/synvelis/issues/279"
    - "https://github.com/scarecrow173/synvelis/issues/280"
    - "https://github.com/scarecrow173/synvelis/issues/284"
    - "https://github.com/scarecrow173/synvelis/issues/330"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/219"
    - "https://github.com/scarecrow173/ruviform/issues/294"
  defers: []
  deferred-by: []
---

# Project Model — Track/Lane/Clip/Source/Take/Time, non-destructive editing, stable ID, save/reload contract

## What Should Be Built

Synvelis Issue #10 の Project Model を、既存の `synvelis-project` 実装と
承認済みの上流契約（VS-A stable ID/command/save-reload、TIM-01 configurable
PPQN、ED-A/ED-B 編集モデル、MD-A TakeGroup/Comp、VCS-01/02/05 snapshot/diff/
apply/restore、ALIB asset provider）に整合する形で文書化・検証する。具体的には:

1. Track/Lane/Clip/Source/Take/Timeline の責務分界と、Audio/MIDI/Instrument/
   Bus/Return/Folder を Track+Capabilities で表現するテンプレート契約を定義する。
2. Source/Asset/Clip/Take の identity・provenance・不変性と、Clip の非破壊
   編集（placement・source region・playback mapping・trim・slip・stretch・
   loop・split・fade・gain・mute・duplicate）の契約を定義する。
3. Musical/Absolute anchor、Natural/Musical/Stretch と TempoMap/PPQN 編集の
   対応規則、複数 Time domain の明示的変換契約を定義する。
4. stable ID が rename/move/duplicate/split/undo/redo/save/reload/VCS
   diff/apply/restore/migration をまたいで維持される規則と、
   `AmbiguousIdentity` の拒否規則を定義する。
5. ProjectController 単一 writer・expected-revision CAS・atomic apply・
   1 gesture = 1 undo・revision 会計（Project/Saved/Recovery/VCS/Execution
   の分離）を定義する。
6. Save/Reload（immutable snapshot・HEAD publish・stale save 防止・crash
   recovery・unknown schema 拒否・migration ladder・blob integrity）と
   remote asset の fetch 失敗/取消・offline 再生の契約を定義する。
7. GUI/RT/Plugin に第二の Project 正本を置かないこと、および Ruviform 共通
   API へ DAW 固有 Track/Clip 型を流入させないことを設計契約として明記する。
8. 既存 `synvelis-project` の test fixture 群を Case ID 付きで台帳化し、
   `evidence/sv10/` に Expected/Observed/NotRun を記録する。

本スライスは設計契約の文書化と headless fixture 照合が主目的であり、
`crates/synvelis-project/` の実装変更は別 Issue が Owner。本 slice では
crate を変更しない。

## Why Now

Issue #10 は Recording（#12）・Timeline tools（#11）・Snapshot/Save/Recovery/
stable ID（#17）・Execution Graph（#18）・Automation（#6）・Routing（#104）・
Asset Library（#9）・VCS（#7）が共有する Project Model の正本を要求する。
`crates/synvelis-project` には既に Track/Lane/Clip/Source/Take/Time/Control・
CAS・snapshot/migration/VCS の実装と test が存在するが、Issue #10 が要求する
「責務分界・非破壊編集・時間 domain・stable ID・単一 writer・save/reload・
失敗契約」を一つの canonical 文書鎖として固定する記述がない。本スライスで
既存実装と承認済み上流契約を統合した設計契約を確立し、後続 Issue が参照する
正本を提供する。

## Users and Value

- Synvelis 実装者（#11/#12/#17/#18 等）: Track/Lane/Clip/Source/Take/Time の
  責務と編集・永続化・失敗契約を誤解なく実装できる正本を得る。
- レビュアー/Coordinator: headless fixture で検証済みの契約と、実機・実
  plugin・実 storage で未検証の領域を区別できる evidence 台帳を得る。
- 後続 Gate（#112/#114 GUI、#176 plugin、#294 RT、#84 storage durability）:
  NotRun 項目の引き継ぎ先が明確になる。

## Scope

### In Scope

- `docs/` 配下の project-model 文書チェーン 7 文書（spec/design/plan/task/
  test-spec/impl-ir/impl-exp）と `evidence/sv10/` 台帳。
- `crates/synvelis-project/` 既存 fixture/test の契約照合（実装変更なし）。
- Track/Lane/Clip/Source/Take/Timeline の責務分界と編集・時間・ID・
  永続化・失敗契約の設計記述。
- Ruviform/Synvelis 境界と第二正本禁止の明文化。

### Out of Scope

- `crates/synvelis-project/` を含む本番 crate の実装変更（別 Issue が Owner）。
- GUI Clip/Track 編集の実装・実機検証（#112/#114 系 gate）。
- 実 plugin・実 RT・実 storage durability の検証（#176/#294/#84/#330 系 gate）。
- Ruviform 側の Time/Media/Graph API の変更。
- Project schema version の新規 bump や serializer 形式の変更。

## Requirements

- PM-R-01: Track/Lane/Clip/Source/Take/Timeline の責務と所有者を定義し、
  Audio/MIDI/Instrument/Bus/Return/Folder を Track+Capabilities で表現する。
  Folder の表示上の階層は Routing を変更しない。
- PM-R-02: Source/Raw Take/native MIDI content は不変で共有可能とし、Clip の
  placement・trim・gain・fade・loop・playback mapping・comp 選択は非破壊の
  参照/導出とする。
- PM-R-03: 1 つの Source/Project Asset を複数 Clip が参照しても Clip 固有の
  編集は独立し、Independent Duplicate と明示 Linked の意味を区別する。
- PM-R-04: Musical/rational beat・project PPQN（既定 960）・native MIDI
  tick/division・audio source frame・media/absolute time・engine frame・
  device clock・recording session frame の各 domain と、明示的変換規則を定義
  する。異なる PPQN 間の整数 Tick の暗黙比較を禁止する。
- PM-R-05: TempoMap/PPQN 編集は元録音 PCM・raw MIDI bytes・source-native
  timestamp を変更せず、変換は明示的かつ checked arithmetic で行う。
- PM-R-06: Move/Trim/Slip/Stretch/Loop/Split/Duplicate/Fade は非破壊で、
  Project 再読込・Undo/Redo 後に同じ配置と Source Mapping を再現する。
  Split は新 ClipId と DerivedFrom lineage を保存し、Undo で元 ClipId を
  復元する。Rename/Move は ID を維持する。
- PM-R-07: TakeGroup/Comp は candidate を参照し所有せず、Adopt は単一の
  CAS+Undo 操作とし、scope 外の非破壊挙動を維持する。
- PM-R-08: Creative Branch は typed VariantScope と versioned
  `BranchContentRef` を持ち、branch-local ID が duplicate/undo 後に衝突・
  誤再 bind しない。
- PM-R-09: ProjectId/TrackId/LaneId/ClipId/SourceId/AssetId/TakeId/NoteId/
  ConnectionId 等の stable ID は rename/move/duplicate/split/undo/redo/
  save/reload/VCS diff/apply/restore/migration をまたいで維持される。同一
  number の曖昧 identity は `AmbiguousIdentity` として拒否し、自動 merge
  しない。stable semantic identity は Blob SHA256・VCS commit OID・Asset
  RemoteLocator と区別する。
- PM-R-10: ProjectController を唯一の project truth とし、GUI/RT/Plugin に
  第二の正本を置かない。command は expected revision と対象 ID を含む操作を
  持ち、全件検証→scratch apply→atomic swap→1 undo unit の順で行う。
  CAS conflict・部分適用・誤対象適用を許さない。
- PM-R-11: live revision は単調増加し、Undo/Redo も新 revision を発行する
  （revision を巻き戻さない）。Project revision と Saved/Recovery/VCS/
  Execution の各 revision を分離する。
- PM-R-12: Save は immutable snapshot + HEAD publication とし、stale save・
  crash 後の不整合・unknown schema・破損 snapshot・migration 失敗を明示的に
  扱う。migration は順序付き ladder で read-only projection とし、source
  bytes・HEAD・assets を書き換えない。
- PM-R-13: Remote Asset の fetch 失敗/取消は Project Manifest と Clip に
  不完全データを残さず、取得済み Asset は offline で再生可能とする。
- PM-R-14: missing Source/Asset・corrupt data・stale CAS・invalid range・
  overflow・unsupported mapping・incomplete remote fetch・malformed
  snapshot・migration failure の各失敗を型付きで区別し、部分適用しない。
- PM-R-15: Arrangement の表示操作だけでは Project 編集や Audio Graph の
  不要な再構築を発生させず、DSP 反映待ち/失敗を正しく可視化できる。
- PM-R-16: Ruviform 共通 Time/Media IO・Graph API に DAW 固有の Track/Clip
  schema 型を流入させない。再利用可能な time/media/graph primitive のみを
  Ruviform が提供する。

## Acceptance Criteria

Issue #10 の受入条件を PM-AC として転記・正規化する。各 AC の検証写像は
`docs/test-specs/project-model.md`（PM-T）と `evidence/sv10/` を参照。

- [ ] PM-AC-01: Audio/MIDI/Instrument/Bus/Return/Folder の Template を
  Track+Capabilities で表現でき、Folder の見かけだけで Routing が変わらない。
- [ ] PM-AC-02: 1 つの Source/Project Asset を複数 Clip が参照しても、
  Clip 固有 Trim/Gain/Placement が独立する。
- [ ] PM-AC-03: Independent MIDI Duplicate 編集では他 Clip が変わらず、
  明示 Linked では共有 Source 変更が反映する。
- [ ] PM-AC-04: Musical/Absolute Anchor、Natural/Musical/Stretch 設定に
  対する TempoMap 編集の挙動が予測可能で、元録音 PCM は変更されない。
- [ ] PM-AC-05: 960 Tick→Sample 変換、サンプルレート変更、Tempo 変更区間、
  Loop 境界、負 Time/Pre-roll で境界の欠落/二重発火が起きない。
- [ ] PM-AC-06: MIDI 録音の高精度 Timestamp を意図せず整数 Tick に破壊的
  量子化しない。
- [ ] PM-AC-07: Move/Trim/Slip/Stretch/Loop/Split/Duplicate/Fade が非破壊で、
  Project 再読込・Undo/Redo 後に同じ配置と Source Mapping を再現できる。
- [ ] PM-AC-08: Split 後は新 ClipId と DerivedFrom が保存され、Undo で元
  ClipId が復元する。Rename/Move は ID を維持する。
- [ ] PM-AC-09: 重複 Audio/MIDI/Take/Automation は各 Lane の明示評価規則に
  従い、勝手に素材削除/上書きしない。
- [ ] PM-AC-10: 複数 Clip 同時編集、Ripple、Command の競合は Project
  Controller で全件検証し、誤対象への部分適用を行わない。
- [ ] PM-AC-11: Remote Asset の Fetch 失敗/取消では Project Manifest と
  Clip に不完全データを残さない。取得済み Asset はオフラインでも再生可能。
- [ ] PM-AC-12: 保存・再起動・VCS Revision A/B で同じ安定 ID、Source
  Provenance、Meaningful Diff を再現できる。
- [ ] PM-AC-13: Arrangement の表示操作だけでは Project 編集や Audio Graph
  再構築が不要に発生せず、DSP 反映待ち/失敗を正しく可視化できる。

## Deferred Design Concerns

- 実 GUI Clip/Track 編集・実 plugin・実 RT・実 storage durability の受入は
  #112/#114/#176/#294/#84/#330 系の別 Gate に委譲する（本 slice は headless
  fixture 照合まで）。
- Creative Branch の UI/編集 workflow 実装は #277/#278 系の別 slice。
- Take/Comp の product E2E・RT engine sample 一致は RCD-04 系の後続 slice。
- PPQN の UI/preview 詳細は TIM-01 系の既存 slice が Owner。

## Sources and Relations

- Issue #10（本 spec の source）。上流契約として VS-A（#2 系 stable
  ID/command/save-reload）、TIM-01 configurable PPQN、ED-A/ED-B 編集
  モデル、MD-A TakeGroup/Comp、CTL-01/02 control owner、VCS-01/02/05
  snapshot/diff/apply/restore、RC-D staged publish/migration、ALIB asset
  provider、Issue #86 verified import wiring、SYN-IO-02 HEAD CAS、
  PC-C4-05 persisted plugin instance、RC-D-04 MIDI undo/stable ID、
  TIM-02 SMF import/export を depends-on/related に記録した。
- 関連 Issue: #7（VCS）、#11（Timeline tools）、#12（Take/Comp/Source）、
  #17（Snapshot/Save/Recovery/stable ID）、#18（Execution Graph）、
  #19（PPQN/time）、#65（media-domain editing）、#101（plugin instance
  identity）、#104（Routing/Port）、#277/#278（Branch/Lineage）、
  #279/#280/#284（Audio/MIDI/Arrangement gate）。
- Ruviform 側の関連 Issue: #189（configurable PPQN/time migration）、
  #219/#294（cross-consumer/RT gate）。references に記録。
