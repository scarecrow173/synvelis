---
id: "ADR-034UvyaJXF0PgtKqoqygMm"
type: "adr"
status: "accepted"
title: "SYN-IO-01 Backup/Import Archive 初期採用範囲 — ZIP コンテナと Limits/Path/Link 方針"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/82"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "docs/discovery/syn-io-01-io-boundary-inventory.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/297"
    - "https://github.com/scarecrow173/ruviform/issues/298"
    - "https://github.com/scarecrow173/ruviform/issues/299"
    - "https://github.com/scarecrow173/ruviform/issues/300"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# SYN-IO-01 Backup/Import Archive 初期採用範囲 — ZIP コンテナと Limits/Path/Link 方針

## Context and Problem Statement

Synvelis の Project Backup/Import は、現行の
`DirectoryBundleStore`（`snapshots/r<N>.json` + `HEAD`）と
検証済み media blob を単一の持ち運び可能な成果物へまとめる
必要がある。Ruviform は `ruviform-io-archive`（ZIP/TAR
reader/writer・`ExtractionSession`/`ExtractedTree`・
`SafeRelativePath`・`ArchiveReadLimits`）と
`ruviform-io-compression`（gzip/zstd/deflate）を提供するが、
どの archive 形式を初期採用範囲とし、どの limits/path/link
規則を Synvelis 側で所有するかは未決定である。新たな必須
保存 Format を発明してはならない一方、既存 bundle layout を
正本として維持したまま安全に持ち運べる形式を選ぶ必要がある。

## Decision Drivers

- Backup 成果物はユーザーが OS 標準 tool で開ける単一ファイル
  であることが望ましい。
- Import は信頼できない入力を扱うため、archive 展開の
  path/link/size 安全性と検証可能性が必須。
- Project 意味論（manifest/schema/blob digest/semantic
  allowlist/transaction）は Synvelis が所有し、Ruviform の
  generic 機構に混入させない。
- 中断した backup/import は元 project を変更せず、target を
  incomplete として扱えること。
- 実装は将来の Issue へ委譲されるため、初期採用範囲は
  明確かつ最小限に限定する。

## Considered Options

- **Option A: ZIP コンテナのみを初期採用**
  `ZipArchiveWriter`/`Reader`（`verify_all` CRC32 receipt）、
  per-entry Stored/Deflate、`ExtractionSession` による
  staged 検証済み展開を使用。TAR・独立 compression stream は
  初期対象外。
- **Option B: TAR(+compression) を初期採用**
  `TarArchiveWriter`/`Reader` と `ruviform-io-compression`
  の gzip/zstd/deflate を組み合わせる。streaming に適するが、
  TAR は payload checksum を持たず（`EntryChecksum::
  NotPresent`）、verify/readback 証拠が ZIP より弱い。
- **Option C: ZIP と TAR の両方を初期採用**
  入力 format の柔軟性は高いが、検証・保守対象が倍増し、
  初期採用範囲として過剰。
- **Option D: Synvelis 独自 format を新設**
  Issue が禁止。既存 bundle layout を正本として維持する
  原則に反する。

## Decision Outcome

Chosen option: **Option A — ZIP コンテナのみを初期採用範囲と
する**。

### 決定内容

- **Backup 出力形式**: 単一 `.zip` ファイル。entry は
  project bundle の file を deterministic な path 順で格納
  する。`ZipArchiveWriter` は `Write+Seek` を要求するため、
  出力先は `StagedFile`（`Write+Seek+Send`）とし、
  `PublishMode::CreateNew`（既存 backup file を上書きしない）
  で publish する。publish 前に `SealedFile` の readback
  verify（reopen + `verify_all`）を行い、CRC32 receipt を
  証拠として保持する。
- **Import 入力形式**: 同一 `.zip` のみ。`ZipArchiveReader`
  で `verify_all` を実行してから `ExtractionSession` で
  `ExtractedTree`（staged・検証済み tree）を生成する。
  `ExtractedTree` は publish ではなく、Synvelis が manifest/
  schema/blob digest/semantic allowlist を検証した後にのみ
  commit 対象とする。
- **Compression**: ZIP entry の compression method は
  `Stored`（無圧縮）と `Deflate` のみを初期採用とする。
  `ruviform-io-compression` の独立 stream（gzip/zstd/deflate
  file 全体への適用）は初期対象外。大きな media blob は
  ZIP Deflate の効果が限定的なため、entry ごとに method を
  選択可能とし、実装時に圧縮率と CPU コストで決定する。
- **TAR**: 初期対象外。将来 streaming export 等の要件が
  発生した場合に再評価する。
- **Limits**: `ArchiveReadLimits` を Synvelis 側で project
  backup 用に絞り込む。初期値は entry 数・展開後総 byte 数・
  path 長/深さ・1 entry あたり byte 数に上限を設け、
  project scale を超える入力は `Unsupported`/reject とする。
  具体値は実装 Issue で確定するが、無制限を許容しない。
- **Path/Link**: `SafeRelativePath` による safe relative
  path 強制をそのまま利用し、absolute path・drive letter・
  UNC・`..` traversal・NUL・末尾 `.`/空白を拒否する。
  symlink/hardlink・特殊 entry（device/fifo/socket）は
  v1 では一切受理しない。case-fold 衝突・Unicode 正規化
  衝突は Ruviform の抽出機構と Synvelis の semantic 検証の
  両方で拒否する。
- **Manifest/format version**: ZIP 内に project manifest
  （format version・project_id・revision・snapshot 一覧・
  blob digest 一覧・plugin state 有無）を含め、Synvelis が
  schema_version と blob SHA-256（#86 で導入予定）で検証する。
  manifest 自体は Synvelis 所有の JSON であり、Ruviform の
  generic 機構には依存しない。
- **保存形式**: 既存 `DirectoryBundleStore` layout
  （`snapshots/r<N>.json` + `HEAD` + `SnapshotEnvelope`）を
  ZIP entry としてそのまま格納する。新たな on-disk 形式は
  発明しない。backup file は Project Asset ではなく、
  明示的に import/commit されるまで asset 扱いしない。
- **中断・回復**: backup 出力は `StagedFile` で staging し、
  publish 前に seal+verify する。中断時は output file が
  `Incomplete` のまま残り、元 project は変更されない。
  import は `ExtractedTree` staging → 検証 → commit の二段
  構成で、検証失敗時は staging を破棄し target project は
  不変とする。

### セキュリティ・秘密情報

- backup/import の entry に credentials・raw media metadata・
  plugin binary・plugin state・sensitive path を含めない。
  plugin state は opaque data として扱い、import 時に
  schema_version で検証する。plugin binary は backup 対象外。
- export された file path・media metadata は import 側で
  信頼せず、Synvelis の semantic 検証を必須とする。

### 実装への影響

- 本 ADR は初期採用範囲の決定であり、最終的な製品 format
  凍結・UI・実機 FS 検証は実装 Issue（#84/#86/RF#288）へ
  委譲する。`ruviform-io-archive`/`ruviform-io-compression`
  への依存追加は実装 Issue で行う。

## Consequences

### Good

- Backup/Import の archive 安全性（path/link/limit/CRC32
  verify）を Ruviform の検証済み機構に委譲しつつ、project
  意味論を Synvelis に保持できる。
- `verify_all` CRC32 receipt と `StagedFile` seal+readback
  verify により、backup 成果物の完全性を証拠として残せる。
- `ExtractedTree` が publish ではなく staged tree を返すため、
  Synvelis が manifest/schema/blob digest 検証を挟んでから
  commit できる。
- 既存 bundle layout を維持するため、snapshot/HEAD/envelope
  の on-disk 形式を変更しない。

### Bad / Trade-offs

- TAR・独立 compression stream を初期対象外とするため、
  streaming export や高圧縮要件には将来再評価が必要。
- ZIP の per-entry compression は Deflate が中心で、
  zstd 等の高効率 codec は使えない（media blob の圧縮率は
  限定的）。
- `ArchiveReadLimits` の具体値は実装時に project scale を
  計測して確定する必要がある。

### 実装 Issue への影響

- #84/#86 系列で `DirectoryBundleStore`/asset CAS の実装が
  進む中、本 ADR の ZIP/TAR/Compression 初期採用範囲・
  Limits/Link/Path/保存形式を前提として Backup/Import を
  設計・実装する。
- RF#288 consumer gate では、本 ADR の archive safety 証拠
  （traversal/link/limit/CRC32 verify）を Synvelis 独立
  証拠として提出する。
