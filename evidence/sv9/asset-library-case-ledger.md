# Asset Library — LIB Case Ledger (synvelis#9)

Issue #9 の受入カテゴリ（ID/BROWSE/PIN/IMPORT/MIGRATION/
CACHE-SEC/FAULT-OS）と #91 の LIB-* case 分類について、
fixture・oracle・実装 Owner・mode/layer・status・証拠を
台帳化する。**本 slice は docs/ 設計契約 + evidence のみ**
であり、実装・GUI・実 device/provider/OS 受入は対象外。
実装 case は `NotRun`（#85〜#91 が Owner）であり、既存
ALIB contract fixture が存在する case はその旨を記すが、
#9 製品レベルの受入・#91 aggregate gate の Pass とはしない。

## Status 語彙

| 軸 | 値 | 意味 |
|---|---|---|
| 要求状態 | `Required` | 現行必須 |
| 要求状態 | `Deferred` | 利用条件成立まで保留（実機/GUI/Provider lane） |
| 要求状態 | `NotApplicable` | 対象機能では不要 |
| 実行結果 | `Pass` | 実行した検証が成功（mode/layer を明記・headless/mock/fixture を実環境と混同しない） |
| 実行結果 | `Fail` | 検証が失敗 |
| 実行結果 | `NotRun` | 未実装または未検証 |
| 実行結果 | `Unsupported` | harness/環境が技術的に非対応（根拠を伴う） |
| 実行結果 | `NotImplemented` | Provider/能力が未実装（段階化・typed） |
| 実行結果 | `Unknown` | 判定不能（証拠不足） |

headless/mock/fixture の Pass を実 Provider/GUI/OS/device
受入に繰り上げない。設計レビュー Pass と実測 Pass を区別する。

## 基準 revision

| 項目 | 値 |
|---|---|
| repo/commit SHA | `86715f9ac2761f9f275e885e0bbd7ee775d9633e`（branch base `origin/main`） |
| Ruviform resolved SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock` git dep） |
| OS/FS/backend | docs 検証は Windows（worktree）・docs-only。実装 case の OS/FS/backend は #91 gate で記録 |
| input/fixture | `crates/synvelis-library/tests/` の ALIB fixture（下記 Fixture 凡例） |

## AL-R → LIB 分類 写像（spec 要件 → 受入ケース）

| AL-R | spec 要件 | 主な LIB 分類 |
|---|---|---|
| AL-R-01/02 | 識別・来歴・read-only projection | LIB-ID-01〜04 |
| AL-R-03/04/05 | metadata-first browse・coverage・metadata inspector | LIB-BROWSE-01〜04 |
| AL-R-06/07/08 | pin（Git 固定・LFS・WebDAV ETag・SourceChanged） | LIB-PIN-01〜05 |
| AL-R-09/10/11 | verified import・ImportIntent・job 状態・cancel/resume | LIB-IMPORT-01〜10 |
| AL-R-12 | v5 migration・lossless・unverified 昇格禁止 | LIB-MIG-01〜05 |
| AL-R-13/14/15 | cache・security・non-RT QoS | LIB-CACHE-01〜05・LIB-SEC-01〜05・LIB-QOS-01〜04 |
| AL-R-16/17 | Browser UX・offline/permission・local-first | #89 UX gate・LIB-BROWSE/IMPORT の UI 面 |

## mode / layer 凡例

- `mode`: `mock`（in-process・controlled doubles）/ `fixture`
  （headless test fixture）/ `real`（real provider・real FS・
  real network・real GUI）。
- `layer`: L1 Unit/Contract・L2 Component Integration・
  L3 In-process E2E・L4 Process/Fault E2E・L5 System Regression・
  L2-OS GUI/EXT（POLICIES.md の GUI-L2/EXT-L3 相当）。
- `required`: #91 の required flag。real provider/GUI/OS lane
  は `Deferred`（利用条件成立まで）。

## Fixture 凡例（実在 file — `crates/synvelis-library/tests/`）

- `contract.rs`・`types.rs` — ALIB-01 provider 境界・型契約
- `local.rs`・`project.rs` — local/project provider
- `git.rs`・`lfs.rs`・`vcs03_git_provider.rs`・
  `vcs06_git_matrix.rs` — ALIB-04 Git/LFS provider
- `webdav.rs` — ALIB-06 WebDAV provider
- `import.rs`・`import_recovery.rs` — ALIB-02 verified import・
  recovery
- `cache.rs`・`rt_cache.rs` — ALIB-03 catalog/preview cache・RT
- `containment_swap.rs` — containment/symlink/path 境界
- `common/` — 共有 fixture helper

## LIB-ID-01〜04（識別・来歴）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| LIB-ID-01 | 同 byte 別 provider は別 AssetId・Blob 共有可 | `contract.rs`・`types.rs`・#91 oracle | #85/#86・#91 | fixture/L1 | **NotRun**（#9 製品受入）・ALIB-01 contract Pass（別 evidence） |
| LIB-ID-02 | Remote rename/branch move が既 import 不変・Stable ID | `git.rs`・`vcs03_git_provider.rs`・#91 oracle | #88・#91 | fixture/L1・real git L2-Deferred | **NotRun** |
| LIB-ID-03 | Local path symlink/race の安全な扱い | `local.rs`・`containment_swap.rs`・#91 oracle | #85・#91 | fixture/L1 | **NotRun** |
| LIB-ID-04 | Project Asset read-only projection | `project.rs`・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |

## LIB-BROWSE-01〜04（metadata-first browse・coverage）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| LIB-BROWSE-01 | cursor pagination/continuation・metadata-first | `contract.rs`・`browse.rs`(src)・#91 oracle | #85・#91 | fixture/L1 | **NotRun** |
| LIB-BROWSE-02 | search `Unsupported` vs empty・stale/offline/partial/never indexed 区別 | `cache.rs`・`contract.rs`・#91 oracle | #85/#87・#91 | fixture/L1 | **NotRun** |
| LIB-BROWSE-03 | principal scope 変更と cursor invalidation | `cache.rs`・`rt_cache.rs`・#91 oracle | #87・#91 | fixture/L1 | **NotRun** |
| LIB-BROWSE-04 | metadata 不明は「不明」・preview は必要 byte 量と provider 制約を踏まえ取得 | `preview.rs`(src)・`cache.rs`・#91 oracle | #85/#87・#91 | fixture/L1 | **NotRun** |

## LIB-PIN-01〜05（immutable pin）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| LIB-PIN-01 | Git fixed commit/tree・mutable branch→commit 固定 | `git.rs`・`vcs03_git_provider.rs`・#91 oracle | #88・#91 | fixture/L1・real git L2-Deferred | **NotRun** |
| LIB-PIN-02 | Git LFS real bytes（pointer≠media・missing は NotFound） | `lfs.rs`・#331・#91 oracle | #88/#331・#91 | fixture/L1・real LFS L2-Deferred | **NotRun** |
| LIB-PIN-03 | WebDAV strong ETag + `If-Match`・weak/missing fail-closed | `webdav.rs`・#91 oracle | #90・#91 | fixture/mock L1・real server L2-Deferred | **NotRun** |
| LIB-PIN-04 | mutable branch update 後も pin は固定 revision を保持 | `git.rs`・`vcs06_git_matrix.rs`・#91 oracle | #88・#97・#91 | fixture/L1 | **NotRun** |
| LIB-PIN-05 | auth revoke / permission denied が pin と conditional get の間で typed | `webdav.rs`・`git.rs`・#91 oracle | #88/#90・#91 | fixture/mock L1 | **NotRun** |

## LIB-IMPORT-01〜10（verified import）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| LIB-IMPORT-01 | chunked huge file bounded memory | `import.rs`・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-IMPORT-02 | verify/decode/metadata 抽出 | `import.rs`・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-IMPORT-03 | SHA-256 mismatch は typed failure・部分 file を残さない | `import.rs`・`import_recovery.rs`・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-IMPORT-04 | cancellation at each phase が typed・resume 可能 | `import_recovery.rs`・#91 oracle | #86/#89・#91 | fixture/L1 | **NotRun** |
| LIB-IMPORT-05 | duplicate ImportId same/different digest（冪等/拒否） | `import.rs`・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-IMPORT-06 | Project Revision/Target changed（stale revision・target deletion） | `import.rs`・`project.rs`・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-IMPORT-07 | multiple windows/並行 import の整合 | `import.rs`・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-IMPORT-08 | Blob Published/ACK lost/manifest crash の recovery | `import_recovery.rs`・#91 oracle | #86・#84・#91 | fixture/L4 | **NotRun** |
| LIB-IMPORT-09 | Save/HEAD は CommittedLive と別状態 | `project.rs`・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-IMPORT-10 | offline playback after cache eviction（copy into project） | `cache.rs`・`import.rs`・#91 oracle | #86/#87・#91 | fixture/L1・real FS L2-Deferred | **NotRun** |

## LIB-MIG-01〜05（v5 migration）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| LIB-MIG-01 | Snapshot v5 read/upgrade | ALIB-02-v5 fixture・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-MIG-02 | old hex16 CRC reference vs new SHA256 namespaced key | ALIB-02-v5 fixture・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-MIG-03 | old missing blob/degraded の安全な扱い | ALIB-02-v5 fixture・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-MIG-04 | legacy collision/no unverified hash promotion | ALIB-02-v5 fixture・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-MIG-05 | Plugin Opaque Bytes/ID/Undo/Source/Recording/PCM Golden untouched | ALIB-02-v5 fixture・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |

## LIB-CACHE-01〜05（cache/preview/index）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| LIB-CACHE-01 | account/principal private cache isolation | `cache.rs`・`rt_cache.rs`・#91 oracle | #87・#91 | fixture/L1 | **NotRun** |
| LIB-CACHE-02 | TTL/LRU/active read lease | `cache.rs`・#91 oracle | #87・#91 | fixture/L1 | **NotRun** |
| LIB-CACHE-03 | auth revocation & clear | `cache.rs`・#91 oracle | #87・#91 | fixture/L1 | **NotRun** |
| LIB-CACHE-04 | Preview full-fetch warning | `cache.rs`・`preview.rs`(src)・#91 oracle | #87・#91 | fixture/L1 | **NotRun** |
| LIB-CACHE-05 | huge catalog & rapid cancellation/resource budget | `cache.rs`・`rt_cache.rs`・#91 oracle | #87・#91 | fixture/L1 | **NotRun** |

## LIB-SEC-01〜05（security 境界）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| LIB-SEC-01 | Path traversal/symlink/redirect | `containment_swap.rs`・`local.rs`・#91 oracle | #85/#90・#91 | fixture/L1・real FS L2-Deferred | **NotRun** |
| LIB-SEC-02 | SSRF/credential forwarding | `webdav.rs`・`git.rs`・#91 oracle | #88/#90・#91 | mock/L1・real server L2-Deferred | **NotRun** |
| LIB-SEC-03 | malformed Content-Length/compression bomb | `webdav.rs`・`import.rs`・#91 oracle | #86/#90・#91 | fixture/mock L1 | **NotRun** |
| LIB-SEC-04 | log token/redaction | `git.rs`・`webdav.rs`・#91 oracle | #88/#90・#91 | fixture/L1 | **NotRun** |
| LIB-SEC-05 | Git process argv safety | `git.rs`・#91 oracle | #88・#91 | fixture/L1 | **NotRun** |

## LIB-QOS-01〜04（RT/no-block・OS/GUI 分離）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| LIB-QOS-01 | RT callback no blocking/disk/network/alloc | `rt_cache.rs`・#91 oracle | #87・#91 | fixture/L1・real device L2-Deferred | **NotRun** |
| LIB-QOS-02 | Capture writer/Prefetch priority | `rt_cache.rs`・#91 oracle | #87・#91 | fixture/L1 | **NotRun** |
| LIB-QOS-03 | f64/PCM time/Golden unchanged | ALIB-02 fixture・#91 oracle | #86・#91 | fixture/L1 | **NotRun** |
| LIB-QOS-04 | Linux/macOS/Windows real device/FS capability/GUI evidence を headless と区別 | #91 oracle・#114 | #91・#114 | real OS/GUI L2-Deferred | **NotRun**（Deferred） |

## DOC-01〜03（本 slice の docs 検証）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| DOC-01 | 文書鎖 relations・status・canonical front matter が整合 | `doc_status.js lint`・`audit_docs.js` | 本 slice | docs/L1 | **Pass**（`asset-library-verification.md` 参照） |
| DOC-02 | 触れた dir の index が `doc-maintenance` で再生成 | `doc_maintenance.js index apply`・`sync_impl_index.js` | 本 slice | docs/L1 | **Pass**（同上） |
| DOC-03 | `mise run fmt-check` が exit 0 | mise task | 本 slice | docs/L1 | **Pass**（同上） |

## 昇格禁止の明記

- 上記 LIB-* の `NotRun` は #9 製品レベル受入・#91 aggregate
  gate の未実施を意味する。既存 ALIB contract test の Pass
  （ALIB-07 evidence 参照）は provider/import/cache の headless
  契約 Pass であり、#9 の Browser UX・実 Provider・実 GUI・
  実 OS/device 受入を構成しない。
- 実 Provider（実 Git remote・実 WebDAV・実 credential
  store）・実 GPUI window・実 OS/実 device の受入は
  `Deferred` であり、#88/#90/#114/#331 の別 gate で実施する。
