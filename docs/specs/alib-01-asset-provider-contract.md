---
id: "SPEC-j8VrtHRiA60WGx27NKFnH4"
type: "spec"
status: "approved"
title: "ALIB-01 — Asset Provider 型付き契約・Registry・Local/Project 基準 Provider"
created: "2026-11-07"
updated: "2026-11-07 (N1 既知差分追記)"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/synvelis/issues/9"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-library"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/discovery/alib-01-asset-provider-contract-discovery.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/alib-01-asset-provider-contract-discovery.md"
  derived-by: []
---

# ALIB-01 Spec — Asset Provider 契約と基準 Provider

## Purpose

Issue #85: #9 の Provider contract を Remote/VCS 依存なしで実装し、
Local Folder と Current Project Assets を共通の型付き Catalog として
閲覧できる基準経路を提供する。ALIB-02〜06 がこの型付き境界に依存
するため、下流が利用できる公開契約を先に安定させる。

## Contracts

1. **Identity 分離**: `ProviderId`（接続設定の識別子）、
   `RemoteAssetRef { provider, locator }`（credential を含まない
   不透明 locator）、`PinnedAssetRef { asset, proof, object_identity,
   expected_size? }` を新設。既存 `AssetId`/`SourceId`/`ImportId`
   （`synvelis-project`）とは別型であり相互代入しない。
   `object_identity` は provider の不変オブジェクト名
   （CAS digest・Git object 等）、`expected_size` は検証済み値でなく
   hint として扱う。
2. **PinProof**: `ImmutableObject { revision }`（Git commit/tree、
   CAS blob 等の不変 revision）と `ConditionalStrong { token }`
   （強 digest/強 validator + 取得時 precondition の実施強制）を
   型で区別。弱 ETag・branch 名・mtime・size 単独を固定証拠に
   しない。`pin` 失敗時は正式 Import に使えない。
3. **ProviderCapabilities**: browse/stat/optional search/pin/
   range_read/preview/fetch/checksum_hint を独立に
   `Capability{Supported|Unsupported{reason}}` で宣言。pin は
   `PinCapability` で固定強度を宣言。access は ReadOnly/ReadWrite。
   未対応は型付き理由を返す。
4. **AssetObservation&lt;T&gt;**: `Observed{value,origin,at}`/
   `Inferred{value,method,at}`/`Unknown{reason}`/`Stale{value,at,
   reason}` を分離。Duration/BPM/Key・権限・ライセンスを架空補完
   しない。
5. **Browse**: `BrowseRequest{scope,page_limit,cursor?,sort,filter}` →
   `BrowsePage{entries,next_cursor,coverage,observed_at}`。
   cursor は Provider/Scope/Query+Sort+Filter/Principal/Revision に
   結び、権限変更・Query 変更後の再利用は `CursorInvalid` で拒否。
   `Coverage` は `Complete/Partial/Stale/OfflineCached/NeverIndexed`
   を区別し、0 件と失敗・未索引を混同しない。`search` は
   optional で `UnsupportedCapability` を型付き返し得る。
6. **AssetProvider trait**: `Send+Sync`、blocking/nonRT worker 契約。
   `capabilities/browse/stat/pin/open_pinned` + optional
   `search`/`open_preview`。`open_pinned` は `Read+Send` の
   chunked stream + claims + completion slot（typed `FetchReceipt`
   /`ProviderError`）を返す。Tokio/HTTP SDK は公開 API に露出
   しない。`ProviderContext` に principal/cancellation/limits/
   credential resolver/clock を明示注入し、token 値を Project へ
   保存しない。
7. **Error**: `ProviderErrorKind` = NotFound/RevisionGone/
   PermissionDenied/AuthExpired/Offline/RateLimited/TransientIo/
   UnsupportedCapability/PreconditionFailed/SourceChanged/
   IntegrityMismatch/InvalidMedia/BudgetExceeded/Cancelled/
   CursorInvalid/InvalidRequest/Internal。`RetryAdvice` =
   Never/After/AfterAuth/AfterUserRePin/MayReconcile を別契約で返す。
8. **Registry**: `ProviderRegistry` が `ProviderId` → provider +
   descriptor を一意に登録・解決する。descriptor は
   `ProviderKind`(LocalFolder/ProjectAssets/Remote 等, non_exhaustive)
   と表示名を持つ。
9. **LocalFolderProvider**: filesystem capability。`/` 区切りの
   root 相対 locator は Ruviform `RelativePath` grammar で検証し、
   全 lookup は構築時に取得した `RootedDir` capability 起点の
   handle-relative resolve（`WithinRoot` follow）で traversal を
   防止する。symlink leaf は `Symlink` kind で列挙するが降りず、
   root 外 target の facts は返さない。loop guard は `NodeId`
   set で切断。`pin` は sha256 streaming 計測の
   `ConditionalStrong{ContentHash}`、`open_pinned` は EOF で
   sha256 照合する検証 reader を返し、可変・rename・差替えを
   `SourceChanged`/`IntegrityMismatch` で固定扱いしない。
10. **ProjectAssetsProvider**: `Mutex<ProjectController>` 経由の
    read-only 投影 port(`ProjectCatalog`)のみを持ち、別 Project
    正本を持たない。`pin` は `ImmutableObject{revision: CAS blob
    digest}`、blob dir は注入 port(`ProjectBlobStore`)で path 規約を
    複製しない。source state/import provenance を facts として返す。

### Resolved gap（2026-11-07 追記→同日修復, N1 / Refs #85）

契約 9/10 の「canonical 照合で traversal 防止」は **check 時点**の
不変条件であり、traverse 時点（`File::open`/`metadata`/`read_dir`）
との間に leaf/intermediate component swap（rename+symlink）で
root 外 object を開く TOCTOU が残存していた。これは root 外の
bytes/facts/entries を返さないという AC1 意図そのものの侵害であり、
containment-at-traverse の強制には handle-relative open
（root capability）が必須 — Ruviform 所有の rooted-read 公開契約として
[ruviform#398](https://github.com/scarecrow173/ruviform/issues/398) で
設計され、PR401（main `f483fe1c`、後続 RF403 `5d48298b`）として
統合された。

**修復済み（task5 → task6 で正規契約へ移行）**: `LocalFolderProvider`/
`LocalBlobDir` の全 read/stat/pin/open_preview/open_pinned/browse-descend
は取得済み `RootedDir` capability 経由の handle-relative lookup で、
capability 配下の path 再 traverse は存在しない。capability 取得は
原則 path-based（`LocalFolderProvider::new`・`LocalBlobDir` の per-op
store open）だが、scope browse 用 `open_scope` は ruviform#409 の
`open_scoped_root` で解決済み directory handle 自体を capability
floor とする scoped capability を取得する — pathname 再 open も
`NodeId`/strong-identity 照合も存在せず、root rename + 同一 pathname
への別 tree 設置は held handle が address する別 object に到達
できないため構造的に rebind できない（held tree を serve し、
weak identity FS も gate しない）。backend が handle 由来 canonical
path を持たない場合に限り Ruviform 文書上の残存 path→capability
race が残る（impl record 記録）。`WithinRoot` link
follow の契約は縮小していない（`NoFollow` への置換なし）。
`tests/containment_swap.rs` は swap 回帰を GREEN 強制
（scope 取得境界・root 置換・取得後 swap・seam sanity を含む）。
browse 列挙は ruviform#409 の bounded `read_dir_each` で kept
window・cancel/deadline を列挙段階へ適用する（task5 の RF API 待ち
BLOCKED 残余は解消）。
互換差（`RelativePath` の厳格文法、保守的 EscapesRoot、scope
capability 化による link 下降の同値化、dir byte_len の
NotApplicable 化、dangling link の Missing 統一、取得済み handle
の rename 後 authority 継続と root 置換時の held-tree serve）は
task5/task6 と impl record に記録する。

## Acceptance

- [ ] AC1: Local/Project の browse/stat/pin/open_pinned が同一正本と
  stable identity を示す。file 変更/rename/権限・symlink/traversal で
  未固定データを固定扱いしない（実テストで変異させて検証）。
- [ ] AC2: Browse/Search の paging・取消・cursor の
  Principal/Revision/Query 失効・0件 vs Partial/Offline/Stale・
  Unknown metadata が実テストで区別される。
- [ ] AC3: 非対応能力は typed `UnsupportedCapability`。CLI/headless
  で動作し GUI framework/Tokio SDK が公開 API に入らない。
- [ ] AC4: 既存 workspace の `mise run` 検証（fmt-check/check/lint/
  test）が回帰しない。Ruviform 変更なし・AudioLens 影響なしを
  記録する。

## Non-goals

- Importer/Schema v8/強 digest 移行・Provenance 永続化（SV86）。
- Catalog/Preview cache・Principal 分離・QoS（SV87）。
- Remote/VCS provider、WebDAV、fault matrix（#88〜#91）。
- `synvelis-player::import_local` 既存経路の変更・統合。
