---
id: "SPEC-alib07-fault-matrix"
type: "spec"
status: "approved"
title: "ALIB-07 — Asset Library 全経路 Fault Matrix（headless slice・synvelis-library）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/87"
    - "https://github.com/scarecrow173/synvelis/issues/88"
    - "https://github.com/scarecrow173/synvelis/issues/90"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/tests/common/webdav_fixture.rs"
      - type: "file"
        path: "docs/specs/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/designs/alib-07-fault-matrix-design.md"
      - type: "file"
        path: "docs/plans/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/tasks/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/test-specs/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/impl/ir/alib-07-fault-matrix.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/git.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/webdav.rs"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
---

# ALIB-07 Spec — Asset Library 全経路 Fault Matrix（headless slice）

Synvelis #91 の headless slice。ALIB-01〜06 で実装済みの
`synvelis-library` 契約・provider・import・cache を、Issue #91 が
固定する Case ID（LIB-ID/BROWSE/PIN/IMPORT/MIG/CACHE/SEC/QOS、
計 42 class）に対して横断検証し、証拠 matrix を整備する。

## 目的と境界

- **目的**: 契約・障害・安全性・精度・性能の各 class について、
  headless で再現可能な fault/golden case を実装し、Expected /
  Observed / Pass-Fail-NotRun を Case ID 単位で記録する。
- **境界**: `crates/synvelis-library` の test/fixture/docs のみ。
  Provider 実装本体（#85〜#90 の owner 範囲）、GUI、実 network /
  実 WebDAV server / 実 Git remote / OS credential store / 実 device
  は対象外であり、結果を `NotRun`/`Unsupported`/`NotImplemented`
  として正直に記録する。`NotRun`・`Unsupported`・`Unknown` を
  Pass に昇格させない。
- **非対象 Provider**: SVN/P4/SMB/S3 は未実装であり、
  `NotImplemented` として明示除外する。暗黙の PASS としない。

## Case ID 体系（Issue #91 固定）

| Class | 範囲 | Class 数 |
| --- | --- | --- |
| LIB-ID | Source/Asset Identity（同 Byte 別 Provider、rename/branch move、symlink/race、read-only projection） | 01〜04 |
| LIB-BROWSE | cursor/continuation、search Unsupported vs empty、stale/offline partial、principal cursor invalidation | 01〜04 |
| LIB-PIN | Git fixed commit/tree、Git LFS 実 Byte、WebDAV strong/weak ETag・If-Match、mutable branch、auth revoke/permission | 01〜05 |
| LIB-IMPORT | chunked bounded memory、verify/decode/metadata、SHA-256 mismatch、cancel 各相、duplicate ImportId、revision/target changed、複数 window、Published/ACK lost/crash、Save/HEAD 分離、offline playback after eviction | 01〜10 |
| LIB-MIG | v5 snapshot read/upgrade、旧 hex16 CRC vs SHA-256 namespace、missing blob degraded、legacy collision、opaque bytes/golden 非回帰 | 01〜05 |
| LIB-CACHE | principal 分離、TTL/LRU/lease、auth revoke & clear、preview full-fetch warning、巨大 catalog/cancel/予算 | 01〜05 |
| LIB-SEC | traversal/symlink/redirect、SSRF/credential forwarding、malformed CL/compression bomb、log redaction、git argv safety | 01〜05 |
| LIB-QOS | RT callback の blocking/disk/network/alloc 回避、capture/prefetch 優先、f64/PCM/golden 非回帰、実 device/OS/GUI 区別 | 01〜04 |

## 受入条件（headless slice）

- 新 provider（git/webdav）の fault case が headless green。
- 既存 fixture 基盤（`tests/common`、cache/import/local/rt_cache の
  injected-fault パターン）を再利用し、重複実装しない。
- Case ID ごとに Expected/Observed・Pass/Fail/NotRun/
  Unsupported/NotImplemented を記録した証拠 matrix
  （`docs/test-specs/alib-07-fault-matrix.md`）を整備する。
- 実 network・実 WebDAV server・実 Git remote・OS credential store・
  実 device・実 GUI の行は `NotRun` であり、Synthetic/Mock の結果を
  実環境の成功と主張しない。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`（および
  `test-msrv`）が exit 0。

## 非対象（Issue #91 全体の残件）

- 実 provider/実 OS/実 device/実 GUI での受入。
- #84 File/FS cross-gate、#97 VCS 交差 case、#114 GPUI Browser、
  Ruviform #219/#294 の Consumer 独立検証との統合記録。
- LIB-MIG の v5→SHA-256 移行本体（`alib-02-v5-snapshot-compat` は
  draft/deferred であり、本 slice では namespace 共存までを証拠化）。
