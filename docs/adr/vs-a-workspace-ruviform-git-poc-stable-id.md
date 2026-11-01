---
id: "ADR-034QdxBCx9w88ptCJEEz7D"
type: "adr"
status: "accepted"
title: "VS-A Workspace構成・Ruviform git依存・永続化PoC・Stable ID生成"
created: "2026-11-01"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/13"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/28"
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
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/ruviform/issues/189"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# VS-A Workspace構成・Ruviform git依存・永続化PoC・Stable ID生成

## Context and Problem Statement

Synvelis Issue #13（F0）は新規 Cargo Workspace 上に最小 Project/Recording
基盤を建てることを求める。Workspace のクレート分割、Ruviform への依存方法、
toolchain/MSRV、永続化 PoC の物理形式、Stable ID の生成・表現、Undo 実装方針を、
#17 設計正本（Single Writer・Immutable Snapshot・Directory Bundle 方向）と
#28 の継続追従契約に矛盾しない形で最初に固定する必要がある。
永続化の最終公開形式は確定しない（PoC 選択として記録する義務がある）。

## Decision Drivers

* F0 Gate: Ruviform 既定ブランチ継続追従 + Cargo.lock resolved revision による
  再現性（特定 SHA の恒久固定・vendor コピーは禁止）。
* #17: 論理 Schema と物理保存手段の分離、Directory Bundle 方向を阻害しない
  Store Port、Immutable Snapshot + Root Publish、Live/Saved Revision 分離。
* #19/#10: 既定 PPQN=960、Musical/Media/Source-local 時間の分離、
  `Tick`/`MediaTime`/`TempoMap` は Ruviform `ruviform-media-core` を再利用。
* VS-B〜VS-D・E2E-L1 が同一の Project Store/Stable ID を消費するため、
  ドメイン境界をクレート分割で早期に固定する。
* headless 決定的検証（乱数・時刻・実デバイスに依存しない ID/テスト）。

## Considered Options

クレート分割・依存・永続化の主軸:

* Option A（採用）: `crates/synvelis-project` + `crates/synvelis-recording` +
  headless `crates/recording-poc` の 3 クレート。`ruviform-media-core` を
  `git = "https://github.com/scarecrow173/ruviform.git"`（既定ブランチ追従）で
  参照し、Cargo.lock に resolved revision を記録。永続化 PoC は
  `<name>.synvelis/` Directory Bundle（`HEAD` + `snapshots/<id>.json`、
  Canonical JSON）。Stable ID は `u128` newtype + 注入式 `IdGenerator`
  （既定は Project 内永続化カウンタ）。
* Option B: 単一モノリスクレートに全型を同居。
* Option C: ローカル path 依存で Ruviform sibling checkout を参照。
* Option D: Ruviform を特定 SHA で恒久 pin、または vendor コピー。
* Option E: 永続化に bincode/CBOR 等バイナリ形式を初期採用。

Stable ID 生成（独立軸・Option A の内部選択として比較）:

* ID-1（採用）: Project 内永続化の単調カウンタ（`id_watermark`）から払い出す
  `u128` 値。`IdGenerator` trait を注入し、テストは同一初期値で完全決定的。
  シリアライズは `<kind>-<32桁hex>` のタグ付き文字列とし、JSON での
  u128 数値表現（2^53 超の精度問題）を回避すると同時に、Reload 時に
  kind タグ不一致を検出できる。
* ID-2: UUIDv7 等の時刻起源 ID。グローバル一意だが、決定的テストのために
  結局 generator 注入が必要になり、値に時刻情報が混入する。P0 の
  単一 Project スコープでは過剰。
* ID-3: Content-addressed（内容ハッシュ）ID。Asset/Blob には将来適合するが、
  作成時に内容が未確定の Track/Take/Session には適用不能。

Undo 実装（独立軸・Option A の内部選択として比較）:

* U-1（採用）: Immutable Snapshot 全体を Undo スタックに積む方式。
  Undo/Redo は Live Project の内容を旧 Snapshot の内容へ置き換えるが、
  `ProjectRevision` は**必ず新規発行**し、Saved HEAD・SavedRevision には
  触れない（#17 §2 の単調増加不変条件）。Undo 到達可能な Snapshot は
  GC 保護 Root として保持し、P0 では自動破棄しない。
* U-2: Command 逆変換（inverse operation）方式。省メモリだが、全 Command に
  逆写像の定義と検証が必要で、VS-A の「基本 Undo/Redo が決定的」という
  受入に対して実装・検証コストが高い。後続最適化候補。
* U-3: Journal/ログ再生方式。開始点からの Command 再適用は P0 では
  遅く、Recovery/Journal 設計（VS-C）と結合しすぎる。

## Decision Outcome

Chosen option: "Option A + ID-1 + U-1", because ドメイン境界
（Project 意味論 / Recording Registry / headless 検証）をクレート単位で
分離でき、Ruviform 継続追従と lockfile 再現性の運用契約（#28）を最初の
Workspace から満たし、#17 の Directory Bundle 方向と論理/物理分離を
阻害しない最小構成だから。ID-1 は外部依存なしに決定的・Reload 安全な ID
を与え、U-1 は #17 の「Undo/Redo も新 Revision へ進める」を型ではなく
単純な不変条件として確実に実装できる。

### Consequences

* Good: Stable ID・時間型・Store Port の所有境界がクレート依存方向で強制される。
  `recording-poc` は実デバイス不要の決定的 E2E 入口になる。
* Good: `cargo test --workspace --locked` が resolved revision で再現可能。
  Ruviform 更新は lockfile 差分として追跡できる（#28 運用）。
* Good: Canonical JSON は diff 可読・デバッグ容易で、Map 順序を BTreeMap で
  安定化すれば同一 Snapshot が同一 Bytes を再現する。
* Bad: git 依存は初回解決時にネットワーク取得を要し、オフライン再現は
  Cargo.lock + cargo cache 前提になる（`--locked` は resolved rev を固定するが
  取得自体は必要）。
* Bad: JSON PoC 形式は最終形式ではなく、将来の物理形式移行（Directory
  Bundle 完成形・圧縮・Archive）では Serializer Port 差替えが必要。
* Bad: カウンタ式 ID は単一 Project 内でのみ一意性を保証する。Project 間の
  素材移動/結合で衝突し得るため、移行・Import 経路では再採番規則が別途必要
  （Deferred）。
* Bad: U-1 の Snapshot Undo は Live Project 全体の複製を履歴数分保持し、
  Project 規模が大きくなるとメモリコストが増える。P0 では正しさ優先で
  受け入れ、履歴上限・構造共有は後続 Issue の最適化対象。
* Bad: Undo で `id_watermark` を含む Snapshot 内容が戻るため、Undo 後に
  再採番される ID が Undo 対象 entity と同じ値になり得る。同一 Live
  Revision 上で同値 ID が 2 つの entity に同時存在することはない
  （Undo された entity は Redo 経由でのみ到達可能、Redo 履歴は新規
  Command で破棄）ため P0 では許容し、恒久的な ID 再利用が問題になる
  差分/VCS 機能で再評価する。

### Confirmation

* `mise run check` / `mise run lint` / `mise run test`（内部で
  `cargo test --workspace --locked` を含む検証列）が緑。
* Cargo.lock に `ruviform-media-core` の git resolved revision（`source =
  "git+...#<sha>"`）が記録される。
* Save → Reload で Stable ID・Session/Pass/Stream/Segment/Take 参照・
  SchemaVersion が同一性を保つ決定的テスト。

## Implementation Plan

* Affected paths:
  * `Cargo.toml`（workspace 正本）、`crates/synvelis-project/`、
    `crates/synvelis-recording/`、`crates/recording-poc/`、`mise.toml`、
    `Cargo.lock`（生成物・追跡対象）、`AGENTS.md`（Rust 規約節の追記）。
* Patterns to follow:
  * Ruviform workspace の `resolver = "2"`・`edition = "2024"`・
    `rust-version = "1.89"`・`[workspace.lints]`・`publish = false` 構成。
  * ID newtype + Registry + Command + `ExpectedRevision` 検証で部分適用を
    排除（全件検証→単一 Mutation）。
  * `MediaTime`/`Tick`/`TempoMap`（PPQN=960）は `ruviform-media-core` 再利用。
    `SessionFrame`/`SourceFrame`/`ProjectTime`/`ClockEpochId` は Synvelis 固有の
    別型として定義し、相互代入不可にする。
  * 時間演算は Synvelis 側の checked API のみを経由する。
    **ruviform `Tick`/`TickDuration` は `Add`/`Sub` のみで checked API が
    存在しない**（debug で overflow panic / release で wrap）。
    Synvelis 側では raw `i64` の `checked_*` 演算で包んで `Err` を返し、
    Ruviform `Add`/`Sub` を直接呼ばない。Ruviform 側の checked API 不足は
    Consumer からの改善要望として Ruviform Issue 起票を追跡する
    （本リポジトリ内の恒久的重複実装では隠さない）。
  * Musical 位置は `tick + ppqn` 文脈つきで永続化し、Project PPQN 設定
    （既定 960）を Snapshot に保持する。#19 の有理数 Beat Position 正本化は
    Ruviform #189 系の後続移行として、現 PoC では 960 固定であることを
    Schema Version と文書に明示する。
  * Save は #17 §6 の順序を守る最小実装。保証下限を明確化:
    temp への完全書出し → flush/sync → `snapshots/` 配下へ immutable 名で
    publish → HEAD 更新（atomic rename 可能な同一 FS 内で実施）→
    失敗時は旧 HEAD・旧 Snapshot を保持。途中失敗で旧 Saved 状態を
    破壊しない。Load は HEAD→snapshot 参照→parse→schema/reference 検証の
    順で、壊れた Snapshot（parse 失敗・未対応 SchemaVersion・参照不整合）を
    明示 Reject し Live を壊さない。Store Port を trait 化し InMemory 実装で
    決定的テスト。
  * Verified Source 参照 Port: `Imported`/`Verified`/`Committed`/`Saved`
    の各状態を区別し、参照は SourceId/AssetId + ContentRef（Digest）+
    Finalization 状態の値のみ保持し、Media bytes は Port 越えしない。
* Patterns to avoid:
  * Ruviform の vendor コピー・特定 SHA 恒久 pin・DAW 固有型の Ruviform への移設。
  * `f64` 時刻演算、`Add`/`Sub` 直接演算（Synvelis checked API のみ）、
    負位置の黙った丸め、HEAD を先に書いてから Snapshot を publish する順序。
  * Media Blob bytes を Project Snapshot へ埋め込むこと（Verified Source 参照のみ）。
  * Undo/Redo で `ProjectRevision` を巻き戻す、Saved HEAD を変更する、
    Undo 到達可能 Snapshot を自動破棄する（P0 は GC 不実施・保持のみ）。
  * Session/Pass/Stream/Segment/Take を `ProjectController` 検証を通さず
    別経路で登録する（Single Writer・二重 Store 禁止）。
* Dependencies or configuration:
  * `serde` + `serde_json`（workspace 依存・lockfile で固定）。
  * mise `[tools] rust = "1.99.0"`（Ruviform toolchain pin に照合）、
    `mise.toml` に `fmt`/`fmt-check`/`check`/`lint`/`test`/`build`/
    `test-msrv` タスク追加。

## Verification

* [ ] `mise run fmt-check` / `check` / `lint` / `test` / `build` が全緑、
  `mise run test`（内部で `cargo test --workspace --locked`）が lockfile の
  まま再現できる。
* [ ] Stable ID の型混同は `compile_fail` doctest でコンパイル時エラーを
  実証、Save/Reload で ID 同一性維持（`ClockEpochId` 含む全種）。
* [ ] `SessionFrame`/`SourceFrame`/`ProjectTime` の境界・負構築・
  オーバーフローで checked arithmetic が `Err` を返す（panic しない）。
* [ ] `ExpectedRevision` 不一致・不存在 Target の Command が全件拒否され
  Project 状態に部分適用が残らない。
* [ ] Undo/Redo が決定的に内容を復元し、新規 Live Revision を発行して
  Saved HEAD を汚さない。Undo 到達 Snapshot は保持され GC されない。
* [ ] Recording Session 状態遷移が合法遷移のみ許可し、Session/Pass/
  Stream/Segment/Take の全参照リンク（存在しない参照・別 Session 所属参照）
  が拒否される。
* [ ] Verified Source 参照 Port が Media bytes を含まないことをスキーマで
  確認し、参照状態が Imported ≠ Verified ≠ Committed ≠ Saved と区別される。
* [ ] `recording-poc` の決定的シナリオを 2 回実行して同一出力になる。
* [ ] `mise run test-msrv` で rust 1.89.0 でもコンパイル・テストできる
  （利用可能な場合; toolchain 取得が不能な環境では未検証として記録）。

## Pros and Cons of the Options

### Option A

* Good: 境界が明確、追従契約と lockfile 再現性を両立、#17 方向を阻害しない。
* Bad: git 依存の初回取得コスト、PoC JSON 形式は将来差替え前提。

### Option B

* Good: 最初のファイル数が最少。
* Bad: Project 意味論と Recording Registry と検証入口が混在し、
  VS-B〜D・E2E-L での責務分離が曖昧になる。却下。

### Option C

* Good: ローカルで即座に Ruviform 変更を試せる。
* Bad: Cargo.lock に resolved revision が記録されず再現性を欠き、
  継続追従契約（#28）と CI での再現を破壊する。却下。

### Option D

* Good: 依存が固定されブレがない。
* Bad: 契約違反（恒久 pin・vendor 禁止）。却下。

### Option E

* Good: コンパクト・高速。
* Bad: PoC 段階で diff 不可読・スキーマ進化の観察が困難、#17 の
  「論理 Schema と物理保存手段の分離」を早期に検証しにくい。却下。

## More Information

* [Synvelis #13](https://github.com/scarecrow173/synvelis/issues/13) —
  F0 Gate・完了条件・lockfile 更新条件（2026-11-02 追記）。
* [Synvelis #23](https://github.com/scarecrow173/synvelis/issues/23) —
  実行計画・横断 Gate・Ruviform 追従運用方針。
* [Synvelis #28](https://github.com/scarecrow173/synvelis/issues/28) —
  git 依存 + lockfile 運用の正本。
* `docs/designs/project-asset-persistence-architecture.md` —
  Single Writer/Revision 分離/Save プロトコル/PAP-01〜18。
* `docs/designs/configurable-ppqn-musical-time-architecture.md` —
  既定 PPQN=960・時間ドメイン分離（Ruviform #189 の実装完了は前提にしない）。
