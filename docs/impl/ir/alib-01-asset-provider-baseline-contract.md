---
id: "IMPL-034T7QIk72dphjZPfVnZ1O"
type: "impl"
status: "completed"
title: "ALIB-01 Asset Provider baseline contract"
created: "2026-11-06"
updated: "2026-11-07"
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
        path: "crates/synvelis-library/tests/containment_swap.rs"
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
        path: "crates/synvelis-library/src/local.rs"
      - type: "file"
        path: "crates/synvelis-library/src/project.rs"
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "tools/check-headless-deps.mjs"
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

実行環境: Windows(Git Bash、rust toolchain pin 1.99.0 / MSRV 1.89)。
worker 並列枠のため `CARGO_BUILD_JOBS=2` を使用。正規入口は mise task。

| チェック | コマンド | 結果 |
| --- | --- | --- |
| Format check | `mise run fmt-check` (`cargo fmt --all -- --check`) | pass (exit 0) |
| Compile | `mise run check` (`cargo check --workspace --all-targets`) | pass (exit 0) |
| Lint | `mise run lint` (`cargo clippy --workspace --all-targets -- -D warnings`) | pass (exit 0) |
| Test | `mise run test` (`cargo test --workspace --locked`) | pass (exit 0; 88 test targets・0 failures、synvelis-library 71 tests・doc tests 含む) |
| Headless deps gate | `mise run check-headless-deps` (`node tools/check-headless-deps.mjs`) | pass (exit 0; 9 headless crates、`synvelis-library` 登録済) |
| ruviform rev 整合 | `mise run check-ruviform-rev` (`node tools/check-ruviform-rev.mjs`) | pass (exit 0; resolved `49de02cc` 22 sources、REV match) |

補足:
- `check-ruviform-rev` は初回 exit 1(vendor REV `8fb665f` != lockfile
  `49de02cc`)。原因は未追跡の `ui/vendor/ruviform/REV` が base #171 由来の
  lockfile rev より古い local vendor staleness で、本差分・lockfile への影響なし
  (baseline `d8bf8ca` 時点の lockfile が既に `49de02cc` を解決)。正規入口
  `mise run sync-ui-deps` で `ui/vendor` を lockfile SHA に再生成して復旧し、
  tracked file への変更はない。
- `check-headless-deps` の負側境界は一時コピー fixture で検証(HEADLESS_CRATES
  を `synvelis-library`/`synvelis-project`、EXACT に `serde` を注入した
  `headless-neg.mjs` が exit 1 で `synvelis-library: serde` を検出)。
  本物の `tools/check-headless-deps.mjs` は変更していない(登録行追加のみ)。
- 補助証拠として同一結果の直接 cargo 実行ログも保持するが、上表の mise
  exit を正規結果とする。

### N1 deterministic fault-barrier 回帰 (2026-11-07 追補)

| チェック | コマンド | 結果 |
| --- | --- | --- |
| seam 配線 | `cargo test -p synvelis-library --test containment_swap` | `pre_traverse_hook_fires_at_tagged_points` pass、7 件 ignored |
| RED 証拠 | `cargo test -p synvelis-library --test containment_swap -- --ignored` | **7 件全件失敗(想定)** — leaf/component swap で out-of-root bytes(`outside-secret-bytes` 20B)が open/preview/pinned 経路へ流出、`stat` は out-of-root facts(len 20)を報告、`pin` は out-of-root file の sha256 を `digest_hint` として返却、`browse` descend は out-of-root entry `out-secret.wav` を列挙 |

上記 7 件は RF rooted-read 契約で修復されるまで `#[ignore]` + 理由付きで
RED 証拠として保持し、green 達成前に成功扱いしない。`mise run test` 本体は
非回帰(ignore 済み分を除く全 test pass)。

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
- post-open 追補後の独立 Validator(2nd round): `LocalBlobDir::open` の
  post-open `is_file()` 再検査、`local.rs` 全 open site の post-open 再検査、
  blob/symlink/非 regular の回帰テスト存在を全件 CONFIRMED。残余指摘:
  - N1(Low–Medium と当時評価): `resolve`→`File::open` 間の path 再traverse は
    component-swap TOCTOU に対し原理的に残存(`std` は openat/dirfd 契約を
    公開しない)。
  - N2(Low): `LocalBlobDir::open` が pre-open stat の `byte_len` を返しており
    local.rs(post-open `file_meta.len()`)と不一致。post-open metadata 由来に
    統一済み。
- 独立 Reviewer A/B + Validator(3rd round, head d27bbee): N1 を CONFIRMED、
  coordinator Judge は **P1 BLOCKING** と判定。当時の N1 評価コメントは
  以下の点で不正確だった(訂正):
  - ~~「影響は `open_preview`(Unverified)の out-of-root regular file 読み取りに
    限定」~~ → **誤り**。`digest.rs` の `VerifyingReader` は EOF 到達 *前* に
    読み取り byte を caller へ返す(streaming 検証は EOF で改竄を「検出」するが、
    既に届いた byte の開示を防げない)。よって `open_pinned`/`open_preview` で
    out-of-root file の bytes が届き、`pin` は out-of-root file の
    sha256+size を `digest_hint`/`expected_size` として返す(content disclosure
    oracle)。`stat`/`browse` の `fs::metadata`/`read_dir` 再traverse も同型で、
    out-of-root object の byte_len/modified/entries が facts として報告される。
    内容整合性(digest)と capability containment は別物であり、digest 事後検証は
    containment の代替にならない。
  - ~~「`is_file()` 再検査により非 regular の読み取り/DoS は遮断」~~ → **誤り**。
    post-open `is_file()` は streaming 前の *type* しか保証せず、swap 後に
    FIFO/device を指した場合 `File::open` 自体が block する/副作用を持ち得る
    (open 呼出しは check より前)。これは残存 DoS(P2)であり、遮断済みではない。
  - 実 race(並行 swap)は未測定であり、本不具合の観測は deterministic fault
    barrier 注入による。Unix 未実測(開発/CI 実測は Windows)。また
    `synvelis-library` は現時点で app crate へ未配線(利用経路なし)で、これは
    重大度の評価とは別論点として記録する。

### N1 containment 欠陥 — 所有境界の診断と現状 (Refs #85)

- 破られた不変条件: 「root 外 object の bytes/facts/entries は如何なる
  stat/pin/open/browse-descend 経路からも返さない」。`canonicalize`+`starts_with`
  は check 時点(T0)の path のみを証明し、traverse 時点(T1)の object を
  証明しない。leaf/single-component(`LocalBlobDir`)と intermediate-component
  (`LocalFolderProvider` の `sub/` 等)の両 swap で T1 が out-of-root object を
  解決する。
- 影響面: `LocalBlobDir::{open,stat}`(project.rs)、`LocalFolderProvider` の
  `stat`/`pin`/`open_preview`/`open_pinned`/`collect_entries`(local.rs)。
- std/既存 dep で閉じられない根拠(実測):
  - Windows: `std::os::windows::fs::MetadataExt` の `file_index`/
    `volume_serial_number` は unstable(`windows_by_handle`, rust#63010)で
    pinned toolchain(1.99.0)ではコンパイル不可。Win32 `CreateFile` に
    `OBJ_DONT_REPARSE` 相当のフラグは存在しない(NT-level only)。
    handle→path(`GetFinalPathNameByHandle`)も std 非公開。
  - Unix: `MetadataExt::dev()/ino()` で post-open identity 検証は std-only で
    可能だが、open 時点の曝露(FIFO block・side-effect open・dir stat race)は
    残り、platform 保証が分岐する。
  - workspace `unsafe_code = "forbid"`(`#[expect]` で解除不可)により SV 内
    FFI 実装は lint ゲートを通過しない。新規 dep は承認前に追加不可。
- 所有境界の結論: handle-relative traversal(openat+O_NOFOLLOW /
  RootDirectory+OBJ_DONT_REPARSE)を所有するのは Ruviform(RF main 59c7e71 時点で
  公開 API なし。`ruviform-io-archive::extract::{fs,unix,windows}.rs` に
  private machinery と `NodeIdentity`、cfg(test) `MockExtractOps` あり)。
  SV 側への汎用 OS/atomic FS の重複実装・unsafe 追加・digest 事後検証による
  代替は行わない。FollowWithinRoot(root 内 symlink を辿る現行契約)を
  NoFollow へ無断縮小しない。
- 現状(この commit): deterministic fault seam(`set_pre_traverse_hook`,
  `#[doc(hidden)]`)を `LocalBlobDir`/`LocalFolderProvider` の再traverse 地点
  (op: `"stat"`/`"open"`/`"read_dir"`)へ追加。`tests/containment_swap.rs` に
  7 件の deterministic swap 回帰を **RED 証拠** として `#[ignore]` 付きで収録
  — 実行すると現実装では全件失敗する(下記 Validation)。containment 修復本体は
  RF rooted-read 公開契約待ちで **Blocked (Refs #85)**。
  依存先: [ruviform#398](https://github.com/scarecrow173/ruviform/issues/398)
  (rooted-read contract、独立設計 review 中 — 確定 API を先取りしない)。
  解除条件: RF 側で rooted `open_file`/`metadata`/`open_dir`
  (FollowWithinRoot 意味論保持)が公開され candidate SHA が共有されたら、
  SV は `local.rs`/`project.rs` の path 再traverse を当該 capability へ
  置き換え、7 件の `#[ignore]` を外して green を確認する。
- 残存 sub-issues(修復後も別途観測が必要): FIFO/device swap 時の
  `File::open` block(P2、真の遮断には leaf を non-regular のまま open しない
  handle-relative open が必須)、dir target の stat race(metadata のみ)、
  hard link(同一 object ゆえ name ベース containment の許容内)。
  `LocalBlobDir::new` は `LocalFolderProvider::new` と異なり root を
  構築時に canonicalize せず `resolve` が呼出毎に `self.dir` を再
  canonicalize するため、store root 自体の swap/ドリフトは stat/open
  呼出間で anchored でない TOCTOU-adjacent 面として残る(rooted-read
  化で吸収されるが独立観測として記録)。

## Risks

- `LocalBlobDir`/`local.rs` の open/stat/read_dir 経路は
  check(canonicalize)→traverse(再解決)の構成であり、T0→T1 の component swap
  で out-of-root regular file を開き得る(N1、上記。実 race 未測定・fault
  barrier で確定再現済み)。恒久対応は RF 所有の rooted-read capability。
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
