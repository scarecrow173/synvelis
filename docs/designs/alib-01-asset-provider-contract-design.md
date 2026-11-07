---
id: "DESIGN-pzR2GiO8Eru6Jw2zRK6l2s"
type: "design"
status: "approved"
title: "ALIB-01 Design — Asset Provider 契約・Local/Project 基準 Provider"
created: "2026-11-07"
updated: "2026-11-07 (N1 注記追補)"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/adr/alib-01-synvelis-library-crate-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/l1-local-wav-verified-playback-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/alib-01-synvelis-library-crate-boundary.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
---

# ALIB-01 Design — Asset Provider 契約と基準 Provider

## 構成（crates/synvelis-library）

```
src/
  lib.rs          crate doc + re-export
  id.rs           ProviderId, OpaqueLocator, PrincipalId, CredentialKey
  refs.rs         RemoteAssetRef, RevisionKey, ObjectIdentity,
                  ContentDigest, StrongRevisionToken, PinProof,
                  PinnedAssetRef
  capability.rs   Capability, UnsupportedReason, PinCapability,
                  AccessMode, ProviderCapabilities
  observation.rs  ObservedAt, ObservationOrigin, InferenceMethod,
                  UnavailableReason, StaleReason, AssetObservation<T>,
                  EntryFacts, AssetKind, Availability, AssetStat
  browse.rs       BrowseScope, SortSpec, FilterSpec, BrowseRequest,
                  BrowseEntry, EntryKind, Coverage, BrowsePage,
                  SearchQuery, OpaquePageCursor(encode/decode+validate)
  context.rs      ProviderContext, CancellationToken, ProviderLimits,
                  Clock, CredentialResolver
  error.rs        ProviderErrorKind, RetryAdvice, ProviderError
  provider.rs     AssetProvider trait, ByteRange, FetchClaims,
                  VerificationPolicy, PinnedRead, FetchCompletion,
                  FetchReceipt
  registry.rs     ProviderDescriptor, ProviderKind, ProviderRegistry
  digest.rs       StreamingDigest trait + Sha256 impl + verify reader
  local.rs        LocalFolderProvider
  project.rs      ProjectAssetsProvider, ProjectCatalog,
                  ProjectCatalogSnapshot, ControllerCatalog,
                  ProjectBlobStore, LocalBlobDir
tests/
  contract.rs     型契約・cursor・error・digest の unit 相当
  local.rs        LocalFolderProvider 実 FS テスト
  project.rs      ProjectAssetsProvider テスト
  types.rs        型・unit テスト
  containment_swap.rs  component-swap TOCTOU 回帰（7 件強制 GREEN
                  + seam sanity、fault barrier 経由）
  common/mod.rs   テスト共有 helper
```

## 型付き公開契約

```rust
pub struct ProviderId(u128);            // "provider-<32hex>" tagged
pub struct OpaqueLocator(String);       // 非空・NULなし・長さ上限
pub struct PrincipalId(String);         // app内部のprincipal scope key(非秘密)
pub struct RemoteAssetRef { provider: ProviderId, locator: OpaqueLocator }

pub struct RevisionKey(String);          // 例 "commit:<oid>" / "blob:<digest>"
pub struct ObjectIdentity(String);       // 例 "<crc16hex>" / "sha256:<hex>"
pub struct ContentDigest { algorithm, hex } // "sha256:<64hex>" algorithm-tagged

pub enum StrongRevisionToken {
    ContentHash { digest: ContentDigest },              // Local
    StrongValidator { validator: String, precondition: PreconditionKind }, // WebDAV等
}
pub enum PreconditionKind { IfMatch, IfUnmodifiedSince }

pub enum PinProof {
    ImmutableObject { revision: RevisionKey },
    ConditionalStrong { token: StrongRevisionToken },
}
pub struct PinnedAssetRef {
    pub asset: RemoteAssetRef,
    pub proof: PinProof,
    pub object_identity: ObjectIdentity,
    pub expected_size: Option<u64>,    // hint
    pub digest_hint: Option<ContentDigest>, // ChecksumHint capability
}
```

`AssetObservation<T>`:
```rust
pub enum AssetObservation<T> {
    Observed { value: T, origin: ObservationOrigin, at: ObservedAt },
    Inferred { value: T, method: InferenceMethod, at: ObservedAt },
    Unknown { reason: UnavailableReason },
    Stale { value: T, at: ObservedAt, reason: StaleReason },
}
```

`AssetProvider`:
```rust
pub trait AssetProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn capabilities(&self, asset: Option<&RemoteAssetRef>) -> ProviderCapabilities;
    fn browse(&self, req: &BrowseRequest, cx: &ProviderContext)
        -> Result<BrowsePage, ProviderError>;
    fn stat(&self, asset: &RemoteAssetRef, cx: &ProviderContext)
        -> Result<AssetStat, ProviderError>;
    fn pin(&self, asset: &RemoteAssetRef, cx: &ProviderContext)
        -> Result<PinnedAssetRef, ProviderError>;
    fn open_pinned(&self, pin: &PinnedAssetRef, cx: &ProviderContext)
        -> Result<PinnedRead, ProviderError>;
    fn search(&self, query: &SearchQuery, cx: &ProviderContext)
        -> Result<BrowsePage, ProviderError>;          // default: Unsupported
    fn open_preview(&self, asset: &RemoteAssetRef, range: Option<ByteRange>,
        cx: &ProviderContext) -> Result<PinnedRead, ProviderError>; // default: Unsupported
}
```

`PinnedRead`/`FetchReceipt`:
```rust
pub struct PinnedRead {
    pub reader: Box<dyn Read + Send>,
    pub claims: FetchClaims,
    pub completion: FetchCompletion,   // Arc<Mutex<Option<Result<FetchReceipt, ProviderError>>>>
}
pub struct FetchClaims {
    pub object_identity: ObjectIdentity,
    pub expected_size: Option<u64>,
    pub verification: VerificationPolicy, // StreamVerified{digest} | IdentityBound{hint} | Unverified
}
pub struct FetchReceipt {
    pub object_identity: ObjectIdentity,
    pub bytes_read: u64,
    pub digest: Option<ContentDigest>,  // stream検証済みなら実測値
    pub verified: bool,
}
```

検証 reader は read 毎に cancel/deadline を確認し、EOF で digest を
照合して completion slot に typed 結果を書く。digest 不一致は
`io::Error`(`InvalidData`) を返し completion は
`ProviderErrorKind::IntegrityMismatch`。EOF 到達前の中断は
completion 未設定のまま → caller が `take()` で `None` を検出し
Incomplete 扱いにできる。

## Cursor

`OpaquePageCursor(String)` = hex(JSON `{v,provider,scope_fp,query_fp,
principal_fp,epoch,offset}`)。offset = emit 済み entry 数 —
resume は scope を再 walk・再 sort して offset から継続する
（live tree snapshot ではないことを明示）。Local epoch =
provider 内単調 browse generation
（新規 browse 毎に ++）。Project epoch = `ProjectRevision.0`。
fingerprint = scope/sort/filter/principal を sha256 した hex 短縮。
不一致・別 provider・別 epoch・decode 失敗は全て
`ProviderErrorKind::CursorInvalid`。`v` は `1` 固定で未知は拒否。

## LocalFolderProvider

- root を `RootedDir::open_with` で capability として取得
  （存在し dir でなければ構築失敗）。以後の全 lookup はこの
  handle 起点の handle-relative resolve で、root 以下の path
  再 traverse は行わない。
- locator: Ruviform `RelativePath` grammar で検証（`/` 区切り
  root 相対。`\\`・`..`・`.`・NUL・絶対・制御文字・
  Windows 予約名・末尾 dot/space・超長 component・非 UTF-8 を
  拒否 — 旧 hand-rolled parser より厳格、互換差は impl record
  記録済み）。resolve は capability 内 `FollowMode::WithinRoot`。
  root 外に出るものは `EscapesRoot`→`PermissionDenied`。
- browse: scoped `RootedDir` handle の iterative DFS。各 dir の
  `read_dir` を `file_name`→entry kind で安定 sort。entry =
  Container(dir)/Asset(file/symlink)。symlink leaf は `Symlink`
  kind で列挙するが降りない（descent `open_dir` は `NoFollow`）。
  link target の facts は `metadata(WithinRoot)` で、root 外
  target は leaf として列挙しつつ facts を返さない
  （`Availability::Unavailable{Other{detail:"symlink target
  outside browse scope"}}`、dangling は `Missing`）。loop guard
  は canonical path ではなく `NodeId` set。
- limits: `max_depth`（default 64）・`max_entries`（walk 総計
  budget）・`page_limit`（page 毎）・`deadline`・`max_bytes`
  （read 系）。超過は coverage=Partial + cursor で継続可能なら
  frontier を残す。budget 上限で frontier 保持不能なら
  next_cursor=None の Partial で終端を明示。
- stat: `metadata(WithinRoot)` で byte_len/modified/kind/readonly
  を `Observed`（link leaf は `leaf_kind` で `Symlink` を保持）、
  media 系と checksum_hint は `Unknown`（pin しないと digest は
  計測しない）。
- pin: `open_regular(WithinRoot)` で非 regular を open 前に拒否し
  同一 object を bounded sha256 streaming（`max_bytes` guard）。
  `ConditionalStrong{ContentHash{sha256}}`、
  object_identity="sha256:<hex>"、expected_size=開いた object の
  `info().byte_len`。dir/container・root 外逃げは typed error。
- open_pinned: pin の asset 一致 + expected_size との同一 object
  照合（不一致→`SourceChanged`）→ 検証 reader（EOF sha256 照合、
  不一致→`IntegrityMismatch`）。digest は integrity であり
  containment の代替ではない。
- search: scope 内 walk で file_name case-insensitive 部分一致。
  crawl が正本なので coverage は Complete/Partial を正直に宣言。
- preview/range: `RootedFile` の `Seek` + `Take` bounded。claims
  は `VerificationPolicy::Unverified`（pin ではないことを明示）。
- scope 付き browse/search は `open_scope` が locator 成分を検証し
  `canonical_root + components` へ `RootedDir::open_with` で新
  capability を取得（`open_dir` では親 capability の root 境界を
  引き継ぐため scope 境界を作れない）。取得後は開いた handle の
  `canonical_root()` が provider root 配下かを照合する — 同じ
  object の検証で check-then-use ではない。

## ProjectAssetsProvider

- `ProjectCatalog` port: `fn snapshot(&self) -> ProjectCatalogSnapshot`
  `{ project_id, revision, assets: Vec<ProjectAssetRecord> }`。
  `ProjectAssetRecord { asset_id, digest, sources: Vec<SourceRef>,
  imported_via: Vec<ImportId> }`。実装は `Mutex<ProjectController>`
  （`catalog()` で `live()`+`live_revision()` を読み `applied_imports`
  を逆引き）と `ProjectCatalogSnapshot` 自身（test/headless 用）。
- locator = `"asset-<32hex>"`（tagged `AssetId`）。browse は
  catalog snapshot を `AssetId` sort で paging（cursor offset）。
  epoch = `ProjectRevision`。project が変われば cursor 失効。
- `ProjectBlobStore` port: `open(identity)->(Read+Send,size)`/
  `stat(identity)->BlobStat`。`LocalBlobDir` は identity を
  filename-safe 検証（`/`・`\\`・`..`・非 `[0-9a-zA-Z_-]`・
  DOS device 名を拒否）の上で単一成分 `RelativePath` へ変換し、
  操作毎に `RootedDir::open_with(store)` で取得した capability
  の `metadata`/`open_regular(WithinRoot)` で解決する（per-op
  open は構築後作成の store dir を許容する旧 lifecycle 維持）。
  app は `media_root/blobs` を渡し、path 規約は本 crate に複製
  しない。leaf swap は capability 内で `EscapesRoot`/
  `NotRegularFile` に落ち、root 外 byte/facts は返らない。
  blob missing は `availability`=MissingMedia、open は
  `NotFound`。
- stat: asset の digest・source states・import provenance を
  `Observed`、byte_len は blob fstat（missing→Unknown{reason}
  + Availability::Missing）。
- pin: blob open→bounded sha256 streaming→
  `ImmutableObject{revision:"blob:<legacy-digest>"}`、
  object_identity=legacy digest、digest_hint=sha256、
  expected_size=実測。
- open_pinned: identity 一致 + expected_size fstat 照合→検証
  reader（digest_hint があれば EOF sha256 照合。なければ
  IdentityBound で CAS binding のみ主張）。
- search は `UnsupportedCapability`（text index 不在）。
- preview/range は blob read（Unverified claims）。

## ProviderContext

```rust
pub struct ProviderContext {
    pub principal: PrincipalId,
    pub cancel: CancellationToken,          // Arc<AtomicBool>
    pub limits: ProviderLimits,             // max_entries/max_bytes/max_depth/deadline
    pub credentials: Option<Arc<dyn CredentialResolver>>,
    pub clock: Arc<dyn Clock>,              // default SystemClock
}
```

`Clock` は `now()->SystemTime` のみ。`CredentialResolver::resolve(
&CredentialKey)->Result<OpaqueCredential,ProviderError>` は
handle（秘密値ではない参照）を返す。Local/Project は未使用。

## Error/Retry

`ProviderError { kind, retry: RetryAdvice, detail: String }`。
`RetryAdvice::After` は `Option<Duration>`。各 kind の default
advice を `ProviderError::new` が付与（NotFound/Unsupported→Never、
TransientIo/RateLimited→After、AuthExpired→AfterAuth、
SourceChanged/PreconditionFailed/RevisionGone→AfterUserRePin、
IntegrityMismatch→Never、Cancelled→Never、CursorInvalid→
AfterUserRePin? いや cursor 再取得なので `After` ではなく Never +
caller が再 browse → `Never`）。

## FS traversal containment（N1, Refs #85 — 修復済み）

旧 `resolve`(canonicalize+root 配下照合)→ `File::open`/`fs::metadata`/
`fs::read_dir` の 2 段構成は T0（check）と T1（traverse）に分かれ、
T0→T1 で leaf または intermediate component が rename+symlink swap
されると T1 は root 外 object を開いていた。post-open の `is_file`/size
は object identity を証明せず、digest 系検証は EOF 前に byte を
caller へ届けるため containment の代替にならなかった。

真の containment は handle-relative traversal（Unix:
openat+per-component no-follow、Windows: RootDirectory+
OBJ_DONT_REPARSE 相当）でのみ成立し、symlink follow は「target が
root 内に解決される間のみ辿る（FollowWithinRoot、bounded hops）」を
維持する — NoFollow への縮小で現契約を変えない。この能力は std に
存在せず、workspace `unsafe_code=forbid` により SV 内実装も不可能。

**所有境界（確定・消費済み）**: 当該 capability は Ruviform が所有し、
`ruviform-io-file::rooted`（`RootedDir`/`RelativePath`/`FollowMode`/
`NodeInfo`/`RootedFile`/`NodePin`/`NodeId`/`IdentityStrength`）として
[ruviform#398](https://github.com/scarecrow173/ruviform/issues/398) →
PR401 で公開・main 統合済み（SV 消費 rev `5d48298b`）。SV 側は
`set_pre_traverse_hook`（`#[doc(hidden)]` fault barrier）を
stat/open/read_dir の各 rooted 呼出地点に持ち、
`tests/containment_swap.rs` の deterministic swap 回帰で契約を固定する
（GREEN 強制）。実装詳細は task5 と impl record を参照。

## 検証方針（実テスト）

実 tempdir に file/dir/symlink を作り、browse/stat/pin/open/
cancel/cursor invalidation/mutation rename/差替え/traversal/
symlink-escape/loop を検証。Project は `Mutex<ProjectController>`
に test-only `Operation`（Asset+Source+AppliedImport を
`CommitImport` と同形で挿入）を適用した実 controller と、
`ProjectCatalogSnapshot` 静的 catalog の双方で検証。
blob dir は tempdir に digest 名 file を配置。

## 依存

`synvelis-project`（model）、`serde`/`serde_json`/`sha2`、
`ruviform-io-file`（workspace 既存 dep + N1 修復で新規追加。
既定 branch 追従 git dep で rev pin なし、解決 rev は
`Cargo.lock` が記録 — 現 `5d48298b`）。Ruviform 側 rooted-read
公開契約は RF#398→PR401 で提供・main 統合済み。cap-std 等の
代替 FS framework は採用しない（unsafe 重複・責務越境のため）。
