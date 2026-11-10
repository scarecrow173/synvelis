---
id: "SPEC-034UAv3chafPsU2fFyQnIb"
type: "spec"
status: "proposed"
title: "VCS-01 Immutable Snapshot Resolver — Live/Saved/Recovery/VCS read-only schema projection (synvelis#92)"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/7"
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
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/93"
    - "https://github.com/scarecrow173/synvelis/issues/94"
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/7"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-01-snapshot-resolver-fixtures.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/97"
  defers:
    - "VCS-02 Semantic Diff ChangeSet・Change Index・Comparison policy（synvelis#93）"
    - "VCS-03 Git Read Provider の実装（synvelis#94 — 本 spec は VCS Bytes Port 境界のみ定義）"
    - "実 Git remote / 実 OS filesystem / 実 plugin host での検証（VCS-06 #97・OS 別 NotRun/Unsupported 記録）"
    - "Plugin opaque state bytes の実検証 probe（plugin host 未接続のため本 slice では Unknown 固定）"
  deferred-by: []
---

# VCS-01 Immutable Snapshot Resolver — Live/Saved/Recovery/VCS read-only schema projection (synvelis#92)

## What Should Be Built

`synvelis-project` に、VCS 非依存の **Immutable Project Snapshot** を
生成・解決する read-only resolver を追加する。`Live` / `Saved` /
`Recovery` / `VcsStaged` / `VcsCommitted` の 5 locator を
`SnapshotIdentity`（ProjectId + source kind + 固定 locator +
algorithm-tagged SHA-256 content digest + schema revision +
capture generation）へ解決し、`ImmutableSnapshot`（identity /
origin / `Arc<ReadOnlyProjectProjection>` / validation /
asset_resolution / opaque_state_refs / completeness）を返す。

- `SnapshotResolver::capture_live(&ProjectController,
  expected_revision)` は受付時点の Controller から clone を作り、
  Revision 一致を検証した後に read-only 変換する。捕捉後の Live
  編集・Undo・Transport・Engine は snapshot に影響しない。
- `resolve_saved` は `ProjectStore` の HEAD を取得して snapshot
  名 + Bytes を固定し、再読込前後の HEAD/bytes 競合と ACK loss を
  reconcile する。名前が指す bytes が capture 中に変化した場合は
  明示 Failure。
- `resolve_recovery` は Saved HEAD とは別 root の recovery store
  から checkpoint 名で snapshot を読み、HEAD を一切変更しない。
- `VcsStaged` / `VcsCommitted` は **VCS Bytes Port**（本 Issue が
  定義し VCS-03 #94 が実装する read 契約）から fixed bytes を
  受け取る。staged index は可変なので、generation/fingerprint を
  file-set 読取りの前後で検査し、揺れがあれば Conflict/Incomplete
  の明示 Failure。部分 Stage は Equal に見せない。
- 旧 schema（v5 系を含む registry 到達版）の snapshot は
  `crate::migration` の既存 ordered ladder で **read-only
  projection** し、元 bytes・HEAD・Plugin opaque bytes を
  一切書き換えない。独立 migration を複製しない。
- 検証は strict schema version・`validate_references`・
  `check_id_watermark`・size budget・cancel を含む。未知/破損
  schema・Missing Asset・未解析 Plugin は Unknown/Missing/
  Degraded として記録し、無言で Equal にしない。比較可能な既知
  領域の Partial 結果は保持するが、全体 Equal 判定は行わない
  （比較自体は VCS-02 の責務）。

## Why Now

Issue #7（VCS / Semantic Visual Diff 設計正本）の実装 DAG で
VCS-01 は **先行 Issue** であり、VCS-02（#93 Semantic Diff）と
VCS-03（#94 Git Read Provider）の両方が本 resolver の
`SnapshotIdentity` / `ImmutableSnapshot` / VCS Bytes Port を
前提とする。`ProjectController::load()` は Live を置き換え
Undo/Redo を消すため履歴閲覧には使用できず、read-only loader /
projection が別途必要。#84 の ProjectStore/HEAD 移行と #86 の
v5 legacy migration の責務を共有・再利用し、独立 migration を
複製しない。

## Users and Value

- **VCS-02 Semantic Diff（#93）の実装者**: Live/Saved/Recovery/
  VCS の各 snapshot を同一 `SnapshotIdentity` 体系で取得し、
  digest・schema・completeness を比較入力にできる。
- **VCS-03 Git Read Provider（#94）の実装者**: `VcsBytesPort`
  trait が要求する read 契約（fixed locator からの bytes +
  index generation 証拠）を実装すれば、そのまま
  `VcsStaged`/`VcsCommitted` snapshot へ接続できる。
- **VCS-04 History UI（#95）/ VCS-05 Safe Apply/Restore（#96）/
  VCS-06 Fault Matrix（#97）の実装者**: headless fixture で
  検証可能な snapshot 解決の oracle を再利用できる。
- **Project 利用者**: 保存済み・復旧・VCS 固定 revision の
  project を、現在の Live/Undo/Transport を一切変更せずに
  比較・閲覧する基盤が得られる。

## Scope

### In Scope

- `synvelis-project` への additive module: `SnapshotLocator` /
  `FixedSnapshotLocator` / `SnapshotIdentity` / `SnapshotOrigin` /
  `ImmutableSnapshot` / `ReadOnlyProjectProjection` /
  `SnapshotValidation` / `AssetAvailabilityManifest` /
  `OpaqueStateRef` / `SnapshotCompleteness` / `ContentDigest`
  （algorithm-tagged SHA-256）/ `IndexGeneration` /
  `SnapshotResolver` / `SnapshotCancel` / `SnapshotLimits` /
  `SnapshotError`。
- `VcsBytesPort` read 契約（`index_generation` /
  `read_project_file` / `read_blob`）と `VcsLocator` /
  `VcsFileRead` / `BlobRead` / `VcsPortError`。
- `ProjectController` への read-only accessor 追加（snapshot
  が save と同じ `id_watermark` を刻むために必要な generator
  peek の公開。Single Writer/Undo/HEAD の意味論は変更しない）。
- SHA-256 content digest の crate 内実装（依存追加を避けるため。
  `sha2` crate は dev-dependency 追加も含め本 slice では不使用 —
  FIPS 既知解への test vector で検証する）。
- headless fixture tests: Live/Saved/Recovery/VCS fake port・
  v5→現行 schema read-only projection・Unknown/破損 schema・
  Missing Asset・HEAD race・index 途中更新・部分 Stage・
  Duplicate ID/Invalid Reference・Cancelled/Size budget・
  同名 revision の識別・read-only 不変性 oracle。
- `docs/` の spec/design/plan/test-spec/task/impl 文書チェーン。

### Out of Scope

- Semantic Diff / ChangeSet / Comparison policy（VCS-02 #93）。
- Git 実 Provider・history/index/commit の実装（VCS-03 #94）。
  本 slice は `VcsBytesPort` 境界と fake fixture のみ。
- History/Visual Diff UI（VCS-04 #95）、Safe Apply/Restore
  Copy（VCS-05 #96）、横断 fault matrix（VCS-06 #97）。
- 実 Git remote・実 OS filesystem atomicity・実 plugin host・
  GUI・RT thread 上の実行（いずれも非対象、NotRun/Unsupported
  として記録）。
- `ProjectController` Single Writer・Undo/Redo・HEAD への
  read 操作による変更、Git WorkTree/Index の書換、Ruviform
  IO・File Reader 共通契約の複製。
- `synvelis-app` / `synvelis-workspace` / `synvelis-library` /
  `synvelis-gateway` / `synvelis-player` / `synvelis-render` /
  `synvelis-theme` / `recording-poc` / `mcp-poc` の変更
  （依存方向上、いずれも synvelis-project に依存する側であり、
  additive API のみで非回帰を確認する）。

## Requirements

- `SnapshotLocator` は `Live { project_id,
  expected_live_revision }` / `Saved { project_id, head_ref }` /
  `Recovery { project_id, checkpoint }` / `VcsStaged { repository_id,
  project_path, index_generation }` / `VcsCommitted { repository_id,
  fixed_revision, project_path }` を表す。`head_ref` /
  `index_generation` は `Option` で「現在値の解決」と「caller
  固定の期待値」を区別する。
- `SnapshotIdentity` は ProjectId + source kind + **固定後**の
  locator + 実 bytes の algorithm-tagged SHA-256 digest +
  schema revision + capture generation で構成する。
  `ProjectRevision` を全分岐横断のグローバル UID とみなさない。
- `capture_live` は clone → revision 検証 → read-only 変換の順で、
  非 RT 処理が長引いても Live 内容は不変。digest は controller
  の `save` が刻む watermark と同じ値で canonical envelope bytes
  を serialize してから測るため、同一 revision/内容の Live と
  Saved は同一 digest を持つ。
- `resolve_saved` は HEAD を読み snapshot 名+bytes を固定し、
  HEAD 再読・bytes 再読の前後一致を要求する。caller の
  `head_ref` 期待値と実 HEAD が異なる場合・capture 中に HEAD
  が動いた場合・同名 bytes が変化した場合は明示 Failure。
- `resolve_recovery` は recovery root の store から checkpoint
  名の bytes を読み、HEAD/saved 状態を一切参照・変更しない。
- VCS staged は `index_generation` を file-set 読取りの前後で
  サンプリングし、不一致なら `IndexChanged`、参照 asset blob が
  index に無ければ `PartialStage` の明示 Failure。committed は
  fixed revision が内容を不変に固定するため generation は
  provider 定義の安定値を要求する。
- 検証は snapshot envelope の strict parse（`deny_unknown_fields`
  継承）→ schema version gate（`migration` ladder が到達する
  版のみ。未対応・新版は `UnsupportedSchema`）→ project_id
  一致 → `validate_references` + `check_id_watermark` →
  size budget → cancel checkpoint の順。asset/blob・plugin
  opaque state は確認できた場合のみ `Known`、それ以外は
  `Missing`/`Unknown` で `completeness: Partial` を記録する。
- すべての resolve は read-only: `ProjectController`・
  `ProjectStore`（HEAD 含む）・VCS index/worktree・RT
  callback の状態を変更しない。

## Acceptance Criteria

Issue #92 本文の受入条件をそのまま採用する。

- [ ] VCS なしの Live vs Saved / Recovery 比較取得。捕捉後の
  Live 編集・Undo・Transport・Engine は不変。
- [ ] v5 Snapshot と新 Schema を read-only 変換。未知/破損
  Schema と Missing Asset を無言で Equal にしない。元
  Snapshot/HEAD/Plugin opaque bytes は書換なし。
- [ ] Saved HEAD race、VCS Index 書換中、Project path/Asset
  bundle の部分 Stage、Duplicate ID/Invalid Reference、
  Cancelled/Size budget で明示 Failure。
- [ ] 同名 Revision でも ProjectId/固定 VCS commit/Schema/
  Digest で識別。SHA256 衝突を前提とした危険な ID 統合を
  しない。
- [ ] ProjectController 単一 Writer・Undo/Redo・
  Recording/Playback/Export/Asset Library #86・Store #84 への
  非回帰、RT callback 非阻害。

headless fixture で検証可能な行のみを本 slice の検証対象とし、
実 OS・実 Git remote・GUI は `NotRun`/`Unsupported` として
PR・impl record に明記する（Issue #92 検証方法の指示どおり）。

## Deferred Design Concerns

- Semantic Diff Engine・ChangeSet・semantic fingerprint・
  comparison policy | VCS-02 (#93) の責務 | #93 着手時 |
  fingerprint と raw byte digest の混同。
- Git Read Provider 実装・credential・LFS・submodule・
  object format 差 | VCS-03 (#94) の責務 | #94 着手時 |
  `VcsBytesPort` の実装が fixture と乖離する。
- Plugin opaque state bytes の実 probe | plugin host 未接続 |
  plugin host slice 着手時 | Resolved binding が恒久的に
  `Unknown` のまま。
- 実 Git/実 FS/実 OS での atomicity・capability 証拠 |
  headless fixture の範囲外 | VCS-06 (#97) | Mock 証拠を
  実 FS 証拠に換算する誤り。

## Sources and Relations

- Issue #92（対象 Issue、型付き契約・受入条件の正本）。
- Issue #7（VCS / Semantic Visual Diff 設計正本: Snapshot
  4 状態・型付き契約・error 分類・実装 DAG）。
- Issue #17 / `docs/designs/project-asset-persistence-architecture.md`
  （Canonical Snapshot・Saved/Recovery/VCS 境界・HEAD publish）。
- Issue #84 / `docs/specs/syn-io-02-snapshot-head-staged-publish.md`
  （ProjectStore/HEAD staged publish 契約）。
- Issue #86 / `docs/specs/alib-02-streaming-verified-import.md`・
  `docs/specs/alib-02-v5-snapshot-compat.md`（asset/sha256・
  v5 snapshot 互換の責務境界）。
- Issue #94（VCS-03 Git Read Provider — `VcsBytesPort` の
  将来実装者。実装は別 Issue）。
- Issue #97（VCS-SNAP-01〜06 fixture/証拠台帳の横断 Gate）。
- `crates/synvelis-project/src/{controller,store,migration,project}.rs`
  （既存の Single Writer・HEAD store・migration ladder・
  reference oracle — すべて再利用し変更しない）。
