---
id: "IMPL-034T7QIk72dphjZPfVnZ1O"
type: "impl"
status: "completed"
title: "ALIB-01 Asset Provider baseline contract"
created: "2026-11-06"
updated: "2026-11-07 (N1 修復追補・task6 RF409 移行追補)"
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
      - type: "file"
        path: "docs/tasks/alib-01-task5-rooted-cutover.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-library/src/local.rs"
      - type: "file"
        path: "crates/synvelis-library/src/project.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/containment_swap.rs"
      - type: "file"
        path: "docs/specs/alib-01-asset-provider-contract.md"
      - type: "file"
        path: "docs/designs/alib-01-asset-provider-contract-design.md"
      - type: "file"
        path: "docs/test-specs/alib-01-asset-provider-contract.md"
      - type: "file"
        path: "docs/tasks/README.md"
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
    - "docs/tasks/alib-01-task5-rooted-cutover.md"
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

本節は task1〜4 適用時点(baseline)の記録。`local.rs`/`project.rs` の
FS traversal 機構・依存一覧は **N1 修復(task5)で後節の記述に更新済み**
— 現行は `RootedDir` handle-relative resolve であり、本節の
canonicalize/post-open 再検査・`visited`/`scope_dir` seed・dep 列は
baseline 履歴として読むこと。

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
- 依存は `serde`/`serde_json`/`sha2`/`ruviform-io-file`(task5 で追加)/
  `synvelis-project` のみ。GUI/Tokio/HTTP/Git SDK は公開 API に入らない。

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
  （baseline 時点の判断。task5 で `resolve`/再検査は撤廃され
  `RootedDir::open_regular` の pre-open `NotRegularFile` へ移行済み）

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

### N1 修復後検証 (task5, 2026-11-07・同日の修復実測)

実行環境: Windows(Git Bash、rust toolchain pin 1.99.0)。`CARGO_BUILD_JOBS=2`。
ruviform 消費 rev は `5d48298b`(RF403・既定 main 追従 git dep、rev pin なし)。

| チェック | コマンド | 結果 |
| --- | --- | --- |
| Format | `mise run fmt` (`cargo fmt --all`) | pass (exit 0) |
| Format check | `mise run fmt-check` | pass (exit 0) |
| Compile | `mise run check` (`cargo check --workspace --all-targets`) | pass (exit 0) |
| Lint | `mise run lint` (`cargo clippy --workspace --all-targets -- -D warnings`) | pass (exit 0) |
| Test | `mise run test` (`cargo test --workspace --locked`) | pass (exit 0; 全 test target 0 failures — `containment_swap` 11/11: 10 swap 回帰 GREEN 強制 + seam sanity) |
| Swap 回帰(個別) | `cargo test -p synvelis-library --test containment_swap` | pass (11/11: `blob_stat_leaf_swap`/`blob_open_leaf_swap`/`local_stat_component_swap`/`local_pin_component_swap`/`local_preview_component_swap`/`local_open_pinned_component_swap`/`local_browse_descend_component_swap`/`local_scope_acquisition_swap_cannot_escape_root`/`local_scope_root_replacement_cannot_rebind_capability`/`local_scope_open_swap_cannot_escape_root` + `pre_traverse_hook_fires_at_tagged_points`) |
| Alias/authority 等価(個別) | `cargo test -p synvelis-library -- symlink rename swap scope` | pass (22 件: 10 swap + escaping/ancestor/dangling/loop/into-root link、blob escape、scope 系、rename pin — `EscapesRoot`→`PermissionDenied`・handle authority・`WithinRoot` follow の等価を固定) |
| Headless deps gate | `mise run check-headless-deps` | pass (exit 0; 10 headless crates) |
| ruviform rev 整合 | `mise run check-ruviform-rev` | pass (exit 0; resolved `5d48298b` 26 lockfile sources、lockfile-primary、REV match) |
| MSRV | `mise run test-msrv` (`mise exec rust@1.89.0 -- cargo test --workspace --locked`) | **fail (既存 main 由来)** — `cargo-platform@0.3.3` が rustc 1.91 を要求。当該 pkg は本差分の `Cargo.lock` delta に無く merge 済み HEAD に同 version で存在するため本変更起因ではない。別途 follow-up 対象。 |

補足:
- 旧表(上段)の `check-ruviform-rev` 行は pre-fix commit での記録
  (`49de02cc`・22 sources)。本差分適用後の正規結果は本表の
  `5d48298b`・26 sources 側。
- review 指摘を受けた後追い修正も検証済み: `LoopDetected`→`InvalidRequest`/
  `LimitExceeded`→`BudgetExceeded` への remap、browse で非 link entry の
  `metadata` が `NotFound` 以外で失敗した際 `truncated=true` とする変更を
  実施し、上表の lint/test は修正後の code で再実行した結果。
- Unix/macOS および実 race は引き続き未実測(Windows 実測のみ)。

### N1 修復後検証 (task6, 2026-11-07・RF409 正規契約移行の実測)

実行環境: Windows(Git Bash、rust toolchain pin 1.99.0、MSRV
検証は rust 1.89.0)。`origin/main` (`22ff63d6`) merge 済み HEAD
で実施。ruviform 消費 rev は `79d1db0a`(RF409/PR412・既定 main
追従 git dep、rev pin なし、26 lockfile sources 単一 rev)。

| チェック | コマンド | 結果 |
| --- | --- | --- |
| Format check | `mise run fmt-check` | pass (exit 0) |
| Compile | `mise run check` (`cargo check --workspace --all-targets`) | pass (exit 0) |
| Lint | `mise run lint` (`cargo clippy --workspace --all-targets -- -D warnings`) | pass (exit 0; `NodeId` は browse loop guard で継続使用、unused import なし) |
| Test | `mise run test` (`cargo test --workspace --locked`) | pass (exit 0; 全 test target 0 failures — `synvelis-library`: containment_swap 11/11・local 34・project 21・contract 2・types 15) |
| MSRV | `mise run test-msrv` (`mise exec rust@1.89.0 -- cargo test --workspace --locked`) | pass (exit 0; task5 記録の `cargo-platform@0.3.3` rustc 1.91 要求は merge 後 lock で解消) |
| ruviform rev 整合 | `mise run check-ruviform-rev` | pass (exit 0; resolved `79d1db0a`・26 lockfile sources・lockfile-primary・REV match) |
| Headless deps gate | `mise run check-headless-deps` | pass (exit 0; 10 headless crates) |

補足:
- 追加回帰: `tests/local.rs` の
  `cancel_during_directory_enumeration_fails_the_page`（`"read_dir"`
  barrier で token cancel → `read_dir_each` visitor が
  `Cancelled` を返す）と dir entry `byte_len`=`Unknown` 明示
  assert。containment_swap は RF409 契約へ更新済み（上記
  「N1 修復」節）。
- Unix/macOS は引き続き未実測(Windows 実測のみ)。

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
- ~~現状(この commit)~~ **修復前の状態(task5 適用前の記録・履歴として保持)**:
  deterministic fault seam(`set_pre_traverse_hook`,
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
  → **解除済み**: RF#398 は PR401(main `f483fe1c`)として統合、消費 rev
  `5d48298b`。task5 で capability 置換・7 件 unignore・GREEN 確認済み
  (次節および後述 Validation 追補)。
- 残存 sub-issues(修復後も別途観測が必要): ~~FIFO/device swap 時の
  `File::open` block(P2、真の遮断には leaf を non-regular のまま open しない
  handle-relative open が必須)~~ **解決済み(task5)** — `open_regular` が
  open 前に `NotRegularFile` を返し blocking open に到達しない。
  dir target の stat race(metadata のみ)は rooted `metadata` の単一
  resolve に統合済み。hard link(同一 object ゆえ name ベース
  containment の許容内)は変わらず残存。store root 自体の
  swap/ドリフトについては `LocalBlobDir` が per-op `RootedDir::open_with`
  で capability を取得するため、各呼出内では anchored — 呼出と呼出の
  間で store dir 自体が置換されれば新 capability は新 object を指す
  (per-op open は旧 canonicalize-per-call と同じ粒度であり、これは
  lifecycle 維持のため意図的)。

### N1 修復 — rooted capability 移行 (task5, 2026-11-07)

Ruviform `ruviform-io-file::rooted`（RF401/ruviform#398、消費 rev
`5d48298b`、既定 main 追従 git dep — SHA 固定・vendor なし）への移行を
実施した。`synvelis-library` に `ruviform-io-file` を追加し、SV 内に
unsafe・OS 呼出重複・汎用 FS framework・`cap-std` は置いていない。

- `LocalFolderProvider` は `RootedDir`（`rooted_options()`:
  max_depth 256 / max_components_total 2048 / max_open_dirs 256 —
  旧 canonicalize 経路の実質 acceptance を下回らない有限 bound）を
  保持し、locator は `RelativePath` で検証する。
  - `stat` = `metadata(WithinRoot)` 一回 — leaf_kind で link を
    `Symlink` と報告しつつ resolved facts を同一 resolve から取得。
  - `pin`/`open_preview`/`open_pinned` = `open_regular(WithinRoot)`
    一回の verified open — `RootedFile`（`Read+Seek`）と `NodeInfo`
    が同一 opened object を記述し、stat→open の 2 段構成は消滅。
    FIFO/device/link-under-NoFollow は open 前に `NotRegularFile`
    で拒否され blocking open に到達しない（旧 post-open `is_file`
    再検査の P2 残存 DoS も同時解消）。
  - `browse` = 保持 `RootedDir` の `read_dir_each`/`open_dir`
    経由の保持 handle DFS — descend は実 dir の scoped handle
    のみで、scope 取得後の component swap は
    `EscapesRoot`/`NotDirectory` に失敗するか取得済み handle が
    透過する。locator scope は ruviform#409 の
    `open_scoped_root(rel, WithinRoot)` で取得する — 解決済み
    directory handle 自体が capability floor となるため
    pathname 再 open の window が存在せず、`NodeId`/
    strong-identity 照合も不要（weak backend を gate しない。
    root rename + 同一 pathname 別 tree 設置は held handle が
    address する別 object で rebind 不可能）。`open_dir` は
    親 capability の `WithinRoot` 境界を引き継ぐため scope
    境界を作れず不使用。公開 `base` prefix は scope canonical
    を held root の handle-derived canonical
    （`open_scoped_root("")` で再 anchor）から strip して求める
    — 両辺 handle 由来のため rename 後も canonical-relative を
    維持し、prefix 不成立は `PermissionDenied` で fail-closed。
    seam tag は `"open"` 1 地点。kept window は
    `read_dir_each` visitor が `remaining+1` 件で
    `EntryFlow::Stop`（`Vec` 全件 materialize なし — 列挙段階の
    memory/work bound と cancel/deadline 適用が回復）。visitor
    内で `cx.check_interrupts()` を観測し `Stop`→`Cancelled`/
    `BudgetExceeded` を返す。kept window は旧 `take(remaining+1)`
    と同じ enumeration-order prefix で sort 後の page 意味論は
    同値。
- `LocalBlobDir` は呼出毎に store `RootedDir` を開き
  `metadata`/`open_regular(WithinRoot)` で同一 object の facts+reader
  を返す。SV 側の filename-safe/DOS device 検証は維持。store dir
  自体の作成を構築時に要求しない lifecycle も維持。
- `FileErrorKind`→`ProviderErrorKind`: `EscapesRoot`→`PermissionDenied`、
  `Unsupported`→`UnsupportedCapability`、`IdentityMismatch`→
  `SourceChanged`、`NotRegularFile`/`NotDirectory`/`InvalidInput`/
  `LoopDetected`→`InvalidRequest`、`LimitExceeded`→`BudgetExceeded`、
  `Io` は source の `io::ErrorKind`（`NotFound`/`PermissionDenied`/
  その他 `TransientIo`）で精緻化。loop/budget 拒否は決定的なので
  retry を唆す `TransientIo` には畳まない。
- `tests/containment_swap.rs` — swap 回帰を GREEN 強制
  （task5 で 7 件 `#[ignore]` 解除 + scope 取得境界の回帰 3 件、
  task6 で RF409 契約へ更新）。seam sanity の期待列は
  blobs `["stat","open"]`、scoped browse `["open","read_dir","stat"]`
  （`open_scope` の touch point は `open_scoped_root` の
  `"open"` 1 地点に集約）。task6 での回帰更新:
  `local_scope_acquisition_swap_cannot_escape_root` は
  `"open"` tag で scope dir→outside link swap→`PermissionDenied`、
  `local_scope_root_replacement_cannot_rebind_capability` は
  root rename+別 tree 設置で held tree を serve し replacement
  を出さないことを固定（旧 `SourceChanged` 期待を正規意味論へ）、
  `local_scope_open_swap_cannot_escape_root` を
  `local_scope_post_acquisition_swap_keeps_held_entries` へ置換
  （取得後 swap は held handle が透過 — 旧 metadata/open_with
  間 window は契約上存在しない）。
- Project provider 側の変更なし — `open_blob_verified` は port
  経由で `LocalBlobDir` を呼び、修正は blob store 内に閉じる。
- `pin` の `ConditionalStrong{ContentHash}` content pin 契約は維持
  （object-identity `NodePin` は契約上の pin 証拠でないため使用せず、
  weak identity FS への追加制限も課さない）。open_pinned の
  expected_size/digest 照合も同一 opened object 上で継続。

### N1 修復の互換差（診断済み・意図的）

- `RelativePath` 文法は旧 `parse_locator` より厳格 — Windows 予約名・
  空白端・末尾 `.`・>255B component・制御文字・非 UTF-8 を `InvalidInput`
  で拒否する。該当名の object は locator では恒久にアクセス不可となり、
  browse では unaddressable として partial skip（旧実装の unaddressable
  扱いと同じ分類）。
- browse の link 処理: 範囲内 dir link は引き続き `Container`+
  `via_symlink` で表示するが、下降は実 dir のみ（link target は
  canonical 実 path 側で必ず列挙されるため出力 entry は同値）。旧
  canonical-path visited set は `NodeId` 集合へ置換（mount/bind alias
  用の loop guard として温存）。
- escaping/dangling link entry の facts は link 自身の最小 facts
  （`Symlink` kind + NotProvided）を返す — 旧実装は follow した
  target の facts を報告しており、これは containment の外側の
  情報流出だったため修正（より正直な報告）。
- ~~`RootedDir::read_dir` は listing を一括 `Vec<DirEntry>` で
  返し、列挙段階の memory/work bound・cancel/deadline 適用が
  RF API に存在しなかった — BLOCKED（RF API 待ち）。~~
  **解決済み(task6)**: ruviform#409/PR412 の `read_dir_each`
  （callback 列挙 + `EntryFlow::Stop`/`EnumerationOutcome`）を
  消費し、kept window（`remaining+1`）を visitor で収集してから
  sort — 旧 `take(remaining+1)` の enumeration-order prefix
  subset 意味論は同値で、`Vec` 全件 materialize がなく列挙
  段階の bound と cancel/deadline が回復した。
- dangling symlink は Windows/Unix で `Missing` に統一 — 旧 Unix 経路は
  `child.metadata()` 失敗で entry 自体を黙って skip しており、
  test-spec の `Missing` 契約は Windows でしか満たされていなかった
  （潜在一个不整合の修正）。
- dir entry の `byte_len` は `NotApplicable` — `NodeInfo` は dir に
  length を持たせない（旧実装は dir size を報告していた）。
- `LocalFolderProvider::new` の open 失敗は `Io`→`TransientIo` で
  正直に分類（旧実装は canonicalize 全失敗を `NotFound` に畳んでいた）。
- RF の `WithinRoot` は canonical-root component の lexical prefix で
  absolute link target を判定する — `/var` vs `/private/var`・8.3 名・
  `subst` drive 等の非 canonical alias は kernel 上 in-root であっても
  `EscapesRoot` を返し得る（RF の文書化された保守的縮小であり、SV が
  隠すものではない）。
- scoped directory handle は inode authority を保持し、取得後の外部
  rename でも読み取り継続する — pathname 継続包含ではなく handle
  authority（RF の設計どおり）。task6 で locator scope 付き
  browse/search も `open_scoped_root` により同じ handle authority
  を持つ: root rename + 同一 pathname 別 tree 設置では held tree
  を serve し replacement（別 object）は列挙されない（task5 の
  pathname 再 open + `NodeId` 照合による `SourceChanged` 拒否は、
  正規契約では不要な補償として撤去 — held handle が address する
  object が変わらないため rebind window が構造上存在しない）。
  in-root の scope dir rename/recreate も取得時点の object を
  serve する（旧実装の `SourceChanged` 厳格化と異なるが、
  capability が address するのは取得済み object でありこれは
  正規の handle-authority 意味論）。
- ~~weak-identity filesystem では locator scope 付き browse/search
  が `UnsupportedCapability` を返す~~ **撤去(task6)**:
  `open_scoped_root` は `NodeId`/strong identity を要求しないため
  weak backend でも scoped browse は動作する（root 置換 rebind は
  handle authority で構造的に不可能 — gate の対象だった
  pathname 再 open window が存在しない）。Root scope・stat/open
  系と同様に制限なし。
- trailing `/` は `RelativePath` が directory marker として受理する
  （旧実装は空 component として拒否 — acceptance 拡大で同じ object
  を指す）。

## Risks

- ~~`LocalBlobDir`/`local.rs` の open/stat/read_dir 経路は
  check(canonicalize)→traverse(再解決)の構成であり、T0→T1 の component swap
  で out-of-root regular file を開き得る(N1、上記。実 race 未測定・fault
  barrier で確定再現済み)。恒久対応は RF 所有の rooted-read capability。~~
  **解決済み(task5)**: 全経路が `RootedDir` の handle-relative lookup で、
  capability 配下の path 再 traverse は存在しない。残るのは RF 文書化の
  documented residual — 各 `RootedDir::open_with` の path→capability 境界
  (handle-derived canonicalization が使えない backend では path
  canonicalize fallback) と、escaping alias の保守的 `EscapesRoot`。
- `RootedDir::open_with` の path→capability 境界には RF 文書化の
  residual race が残る(backend が handle-derived canonical を提供しない
  場合の `fs::canonicalize` fallback)。task6 で capability 取得地点は
  2 箇所に縮小: `LocalFolderProvider::new`（一度きり）、
  `LocalBlobDir::store`（呼出毎、構築後作成の lifecycle 維持）。
  locator scope 付き browse/search の `open_scope` は task5 の
  `open_with`（pathname 再 open + `NodeId` 照合）から ruviform#409
  の `open_scoped_root` へ移行し、保持 handle 起点の scoped
  capability 取得となったため path→capability 境界を持たない。
  いずれの取得後 lookup も handle-relative であり、境界は
  acquisition 一点に限定される。真の rooted-open 強化が必要なら
  RF 側の責務として起票する。
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
