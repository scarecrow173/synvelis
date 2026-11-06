---
id: "IMPL-034T7QIk72dphjZPfVnZ1O"
type: "impl"
status: "completed"
title: "ALIB-01 Asset Provider baseline contract"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/src/id.rs"
      - type: "file"
        path: "crates/synvelis-library/src/refs.rs"
      - type: "file"
        path: "crates/synvelis-library/src/capability.rs"
      - type: "file"
        path: "crates/synvelis-library/src/observation.rs"
      - type: "file"
        path: "crates/synvelis-library/src/browse.rs"
      - type: "file"
        path: "crates/synvelis-library/src/context.rs"
      - type: "file"
        path: "crates/synvelis-library/src/error.rs"
      - type: "file"
        path: "crates/synvelis-library/src/digest.rs"
      - type: "file"
        path: "crates/synvelis-library/src/provider.rs"
      - type: "file"
        path: "crates/synvelis-library/src/registry.rs"
      - type: "file"
        path: "crates/synvelis-library/src/local.rs"
      - type: "file"
        path: "crates/synvelis-library/src/project.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/local.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/project.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/types.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/contract.rs"
      - type: "file"
        path: "docs/discovery/alib-01-asset-provider-contract-discovery.md"
      - type: "file"
        path: "docs/specs/alib-01-asset-provider-contract.md"
      - type: "file"
        path: "docs/adr/alib-01-synvelis-library-crate-boundary.md"
      - type: "file"
        path: "docs/designs/alib-01-asset-provider-contract-design.md"
      - type: "file"
        path: "docs/plans/alib-01-asset-provider-baseline.md"
      - type: "file"
        path: "docs/test-specs/alib-01-asset-provider-contract.md"
      - type: "file"
        path: "docs/tasks/alib-01-task1-contract-types.md"
      - type: "file"
        path: "docs/tasks/alib-01-task2-local-provider.md"
      - type: "file"
        path: "docs/tasks/alib-01-task3-project-provider.md"
      - type: "file"
        path: "docs/tasks/alib-01-task4-acceptance.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/discovery/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-01-asset-provider-baseline.md"
    - "docs/designs/alib-01-asset-provider-contract-design.md"
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/adr/alib-01-synvelis-library-crate-boundary.md"
    - "docs/tasks/alib-01-task1-contract-types.md"
    - "docs/tasks/alib-01-task2-local-provider.md"
    - "docs/tasks/alib-01-task3-project-provider.md"
    - "docs/tasks/alib-01-task4-acceptance.md"
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
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-01 Asset Provider baseline contract

## Summary

Issue #85「Asset Provider基準契約」の実装。Remote/VCS 依存なしで Local Folder と
Current Project Assets を共通 Catalog として閲覧できる基準経路を、新規 crate
`crates/synvelis-library` として実装した。Provider identity、PinProof、
typed capability、observation/provenance、paging/cursor、中断・期限・budget、
streaming digest 検証、`AssetProvider` trait、provider registry、
`LocalFolderProvider`、read-only `ProjectAssetsProvider` を含む。
ALIB-02/03/04/05/06 が依存する型付き境界を公開する。

## Context

- Issue: scarecrow173/synvelis#85(親・設計オーナー: #9。関連契約: #17/#21/#7/#23)
- 本 IR は `docs/plans/alib-01-asset-provider-baseline.md` 配下の
  task1〜task4 に対応する実装記録。
- 配置は ADR `docs/adr/alib-01-synvelis-library-crate-boundary.md` で決定した
  独立 crate 方針に従う(UI/ProjectStore/Git SDK/Tokio 等への依存なし)。

## Implementation

### Changes

- `crates/synvelis-library`: Asset Provider/Catalog 契約 crate を新設。
  - `id.rs`: `ProviderId`、`OpaqueLocator`、`PrincipalId`、`CredentialKey`、
    `OpaqueCredential`(Debug で credential 値を `<redacted>` にする)。
    検証付きコンストラクタを経由する `Deserialize`。
  - `refs.rs`: `RemoteAssetRef`、`PinnedAssetRef`、`PinProof`
    (`ImmutableObject`/`ConditionalStrong`)、`RevisionKey`、`ObjectIdentity`、
    `ContentDigest`。全て検証付きデシリアライズ。
  - `capability.rs`: `ProviderCapabilities` と型付き `UnsupportedReason`。
  - `observation.rs`: `AssetObservation<T>`(Observed/Inferred/Unknown/Stale)、
    `observed_at` で注入 clock 使用可能。
  - `browse.rs`: `BrowseRequest`/`BrowsePage`/`BrowseEntry`/`Coverage`/
    `OpaquePageCursor`(非空・上限長を検証)。cursor は provider・scope・query・
    principal・epoch を束縛。Unknown sort 値は昇順・降順とも末尾固定。
  - `context.rs`/`error.rs`: `ProviderContext`(中断・deadline・byte/entry/depth
    budget・clock)。`ProviderErrorKind` は NotFound/PermissionDenied/
    UnsupportedCapability/SourceChanged/IntegrityMismatch/BudgetExceeded/
    Cancelled/CursorInvalid(=RetryAdvice::Never、再 browse 要求)など型付き。
  - `digest.rs`: SHA-256 streaming 検証。空 buffer `read(&mut [])` は no-op で
    擬似 EOF しない。`FetchSlot` は端末状態を保持し、結果を `take()` しても
    `is_finished()` は維持される。
  - `provider.rs`: `AssetProvider: Send + Sync`(blocking、RT/SDK 非公開型)。
    `id`/`kind`/`capabilities`/`browse`/`stat`/`pin`/`open_pinned`/optional
    `search`/`open_preview`。chunked `Read + Send` と `FetchReceipt`。
  - `registry.rs`: ProviderId + ProviderKind の両方を descriptor と照合。
  - `local.rs` (`LocalFolderProvider`): canonical root 封じ込め、安全な
    locator parse、`visited` を要求 `scope_dir` で seed して symlink 再入・
    祖先ループ・範囲外リークを防止、dangling link を `Unavailable`、post-open
    regular-file 再検査、continuation cursor が出る時だけ epoch を mint、
    `ConditionalStrong` content-hash pinning。
  - `project.rs` (`ProjectAssetsProvider`): `ProjectCatalog` projection と
    `ProjectBlobStore` port による read-only provider。cursor は project
    identity + revision を束縛、`observed_at(cx.now())`、canonical
    `source_state_name` provenance、`ImmutableObject` pinning、
    `open_pinned` は pin の `object_identity` で解決、pinned read 時に digest
    検証。`LocalBlobDir` は名称文字・DOS デバイス名拒否、candidate+root を
    canonicalize して containment 強制、regular file 要求、open 後再検査。
- `Cargo.toml`: workspace members に `crates/synvelis-library` を追加。
- 依存は `serde`/`serde_json`/`sha2`/`hex`/`thiserror`/`synvelis-project` のみ。
  GUI/Tokio/HTTP/Git SDK は公開 API に入らない。

### Decisions During Implementation

- Provider 層の `ProviderId`/`OpaqueLocator` は project の `AssetId`/`SourceId`/
  `ImportId` と別型を維持し、偶発的な混同を防ぐ。
- 未対応操作は `UnsupportedCapability` + typed `UnsupportedReason` を返し、
  空集合成功や黙って近似結果を返すことはしない。
- Provider は弱い証拠(弱 ETag、branch 名、mtime のみ)を pin 証拠としない。
  Local は content-hash `ConditionalStrong`、Project は immutable object を使う。
- Project provider は `ProjectController` を所有・変更せず、注入された
  read-only projection のみ消費する。token/ credential は Project データに保存しない。
- Lazy epoch: continuation cursor が必要な時だけ epoch を消費し、不要な live
  epoch 消費を避ける。
- `LocalBlobDir::resolve`/`open` は stat/open TOCTOU を縮小するため open 後の
  regular-file 再検査を行う(残存 race は「残リスク」参照)。

## Acceptance Matrix Traceability

- Local/Project の list/stat/pin/read で同一正本と Stable Identity:
  `tests/local.rs`/`tests/project.rs`/`tests/contract.rs` で provider 横断の
  identity・pin・read 経路を検証。rename/権限/symlink/変更で未固定データを
  固定扱いしない経路をカバー。
- Browse/Search: paging、取消、cursor/Principal/Revision 失効、0件 vs
  Partial/Offline/Stale、Unknown metadata:
  `tests/types.rs`/`tests/local.rs`/`tests/project.rs` で cursor 失効、
  foreign cursor、coverage、未知値ソート、credential redaction を確認。
  Search 非対応は typed Unsupported。
- typed Unsupported、CLI/headless 動作、Tokio/GUI 非公開:
  依存は std+serde+sha2+hex+thiserror+synvelis-project のみで検証。
- Local 既存 Import/Project/録音テストの非回帰、Ruviform/AudioLens 影響判定:
  フル workspace test 88 ターゲット全 pass で非回帰を確認。Ruviform 変更なし、
  AudioLens への影響なし(Synvelis 内閉じた crate 追加)。

## Related Experiments

### Adopted

- なし。

### Rejected

- なし。

## Validation

実行環境: Windows(Git Bash 上の cargo toolchain)。worker 並列枠のため
`CARGO_BUILD_JOBS=2` を使用。

| チェック | コマンド | 結果 |
| --- | --- | --- |
| Format | `cargo fmt --all` | pass |
| Format check | `cargo fmt --all -- --check` | pass |
| Compile | `cargo check --workspace --all-targets` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| Test | `cargo test --workspace --locked` | pass(88 test targets、0 failures;library 71 tests + workspace 他 + doc tests 2 件含む) |

最終シーケンスは `LocalBlobDir::open` の post-open regular-file 再検査追加後に
上記すべてを再実行し、全て pass。

### Review Protocol

- Reviewer A(協調): 10 candidates。
- Reviewer B(敵対): 複数 candidates。
- Validator: dedup 後の 23 candidates を全件 CONFIRMED。
- Judge: V2(`LocalBlobDir` containment/type-safety)と V3(local traversal の
  scope seed/confine 欠如)を blocking P1、V1/V5/V9/V10/V14 等を scope 内推奨として
  判定。
- 対応: 23 件全てを診断し、P1 両件 + scope 内の P2/P3 を修正。回帰テストを追加
  (symlink 範囲リーク/ループ、dangling link、blob-store escape/非 regular/
  DOS device、空 read、降順 unknown-last、cursor バリデーション、credential
  redaction、FetchSlot terminal latch、registry kind mismatch)。
- 再検証: focused re-validator が P1 両件を fixed と確認。残存指摘だった
  `LocalBlobDir::open` の stat/open TOCTOU も post-open 再検査で対処。

## Risks

- `LocalBlobDir`/`local.rs` の open 経路は stat→open→post-open check の構成であり、
  原子性は OS 依存。post-open `is_file` 再検査で streaming 前の型は保証するが、
  open 自体と別プロセスの置換の完全な排除は保証しない。
- `OpaqueCredential`/`PrincipalId` の transport 契約は型で制約するが、実際の
  credential vault/OS 認証統合は本スコープ外。
- Search の索引範囲・Preview の内容は provider capability 依存で、基準 provider は
  いずれも optional/unsupported を typed で返す。

## Known Limitations / Report

- `ProviderContext` の deadline/budget は協調的であり、blocking I/O 自体を
  preempt しない。
- `ProjectAssetsProvider` は catalog スナップショット供給者の一貫性に依存する;
  provider 側は revision 束縛 cursor で変更を `SourceChanged` として可視化する。
- macOS 未実測(NotRun)。Windows symlink 関連テストは本機開発者モード有効で実測。

## Follow-ups

- ALIB-02/03/04/05/06 が本公開型に依存して利用側契約を構築する。
- Remote/VCS provider(SVN/P4/SMB/S3)は capability 拡張として追加し、
  本契約を mandatory feature 化しない。
- `FetchSlot`/`ProviderContext` の下流所要(進捗、詳細な credential flow)は
  ALIB 後続 Issue で具体化する。

## Rollback

- `crates/synvelis-library` 全体と ALIB-01 doc 群、workspace `Cargo.toml` の
  member 追加を revert すれば本変更は除去できる。既存 crate への code 変更は
  workspace member 追加以外にない。
