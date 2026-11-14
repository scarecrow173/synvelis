---
id: "EXP-034X9Dk2mNpLw8dYh4cJx"
type: "experiment"
status: "completed"
title: "Asset Library 文書鎖・受入写像の試行 (synvelis#9)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/9"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/asset-library.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/asset-library-design.md"
    - "docs/specs/asset-library.md"
    - "docs/test-specs/asset-library.md"
    - "docs/specs/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/asset-library.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/asset-library-task.md"
    - "docs/test-specs/asset-library.md"
    - "evidence/sv9/asset-library-case-ledger.md"
    - "evidence/sv9/asset-library-verification.md"
  defers: []
  deferred-by: []
---

# Asset Library — Issue #9 文書鎖・受入写像の試行

## 問題設定

Issue #9 の Remote-aware Asset Library 要件（metadata-first
remote browse・pinned intent・verified import・offline/
permission UX・失敗 Case）を canonical 文書鎖として確立する
必要があった。ALIB-01〜07 の provider/import/cache/fault 契約・
#89 UX・#91 fault matrix・#88/#90/#331 provider gate・#97 VCS
snapshot・#114 実 GUI・Ruviform 共通契約は存在するが、それらを
Synvelis 製品の Browser UX・ImportIntent・coverage/capability
typed state・LIB-* 受入台帳として束ねる正本がなかった。本
slice は docs のみで crate を変更しないため、(a) 7 文書鎖の
生成、(b) AL-R-01〜17/AC-1〜16 の LIB-* 写像、(c) 全受入
case の fixture/oracle/Owner/mode/layer/status 分類、(d) doc
graph/index/review の整合確認が試行対象となった。

## 試行と結果

### E1: 上流契約と実装の照合

Issue #9 本文を正本として読み、12 受入条件を ID/BROWSE/PIN/
IMPORT/MIGRATION/CACHE-SEC/FAULT-OS・UX の要件へ整理した。
`docs/specs/alib-01-asset-provider-contract.md`（AssetProvider/
PinnedAssetRef/capabilities/cursor/coverage）・
`docs/specs/alib-02-streaming-verified-import.md`（bounded
streaming/SHA-256/staged readback/atomic publish/Project CAS）・
`docs/specs/alib-02-v5-snapshot-compat.md`（旧 CRC/opaque
digest の lossless 互換）・`docs/specs/alib-03-catalog-preview-
cache.md`（非権威的 cache/principal 分離/offline/stale/
partial coverage）・`docs/specs/alib-04-git-remote-provider.md`
（固定 Commit/Tree/blob/LFS pointer vs 実 Byte）・
`docs/specs/alib-04-lfs-remote-fetch.md`（LFS Batch API/HTTP
download）・`docs/specs/alib-06-webdav-provider.md`（strong
ETag+If-Match/weak・missing fail-closed）・
`docs/specs/alib-07-fault-matrix.md`・
`docs/specs/alib-07-import-cancel.md`（LIB-* case 分類・import
cancel/resume）を AL-R-01〜17 の根拠として照合した。#89
（Browser/Import UX）・#91（LIB-* 受入 matrix）・#85〜#90
（provider/import/cache）・#97（VCS snapshot）・#114（実
GUI）・#331（LFS remote fetch）・#84（File/FS cross-gate）・
Ruviform #219/#294（consumer 互換）を Owner/Deferred として
固定した。

### E2: Owner 境界の固定

Browser/Asset Library UX（#89・#114）・AssetProvider 境界
（#85・ALIB-01）・Verified Importer（#86・ALIB-02）・
Catalog/Preview Cache（#87・ALIB-03）・Git Provider（#88・
ALIB-04・#331）・WebDAV Provider（#90・ALIB-06）・Project
Controller/Persistence（#86・project-persistence）・VCS
Snapshot 統合（#97）・Ruviform Rooted File/IO/Security・
検証 Harness（#91・ALIB-07）を Owner 表で固定し、第二正本・
Ruviform 重複・cache/metadata の verified asset 誤認を防ぐ
原則を design に固定した。

### E3: typed state と evidence honesty の分類規則

coverage state（Complete/Partial/Stale/Offline/
IndexIncomplete/Unsupported/Unauthorized/PermissionDenied/
Empty）・capability・pin state・import job 状態機械を typed
に固定し、「No Results」への退行を防ぐ規則を確立した。
要求状態（Required/Deferred/NotApplicable）と実行結果
（Pass/Fail/NotRun/Unsupported/NotImplemented/Unknown）を
別軸で管理し、headless/mock/fixture の Pass を実 Provider/
GUI/OS/device 受入に繰り上げない規則を確立した。POLICIES.md
の OS-L1/GUI-L2/EXT-L3・Required/Deferred/NotApplicable・
Pass/Fail/NotRun/Unsupported と整合させた。

### E4: 文書鎖生成と index/relation 整合

spec→design→plan→task→test-spec→impl-ir/exp の 7 文書鎖を
canonical front matter（id/type/status/title/created/updated/
owners/relations）で生成し、relations（implements/
derived-by/verifies/references/depends-on 等）を相互に整合
させた。`docs/{specs,designs,plans,tasks,test-specs,impl/ir,
impl/exp}/README.md` は `doc_maintenance.js index apply`・
`sync_impl_index.js` で再生成し、手編集しない。
`evidence/sv9/` の case ledger と verification を正本とし
て配置した。

## 採用した判断

- **A-1 7 文書鎖 + evidence/sv9/**: command-gateway（#45）・
  product-value-core（#1）の先例に従い canonical 配置。
- **A-2 AL-R-01〜17 + AC-1〜16**: Issue #9 の 12 受入条件を
  要件番号と acceptance criteria へ整理し、#91 の LIB-*
  分類へ写像。
- **A-3 typed state 固定**: coverage/capability/pin/import
  job を design で固定し、preview 成功 ≠ verified import・
  CommittedLive ≠ saved HEAD を明記。
- **A-4 status 語彙**: 要求状態と実行結果を別軸、mode/layer/
  required を ledger に明記。headless/mock/fixture Pass を
  real 受入に繰り上げない。
- **A-5 impl/exp canonical 名**: `docs/impl/ir/asset-
  library.md`・`docs/impl/exp/asset-library.md`（command-
  gateway 先例一致）。scaffold の slug 名は canonical 名へ
  rename。
- **A-6 docs-only slice**: crate・CI・Cargo・Ruviform 共通
  契約を変更せず、実装は #85〜#91 Owner へ委譲。

## 結論

Issue #9 の設計契約は docs/ 7 文書鎖 + `evidence/sv9/` と
して確立した。AL-R-01〜17・AC-1〜16・LIB-* case 分類・
Owner 境界・typed state・evidence honesty を固定し、実装は
#85〜#91・受入は #91 fault matrix・provider/GUI/OS gate へ
委譲した。本 slice は docs + evidence のみで、実装コード・
GUI・実 device/provider/OS 受入は対象外。
