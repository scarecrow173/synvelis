---
id: "SPEC-034X9CdwK3h8dbogUdZSER"
type: "spec"
status: "approved"
title: "Remote-aware Asset Library — metadata-first browse, pinned intent, verified import, offline/permission UX (synvelis#9)"
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
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
    - "docs/specs/alib-04-git-remote-provider.md"
    - "docs/specs/alib-06-webdav-provider.md"
    - "docs/specs/alib-07-fault-matrix.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/specs/alib-04-lfs-remote-fetch.md"
    - "docs/specs/alib-07-import-cancel.md"
    - "docs/specs/alib-02-residual-closeout.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/project-model.md"
    - "docs/designs/alib-01-asset-provider-contract-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/89"
    - "https://github.com/scarecrow173/synvelis/issues/91"
  derived-by:
    - "docs/designs/asset-library-design.md"
    - "docs/plans/asset-library.md"
    - "docs/test-specs/asset-library.md"
  verifies: []
  verified-by:
    - "docs/test-specs/asset-library.md"
  references: []
  defers: []
  deferred-by: []
---

# Remote-aware Asset Library — metadata-first browse / pin / verified import / offline・permission UX 設計契約 (synvelis#9)

## What Should Be Built

Synvelis の Browser を「ローカルフォルダのファイル選択」から、
Provider 非依存の metadata-first Asset Library へ進化させるための
製品レベル設計契約を確立する。対象は次の統合面である。

- Local / Project / Remote（Git・WebDAV・将来の FileServer 系）を
  単一の Browser 体験で選択・閲覧できる Source/Provider selector。
- メタデータ中心の一覧・検索・filter・paging/virtualization。
  未選択ファイルの本体を全量同期しない。
- Provider capability を正しく UI へ返す検索・revision・preview
  能力の表示（Git ホスト API と汎用 Git の能力差、SVN/P4/
  FileServer の Revision/検索 Capability を「不明/未対応」と
  誤魔化さず typed に返す）。
- Immutable な Pin Intent（Git は mutable ref → 固定
  Commit/Tree/blob、WebDAV は strong ETag + `If-Match`、
  LFS Pointer ≠ 実 Byte）。
- 検証済み Byte のみを Project へ Import する verified import
  （bounded streaming・SHA-256・staged readback・atomic
  publish・Project CAS・manifest/receipt recovery）。
- ImportIntent（import ID・pinned asset・expected Project
  revision・target track/lane/placement・copy policy・source
  principal reference）と import job 状態機械、Cancel/Resume/
  同一 ImportId replay。
- Offline・Partial・Stale・Unauthorized・Permission denied・
  credential revoked・Source changed・LFS missing・unsupported
  capability・unknown metadata・index incomplete を typed state
  として可視化し、「No Results」や成功と誤認させない UX。
- Cache（catalog/preview・非権威的）・Search index coverage・
  principal 分離・quota/TTL/eviction と non-RT QoS。
- v5 旧 Snapshot（CRC/opaque digest・Blob Path・SourceId/
  AssetId・Recording/PCM/Plugin Opaque Bytes）の安全な移行。
- 失敗 Case（LIB-ID/BROWSE/PIN/IMPORT/MIG/CACHE/SEC/QOS）と
  独立 Oracle・実装 Owner・evidence の対応台帳。

本 slice は docs のみを変更し、crate・CI・Cargo 設定・
Ruviform 共通契約には触れない。実装は #85（Provider/
Metadata-first/Handle Scope）・#86（Verified Import/Project
CAS/v5 Migration）・#87（Cache/Principal/Preview/Index
Coverage/QoS）・#88（Git/LFS/固定 Commit/Secret）・#89
（Browser/Import UX・GPUI）・#90（WebDAV/If-Match/実
Server Capability/Security）・#97（VCS snapshot 統合）・
#114（実 GPUI GUI）が Owner。

## Why Now

- ALIB-01〜07 の provider/import/cache/fault 契約は個別 spec
  として merge 済みだが、それらを Synvelis 製品の Browser UX・
  Import Intent・offline/permission 表現・受入台帳として束ねる
  正本がない。#9 はその Parent/Design Owner であり、#89（UX）・
  #91（fault matrix）の上位契約を必要とする。
- 「メタデータは見えるが Byte は未取得」「remote source が
  変わったが Project 内 Asset は不変」「offline でも Project
  再生は継続する」という製品不変条件を、実装 Issue ごとの
  部分契約から独立して固定しないと、UI が cache/metadata を
  verified asset と誤表示する退行を防げない。
- #91 の LIB-* fault matrix は本 spec の受入カテゴリ
  （ID/BROWSE/PIN/IMPORT/MIGRATION/CACHE-SEC/FAULT-OS）の
  検証側正本であり、両者の case ID・evidence 語彙をここで
  固定する必要がある。

## Users and Value

- 音楽制作者: remote/VCS 上の素材を metadata-first で探し、
  必要な revision だけを確実に Project へ取り込み、network
  断後も制作を継続できる。
- VCS 未接続ユーザー: local/project source のみで従来どおり
  DAW 制作・録音・保存・再生ができる（remote は opt-in）。
- 実装者・レビュア: Provider/Remote/Project/Blob の区別、
  pin/verify/commit の状態分離、failure の typed 表現を
  一つの正本から引ける。

## Scope

### In Scope

- Browser/Asset Library の製品要件: source/provider 選択、
  catalog/list/search/filter、coverage/provenance 表示、
  revision selector、metadata inspector、preview affordance、
  selected/pinned intent、import progress、cancel、
  conflict/retry/recovery、offline/permission messaging、
  pending drag/drop ghost。
- `ImportIntent` と import job 状態（Queued/Fetching/
  Verifying/Materializing/CommitPending/CommittedLive/
  CancelRequested/Cancelled/Failed/Recoverable/
  AlreadyApplied）の製品契約。
- 受入カテゴリ ID/BROWSE/PIN/IMPORT/MIGRATION/CACHE-SEC/
  FAULT-OS と LIB-* case ID の写像、fixture/oracle/Owner/
  evidence の台帳化方針。
- 既存 ALIB-01〜07 契約への参照・非重複・昇格禁止規則
  （headless/mock/fixture 結果を real provider/GUI/OS 受入へ
  昇格しない）。

### Out of Scope

- Provider Adapter・Project Controller 正本・cache/importer
  実装そのもの（#85〜#91 が Owner。本 spec はそれらの製品
  契約を束ねる）。
- Ruviform Rooted File/IO/Security 共通契約の定義・変更
  （参照のみ。重複実装禁止）。
- 実 GUI window 実装（#114）、実機/実 network/実 OS の
  受入実行（Deferred gate）。
- Cargo.toml/Cargo.lock/mise.toml/CI/本番 crate の変更。
- SVN/P4/SMB/S3 等の未採用 Provider の能力保証（typed
  `Unsupported`/`NotImplemented` のまま段階化）。

## Requirements

### 識別・来歴（LIB-ID）

- **AL-R-01** Provider/Remote source/Project Asset/Blob を
  別 identity として区別する。同一 Byte が別 Provider/別来歴
  から来た場合は別 AssetId としつつ、検証済み Blob は共有
  可能とする。Remote rename・branch move・Local rename は
  既存 Import/Pin を不変に保つ（Stable ID）。
- **AL-R-02** Project Asset は read-only projection であり、
  source 更新によって黙って差し替わらない。更新候補は
  provenance/diff として提示し、置換は明示操作のみ。

### Browse（LIB-BROWSE）

- **AL-R-03** Remote catalog は metadata-first の paging/
  virtualization で閲覧し、cursor+Principal+Revision の一致を
  要求する。未選択 file の本体を全量同期しない。
- **AL-R-04** No Results・Partial・Offline・Stale・
  Unsupported・IndexIncomplete・権限不足を相互に区別する
  typed coverage state を持ち、検索未完了を検索完了と
  誤認させない。検索 capability 非対応は empty ではなく
  `Unsupported` とする。
- **AL-R-05** Audio metadata が得られない場合は「不明」と
  表示し、preview に必要な Byte 量と Provider 制約を踏まえて
  取得する（全量先行取得しない）。

### Pin（LIB-PIN）

- **AL-R-06** Git の mutable branch/ref は固定
  Commit/Tree/blob identity へ解決して pin する。LFS
  Pointer text は media ではなく、実 Byte のみを verified
  対象とする。
- **AL-R-07** WebDAV は strong ETag + `If-Match` conditional
  read を要求し、weak/missing ETag は fail-closed で拒否
  する。pin 後の `412 Precondition Failed` は typed
  `SourceChanged` として扱う。
- **AL-R-08** pin と import の間で source が変化した場合、
  credential が失効した場合は typed failure とし、部分
  File・偽 Manifest を残さない。

### Import（LIB-IMPORT）

- **AL-R-09** Import は bounded-memory streaming で SHA-256
  検証・decode・metadata 抽出を行い、staged readback と
  atomic publish を経て Project CAS へ commit する。
  manifest/receipt により crash 後も復旧可能とする。
- **AL-R-10** `ImportIntent` は import ID・pinned asset・
  expected Project revision・target track/lane/placement・
  copy policy・source principal reference を保持し、
  job 状態機械（Queued→Fetching→Verifying→Materializing→
  CommitPending→CommittedLive、CancelRequested/Cancelled/
  Failed/Recoverable/AlreadyApplied）を typed に遷移する。
  同一 ImportId の replay は digest 一致時に冪等、digest
  相違時は拒否する。preview 成功は verified import・save
  完了を意味しない。
- **AL-R-11** cancel/resume・stale Project revision・target
  track/lane 消失・duplicate ImportId・indeterminate
  publish・Save/HEAD 失敗を個別の typed failure/recovery
  として扱い、CommittedLive と saved HEAD/SavedRevision を
  別状態として保持する。

### Migration（LIB-MIG）

- **AL-R-12** v5 snapshot の旧 CRC/opaque digest・Blob
  Path・SourceId/AssetId・Recording/PCM/Plugin Opaque
  Bytes を lossless に読み、byte-level 検証なしに verified
  SHA-256 identity へ昇格しない。schema 不明/欠落/不正は
  安全側（degraded/typed error）へ倒す。

### Cache/Security/QoS（LIB-CACHE/SEC/QOS）

- **AL-R-13** catalog/preview cache は非権威的で、TTL/LRU/
  byte budget・principal 分離・generation-aware 通知を持つ。
  eviction は正式 Project Asset を削除しない。auth
  revocation・cache clear を typed に扱う。
- **AL-R-14** path traversal・symlink・redirect・SSRF・
  credential forwarding・malformed Content-Length/compression
  bomb・log token 漏洩・Git subprocess argv を境界で検証
  し、失敗は typed にする。
- **AL-R-15** Browser/Import の network・disk・hash・
  blocking・allocation を audio callback（RT）で行わない。
  import は non-RT job であり、再生中の操作でも RT を
  block しない。

### UX 統合

- **AL-R-16** Browser は source/provider selector・catalog/
  list・search/filter・coverage/provenance indicator・
  revision selector・metadata inspector・preview affordance・
  selected/pinned intent・import progress・cancel・
  conflict/retry/recovery・offline/permission message・
  pending drag/drop ghost（Project Asset/Source/Clip record
  を verified import+commit 前に作らない）を提供する。
- **AL-R-17** 特定 revision の Audio を import すれば、
  network 切断後も Project 再生に必要な data が Project
  内に残る（copy into Project 既定）。VCS 未接続でも local
  素材 import と通常 DAW 制作は成立する。

## Acceptance Criteria

受入は #91 の LIB-* 分類に従い、各 case を独立 Oracle・
fixture・実装 Owner・evidence で判定する。headless/mock/
fixture の Pass を real provider/GUI/OS 受入へ昇格しない。

| # | 受入条件（Issue #9 転記） | 主な case class | Owner |
|---|--------------------------|-----------------|-------|
| AC-1 | Provider/Remote/Project/Blob 区別、同 Byte 別来歴、Local rename、Stable ID | LIB-ID-01〜04 | #85/#86 |
| AC-2 | paging/Query cursor+Principal+Revision 一致、No Results/Partial/Offline/Stale/Unsupported 区別 | LIB-BROWSE-01〜04 | #85/#87 |
| AC-3 | Git mutable branch→commit 固定、LFS Pointer≠実 Byte、WebDAV weak/strong ETag+If-Match、SourceChanged | LIB-PIN-01〜05 | #88/#90/#331 |
| AC-4 | bounded streaming/sha256+decode/Cancel/Resume/同 ImportId Replay/Stale ProjectRevision/Target deletion/atomic Project Command | LIB-IMPORT-01〜10 | #86/#89 |
| AC-5 | v5 旧 CRC/Blob Path/SourceId/AssetId/Recording/PCM/Plugin Opaque Bytes と Schema 不明/欠落/不正の安全な移行 | LIB-MIG-01〜05 | #86 |
| AC-6 | Search Index coverage、Auth/Principal cache isolation、Log/URL/Redirect/Path traversal、quota/TTL/eviction、nonRT QoS | LIB-CACHE-01〜05 / LIB-SEC-01〜05 / LIB-QOS-01〜04 | #87/#90 |
| AC-7 | Blob Published/DirSync/ACK lost、Import commit/Save HEAD 別状態、Windows/macOS/Linux、RT no-block/no-alloc、AudioLens 精度/性能/GUI 非回帰を個別記録 | LIB-IMPORT-08〜10 / LIB-QOS-03〜04 | #91/#97 |
| AC-8 | Remote 多数 file でも metadata 中心 paging、未選択本体の全量同期なし | LIB-BROWSE-01〜02 | #85/#87 |
| AC-9 | Git ホスト API と汎用 Git の能力差、SVN/P4/FileServer の Revision/検索 Capability を UI へ正しく返す | LIB-BROWSE-03 / capability matrix | #85/#88/#90 |
| AC-10 | Audio metadata 不明時は不明表示、preview は必要 Byte 量と Provider 制約を踏まえ取得 | LIB-BROWSE-02 / preview contract | #85/#87 |
| AC-11 | 特定 Revision import 後 network 切断でも Project 再生に必要 data が残る | LIB-IMPORT-10 / offline playback | #86/#87 |
| AC-12 | 同名/別 Revision/LFS 欠落/サーバー変更/通信断/認証拒否でも部分 File・偽 Manifest を残さず正確に失敗・復旧 | LIB-PIN/IMPORT/SEC 各 class | #86/#88/#90/#91 |
| AC-13 | VCS Source 更新後も既 Import Asset は勝手に差し替わらず、来歴・差分/更新候補を確認可能 | LIB-ID-02 / update-availability | #89/#97 |
| AC-14 | Browser/Import は再生中でも Audio Callback を block しない | LIB-QOS-01〜02 | #87/#91 |
| AC-15 | VCS 未接続でも local 素材 Import・通常 DAW 制作が可能 | local-source regression | #85/#89 |
| AC-16 | Remote Search の索引未完了/Offline/Stale を区別し検索完了と誤解させない | LIB-BROWSE-03〜04 | #87 |

## Deferred Design Concerns

- 実 Git remote・実 WebDAV server・実 credential store との
  E2E: #88/#90/#331 の provider gate で実施。本 spec は
  契約のみ固定する。
- 実 GPUI Browser 画面の操作受入: #114。pending drag ghost
  の見た目・実 window 挙動は実装 gate。
- OS 別（Windows/macOS/Linux）実機 FS capability・実
  device・実 network の L2/L3 受入: POLICIES.md の検証
  レベルに従い、利用される機能のみ Required、他は
  Deferred。
- SVN/P4/SMB/S3 等の追加 Provider: capability を typed
  `Unsupported`/`NotImplemented` で返す段階化。全 Provider
  対応を完成と呼ばない。
- Ruviform Rooted File/IO/Security の共通契約変更:
  Ruviform 側 Issue（#219/#294 等）が Owner。本 spec は
  参照関係のみ記録する。

## Sources and Relations

- `https://github.com/scarecrow173/synvelis/issues/9` — 本
  spec の正本（Parent/Design Owner）。
- `https://github.com/scarecrow173/synvelis/issues/89` —
  Browser/Import UX 実装 Issue（本 spec の UX 契約を消費）。
- `https://github.com/scarecrow173/synvelis/issues/91` —
  LIB-* fault matrix 受入 Issue（本 spec の AC を検証）。
- `https://github.com/scarecrow173/synvelis/issues/85`〜`#90`・
  `#97`・`#114`・`#331`・`#84` — provider/import/cache/
  UX/GUI/VCS/LFS/FS gate の実装・受入 Owner。
- `docs/specs/alib-01-asset-provider-contract.md` —
  `AssetProvider`/`PinnedAssetRef`/capabilities/cursor・
  coverage semantics の provider 境界正本。
- `docs/specs/alib-02-streaming-verified-import.md`・
  `docs/specs/alib-02-v5-snapshot-compat.md`・
  `docs/specs/alib-02-residual-closeout.md` — bounded
  streaming verified import・SHA-256・staged readback・atomic
  publish・v5 互換の正本。
- `docs/specs/alib-03-catalog-preview-cache.md` — 非権威的
  catalog/preview cache・TTL/LRU/byte budget・principal
  分離・offline/stale/partial coverage の正本。
- `docs/specs/alib-04-git-remote-provider.md`・
  `docs/specs/alib-04-lfs-remote-fetch.md` — Git 固定
  Commit/Tree/blob・selective fetch・LFS pointer vs 実 Byte・
  safe argv/credential の正本。
- `docs/specs/alib-06-webdav-provider.md` — WebDAV
  metadata-first browse・strong ETag + `If-Match`・weak/
  missing ETag fail-closed の正本。
- `docs/specs/alib-07-fault-matrix.md`・
  `docs/specs/alib-07-import-cancel.md` — LIB-* case 分類と
  import cancel/resume の正本。
- `docs/specs/project-persistence.md`・
  `docs/specs/project-model.md` — Project CAS・manifest/
  receipt・SavedRevision/HEAD の永続化正本。
