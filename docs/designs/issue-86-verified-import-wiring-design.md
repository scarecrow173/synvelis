---
id: "DESIGN-vaYFK7BnCVN715EASswzTe"
type: "design"
status: "approved"
title: "Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 設計"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/issue-86-verified-import-wiring.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/blob.rs"
      - type: "file"
        path: "crates/synvelis-player/src/verified_import.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/verified_import.rs"
      - type: "file"
        path: "docs/designs/issue-86-verified-import-wiring-design.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-player/src/resolver.rs"
      - type: "file"
        path: "crates/synvelis-player/src/error.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-render/src/prepare.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/designs/README.md"
  implements:
    - "docs/specs/issue-86-verified-import-wiring.md"
  implemented-by: []
  depends-on:
    - "docs/designs/alib-02-streaming-verified-import-design.md"
    - "docs/designs/alib-02-v5-snapshot-compat-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-86-verified-import-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 設計

## Overview

本設計は Issue #86 DoD(2) の 2 つのギャップを `synvelis-player` で閉じる。

1. **blob 解決の単一権威** — `resolver::open_verified` と
   `render::prepare::open_source` に重複していた flat-`blobs/<digest>` +
   CRC fold 解決を、`AssetDigest` kind 分岐の `player::blob`
   module へ集約する。`Sha256`/`Crc32Fold`/`Opaque` の 3 経路を
   一箇所で型付きに扱う。
2. **consumer 入口** — `player::verified_import::import_verified_local`
   が、任意 path 1 file を provider-bounded pin → library
   `import_verified` → published blob の decode 検証 →
   `SetSourceState(→Verified)` までを 1 entry point として実行する。

## 決定

### 1. digest-kind 解決は `player::blob` module の単一権威とする

新規 `crates/synvelis-player/src/blob.rs`:

```rust
pub struct VerifiedBlob {
    pub file: File,     // digest 計測に使った同一 object。cursor は byte 0
    pub path: PathBuf,  // 解決した blob path
    pub digest: String, // source に記録された digest（そのまま）
}

pub fn open_blob_verified(
    media_root: &Path,
    digest: &str,
) -> Result<VerifiedBlob, ResolveError>
```

- `AssetDigest::parse(digest).kind()` で分岐:
  - `Sha256` → `media_root/blobs/sha256/<hex-lowercase>` を開き、
    `sha2::Sha256` chunked 計測 → measured hex と recorded hex
    （lowercase 正規化）を比較。path 構成は `LocalBlobDir::object_rel`
    の `"<alg>:<hex>"` → `<alg>/<hex>` 規則と同一。
  - `Crc32Fold` → `import::blob_path`(flat) + `import::content_digest_stream`
    （既存の同一 fd streaming CRC — 再利用のみ、import.rs 自体は不変）。
  - `Opaque` → `ResolveError::UnsupportedDigest { digest }`（新 variant）。
    verified content address でない digest は解決しない — fail-closed。
- `File::open` 失敗は従来どおり `path.exists()` で
  `MissingMedia`/`Io` を分岐。digest 不一致は `HashMismatch`
  （`actual` は `sha256:<hex>` 形式で expected と対称にする）。
- digest 計測は open した `File` 上で行い、計測後 `Seek` で 0 へ
  巻き戻して返す。呼び出し側はその `file` を demuxer へ渡すだけで
  verify→decode 間の path swap 窓がない（render 側が既に取っていた
  方式に player resolver も揃う）。

call site 更新:

- `resolver::open_verified`: `fs::read`+`content_digest` の全件読み
  置き換え → `open_blob_verified` の返す `file` をそのまま
  `open_demuxer` へ。state gate・asset gate は従来どおり resolver 側。
- `render::prepare::open_source`: 同一の open+digest ブロックを
  `open_blob_verified` へ置き換え（`blob_path`/`content_digest_stream`
  の直接 import は不要になる）。`AssetDigest` でなく生の source digest
  を見る点・後段の codec gate は不変。

**なぜ resolver 側も同一 fd 化するか**: 重複していた 2 実装のうち
render 側の「1 fd = digest + decode」が厳密に強い方式であり、helper が
`File` を返す形にすれば両者は自然に同じ保証を得る。resolver の
「`fs::read` で全件読んで digest → 別 `File::open` で decode」は
verify-then-open gap を残しており、helper 集約と同時に閉じる。

### 2. consumer 入口 `verified_import::import_verified_local`

新規 `crates/synvelis-player/src/verified_import.rs`:

```rust
pub struct VerifiedImportRequest {
    pub path: PathBuf,            // import 対象のローカルファイル
    pub staging_root: PathBuf,    // <staging_root>/import-<id>/
    pub media_root: PathBuf,      // <media_root>/blobs/sha256/<hex>
    pub target: Option<ImportTarget>, // clip 同時 commit（任意）
    pub import_id: Option<ImportId>,  // None=draw_raw_id 採番 / Some=manifest resume
}

pub struct VerifiedImportOutcome {
    pub import: synvelis_library::ImportOutcome, // pipeline receipt
    pub media: VerifiedMedia,                    // decode 検証の実測値
    pub verified_revision: ProjectRevision,      // SetSourceState の commit rev
}

pub fn import_verified_local(
    controller: &mut ProjectController,
    request: &VerifiedImportRequest,
    cx: &ProviderContext,
) -> Result<VerifiedImportOutcome, VerifiedImportError>
```

実行手順:

1. `path` を `fs::canonicalize` → `parent()` を provider root、
   `file_name()` を `OpaqueLocator` として `LocalFolderProvider` を
   組み立てる（任意 path の単一 file import は「file の parent を
   root とする provider」を入口が作る設計。root-bounded により
   locator は常に root 直下 1 component で、containment 破れを
   構造的に作れない）。
   provider id は module 定数
   `LOCAL_VERIFIED_IMPORT_PROVIDER: ProviderId` — provider 実体は
   call ごとの ephemeral で、pin の `asset.provider` が manifest に
   残るのは identity のみ。固定値にすることで resume 時の pin 再発行が
   呼び出し側の状態管理不要で一致する。
2. `provider.pin(&RemoteAssetRef{provider, locator}, cx)` →
   `PinnedAssetRef`（`ContentHash` conditional pin + digest hint +
   expected_size）。
3. `import_id = request.import_id.unwrap_or_else(|| ImportId::new(controller.draw_raw_id()))`。
   `Some` は manifest resume / undo 後の再 import に使う
   （manifest の `import_id`/`source` 一致・`applied_imports`
   integrity は library が検証する）。
4. `ImportIntent { import_id, expected_project_revision:
   controller.live_revision(), source: pin, target }`。
   **新規 manifest のみ**この revision が capture される — 既存
   manifest resume 時は persisted expectation が権威
   （library が manifest の値で commit する）。
5. `ImportStore::new(staging_root, media_root)`、
   `LocalBlobDir::new(store.blob_dir())`、`StagedBlobPublisher` で
   `import_verified` 実行 → `Imported` で commit。
6. **published blob の decode 検証**: `open_blob_verified(media_root,
   &outcome.content_digest.to_string())` — resolver が使うのと同一の
   解決経路で blob を取得し、`open_demuxer` → audio stream →
   `CodecId::Pcm` gate → `SourceReader`(Canonical) → EOF まで decode
   して `frames`/`channels`/`sample_rate`/`container` を実測する
   （`import_local` の verify stage と同列の検証内容）。失敗は
   `Corrupt`/`UnsupportedCodec`/`Resolve` として返り、source は
   `Imported` のまま（再生 gate を満たさない = 安全側に倒れる）。
7. `controller.apply(Command{expected_revision: live_revision(), op:
   ProjectOp::SetSourceState{source_id, state: Verified}})` →
   `verified_revision`。

`VerifiedMedia`（`import::VerifiedMedia`）を再利用して実測値を返す。
`durability` には `outcome.blob.durability`（達成証拠）と
`directory_sync` 証拠を記録する — Windows で `Unsupported` はそのまま
正直に記録し、失敗扱いにしない。

### 3. エラー分類 `VerifiedImportError`

`error.rs` に新 enum を追加（既存 `ImportError`/`ResolveError` は
不変のまま別系統）:

- `Provider(ProviderError)` — provider 構築・locator・pin。
- `Import(synvelis_library::ImportError)` — stage machine /
  StaleRevision / IntegrityConflict / PublishFailed などを透過
  （呼び出し側が rebase 判断に使うため潰さない）。
- `Resolve(ResolveError)` — published blob の解決失敗
  （MissingMedia/HashMismatch/UnsupportedDigest）。
- `Corrupt{detail}` / `UnsupportedCodec{detail}` — decode 検証失敗。
- `CommitRejected{source: synvelis_project::Error}` —
  `SetSourceState` apply 拒否（CAS stale・illegal transition）。
- `Io{detail}` — canonicalize/file_name など入口前段の失敗。

### 4. `Verified` 止め（undo 粒度差は明示の仕様差）

`import_verified` commit は `Imported` 登録、`SetSourceState` は
1 stage forward のみ許可。`Verified` は再生 gate 最低条件であり、
`Committed` まで進めると旧経路（`import_local` の 1 command）から
さらに undo step が 1 つ増えるため採用しない。結果として新経路は
2 command = 2 undo step（spec「Undo 粒度の変更」参照）。state 前進を
import commit と同一 command に畳むことは構造的にできない
（`CommitVerifiedImport` は `ProjectOp` ではなく `Composite` にも
入らない）ため、この分離は責務境界どおりの形になる。

## Trade-offs

- **同一 fd verify→decode への統一**: resolver の全件読み経路が消え、
  render と完全に同じ保証になる。代償として `fs::read`+in-memory CRC
  の単純さは失われるが、同一 fd 保証はその複雑さを正当化する。
- **decode 検証を entry に入れる設計**: library stage machine は
  byte-integrity のみを保証し、media decode は consumer 責務という
  library 契約どおり。decode 検証せず `Verified` に進めると、壊れた
  container でも再生 gate を通ってしまうため、entry 内で
  demuxer→decode 済ませてから state を進める。
- **provider_id を定数化**: request field にすると caller が resume
  間で同じ値を維持する責務を負う。ephemeral provider なので固定値で
  十分であり、間違いどころを減らす。
- **新規 module 2 本**: `blob.rs`（media store 解決権威）と
  `verified_import.rs`（consumer 入口）は責務が別。`import.rs` へ
  混ぜると legacy `import_local` と新経路が同居して読みにくく、
  `resolver.rs` へ混ぜると render から「resolver の公開面」に
  依存してしまい境界が曖昧になる。
