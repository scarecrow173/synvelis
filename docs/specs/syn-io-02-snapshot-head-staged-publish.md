---
id: "SPEC-034T29C9D8CIvYjMxmQIy8"
type: "spec"
status: "approved"
title: "SYN-IO-02 Snapshot/HEAD StagedFile Publish 移行"
created: "2026-11-06"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
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
  depends-on:
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
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
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/282"
    - "https://github.com/scarecrow173/ruviform/issues/295"
    - "https://github.com/scarecrow173/ruviform/issues/296"
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/25"
  defers: []
  deferred-by: []
---

# SYN-IO-02 Snapshot/HEAD StagedFile Publish 移行

## What Should Be Built

`crates/synvelis-project/src/store.rs` の
`DirectoryBundleStore` が持つ独自 `.tmp` → `File::sync_all` →
`fs::rename` 直書きパイプライン（`write_atomic`）を、Ruviform
`ruviform-io-file` の型状態付き staged publish
（`StagedFile` → `SealedFile` → `publish` →
`PublishReceipt`/`PublishFailure`）へ移管する。

- **Snapshot**（`snapshots/r<N>.json`）: immutable name の
  `PublishMode::CreateNew` で公開する。同一 revision の再公開は
  `DestinationExists` で拒否されるが、次の 2 経路のみ受理する:
  (a) byte 完全一致の再送は冪等に受理 — snapshot publish と
  HEAD publish の間で crash した孤児 snapshot を journal replay
  等の回復経路が再 save できる。
  (b) envelope が `id_watermark` のみ異なる同一論理 snapshot は
  watermark 単調増方向に refresh（新 byte へ `Replace`）—
  `controller.save` の save 時 watermark stamp が `draw_raw_id`
  の apply なし予約を永続化する機構を維持する。それ以外の内容
  差異を持つ同名再公開は拒否し、既存 byte を上書きしない。
  いずれの受理経路もスキップではない: 既存名の存在＋byte 一致
  は `FileAndDirectorySync` 達成の証明にならない（先の publish
  が DirectorySync 失敗で `Published` だった可能性を区別不能、
  かつ HEAD 親 root の dir sync は snapshots/ 親の entry を
  cover しない）ので、保持する勝者 byte を `Replace` で再公開
  して耐久性を再確立してから HEAD へ進む。再公開を完遂でき
  なければ `Err` で HEAD は不変 — 耐久性未達のまま成功は
  報告しない。
- **HEAD**（`HEAD`）: `PublishMode::Replace` で原子的に差し替える。
  単一 file replace は Project CAS ではない。Synvelis 側の
  ExpectedRevision/単一 Writer/Lock/Recovery 判断は従来どおり
  `ProjectController` と呼び出し側に残し、store port の署名
  （`save`/`head`/`read_snapshot`）は変更しない。なお、
  expected-head CAS は後続 spec `syn-io-02-head-cas`
  （synvelis #330）で `save_cas` として additive に追加済み
  — 既存署名・HEAD/snapshot format は本 spec の契約どおり
  不変。
- **Durability**: snapshot/HEAD とも
  `Durability::FileAndDirectorySync` を要求する（アーキテクチャ
  設計の「Atomic Replace できる backend では置換・Directory Sync
  する」に整合）。`VisibilityRequirement::RequireAtomic`。
- **失敗の分離**: `PublishFailure` の effect
  （`NotPublished`/`Published`/`Indeterminate`）・phase・達成済み
  durability・temp path を `Error::Store` メッセージへ転写する。
  publish が `Err` を返した場合 HEAD 書込みへ進まない — DirSync
  失敗や Indeterminate で HEAD を勝手に進めず、呼び出し側の
  再読込/Recovery 判断へ接続する。`Indeterminate` の残留 temp は
  自動削除しない。

## Why Now

- Ruviform IO-IMP-01/02（#295/#296）で `StagedFile`/OS publish/
  readback 契約が候補 revision で利用可能になった。Synvelis 側の
  `.tmp`+rename 独自実装は、Directory Sync・published-but-unsynced・
  indeterminate の分類を持たず、Crash/再 Open 時の状態を正直に
  報告できない。
- `fs::rename` ベースの旧実装は同一 revision snapshot の上書きを
  拒否しない。immutable snapshot 名に対する CreateNew 保証を
  型契約で得る段階にある。
- RF #305 の旧 Media API 撤去候補・AL #143 移行候補と
  RF #294 scratch candidate-SHA 検証で組み合わせる横断依存の
  Consumer 側作業単位として切り出されている。

## Users and Value

- Synvelis の Project 保存経路（`ProjectController::save`、将来の
  Autosave/Backup/Import 実装）: publish 失敗時に「何が起きたか」
  （未公開/公開済/不明・どの phase で・どこまで durable か）を
  分類済みで受け取り、旧 Saved HEAD を維持したまま再読込/
  Recovery へ進める。
- ユーザー: crash/電源断・外部変更・古い writer 後の再 Open で、
  store が虚偽の成功を報告せず、immutable snapshot が
  上書き破壊されない。

## Scope

### In Scope

- `crates/synvelis-project/src/store.rs` の
  `DirectoryBundleStore::write_atomic` 撤去と staged publish への
  移管（snapshot `CreateNew`・HEAD `Replace`・
  `FileAndDirectorySync`・`RequireAtomic`）。
- 上記に必要な `ruviform-io-file` 依存宣言（workspace
  `[workspace.dependencies]` と `crates/synvelis-project/Cargo.toml`）
  と `Cargo.lock` 再生成。
- `ProjectStore` port・`SnapshotEnvelope`（schema_version/
  serialize 順序/`BTreeMap` canonical byte）・Saved/Live
  revision・`head()`/`read_snapshot()` の既存契約維持。
  `InMemoryStore` も immutable name 契約（byte 同一再送は冪等
  受理・異 byte 同名は拒否）に整合させる。
- save/head の失敗分類（NotPublished/Published/Indeterminate/
  DirSync 失敗）と orphan/残留 temp の store 契約テスト。

### Out of Scope

- `synvelis-recording` の staging writer、`synvelis-gateway`
  mcp_permit、`synvelis-player` import 等の他 `.tmp`/rename
  パイプライン（別 Issue/別 owner の所有範囲）。
- WAV Export（Synvelis #25 が owner）、ZIP/TAR Backup/Import
  format、Asset Schema migration（SV86）、UI（SV112）、
  Provider（SV85）。
- Ruviform 側の変更・ProjectId/Revision/Undo 等の DAW 概念の
  基盤への追加。
- 複数 writer 間の真の HEAD CAS — 本 spec 起票時は Out of
  Scope だったが、後続 `syn-io-02-head-cas`（synvelis #330）
  で `ProjectStore::save_cas` の additive expected-head CAS
  として実装済み（機構・保証境界は同 design を正本とする）。

## Requirements

- Snapshot は immutable name `r<N>.json` を `CreateNew` で公開する。
  同一 revision の再公開は、byte 完全一致または envelope が
  `id_watermark` のみ異なる同一論理 snapshot（単調増 refresh /
  冪等受理）に限り受理して HEAD を再指向し、内容差異がある場合は
  拒否する（`Error::Store`、既存 byte 不変）。
- 衝突受理は「勝者 byte を `Replace` + `FileAndDirectorySync`
  で再公開する」ことで snapshot 自身の directory entry 耐久性を
  （再）確立する。既存名の存在のみを証拠に HEAD 前進・成功を
  報告してはならず、再公開が完遂できなければ `Err` とし HEAD
  は不変とする（受理が `Replace` でも勝者 byte 不変なので
  immutable name 契約は維持される）。
- HEAD は snapshot publish 成功後にのみ `Replace` で差し替える。
  HEAD publish が `NotPublished` で失敗した場合、snapshot は
  orphan として残り旧 HEAD を保持する（旧 snapshot を消さない）。
  `Published`/`Indeterminate` 失敗は HEAD が既に進んだ可能性を
  正直に分類して報告し、呼び出し側の `head()` 再読込が実態を
  決める。
- `StagedFile` の write 失敗・`create` 失敗・`publish` 失敗は
  すべて `Error::Store` へ写像し、失敗時の `PublishFailure`
  分類（effect/phase/durability/operation id/temp path）を
  メッセージに含める。
- `PublishEffect::Indeterminate` の残留 temp は自動削除せず、
  `NotPublished`/`Published` の recovery handle は best-effort
  cleanup する。
- `Error`/`ProjectStore`/公開 API・永続 byte format の互換性は
  変更しない。

## Acceptance Criteria

- [ ] 旧 `write_atomic` の全使用箇所が staged publish に置換
  され、独自 `.tmp`/`fs::rename`/`sync_all` 直書きが
  `synvelis-project` に残らない。
- [ ] 同一 revision の `save` 再実行で内容が異なる場合は失敗し、
  既存 snapshot byte と HEAD が不変であること、byte が一致する
  再送は冪等に受理されること、`id_watermark` のみ増加した同一
  論理 snapshot は refresh 受理されることを契約テストで確認する。
- [ ] HEAD publish が `NotPublished` で失敗した場合に snapshot が
  orphan として残り旧 HEAD が保持されること、snapshot publish
  失敗時に HEAD が進まないことを契約テストで確認する。
- [ ] 残留 `*.rvf-tmp-*`・外部 HEAD 書換え・HEAD 不在・再 Open
  経路が契約テストで観測できる。
- [ ] `mise run check` / `mise run lint` / `mise run test`
  （workspace, `--locked`）が pass し、Snapshot/HEAD/Project
  schema の既存 golden を退行しない。
- [ ] RT 非阻害: store API を RT context から呼ばない契約が維持
  される（ruviform-io-file は non-RT のみ）。

## Deferred Design Concerns

- Recording staging（`synvelis-recording`）・gateway permit・
  player import の staged-file 移行 — 別 Issue の所有範囲であり
  本 spec では扱わない。
- Autosave/Backup/Import への `ruviform-io-file` 展開 — 製品
  format/採用要件が未決（Consumer #82 追記）。本移行は
  DirectoryBundleStore に限定する。

## Sources and Relations

- Issue #84（実装依頼の作業単位・Acceptance の正本）。
- Issue #82（Consumer 設計正本: Synvelis が Schema/Revision/HEAD/
  ProjectStore 整合性・Recovery を所有し、Ruviform が汎用
  File/Structured/Archive を所有）。
- `docs/designs/project-asset-persistence-architecture.md`
  （「Atomic Replace できる backend では置換・Directory Sync する」
  「失敗時も旧 Snapshot を消さない」「ACK lost は再 Open で判定」
  の設計根拠）。
- Ruviform #295/#296（StagedFile/SealedFile/PublishFailure と OS
  backend の公開契約）。
