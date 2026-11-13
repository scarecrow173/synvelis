---
id: "SPEC-034T7fQpLmNvRwXk2jHs9"
type: "spec"
status: "approved"
title: "SYN-IO-02 HEAD CAS expected-head store port"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/330"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "SPEC-034WVJji6Ejtn5EnEV5RkC"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/ruviform/issues/288"
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS expected-head store port

## What Should Be Built

`ProjectStore` port に expected-head 型の compare-and-swap を
**additive** に導入し、複数 writer/process が同一 project root
へ同時 save しても lost-update が起きないことを store 契約とし
て保証する（synvelis #330、SYN-IO-02 後続）。

- **`ProjectStore::save_cas`**: 追加メソッド
  `save_cas(project_id, expected_head, revision, bytes) ->
  Result<String, HeadCasError>`。`expected_head` は `head()` が
  報告する値そのもの（snapshot 名、または HEAD 不在を表す
  `None`）。default 実装は `Err(HeadCasError::Unsupported)` を
  返し、無条件 `save` への silent degrade は禁止。
- **`HeadCasError`**: `Conflict { expected, actual }`（HEAD が
  期待と不一致 — typed・retryable-after-reload）/
  `Unsupported`（store が CAS 非対応）/ `Store(Error)`（lock・
  publish・I/O 失敗、既存の publish 分類を保持）の 3 variant。
- **`DirectoryBundleStore`**: consumer 側 CAS — `HEAD.lock`
  ファイルの atomic `CreateNew` による mutex 取得、stale-lock
  検出・rename-aside による break、lock 内での HEAD read→
  compare→snapshot publish→fencing→HEAD publish、token 照合
  付き解放。機構選択と根拠は
  `docs/designs/syn-io-02-head-cas-design.md` を正本とする。
- **`InMemoryStore`**: 同一契約の parity 実装（port の CAS
  意味論が filesystem 無しで再現可能であること）。
- **`ProjectController::save`**: `self.saved.head` を expected
  として `save_cas` を呼ぶ。`Unsupported` のみ従来の無条件
  `save` へ fallback（後方互換）。公開 signature は不変。
- **journal replay restore**（`synvelis-gateway` commit 経路）:
  `controller.load` が採用した HEAD を expected として
  `save_cas` で復元 publish する。load 後に並行 writer が HEAD
  を動かしていれば loud failure とし、journaled state への
  silent rewind を許さない。

## Why Now

現行 `DirectoryBundleStore::save` は snapshot を `CreateNew` で
公開した後、HEAD を無条件 `PublishMode::Replace` で差し替える。
rename 自体は原子的だが compare-and-swap ではないため、2 つの
writer/process が同じ HEAD を読んでから各々 save すると、後勝ち
の repoint が先勝ちの commit を検出なしに上書きする
（lost-update）。`Command::expected_revision` は live revision
の楽観ロックであり、store の HEAD pointer に対する検査ではない
— 別 process・別 controller・journal restore はこの防御の外に
いる。SYN-IO-01 の境界 spec で HEAD CAS は Synvelis 責務と確定
済みであり、ruviform の File API（`PublishMode` =
`CreateNew`/`Replace` のみ、CAS プリミティブなし）の primitive
上に consumer 側で実装する。

## Users and Value

- `ProjectController::save` の全呼び出し側（app/PoC/gateway
  commit）: 自身が最後に採用した HEAD を expected として供給
  するため、間に割り込んだ writer の commit を `Err` で検出し、
  `saved` を進めずに再 `load` で復帰できる。
- gateway journal replay: crash 復旧時に「journal に残る最新
  commit を HEAD へ復元する」操作が、復元中に割り込んだ writer
  の HEAD を巻き戻さない。
- 将来の multi-process/複数 writer 構成: port 契約として
  expected-head CAS が存在し、store 実装ごとの再発明を防ぐ。

## Scope

### In Scope

- `crates/synvelis-project/src/store.rs`: `HeadCasError`、
  `ProjectStore::save_cas`（default `Unsupported`）、
  `DirectoryBundleStore` の CAS 実装（`HEAD.lock` mutex・
  stale-lock break・fencing）、`InMemoryStore` parity、
  `publish_snapshot`/`publish_head` への `save` 分解。
- `crates/synvelis-project/src/controller.rs`:
  `ProjectController::save` の expected-head 供給と
  `Unsupported` fallback。
- `crates/synvelis-gateway/src/commit.rs`: journal replay
  restore の `save_cas` 化。
- `crates/synvelis-project/tests/head_cas.rs`（新規）と
  `tests/store_publish.rs` の CAS 順序に伴う整合。
- 本 doc chain（`syn-io-02-head-cas`）と syn-io-02 既存
  spec/design の残件記述の整合。

### Out of Scope

- ruviform 側の File API/StagedFile/Publish への CAS 機構追加
  （境界 spec で禁止・Synvelis 責務）。
- `ProjectStore` への required method 追加（範囲外の Fake 実装
  を壊すため additive default を採用）。
- `save`/`head`/`read_snapshot` の既存契約・HEAD file content
  （snapshot 名 verbatim）・snapshot 名 `r<N>.json`・
  `SnapshotEnvelope` format の変更。
- `synvelis-recording` staging writer・他 consumer 固有コード、
  `tests/fixtures/`、`Cargo.lock`、docs index。
- 実 2-process 協調・macOS/Linux 実機・電源断の検証
  （本 spec の時点では外部 gate・NotRun。実 2-process の
  Windows 実測は後続 slice `syn-io-02-head-cas-e2e` で
  消化済み。macOS/Linux・電源断は引き続き外部 gate）。

## Requirements

- `save_cas` は additive であり、default は typed
  `HeadCasError::Unsupported` を返す。CAS 非対応 store を
  無条件 `save` へ silent degrade させてはならない。
- expected-head の比較は store の相互排除境界の内側で行い、
  同一 root 上の協調する 2 つの `save_cas` caller が同じ
  expected HEAD を観測して両方 commit することを許さない。
  loser は `HeadCasError::Conflict { expected, actual }` を
  受け取り、HEAD・snapshot のいずれも変更されない。
- 競合・失敗のいかなる経路でも caller に成功を誤報しない。
  publish 失敗は既存の effect/phase/durability 分類を保持し、
  lock 喪失後の不確定結果は `Indeterminate` 相当の store error
  として正直に報告する。
- `ProjectController::save` は `saved.head`（未 save/load なら
  `None`）を expected として供給し、`Unsupported` のときのみ
  従来の無条件 `save` を呼ぶ。CAS conflict/store 失敗時は
  `saved` を進めない。
- journal replay restore は `save_cas` を用い、CAS 非対応・
  競合・失敗はすべて loud failure とする（force-save への
  silent fallback は行わない — 復元が HEAD を巻き戻す経路を
  残さない）。
- HEAD file content・snapshot file format・`save` の無条件
  semantics は不変。CAS は opt-in の追加契約である。
- CAS の保証境界（rename atomicity・directory entry 耐久性・
  snapshot/HEAD の multi-file transaction 限界・crash window・
  stale lock・協調 writer 限定性・長時間 hold の residual
  race・電源断/全 platform 保証の非対象）を design/spec に
  明記する。

## Acceptance Criteria

- [ ] `ProjectStore::save_cas` が additive（default
  `Unsupported`）で追加され、既存 `save` signature・全
  call-site・port 実装（範囲外 Fake 含む）が変更なしで
  コンパイル・動作する。
- [ ] `DirectoryBundleStore::save_cas` が expected 一致時のみ
  snapshot+HEAD を commit し、不一致時に typed
  `HeadCasError::Conflict` を返して HEAD・snapshot を不変に
  保つ。`expected=None` は HEAD 不在の assert として機能する。
- [ ] 同一 root に 2 つの `DirectoryBundleStore` で同一 expected
  HEAD の同時 `save_cas` が single-winner となり、loser が
  typed conflict を受け、最終 HEAD が勝者の snapshot を指す
  ことを契約テストで確認する。
- [ ] `InMemoryStore::save_cas` が同一契約（conflict/None
  assert/parity）を filesystem 無しで再現する。
- [ ] `ProjectController::save` が `saved.head` を expected に
  使い、HEAD が進んだ store への stale-baseline save が
  conflict で失敗して `saved` が進まないこと、`Unsupported`
  store では従来どおり save が成功することをテストで確認する。
- [ ] journal replay restore が `save_cas` 経由で、load 後の
  HEAD 移動を loud failure として扱う。
- [ ] `HEAD.lock` の stale break・fresh lock の待機・失敗時の
  lock 解放が契約テストで観測できる。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` を
  実走し、結果を正直に記録する。実 2-process（Windows）は
  後続 slice `syn-io-02-head-cas-e2e` で実測済み。
  macOS/Linux・電源断は NotRun として外部 gate に委譲する。

## Deferred Design Concerns

- 実 2-process/複数 OS での協調検証、電源断・durability の
  実機 gate — 実 2-process（Windows）は後続 slice
  `syn-io-02-head-cas-e2e` で実測済み。macOS/Linux・
  電源断は外部検証事項として NotRun 記録し、必要なら
  別 Issue で追跡する。
- 非協調 writer（`save` 直接呼出・外部プロセスの HEAD 書換え）
  に対する排他 — CAS は協調する `save_cas` caller 間の契約で
  あり、lock を取らない書き込みは検出も防止もしない
  （次回 CAS の conflict で観測はされる）。
- `HEAD.lock` stale 判定を超える長時間 hold の residual race
  — design の保証境界に記録（post-publish fencing で不確定
  結果は誤報しないが、物理的な lost-update 余地は advisory
  lock の限界として残る）。
- kernel-level file lock（`flock`/`LockFileEx`）や ruviform への
  CAS primitive 追加 — 境界 spec で Synvelis 責務外/ruviform
  非変更のため対象外。必要になれば ruviform Issue として
  別途起票する。

## Sources and Relations

- Issue #330（本 spec の Acceptance 正本）。
- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`
  （staged publish・immutable snapshot・失敗分類の前提契約）。
  同 spec の「複数 writer 間の真の HEAD CAS は Out of Scope」
  記述は本 spec の実装により更新済み。
- `docs/specs/syn-io-01-ruviform-common-io-boundary.md`
  （HEAD CAS は Synvelis 責務・ruviform File API に CAS を
  持ち込まない境界確定）。
- `docs/designs/project-asset-persistence-architecture.md`
  （orphan snapshot・ACK-lost・recovery の設計根拠）。
- Ruviform #288（`PublishMode` が `CreateNew`/`Replace` のみで
  CAS を持たないことの確認元）。
