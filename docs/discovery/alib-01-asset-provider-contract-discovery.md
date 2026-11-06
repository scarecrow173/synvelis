---
id: "DISC-yt7pUuJb03lx8z07ulXN1r"
type: "discovery"
status: "resolved"
title: "ALIB-01 Asset Provider 契約・基準 Provider 発掘調査"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/synvelis/issues/9"
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
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
---

# ALIB-01 Discovery — Asset Provider 契約と基準 Provider 経路

## 依頼と範囲

Issue #85（ALIB-01）: #9 の Provider contract を実装し、Remote/VCS
依存なしで Local Folder と Current Project Assets を共通 Catalog
として閲覧できる基準経路を作る。SV86(#86 Importer/Schema/hash
移行)、SV87(#87 cache)の先行実装は行わない。SV84 Store、SV112
Core UI は変更しない。

## 現行コードとの実接続点（実測）

| 実在構造 | 位置 | 本件での扱い |
| --- | --- | --- |
| `AssetId`/`SourceId`/`ImportId` | `synvelis-project::id` | Project 正本 ID。Provider locator/proof とは別型 |
| `Asset { id, digest }` / `Source { id, asset_id, content_digest, state }` / `AppliedImport` | `synvelis-project::project` | Project Assets catalog の正本フィールド |
| `ProjectController::live()` / `live_revision()` | `synvelis-project::controller` | read-only 投影の唯一の入口。書き込みは `Command`/`Operation` のみ |
| `media_root/blobs/<digest>` CAS 配置 | `synvelis-player::import::blob_path` | 配置規約の正本は import 側。provider は blob dir を注入され規約を複製しない |
| `content_digest` (crc32 pair hex16) | `synvelis-player::import` / `synvelis-recording::crc32` | 強 digest ではない(#9 §1/§3)。provider は sha256 を pin 時に計測し legacy digest は object identity としてのみ扱う |
| `ProjectStore` port | `synvelis-project::store` | 依存禁止対象。snapshot/HEAD には触れない |

## 発見と判断

1. **crate 配置**: Issue #85 は「`synvelis-library` 新 crate または
   workspace 内独立 module、ADR で決定」と指定。Provider 契約は
   `synvelis-project` の値型（stable ID/Revision）だけを参照し、
   Store・UI・Git SDK・RT graph に依存しない。`synvelis-project` に
   同居させると「Project の正本を所有しない」境界が曖昧になるため、
   新 crate `synvelis-library` を採用する（ADR 参照）。
2. **`synvelis-project` への依存可否**: 禁止対象は UI・ProjectStore・
   Git SDK・RT graph であり、model crate 自体は含まない。
   `ProjectAssetsProvider` は `Mutex<ProjectController>` 経由の
   read-only 投影 port（`ProjectCatalog`）を受け取る。`Mutex<T:Send>`
   は Sync となり `AssetProvider: Send+Sync` 契約を満たす。
3. **Pin の正直な固定根拠**:
   - Local file は可変。`mtime`/`size`/path は固定証拠にしない
     (Issue 明記)。`pin` は sha256 を streaming 計測し
     `PinProof::ConditionalStrong{ ContentHash }` を返す。
     `open_pinned` は読みながら sha256 を再計算し EOF で照合、
     不一致は `IntegrityMismatch` で可変バイトを固定済みとして
     返さない。
   - Project blob は content-addressed（名前=legacy digest）で
     不変オブジェクト。`PinProof::ImmutableObject{ revision: blob
     digest }`。pin 時に sha256 も計測し `digest_hint` として
     `ChecksumHint` capability の下で返し、`open_pinned` は同一
     検証 reader で byte integrity を EOF 時に確認する。
   - `open_pinned` は `Read+Send` stream + `FetchReceipt` 系契約。
     stream 中の検証結果は reader の終端エラーと
     completion slot（typed `FetchReceipt`/`ProviderError`）で返す。
4. **Local walk の明示対応**: symlink は canonical target が root
   内かつ未訪問の dir のみ辿る（visited canonical dir set で loop
   切断）。root 外へ逃げる locator は列挙は観測事実として行うが
   stat/pin/open は拒否。locator は root 相対の `/` 区切りで、
   `..`・絶対・NUL を拒否し canonical 照合で traversal を防ぐ。
   open-after-stat race は「pin 時に sha256 を計測し open 時に
   再検証」で閉じる（inode 同一性ではなく byte 同一性を固定）。
5. **Cursor 設計**: opaque token（hex-encoded JSON）に
   provider/fingerprint(scope+sort+filter+principal)/epoch/frontier
   を同梱。Local epoch は provider 内の単調 browse generation、
   Project epoch は `ProjectRevision`。Query/Principal/Revision
   変更後の cursor 再利用は `CursorInvalid` で拒否。cursor 復号の
   frontier は DFS 継続位置（dir path 列）で bounded。
6. **Coverage 区別**: `Complete/Partial/Stale/OfflineCached/
   NeverIndexed`。Local は walk 完了=Complete、budget/深度/訪問
   上限で打ち切り=Partial。Project は catalog snapshot 全域
   =Complete（ProjectStore offline 概念なし）。0 件結果と
   失敗/未索引を混同しない。
7. **Search/Preview**: `search` は optional。Local はファイル名
   部分一致（walk=crawl が正本なので coverage は正直に宣言）。
   Project は検索可能なテキスト索引が存在しないため
   `UnsupportedCapability` を型付き返却。`open_preview` は
   bounded range read（検証なしを `claims` で明示）。
8. **Ruviform への還元**: std `Read`+`Send` と sha2 で足り、
   汎用 File/Stream primitive の具体不足は本範囲では未確認。
   Ruviform IO #282 への切り出し事由は発生しなかった。

## 未解決・後続へ渡す論点

- `ImportIntent`/`ContentDigest` schema・Provenance 永続化・
  `applied_imports` 拡張は SV86(#86) の範囲。
- Catalog/Preview cache、Principal 分離 cache、QoS は SV87(#87)。
- Remote/VCS provider、WebDAV、fault matrix は #88〜#91。
- `Asset.digest` の opaque 文字列を object identity として path 化
  する際の sanitize（`/`・`\\`・`..`・非 filename 文字の拒否）は
  provider 側の責務として本件で固定。

## 結論

spec→design→plan→test-spec/task チェーンを `alib-01-*` で起票し、
`synvelis-library` 新 crate に契約型・registry・
`LocalFolderProvider`・`ProjectAssetsProvider` とそのテストを実装
する。
