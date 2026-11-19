# アセットライブラリと Asset Provider 契約 — 実行証拠

この文書は、Synvelis の Asset Library 系作業（ALIB-01〜07、synvelis-library クレート、製品レベル Asset Library 設計契約 synvelis#9）で実施された検証と、その結果得られた実測値・発見された不具合と処置・下された判断を記録する。計画段階の記述と実施済みの検証を区別し、headless/mock/fixture の Pass を実環境の受入に読み替えない方針で記述する。

## 検証構造

検証は `synvelis-library` の test 群と `mise run` の canonical lane（fmt-check/check/lint/test/build/test-rt/test-msrv）を組み合わせて行われた。provider 横断の contract/local/project/types/containment_swap test で identity・pin・read 経路・cursor 失効・coverage・credential redaction を検証し、`containment_swap.rs` は deterministic fault-barrier seam（`set_pre_traverse_hook`）で leaf/component swap を注入する 10 swap 回帰 + scope 取得境界回帰を持つ。seam sanity の期待 op 列は blobs `["stat","open"]`、scoped browse `["open","read_dir","stat"]`（task6 で `open_scoped_root` の `"open"` 1 地点に集約）である。`tests/local.rs` には `cancel_during_directory_enumeration_fails_the_page`（`"read_dir"` barrier で token cancel → `read_dir_each` visitor が `Cancelled` を返す）と dir entry `byte_len`=`Unknown` 明示 assert を含む（synvelis#85）。

import 系は `tests/import.rs`（bounded streaming 1 MiB @ 7-byte chunks、opaque bytes、replay 冪等/conflict、digest/size/budget integrity、stale+明示 rebase、missing/wrong-track lane 拒否、publish 失敗 resume、indeterminate reconcile、DirSync 失敗記録、orphan GC、namespace 分離、live≠saved）、`tests/import_recovery.rs`（save/load replay・save 失敗回復・undo 再 import・stale→target 消失・複数 track 配置・legacy namespace 共存）、`tests/verified_import.rs`（consumer 配線、PR #326）、`tests/alib02_digest.rs`/`alib02_v5_real.rs`（v5 互換）で recovery・consumer・migration を検証した（synvelis#86）。

Git provider は `tests/git.rs` で offline `file://` fixture（実 work repo + bare remote + 手配置 `lfs/objects` store + gitlink + symlink + secrets）による browse/stat/pin/open の固定 commit roundtrip、LFS pointer→実 object 検証、selective fetch 証拠（`rev-list --missing=print` で未選択 object が mirror に来ないこと）、pin 固定性（branch 更新・force-push 後も同 bytes）、import_verified 接続、cursor 拒否、cancel/budget/retry 分類、secret 非漏洩、worktree/index/HEAD/remote 非破壊を検証した（synvelis#88）。

LFS remote は `tests/lfs.rs` で in-process `MockServer`（`tests/common/webdav_fixture.rs` の `TcpListener`）が batch POST と download GET を scripted response で返し、git objects は `git clone --mirror` で seed した local mirror から供給した（`reference="main"` で `ls-remote` を回避し git network 操作を排除）（synvelis#331）。

Browser/Import UX は `tests/common/library.rs` の scripted `FakeProvider` + temp-dir `ImportStore`/`LocalBlobDir`/`StagedBlobPublisher` fixture で `syn.library.*` op surface を `tests/library_browser.rs`（16 test）で検証し、`library_ui.rs` に `#[cfg(test)]` 14 unit test（`search_mode_survives_refresh_and_more`/`preview_cancel_does_not_poison_later_plan` regression 含む）を持つ（synvelis#89）。

WebDAV は `tests/webdav.rs` + in-process fixture（`TcpListener` で request を capture し scripted response）で URL/authority 受理拒否、root confinement、browse 完全性/pagination/partial coverage/truncation/foreign href/file scope 拒否、strong/weak/missing ETag pin、If-Match proof・conditional read、応答 ETag 不一致/412/404 分類、range 206/ignored-range 200/416、max_bytes、cancel、credential 解決・secret 非漏洩、same-origin redirect・cross-origin 拒否、非 WebDAV server 拒否、foreign proof 拒否、https typed unsupported、import 接続を検証した（synvelis#90）。

ALIB-07 は Case ID 別 matrix（LIB-* 42 class）で Expected/Observed/Status を記録し、git 3 件・webdav 9 件・import 2 件（slice1）+ import 4 件（slice2: verify/publish/commit 相 cancel + subprocess crash 2 point）を追加した（synvelis#91）。

ALIB-03 は scripted provider fixture（online/offline/permission/auth-expired/search-unsupported/truncated/gated slow stream/per-principal preview bytes/open-stream counter）と `stats_alloc` Region による RT alloc 計測を計画したが、plan/task は in-progress で実施記録はない（synvelis#87）。

asset-library（synvelis#9）は case ledger に LIB-* case → fixture/oracle/Owner/mode/layer/expected-observed/status/diagnostic/link を配置し、実装 case は `NotRun`、docs 検証 case のみ実行結果を記録する方針である（synvelis#9）。

## ALIB-01 Asset Provider 基準契約（synvelis#85）

baseline として `mise run fmt-check`/`check`/`lint`/`test` 全 pass、`mise run test`（`cargo test --workspace --locked`）で 88 test targets・0 failures、synvelis-library 71 tests pass（doc tests 含む）。`check-headless-deps`（9 headless crates、`synvelis-library` 登録済）・`check-ruviform-rev` pass（synvelis#85）。

`check-ruviform-rev` 初回は exit 1（vendor REV `8fb665f` != lockfile `49de02cc`）で、原因は未追跡 `ui/vendor/ruviform/REV` が base #171 由来の lockfile rev より古い local vendor staleness であり、本差分・lockfile への影響なし（baseline `d8bf8ca` 時点の lockfile が既に `49de02cc` を解決）。正規入口 `mise run sync-ui-deps` で `ui/vendor` を lockfile SHA に再生成して復旧し、tracked file への変更なしである（synvelis#85）。

`check-headless-deps` の負側境界は一時コピー fixture で検証した（`HEADLESS_CRATES` を `synvelis-library`/`synvelis-project`、EXACT に `serde` を注入した `headless-neg.mjs` が exit 1 で `synvelis-library: serde` を検出）。本物の `tools/check-headless-deps.mjs` は変更していない（登録行追加のみ）（synvelis#85）。

N1 修復（task5、ruviform `5d48298b`/RF403・PR401）では 7 件の swap 回帰を `#[ignore]` 解除し GREEN 強制（scope 取得境界の追加回帰を含め計 10 件 + seam sanity = containment_swap 11/11）。`mise run test` 全 target 0 failures、local 33・project 21・contract・types 非回帰。alias/authority 等価テスト 22 件 pass（`EscapesRoot`→`PermissionDenied`・handle authority・`WithinRoot` follow の等価を固定）。`check-ruviform-rev` resolved `5d48298b` 26 lockfile sources・REV match（synvelis#85）。

N1 修復（task6、RF409/PR412・rev `79d1db0a`）では `open_scoped_root` + `read_dir_each` へ移行し containment_swap 11/11、local 34・project 21・contract 2・types 15 全 pass。`check-ruviform-rev` resolved `79d1db0a`・26 lockfile sources・lockfile-primary・REV match。`mise run test-msrv`（rust 1.89.0）は pass（exit 0）（synvelis#85、ruviform#409）。

review での発見と処置として、Reviewer A 10 candidates + Reviewer B 複数 candidates → Validator dedup 後 23 candidates 全件 CONFIRMED → Judge が V2（`LocalBlobDir` containment/type-safety）と V3（local traversal の scope seed/confine 欠如）を blocking P1、V1/V5/V9/V10/V14 等を scope 内推奨と判定した。23 件全てを診断し P1 両件 + scope 内 P2/P3 を修正、回帰テスト追加（symlink 範囲リーク/ループ、dangling link、blob-store escape/非 regular/DOS device、空 read、降順 unknown-last、cursor バリデーション、credential redaction、FetchSlot terminal latch、registry kind mismatch）。focused re-validator が P1 両件を fixed と確認した（synvelis#85）。

残存指摘の `LocalBlobDir::open` stat/open TOCTOU も post-open 再検査で対処 → その後 N1 として P1 BLOCKING に格上げ・rooted 移行で解決。review 指摘を受けた後追い修正（`LoopDetected`→`InvalidRequest`/`LimitExceeded`→`BudgetExceeded` remap、browse で非 link entry の `metadata` が `NotFound` 以外で失敗した際 `truncated=true`）も検証済みである（synvelis#85）。

`test-msrv` の推移として、task5 時点で fail（`cargo-platform@0.3.3` が rustc 1.91 を要求。当該 pkg は本差分の `Cargo.lock` delta に無く merge 済み HEAD に同 version で存在するため本変更起因でないと診断、別途 follow-up）→ task6（`origin/main` `22ff63d6` merge 後）で pass（exit 0）。residual-closeout でも `mise run test-msrv` pass（rust 1.89.0、2,091 tests / 0 failed）（synvelis#85、#86）。

## ALIB-02 Streaming Verified Import（synvelis#86）

PR #220（squash `929a5e9`）では `cargo test -p synvelis-library --test import` 15/15 pass、`mise run fmt-check`/`check`/`lint`/`test` 全 exit 0（merge 後再実行も exit 0）。review-protocol 1 サイクルで blocker なし。当時 `test-msrv`/`test-rt`/`build` は時間切れで NotRun（後続 lane で workspace 全体 green を確認）（synvelis#86）。

v5 compat（PR #253）では `cargo test -p synvelis-project --test alib02_digest` 5 passed、`cargo test -p synvelis-project`（workspace project crate）全 pass、`cargo test -p synvelis-library --test import_recovery` 6 passed、`cargo test -p synvelis-library --test import` 17 passed（回帰なし）。`validate_references` 強化が既存 fixture を壊すリスクは workspace test で回帰なしを確認した（synvelis#86）。

v5 real fixture（PR #328）では `cargo test -p synvelis-project --test alib02_v5_real` 8 passed。v5-era build（commit `a401e5c`、`SCHEMA_VERSION=5`）実出力 fixture（`alib02-v5-legacy.json` 6,328B / `alib02-v5-legacy.blob` 9,600B、CRC fold `5fea10b959fe82a9`）で decode→STEPS 全 ladder→digest verbatim→receipt 整合→save→reload、routing 投影（track strip + `Main` connection のみ、send/processor 合成なし）、`scope` owner typed 投影、revision `22→23`、旧 bytes 不変、`resolve_saved`×2 で `PartialEq` deterministic projection、`schema_version 5 → projection_version 13`、`Valid`/`Complete` を確認した（synvelis#86）。

同 fixture で missing blob→`Missing`/`Unknown`→`Degraded`+`Partial`（`Present` 捏造なし）、unknown schema 4/99→両 reader で `UnsupportedSchema` reject、corrupt bytes→`Deserialize`・非 default `RoutingModel` 混入→`Validation`（foreign reject）、CRC collision 2 asset 別 identity・非 content-addressed を固定した（synvelis#86）。

residual closeout（synvelis#86、文書と evidence のみの変更）では canonical lane 7 本全 Pass — `mise run fmt-check`/`check`/`lint`/`test`/`build`/`test-rt`/`test-msrv`。`mise run test` で 2,091 tests / 202 binaries（ALIB-02 5 binaries 48 tests all ok）、`test-rt` 104 `rt_` tests、`test-msrv` rust 1.89.0 で 2,091 tests / 0 failed。`import.rs` 実測 21 tests（test-spec の 15 記述を訂正）。Issue #86 の AC-1〜4・残実装 DoD(1)〜(4) は headless 実測で充足、AC-5（Linux/Windows/macOS FS 差・RT 非阻害・AudioLens baseline・Ruviform IO-01 責務差の実機証拠）は外部 gate 残存で **Issue open 継続**である（synvelis#86）。

fault-injection followup（synvelis#86）では `tests/import.rs` 21→25 tests、新規 4 tests（`size_mismatch_is_integrity_error`/`staged_drift_is_corrupt`/`existing_blob_mismatch_is_integrity_error`/`directory_sync_unsupported_is_recorded_not_hidden`）全 ok。`mise run test synvelis-library` で unit 27・import 25・import_recovery 6 他全 Pass、doc-test 失敗なし。spec AC 2 行を実在 test 名と 1:1 で `[x]` 化。`test-msrv` は POLICIES §5 により NotRun 記録。review-protocol で CONFIRMED blocker 0 件（P0/P1 なし）ACCEPT、指摘は P3 のみ（synvelis#86）。

## ALIB-04 Git Remote Provider と LFS remote fetch（synvelis#88、synvelis#331）

ALIB-04 git（synvelis#88）では `cargo test -p synvelis-library --test git` 25/25 pass（offline `file://` fixture、実 git CLI 使用）。`mise run fmt-check`/`check`/`lint`/`test`/`test-rt` 全 pass（workspace 全 target、locked dependency graph）。依存追加なし（Cargo.toml/Cargo.lock 不変）。実 Git server（HTTP/SSH/git protocol 対 live host）・実 OS credential store・GUI・実 network 障害は NotRun/Unsupported（synvelis#88）。

ALIB-04 LFS remote（synvelis#331）では `cargo test -p synvelis-library --test lfs` 11/11 pass、`cargo test -p synvelis-library --test git` 28/28 pass（file:// LFS regression 含む）。`mise run` 全 lane（fmt-check/check/lint/test/test-rt/check-app-native/lint-app-native/test-msrv/build）実走 pass、`test-msrv` は rust 1.89.0 toolchain で全 suite 実走、`test`・`test-msrv` は workspace 全 target。実 remote（GitHub LFS 等）・SSH・`git://`・`.lfsconfig`/`lfs.url` override・custom transfer・実 OS credential store・GUI は NotRun/Unsupported（外部 gate、Pass 対象外）。https は当時 `UnsupportedCapability`（後に synvelis#521 で TLS 実装・`remote_lfs_https_remote_uses_tls_transport` に更新）（synvelis#331）。

LFS remote の request trace 検証として、`POST /repo.git/info/lfs/objects/batch`→`GET /objects/<oid>?sig=test-token`（署名 query verbatim）を確認し、返却 bytes が fixture content と一致・`receipt.verified==true`・`bytes_read==size` を assert した。2 listener で credential が remote origin のみに付き cross-origin 署名 href に付かないことを検証した（synvelis#331）。

## ALIB-05 Browser/Import UX（synvelis#89）

`library_ui` unit 14 件・`library_browser.rs` 16/16 pass。`mise run fmt`/`check synvelis-workspace`/`check synvelis-app`/`lint`（warning 0）/`check-headless-deps`（12 headless crates、"no GUI framework deps"）/`check-app-native`/`prek run --all-files` 全 pass。`check-app-native` の future-incompat warning は既存依存 `proc-macro-error2` 由来で本 change set と無関係と記録した。IR status は in-progress のまま（実 GUI mount synvelis#114・実 provider synvelis#91・v5 real-data synvelis#86 は外部 gate）（synvelis#89）。

## ALIB-06 WebDAV Provider（synvelis#90）

`cargo test -p synvelis-library --test webdav` 34/34 pass（in-process local fixture）。`mise run` fmt-check/check/lint/test/test-rt/build/test-msrv/check-app-native/lint-app-native 全 pass（workspace 全 target、locked dependency graph、MSRV rust 1.89.0）。実 network・実 WebDAV server・`https`・実 OS credential store・GUI・実 server 障害は NotRun/Unsupported（synvelis#90）。

## ALIB-07 Fault Matrix（synvelis#91）

slice1（synvelis#91、PR #236）では `cargo test --test webdav`（library）41 passed / 0 failed（新規 9 case 含む）+ `requests_declare_identity_encoding` 個別 1 passed、`mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`（rust 1.89）全 exit 0。`mise run lint` 初回は fixture の inner attribute 重複で `duplicated_attributes` を検出し修正後 green。`mise run test` は workspace 全件（library git 28 / import 17 / webdav 42 / rt_cache 1 を含む）。`doc_maintenance.js apply` で managed index 5 件再生成、findings 519→502（残存は alib-06 と同一 category のみで新規 category 増加なし）（synvelis#91）。

slice2（synvelis#91、PR #256）では `cargo test -p synvelis-library --test import` 21 passed / 0 failed（新規 4 case 含む）、`cargo clippy -p synvelis-library --all-targets -- -D warnings` exit 0、canonical lane（fmt-check/check/lint/test/test-rt/test-msrv）全 exit 0。`test-msrv` 初回は無関係の `webdav::status_401_maps_auth_expired` が flake 失敗し、単独・file 単位・lane 再実行で green を確認した（synvelis#91）。

## 製品 Asset Library（synvelis#9）

docs-only slice として 7 文書鎖 + case ledger/verification の evidence を作成。検証計画に `doc_status.js lint`（spec/design/plan/task/test-spec 各 type）・`doc_maintenance.js index apply`・`audit_impl_record.js`/`audit_experiment_log.js`・`sync_impl_index`（ir/exp）・`audit_docs.js --type all`・`mise run fmt-check` を列挙した。実行結果の正本は verification 記録であり、本記録グループには含まれない（synvelis#9）。

## 横断的な観測

ALIB-04 git の非回帰確認として、既存 `tests/*.rs` 全 pass（ALIB-01/02/03）を verification matrix の非回帰項目とし、ALIB-04 merge 後も ALIB-01/02/03 の test が green であることを確認する運用を行った（synvelis#88）。

環境 flake の切り分け例として、`mise run test-msrv` 初回で無関係の `webdav::status_401_maps_auth_expired` が flake 失敗 → 単独・file 単位・lane 再実行で green を確認し正直に記録した。過去に webdav flake・LNK1102 の観測例もあり、Fail が出た場合は単独再実行で切り分ける方針である（synvelis#91、#86）。

Windows 環境の注意として、canonical path は `\\?\` prefix を持ち、git-for-windows は `:` 入り filename を U+F03A に変換、symlink 関連テストは開発者モード有効で実測し、Unix/macOS は多くの slice で未実測（NotRun）と明記される（synvelis#85、#88）。

## 文書間の齟齬・注意点

status drift が複数ある。ALIB-03・ALIB-04・ALIB-06 の plan/task 文書と asset-library task 文書は in-progress のままだが、対応する IR（alib-04-git、alib-06、asset-library）は completed で実測結果を持つ。ALIB-05 の IR も status in-progress だが Validation 節は全 lane Pass を記録している（synvelis#87、#88、#89、#90、#9）。

ALIB-03 については IR が存在せず、plan/task も in-progress のまま — 実装完了の実績は確認できない。cache の設計要素は計画段階の記述に留まる（synvelis#87）。

ALIB-02 の import IR は merge 後 backfill であり、検証結果は PR #220 本文の転記（再測定ではない）と自ら明記している（synvelis#86）。v5 snapshot compat slice と verified-import-wiring slice の IR は `audit_impl_record` で missing-section error が HEAD 時点の既存 baseline と記録されている（residual-closeout の doc 監査節）（synvelis#86）。

alib-01 の `test-msrv` 記録は文書内に新旧 2 表が併記: task5 時点は fail（`cargo-platform@0.3.3` が rustc 1.91 要求、main 由来）、task6（merge 後）は pass。新しい方が最新実測（synvelis#85）。

test 数の drift は実測追従で修正された例があり、ALIB-02 の test-spec は `import.rs` を 15 tests と記述していたが PR #236/#256 の追加で実測 21、fault-injection followup で 25 に更新。webdav tests は ALIB-06 時点 34 → fault matrix で 42（`cargo test --test webdav` 41 passed + identity encoding 個別 1 passed）に増加した（synvelis#86、#90、#91）。

doc graph の pre-existing findings として、repo 全体に legacy invalid-id 系・broken relation・reciprocal warning の baseline があり、複数 slice で「新規 finding を増やさない」ことを判定基準とし既存不具合の修復はスコープ外とする扱いが繰り返される（synvelis#86、#91）。編集境界外の残存簿記は後続の bookkeeping PR または Coordinator の merge 時判断に委ねる運用である（synvelis#86）。

## 出処

この記録の由来となった Issue は以下のとおりである。ALIB-01 Asset Provider 基準契約（synvelis#85）、ALIB-02 Streaming Verified Import・v5 互換（synvelis#86）、ALIB-03 Catalog/Preview Cache（synvelis#87、計画のみ）、ALIB-04 Git Remote Provider（synvelis#88）と Git LFS remote fetch（synvelis#331）、ALIB-05 Browser/Import UX（synvelis#89）、ALIB-06 WebDAV Provider（synvelis#90）、ALIB-07 Fault Matrix・import cancel（synvelis#91）、および製品レベル Asset Library 設計契約（synvelis#9）である。containment 修復の検証は ruviform#398（PR401）・ruviform#409（PR412）に対応し、https LFS transport は synvelis#521 に関連する。
