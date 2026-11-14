---
id: "DESIGN-034X9CpHP2HhkIZ6GzyZI6"
type: "design"
status: "approved"
title: "Remote-aware Asset Library Browser/Import UX 設計 (synvelis#9)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/89"
    - "https://github.com/scarecrow173/synvelis/issues/91"
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
    - "docs/specs/asset-library.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/alib-01-asset-provider-contract-design.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
    - "docs/specs/alib-04-git-remote-provider.md"
    - "docs/specs/alib-06-webdav-provider.md"
    - "docs/specs/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/asset-library.md"
  derived-by:
    - "docs/plans/asset-library.md"
    - "docs/test-specs/asset-library.md"
  verifies: []
  verified-by:
    - "docs/test-specs/asset-library.md"
  references:
    - "evidence/sv9/asset-library-case-ledger.md"
    - "evidence/sv9/asset-library-verification.md"
  defers: []
  deferred-by: []
---

# Remote-aware Asset Library — Browser/Import UX・状態・失敗 Case 設計 (synvelis#9)

## Context

`synvelis#9` は Asset Library を metadata-first・remote-aware な
Browser へ進化させる Parent/Design Owner である。provider 境界
（ALIB-01）・verified import（ALIB-02）・cache（ALIB-03）・
Git/WebDAV provider（ALIB-04/06）・fault matrix（ALIB-07）は
個別 spec/design/plan/task/test-spec/impl 文書鎖として merge 済み
であり、本設計はそれらを Synvelis 製品の Browser UX・Import
Intent・offline/permission 表現・受入台帳として束ねる。実装
Owner は #85（provider/metadata-first/handle scope）・#86
（verified import/Project CAS/v5 migration）・#87（cache/
principal/preview/index coverage/QoS）・#88（Git/LFS/固定
Commit/secret）・#89（Browser/Import UX・GPUI）・#90（WebDAV/
If-Match/実 server capability/security）・#97（VCS snapshot
統合）・#114（実 GPUI GUI）。本 slice は docs のみで crate・
CI・Cargo 設定・Ruviform 共通契約を変更しない。

## Scope

### In Scope

- Browser 画面構成と各 UI 要素が消費する typed state の対応表。
- `ImportIntent`・import job 状態機械・Pin 状態・coverage
  state の製品契約。
- LIB-ID/BROWSE/PIN/IMPORT/MIG/CACHE/SEC/QOS の失敗 Case 分類と
  独立 Oracle/実装 Owner/evidence の対応方針。
- 既存 ALIB 契約への参照・非重複・昇格禁止規則。

### Out of Scope

- Provider Adapter・cache/importer・Project Controller の実装
  （#85〜#91 Owner）。
- 実 GPUI window・実描画・実 OS/実 network の受入実行。
- Ruviform Rooted File/IO/Security 共通契約の定義・変更。
- Cargo/CI/本番 crate の変更。

## Components and Responsibilities

### Owner 分離（固定表）

| 構成要素 | Owner | 責務 | 非責務 |
|----------|-------|------|--------|
| Browser/Asset Library UX | Synvelis app（#89・#114） | source/provider selector・catalog/list/search/filter・coverage/provenance indicator・revision selector・metadata inspector・preview affordance・selected/pinned intent・import progress/cancel/conflict/retry/recovery・offline/permission message・pending drag ghost | provider protocol・byte 検証・Project CAS の正本を持たない |
| AssetProvider 境界 | `synvelis-library`（#85・ALIB-01） | `AssetProvider`/`PinnedAssetRef`/capabilities/cursor/coverage semantics・metadata-first browse・selective fetch | Project Asset の永続化・UI 表示判断を持たない |
| Verified Importer | `synvelis-library`/`synvelis-project`（#86・ALIB-02） | bounded streaming・SHA-256・decode/metadata・staged readback・atomic publish・manifest/receipt recovery・Project CAS commit | remote protocol・cache eviction・UI 状態機械を持たない |
| Catalog/Preview Cache | `synvelis-library`（#87・ALIB-03） | 非権威的 cache・TTL/LRU/byte budget・principal 分離・generation-aware 通知・offline/stale/partial coverage | verified asset・Project asset の正本を持たない |
| Git Provider | `synvelis-library`（#88・ALIB-04・#331） | 固定 Commit/Tree/blob identity・selective fetch・LFS pointer vs 実 Byte・safe argv/credential | WebDAV・generic FileServer・UI を持たない |
| WebDAV Provider | `synvelis-library`（#90・ALIB-06） | metadata-first browse・strong ETag + `If-Match`・weak/missing ETag fail-closed | Git・cache policy・UI を持たない |
| Project Controller/Persistence | `synvelis-project`（#86・project-persistence spec） | Project CAS・manifest/receipt・SavedRevision/HEAD・Import commit の単一 writer | remote browse・cache・provider protocol を持たない |
| VCS Snapshot 統合 | `synvelis`（#97・VCS-06） | VCS immutable history・missing LFS・snapshot 解決との交差 | ALIB provider 個別契約の重複を持たない |
| Ruviform Rooted File/IO/Security | Ruviform（#84・#219・#294） | Rooted handle・IO/Security 共通契約 | Synvelis 固有の Asset/Import 意味論を持たない |
| 検証 Harness | `synvelis-library`/`synvelis-project` test（#91・ALIB-07） | LIB-* case の fixture/oracle・fault matrix・evidence 集約 | 実装本体・UI・real provider 受入を兼ねない |

### 要件写像（spec AL-R-01〜17 → 本設計）

| Spec 要件 | 設計上の対応 |
|-----------|--------------|
| AL-R-01/02 識別・来歴 | identity 分離表・provenance/update-availability・read-only projection |
| AL-R-03/04/05 browse | metadata-first paging・coverage state 機械・capability matrix・metadata inspector |
| AL-R-06/07/08 pin | pin state 機械・Git 固定 identity・WebDAV strong ETag+If-Match・SourceChanged |
| AL-R-09/10/11 import | ImportIntent 型・job 状態機械・idempotent replay・cancel/resume・staged readback |
| AL-R-12 migration | v5 互換・lossless read・unverified 昇格禁止・degraded/typed error |
| AL-R-13/14/15 cache/sec/qos | cache 分類・principal 分離・境界検証・non-RT job 分離 |
| AL-R-16/17 UX 統合 | Browser 画面対応表・pending ghost・offline/permission message・local-first |

## Key Contracts

### 識別・来歴

- `Provider + pinned source identity` が Asset の source-level
  identity。content identity は検証済み byte の exact SHA-256。
- 同一 byte が別 Provider/別来歴から来た場合は別 AssetId とし、
  検証済み Blob は共有可能（ALIB-01/ALIB-02 v5 互換を継承）。
- Remote rename・branch move・Local rename は既存 pin/import を
  不変に保つ。Project Asset は read-only projection で、source
  更新は provenance/diff の更新候補として提示し、置換は明示操作
  のみ。

### Coverage / Capability state（typed・相互排他ではない複合可）

`BrowseCoverage` は少なくとも次を区別する: `Complete`・
`Partial{indexed, total_hint}`・`Stale{as_of, reason}`・
`Offline{last_known}`・`IndexIncomplete{progress}`・
`Unsupported{capability}`・`Unauthorized{scope}`・
`PermissionDenied{scope}`・`Empty`（真の 0 件のみ）。
検索 capability 非対応は `Unsupported` であり `Empty` としない。
UI はこれらを個別 icon/label で表示し、「No Results」へ丸めない。

`ProviderCapabilities` は search/index・revision listing・
preview fetch・conditional read（ETag/If-Match）・LFS 実
Byte 取得・max page size・credential 方式を typed に返す。
Git ホスト API と汎用 Git、SVN/P4/FileServer の能力差を
`Unsupported`/`Unknown` として正直に返し、UI は capability
に応じて affordance を出し分ける。

### Pin 状態

`PinState` = `Unpinned` → `Resolving` → `Pinned{provider,
source_identity, revision_identity, content_hint}` または
`PinFailed{typed_reason}`。Git は mutable ref を固定
Commit→Tree→blob へ解決し、LFS pointer text は media では
なく `lfs_pointer` capability として扱う。WebDAV は strong
ETag のみ pin 可能とし、weak/missing は fail-closed。
pin 後の source 変化は `SourceChanged`（WebDAV では `412`）、
credential 失効は `Unauthorized`/`CredentialRevoked` として
import 前に検出する。

### ImportIntent / job 状態機械

`ImportIntent` は `import_id`（idempotency key）・
`pinned_asset`（provider+source identity+revision）・
`expected_project_revision`（Project CAS の base）・
`target{track, lane, placement}`・`copy_policy`（既定
`CopyIntoProject`）・`source_principal_ref` を保持する。

job 状態: `Queued` → `Fetching` → `Verifying` →
`Materializing` → `CommitPending` → `CommittedLive`。
副状態: `CancelRequested`・`Cancelled`・`Failed{typed}`・
`Recoverable{resume_token}`・`AlreadyApplied{import_id}`。
`CommittedLive` は Project commit 済みを意味し、saved
HEAD/SavedRevision とは別状態として UI に表示する
（import 成功 ≠ save 完了）。同一 `import_id` の replay は
intent digest 一致時に冪等（`AlreadyApplied` または resume）、
digest 相違時は `Failed{ImportIdConflict}`。

### Cache / Security / QoS

catalog/preview cache は非権威的・principal 分離・TTL/LRU/
byte budget・generation-aware 通知を持ち、eviction は正式
Project Asset を削除しない。path traversal・symlink・
redirect・SSRF・credential forwarding・malformed
Content-Length/compression bomb・log token 漏洩・Git
subprocess argv は provider/importer 境界で検証する。
network・disk・hash・blocking・allocation は全て non-RT job
に限定し、audio callback からは enqueue のみ行う。

### Migration（v5 互換）

v5 snapshot の旧 CRC/opaque digest・Blob Path・SourceId/
AssetId・Recording/PCM/Plugin Opaque Bytes を lossless に読む。
byte-level 検証なしに verified SHA-256 identity へ昇格せず、
schema 不明/欠落/不正は degraded/typed error へ倒す。
Golden（PCM/Undo/Source/Recording/Plugin Opaque）の byte を
変更しない。

## Data and Control Flow

### Browse → Pin → Preview → Import → Commit

1. User が source/provider を選択 → `AssetProvider` が
   metadata-first catalog を cursor paging で返す。coverage
   state・capability・provenance を UI へ返す。
2. User が entry を選択 → metadata inspector に表示。不明
   metadata は「不明」。preview は別 affordance で、必要
   byte 量と provider 制約を表示してから fetch する。
3. User が pin/import を選択 → `ImportIntent` を構築し
   `PinnedAssetRef` を確定（Git は ref→commit/tree/blob、
   WebDAV は strong ETag）。
4. Import job が `Fetching→Verifying→Materializing→
   CommitPending→CommittedLive` を遷移。各段で typed
   failure/recovery。cancel は `CancelRequested→Cancelled`
   または安全な `Recoverable` resume。
5. Project Controller が CAS で commit。`CommittedLive` と
   saved HEAD/SavedRevision は別表示。crash/ACK lost は
   manifest/receipt で `Recoverable` へ復旧。

### Offline / Partial 経路

- Remote offline: catalog cache（あれば `Stale`/`Offline`
  表示）・既 import Project asset はそのまま再生可能。
- Preview fetch 失敗: metadata は表示維持、preview のみ
  typed failure。verified asset と誤表示しない。
- Import 途中の network 断: `Recoverable` で resume token
  を保持、部分 file・偽 manifest を残さない。

## State Machines

### BrowseCoverage（複合状態）

`Loading` → `Complete` / `Partial` / `Stale` / `Offline` /
`IndexIncomplete` / `Unsupported` / `Unauthorized` /
`PermissionDenied` / `Empty`。`Stale` は cache hit +
refresh 失敗、`Offline` は network 不通、`IndexIncomplete`
は search index 未完了を意味し相互に区別する。

### PinState

`Unpinned` → `Resolving` → `Pinned` / `PinFailed{reason}`。
`Pinned` は immutable revision identity を保持し、後続の
source 更新で自動的に `SourceChanged` 候補へは遷移しない
（import 時に conditional read/commit pin で検出）。

### ImportJob

`Queued` → `Fetching` → `Verifying` → `Materializing` →
`CommitPending` → `CommittedLive`。`CancelRequested` は
任意状態から遷移可能で `Cancelled` または `Recoverable`
へ。`Failed{typed}` は retry 可否を typed reason で表す。
`AlreadyApplied` は同一 import_id replay の冪等終端。

## Failure taxonomy（LIB-* 写像）

| 分類 | case 例 | typed failure | recovery |
|------|---------|---------------|----------|
| LIB-ID | 同 byte 別 provider・remote rename・local rename・symlink race | `IdentityConflict`/`StaleHandle` | pin 再解決・cache invalidate |
| LIB-BROWSE | cursor 継続・search unsupported・stale/offline・principal 変更 | `Unsupported`/`Stale`/`Offline`/`Unauthorized` | cursor 再発行・index rebuild |
| LIB-PIN | branch→commit・LFS pointer・weak/missing ETag・412・auth revoke | `SourceChanged`/`WeakValidator`/`CredentialRevoked` | re-pin・credential 再取得 |
| LIB-IMPORT | chunked huge・sha mismatch・cancel 各相・dup import id・stale revision・target 消失・blob publish/ack lost・save/head 別・offline playback | `HashMismatch`/`ImportIdConflict`/`StaleRevision`/`TargetGone`/`IndeterminatePublish`/`SavePending` | resume/retry/`AlreadyApplied` |
| LIB-MIG | v5 read/upgrade・旧 CRC vs SHA256・missing blob・collision・opaque bytes | `Degraded`/`UnknownSchema`/`UnverifiedDigest` | degraded read・manual re-import |
| LIB-CACHE | principal isolation・TTL/LRU/lease・auth revoke/clear・preview full-fetch・huge catalog | `CacheEvicted`/`PrincipalMismatch`/`PreviewTooLarge` | refetch・budget 調整 |
| LIB-SEC | traversal/symlink/redirect・SSRF・credential forwarding・malformed length/compression・log token・git argv | `PathRejected`/`SsrfBlocked`/`CredentialLeakPrevented`/`ArgvUnsafe` | reject・redact・retry |
| LIB-QOS | RT no block/alloc・capture/prefetch priority・f64/PCM  golden・OS/GUI 分離 | `RtViolation`/`BudgetExceeded` | non-RT job へ退避・budget 調整 |

## UX 行動仕様（#89 消費契約）

- Source/Provider selector: Local/Project/各 remote source を
  列挙。offline/unauthorized は selector 段階で typed badge。
- Catalog/List: metadata-first・virtualized。coverage badge
  （Partial/Stale/Offline/IndexIncomplete/Unsupported/
  Unauthorized/PermissionDenied）を常時表示。
- Search/Filter: capability-aware。`Unsupported` は empty
  ではなく理由付き disabled/typed message。
- Revision selector: Git は commit history、WebDAV は
  ETag revision、local は fs revision。mutable ref は
  「最新」と「固定」を区別。
- Metadata inspector: audio metadata 不明は「不明」。
  provenance（provider・source identity・revision・
  content hint）を表示。
- Preview affordance: 「preview fetch」明示・必要 byte 量・
  provider 制約を表示。preview 成功 ≠ verified import。
- Selected/Pinned intent: pin 状態・import intent の
  target/copy policy を inspector に表示。
- Import progress: job 状態機械をそのまま progress
  として表示。`CommittedLive` と saved HEAD を区別。
- Cancel/Conflict/Retry/Recovery: `CancelRequested`・
  `Recoverable`・`AlreadyApplied`・`Failed{reason}` を
  個別 affordance。conflict（ImportIdConflict/StaleRevision/
  SourceChanged）は user 判断を求める。
- Offline/Permission message: offline・unauthorized・
  permission denied・credential revoked を typed message
  で表示し、cache 利用可否を明示。
- Pending drag/drop ghost: drag 中は ghost のみ。Project
  Asset/Source/Clip record は verified import+commit 完了
  まで作らない。

## Risks

- coverage state の複合性が UI を複雑にする → typed state
  を単一 model に集約し、icon/label 写像を一箇所に置く。
- preview と verified import の混同 → 両者を別 affordance・
  別 state として固定し、test-spec で保証。
- headless/mock Pass の real 受入への誤昇格 → evidence
  ledger で mode/layer/required を分離し、#91 gate でのみ
  real 判定。
- ImportId replay の digest 設計ミス → intent 全 field の
  canonical digest を idempotency key に含める。

## Deferred Design Concerns

- 実 GPUI window の具体的 layout/実描画: #114。
- 実 Git remote/WebDAV server/credential store の E2E:
  #88/#90/#331。
- OS 別実機 FS capability・実 device・実 network の L2/L3:
  POLICIES.md に従い利用時まで Deferred。
- SVN/P4/SMB/S3 等の追加 Provider capability: typed
  `Unsupported`/`NotImplemented` のまま段階化。
- Ruviform Rooted File/IO/Security の共通契約変更:
  Ruviform 側 Issue Owner。

## Verification notes

- 本設計の検証は `docs/test-specs/asset-library.md` と
  `evidence/sv9/` の ledger に委ねる。実装検証は #91 の
  LIB-* matrix と各 provider Issue の gate で行う。
- docs-only slice のため `mise run fmt-check` と
  doc-status/doc-maintenance/impl-doc audit が本 slice の
  検証。実 crate の check/test/build は対象外。
