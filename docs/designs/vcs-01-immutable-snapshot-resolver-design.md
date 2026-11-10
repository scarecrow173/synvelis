---
id: "DESIGN-034UAwTwlzWbToxqWtFM2B"
type: "design"
status: "approved"
title: "VCS-01 Immutable Snapshot Resolver — crate 内完結設計"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/7"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-project/src/snapshot"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs01_snapshot.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
  implemented-by: []
  depends-on:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/93"
    - "https://github.com/scarecrow173/synvelis/issues/94"
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "https://github.com/scarecrow173/synvelis/issues/7"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/97"
  defers:
    - "VCS-02 Semantic Diff ChangeSet（synvelis#93）"
    - "VCS-03 Git Read Provider 実装（synvelis#94）"
    - "Plugin opaque state bytes の実 probe（plugin host 未接続）"
    - "実 Git/実 OS/実 plugin host での検証（VCS-06 #97）"
  deferred-by: []
---

# VCS-01 Immutable Snapshot Resolver — crate 内完結設計

## Context

spec `vcs-01-immutable-snapshot-resolver` の headless slice を
`synvelis-project` 内の additive module で実現する。Issue #7 の
実装 DAG で VCS-01 は先行であり、VCS-02 (#93)・VCS-03 (#94)
が本設計の型を前提とする。

既存事実（調査済み）:

- `ProjectController` は Single Writer。`live()`/`live_revision()`/
  `saved()` は read-only で、`apply` は scratch clone →
  `validate_references` → watermark stamp → swap の atomic
  commit。`load()` は Live 置換 + Undo/Redo clear のため
  履歴閲覧に使用不可（Issue #92 明記）。
- `ProjectStore` port は `save`/`head`/`read_snapshot` のみ。
  `DirectoryBundleStore` は `snapshots/rN.json` + `HEAD` の
  staged publish（`ruviform-io-file`）で、snapshot bytes は
  immutable 名だが `id_watermark` 差分の `Refresh` collision
  で**同名 bytes が更新され得る**。`InMemoryStore` も同じ
  collision 規則を持つ。
- `SnapshotEnvelope { schema_version, project_id, revision,
  project }` は `deny_unknown_fields` で strict parse。
  `SCHEMA_VERSION = 9`。`migration::migrate_to_current` が
  v5→v9 の ordered read-only ladder（source bytes 不変）を
  既に提供し、`load()` が `validate_references` +
  `check_id_watermark` の oracle を共有している。
- `Project` は `Clone` 可能な pure data aggregate。永続化
  される全参照（assets/sources/clips/notes/routing/control/
  recording/comp/midi/smf_documents）は map 内で完結し、
  外部 bytes を持たない。`Asset.digest`・`Source.content_digest`
  は opaque string で、実 media bytes は本 crate の管理外。
- `ProcessorBinding::Resolved(PluginRef)` は descriptor/
  version/capability_revision のみで opaque state bytes を
  持たない。`Unresolved` は明示的な未解決 binding。
- SHA-256 実装は workspace に `sha2 = "0.10"` が存在するが、
  `synvelis-project` の依存木には無い。タスク制約により
  依存追加は dev-dep 限定のため、crate 内に最小の
  SHA-256（FIPS test vector で検証）を実装する。
- doc-driven-dev-graph router は main 既存の broken relation
  （他 doc の `defer`/`depends-on` 参照切れ）で `focus-required`
  blocked を返す。本 chain は新規 focus として進め、既存
  broken relation は本 slice の対象外として記録する。

## Decision

### 1. Module 構成

`crates/synvelis-project/src/snapshot/` を新設し、関心毎に分割:

| file | 内容 |
| --- | --- |
| `mod.rs` | `SnapshotResolver`・resolve orchestration・`SnapshotError`・`SnapshotCancel`・`SnapshotLimits` |
| `types.rs` | `SnapshotLocator`/`FixedSnapshotLocator`/`SnapshotIdentity`/`SnapshotOrigin`/`ImmutableSnapshot`/`ReadOnlyProjectProjection`/`SnapshotValidation`/`AssetAvailabilityManifest`/`OpaqueStateRef`/`SnapshotCompleteness`/`IndexGeneration`/`ContentDigest` |
| `digest.rs` | SHA-256 compression + `ContentDigest` の parse/format（FIPS vector test は `#[cfg(test)]` unit test） |
| `vcs.rs` | `VcsLocator`/`VcsFileRead`/`BlobRead`/`VcsPortError`/`VcsBytesPort` |

`lib.rs` に `pub mod snapshot;` と主要型の re-export を追加
（既存 re-export 群と同じく flat に公開）。

`ProjectController` には **read-only accessor 1 本のみ**追加:

```rust
/// The watermark a snapshot of the current live state would carry —
/// the generator's issuance position, identical to what
/// [`ProjectController::save`] stamps into the envelope.
#[must_use]
pub fn snapshot_id_watermark(&self) -> u128 {
    self.generator.peek()
}
```

Single Writer/Undo/HEAD の意味論・apply/load/save の動作は
一切変更しない（additive read API）。

### 2. 型付き契約

```rust
/// Resolution request — the *unfixed* locator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SnapshotLocator {
    Live { project_id: ProjectId, expected_live_revision: ProjectRevision },
    /// `head_ref: None` resolves the store's current HEAD;
    /// `Some(name)` pins an expected HEAD snapshot name.
    Saved { project_id: ProjectId, head_ref: Option<String> },
    /// `checkpoint` names a snapshot inside a *recovery-rooted*
    /// store — never the saved HEAD.
    Recovery { project_id: ProjectId, checkpoint: String },
    /// `index_generation: None` resolves the current staged state;
    /// `Some(g)` requires the stable captured generation to equal `g`.
    VcsStaged { repository_id: String, project_path: String,
                index_generation: Option<IndexGeneration> },
    VcsCommitted { repository_id: String, fixed_revision: String,
                   project_path: String },
}
```

```rust
/// Where a resolved snapshot came from (identity.source / origin).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotOrigin { Live, Saved, Recovery, VcsStaged, VcsCommitted }

/// The locator *after* bytes+generation were pinned — what identity
/// records. Every field is concrete.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FixedSnapshotLocator {
    Live { project_id: ProjectId, live_revision: ProjectRevision },
    Saved { project_id: ProjectId, head_ref: String },
    Recovery { project_id: ProjectId, checkpoint: String },
    VcsStaged { repository_id: String, project_path: String,
                index_generation: IndexGeneration },
    VcsCommitted { repository_id: String, fixed_revision: String,
                  project_path: String },
}
```

```rust
pub struct SnapshotIdentity {
    pub project_id: ProjectId,
    pub source: SnapshotOrigin,
    pub locator: FixedSnapshotLocator,
    /// SHA-256 of the exact captured envelope bytes.
    pub digest: ContentDigest,
    /// `schema_version` declared by the snapshot bytes.
    pub schema_version: u32,
    /// `SCHEMA_VERSION` the projection was migrated to.
    pub projection_version: u32,
    /// Resolver-side monotonic capture counter — distinguishes
    /// repeated captures of the same locator.
    pub capture_generation: u64,
}
```

```rust
/// Versioned read-only projection: the migrated, validated project
/// behind `Arc`, tagged with source/projection schema versions.
pub struct ReadOnlyProjectProjection {
    pub source_schema_version: u32,
    pub projection_version: u32,
    pub project: Arc<Project>,
}

pub struct ImmutableSnapshot {
    pub identity: SnapshotIdentity,
    pub origin: SnapshotOrigin,
    pub project: Arc<ReadOnlyProjectProjection>,
    pub validation: SnapshotValidation,
    /// asset_id → resolution status (sorted, deterministic).
    pub asset_resolution: AssetAvailabilityManifest,
    /// One entry per routing processor binding, sorted by
    /// `ProcessorId`.
    pub opaque_state_refs: Vec<OpaqueStateRef>,
    pub completeness: SnapshotCompleteness,
}
```

`origin` は `identity.source` と同値を持つ別アクセス経路
（Issue #92 の `ImmutableSnapshot` field 構成どおり）。

```rust
/// External-content resolution status shared by asset blobs and
/// plugin opaque state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContentStatus {
    /// Existence and actual bytes confirmed.
    Known { byte_len: u64 },
    /// Definitively absent.
    Missing,
    /// Cannot be determined in this context.
    Unknown,
}

pub struct AssetAvailabilityManifest {
    /// `BTreeMap<AssetId, ContentStatus>` — deterministic order.
    pub by_asset: BTreeMap<AssetId, ContentStatus>,
}

pub struct OpaqueStateRef {
    pub processor_id: ProcessorId,
    /// Plugin descriptor identity (`PluginRef.descriptor` or the
    /// `Unresolved` descriptor).
    pub descriptor: String,
    pub status: ContentStatus,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotCompleteness { Complete, Partial }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotValidation {
    /// Passed strict schema + reference + watermark checks with no
    /// unresolved external references.
    Valid,
    /// Passed, with recorded degradations (missing/unknown assets or
    /// opaque state). Never silently treated as fully known.
    Degraded { reasons: Vec<Degradation> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Degradation {
    /// `missing` asset blobs definitively absent.
    MissingAssets { count: usize },
    /// `unknown` asset blobs unverifiable.
    UnknownAssets { count: usize },
    /// Plugin opaque state that could not be confirmed.
    UnresolvedOpaqueState { count: usize },
}
```

`SnapshotValidation::Rejected` は置かない — reject は常に
`Err(SnapshotError)` であり、構築済み `ImmutableSnapshot` が
Rejected になる経路は存在しない（Issue #7 の 3 値は resolve
前の分類を含むため、本 resolver では Err に写像する）。

### 3. Digest

`ContentDigest` は algorithm tag + 32 byte を持つ:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentDigest { algorithm: DigestAlgorithm, bytes: [u8; 32] }

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigestAlgorithm { Sha256 }
```

文字列表現は `"sha256:<64 lowercase hex>"`。`Display`/`FromStr`/
serde はこの形式のみ受理し、未知 algorithm tag・長さ不正・
大文字 hex を拒否する（algorithm-tagged であることが
`Project content digest` と Git object id 等の混同を防ぐ）。

SHA-256 は `digest.rs` に FIPS 180-4 の compression を実装
（`sha2` 依存を追加しない制約のため）。padding・length
encoding は規格どおり。unit test で既知解
（`""`/`"abc"`/56byte 境界/`"a"*1_000_000`）を検証する。
これは暗号 primitive ではなく content addressing 用の
deterministic fingerprint であり、衝突耐性は SHA-256 規格に
依存する — 「SHA256 衝突を前提とした危険な ID 統合をしない」
は、本型が identity の**一部**（ProjectId+locator+schema+
generation と複合）であって単独の同一性根拠にしないことで
満たす。

### 4. VCS Bytes Port（VCS-01 が所有・VCS-03 が実装）

```rust
/// Provider-scoped addressing handed to [`VcsBytesPort`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum VcsLocator {
    Staged { repository_id: String, project_path: String,
             index_generation: Option<IndexGeneration> },
    Committed { repository_id: String, fixed_revision: String,
                project_path: String },
}

/// Opaque provider fingerprint of the staged index state covering
/// the locator's file set. Only *equality* is meaningful — the
/// resolver never interprets or orders it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
          Serialize, Deserialize)]
#[serde(transparent)]
pub struct IndexGeneration(pub String);

pub enum VcsFileRead {
    /// Bytes pinned at the locator.
    Found(Vec<u8>),
    /// Definitively absent from the fixed set.
    Absent,
    /// Cannot be determined (e.g. unresolved LFS pointer).
    Unknown,
}

/// Tri-state content probe result (asset blob / opaque state).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlobRead {
    /// Blob exists; `byte_len` is the confirmed byte count of the
    /// actual bytes the provider read.
    Found { byte_len: u64 },
    Absent,
    Unknown,
}

pub enum VcsPortError {
    /// Provider infrastructure failure (IO/transport/auth).
    Provider(String),
    /// Locator invalid for this provider (bad path/repo).
    InvalidLocator(String),
    /// Required capability absent.
    Unsupported(String),
}

/// Read-only bytes port owned by VCS-01 and implemented by VCS-03
/// (#94). No checkout/stage/commit/write methods exist by design.
///
/// Contract for implementors:
/// - `index_generation` must fingerprint the index state covering
///   `project_path`'s file set (entries + object identity), so that
///   equal generations ⇒ the staged bytes are stable. For
///   `Committed` locators it must return a value derived from the
///   fixed revision (stable constant).
/// - `read_project_file`/`read_blob` must read within the locator's
///   fixed set only — never the mutable worktree for `Committed`,
///   never outside `project_path`'s scope.
/// - `BlobRead::Found` asserts the provider actually read the bytes
///   (not merely a pointer/manifest entry).
pub trait VcsBytesPort {
    fn index_generation(&self, locator: &VcsLocator)
        -> Result<IndexGeneration, VcsPortError>;
    fn read_project_file(&self, locator: &VcsLocator)
        -> Result<VcsFileRead, VcsPortError>;
    fn read_blob(&self, locator: &VcsLocator, digest: &str)
        -> Result<BlobRead, VcsPortError>;
}
```

### 5. Asset probe（Saved/Live/Recovery 用）

project snapshot の外にある content-addressed blob を確認する
任意 port:

```rust
/// Read-only probe for blobs outside the snapshot bytes.
pub trait BlobProbe {
    /// `Found` only when existence AND actual bytes were confirmed.
    fn probe(&self, digest: &str) -> BlobRead;
}
```

`resolve_*` は `Option<&dyn BlobProbe>` を受け取る。`None` の
場合は全 asset を `Unknown`（`Known` に見せない）。VCS resolve
では blob 確認を `VcsBytesPort::read_blob` が担うため
`BlobProbe` は使わない（locator 固定集合内の確認であり、
global asset domain とは別問い）。

### 6. Resolve orchestration

`SnapshotResolver` は capture generation counter
（`AtomicU64`）と `SnapshotLimits` を持つ:

```rust
pub struct SnapshotLimits {
    /// Max accepted snapshot envelope bytes. Default 64 MiB.
    pub max_snapshot_bytes: u64,
}
impl Default for SnapshotLimits { /* 64 MiB */ }

pub struct SnapshotResolver {
    limits: SnapshotLimits,
    next_generation: AtomicU64,
}
```

`SnapshotCancel` は `Arc<AtomicBool>` の cooperative token
（`cancel()`/`is_cancelled()`）。resolve は phase 境界
（capture 開始・bytes 確定後・parse 後・asset loop 各反復・
return 直前）で検査し `Err(SnapshotError::Cancelled)`。

#### 共通 pipeline `resolve_bytes`

全 source が共有する「固定 bytes → identity + projection」:

1. size check: `bytes.len() > max_snapshot_bytes` →
   `BudgetExceeded`。
2. digest = `ContentDigest::sha256(&bytes)`（**固定 bytes を
   読んでから**測る）。
3. strict parse `SnapshotEnvelope`（serde error →
   `InvalidSnapshot(Error::Deserialize)`）。
4. `envelope.project_id != locator.project_id` →
   `ProjectMismatch`。
5. schema gate: `schema_version > SCHEMA_VERSION` →
   `UnsupportedSchema`。`< SCHEMA_VERSION` は
   `migration::migrate_to_current`（未到達版 →
   `UnsupportedSchema`、step error → `InvalidSnapshot`）。
   **read-only**: source bytes・envelope は消費後も不変。
6. `validate_references` + `check_id_watermark`（load と同じ
   oracle。failure → `InvalidSnapshot(Error)`）。
7. asset/opaque-state resolution（後述）→ manifests・
   completeness・validation。
8. identity 構築（`capture_generation` = counter fetch_add）。

#### capture_live

```rust
pub fn capture_live(
    &self,
    controller: &ProjectController,
    expected_revision: ProjectRevision,
) -> Result<ImmutableSnapshot, SnapshotError>
```

1. `revision = controller.live_revision()`・
   `project = controller.live().clone()`・
   `project.id_watermark = controller.snapshot_id_watermark()`
   を同一 `&self` 借用内で取得（save と同じ watermark を刻む
   ため Live/Saved の digest が一致する）。
2. `revision != expected_revision` → `StaleLiveRevision`。
3. `bytes = serde_json::to_vec(&SnapshotEnvelope { schema_version:
   SCHEMA_VERSION, project_id, revision, project })` —
   serialization error → `InvalidSnapshot`。
4. 以降は共通 pipeline（locator =
   `Live { live_revision: revision }`）。
   asset probe は caller 供給の `Option<&dyn BlobProbe>`。

clone→serialize→parse の往復は冗長に見えるが、digest が
「実際に store へ書かれる canonical bytes」に一致することを
保証し、全 source で同一 pipeline を共有するための意図的な
統一である。

#### resolve_saved

```rust
pub fn resolve_saved(
    &self,
    store: &dyn ProjectStore,
    locator: &SnapshotLocator,   // Saved variant のみ受理
    cancel: &SnapshotCancel,
    blobs: Option<&dyn BlobProbe>,
) -> Result<ImmutableSnapshot, SnapshotError>
```

便宜上 `resolve_saved_head(store, project_id, ...)` も提供
（`head_ref: None` の locator を構築して委譲）。

1. `head1 = store.head()?` → `None` → `NotFound("HEAD")`。
2. locator が `Some(expected)` かつ `expected != head1` →
   `HeadConflict { expected, found: head1 }`。
3. `bytes1 = store.read_snapshot(&head1)?`（missing →
   `NotFound`）。
4. `head2 = store.head()?` → `head2 != head1` →
   `HeadConflict`（capture 中の HEAD race / ACK loss 後の
   別 HEAD）。
5. `bytes2 = store.read_snapshot(&head1)?` →
   `bytes2 != bytes1` → `SourceChanged`（同名 bytes の
   Refresh/foreign write — immutable 名を信用しない）。
6. 共通 pipeline（locator = `Saved { head_ref: head1 }`）。

#### resolve_recovery

`store` は **recovery root を指す `ProjectStore`**（caller が
別 root で開く。saved HEAD とは別物）。`checkpoint` 名の
snapshot を `read_snapshot` で読み、bytes を 2 回読んで一致を
要求（`SourceChanged`）。HEAD は一切読まない・書かない。
locator = `Recovery { checkpoint }`。

#### resolve_vcs

```rust
pub fn resolve_vcs(
    &self,
    port: &dyn VcsBytesPort,
    locator: &SnapshotLocator,   // VcsStaged/VcsCommitted のみ
    cancel: &SnapshotCancel,
) -> Result<ImmutableSnapshot, SnapshotError>
```

staged/committed 共通の generation window:

1. `vcs_locator` を構築（`SnapshotLocator` → `VcsLocator`）。
   `project_path` の軽量検証（非空・NUL 無し・`..` segment
   無し・絶対 path 無し → `InvalidLocator`）。
2. `gen_before = port.index_generation(&vcs_locator)?`。
3. `project_read = port.read_project_file(&vcs_locator)?`:
   - `Found(bytes)` → size check。
   - `Absent` → `NotFound("project file")`。
   - `Unknown` → `VcsUnavailable`（fixed set から
     project file を確認できない = snapshot を構成不能）。
4. bytes を parse（schema gate・project_id・migrate・
   validate は共通 pipeline と同じ helper を共有 — ただし
   asset resolution は window 内で行うため pipeline を
   2 相に分ける: `parse_and_validate(bytes) ->
   (envelope/project/digest/schema)` と `finish(...)`）。
5. asset digest を収集し `port.read_blob(&vcs_locator, digest)`
   を各 asset に実行（cancel を各反復で検査）。
6. `gen_after = port.index_generation(&vcs_locator)?`。
   `gen_before != gen_after` → `IndexChanged { before, after }`
   — file-set 全体を 1 つの window で固定した証拠。
7. staged の場合: `BlobRead::Absent` が 1 件でもあれば
   `PartialStage { missing: Vec<digest> }`（部分 Stage は
   明示 Failure — Issue #92 受入）。`Unknown` は
   `ContentStatus::Unknown` として degraded 記録。
   committed の場合: `Absent` → `ContentStatus::Missing`、
   `Unknown` → `Unknown`（fixed revision の事実として
   degraded 記録 — "Committed != Reproducible" の正直な状態）。
8. opaque state refs 構築 → `finish` で identity/completeness/
   validation を組み立てる。

「ファイル集合全体を一時領域へ固定」は、resolver が
`[gen_before, gen_after]` window 内で必要 file（project +
参照 blob）をすべて読み、generation 一致を要求することで
実現する。provider が内部で temp 領域 staging を行うかは
実装詳細であり、契約上は generation の等価性が固定の証拠。

#### asset / opaque-state resolution

- asset: `project.assets` の各 `(AssetId, Asset{digest})` について
  probe 結果を `ContentStatus` に写像（`BlobRead::Found{byte_len}`
  → `Known{byte_len}`、`Absent` → `Missing`、`Unknown` →
  `Unknown`）。probe 未供給 → 全 `Unknown`。
- opaque state: `project.routing.processors` を
  `ProcessorId` 順に走査。`ProcessorBinding::Unresolved` →
  `Missing`（plugin が解決不能であることが binding 自体に
  記録済み）。`Resolved` → `Unknown`（opaque bytes を本 slice
  で検証する probe が存在しない — `Known` にしない）。
- `completeness`: 全 entry が `Known` → `Complete`、1 件でも
  `Missing`/`Unknown` → `Partial`。
- `validation`: `Partial` → `Degraded { reasons }`（
  Missing/Unknown の件数を集計）、`Complete` → `Valid`。

### 7. SnapshotError

```rust
#[derive(Debug)]
pub enum SnapshotError {
    /// Store port failure.
    Store(Error),
    /// VCS port failure.
    Vcs(VcsPortError),
    /// HEAD/snapshot/checkpoint/project file not found.
    NotFound(String),
    /// HEAD moved or disagrees with the pinned expectation.
    HeadConflict { expected: Option<String>, found: Option<String> },
    /// Bytes under a pinned name changed during capture.
    SourceChanged(String),
    /// Envelope parse / project validation / migration failure.
    InvalidSnapshot(Error),
    /// Schema version outside the migration ladder's reach.
    UnsupportedSchema { found: u32 },
    /// Envelope project_id differs from the locator's.
    ProjectMismatch { expected: ProjectId, found: ProjectId },
    /// Live revision differs from `expected_live_revision`.
    StaleLiveRevision { expected: u64, actual: u64 },
    /// Staged index generation changed across the capture window.
    IndexChanged { before: IndexGeneration, after: IndexGeneration },
    /// Staged file set incomplete — referenced blobs absent from
    /// the index (partial stage).
    PartialStage { missing: Vec<String> },
    /// Fixed set cannot yield the project file.
    SourceUnavailable(String),
    /// Cooperative cancellation observed.
    Cancelled,
    /// Snapshot bytes exceed `max_snapshot_bytes`.
    BudgetExceeded { limit: u64, actual: u64 },
    /// Malformed locator (path/repo fields).
    InvalidLocator(String),
}
```

`Display` + `std::error::Error` + `From<serde_json::Error>` 等の
変換を実装。`Error`（domain）と `VcsPortError` は variant として
保持し、文字列 flatten しない。

### 8. Read-only 保証

- `capture_live` は `&ProjectController` のみ使用。
  `snapshot_id_watermark()` は `peek()` の read。
- `resolve_saved`/`resolve_recovery` は `&dyn ProjectStore` の
  `head`/`read_snapshot` のみ（`save` を呼ばない）。
- `resolve_vcs` は `&dyn VcsBytesPort` の read 3 method のみ
  （port trait に write method が存在しない）。
- `ImmutableSnapshot.project` は `Arc<ReadOnlyProjectProjection>`
  で、resolver が構築した Project を共有するだけ — caller が
  mutate する経路は `Arc::try_unwrap` 以外なく、snapshot は
  capture 時点の内容を保持し続ける。
- RT callback 非阻害: resolver は非 RT 前提の同期 API で、
  RT thread から呼ばない（Issue #92「非RT」明記）。

### 9. Error 分類対応（Issue #7 §5）

| #7 分類 | 本 slice の写像 |
| --- | --- |
| CompareError::InvalidSnapshot | `InvalidSnapshot(Error)` |
| CompareError::UnsupportedSchema | `UnsupportedSchema{found}` |
| CompareError::MigrationFailed | `InvalidSnapshot(Error)`（migration step の Error） |
| CompareError::ResourceBudget | `BudgetExceeded` |
| CompareError::Cancelled | `Cancelled` |
| VcsError::IndexChanged | `IndexChanged` |
| VcsError::PartialStage | `PartialStage` |
| VcsError::MissingProjectFile | `NotFound` |
| AmbiguousIdentity / UnknownDomain | VCS-02 の責務（本 slice 対象外） |

## File map

- `crates/synvelis-project/src/snapshot/mod.rs`（新規）
- `crates/synvelis-project/src/snapshot/types.rs`（新規）
- `crates/synvelis-project/src/snapshot/digest.rs`（新規）
- `crates/synvelis-project/src/snapshot/vcs.rs`（新規）
- `crates/synvelis-project/src/lib.rs`（module + re-export 追加）
- `crates/synvelis-project/src/controller.rs`
  （`snapshot_id_watermark` accessor 追加のみ）
- `crates/synvelis-project/tests/vcs01_snapshot.rs`（新規）

`Cargo.toml`/`Cargo.lock`/`mise.toml` は変更しない
（依存追加なし — SHA-256 は crate 内実装）。

## Risk / rollback

- SHA-256 自前実装の正確性: FIPS 既知解 test vector
  （unit test）+ `ContentDigest` の parse/format test で担保。
  万一誤りがあれば digest のみ修正すればよく、identity の
  他 field・resolve pipeline には影響しない。
- `snapshot_id_watermark` accessor 追加は additive で、
  既存 consumer（`apply`/`save`/`load`/undo/redo）に影響
  しない。revert は accessor 削除 + snapshot module 削除で
  完結する。
- 新 module は既存型を変更しない additive のため、他 crate
  （synvelis-app/workspace/library/gateway/player/recording/
  render/theme/poc）の非回帰は `cargo test --workspace` で
  確認する。
