---
id: "SPEC-alib07-import-cancel"
type: "spec"
status: "approved"
title: "ALIB-07 slice 2 — LIB-IMPORT-04 publish/commit 相 cancel・LIB-IMPORT-08 subprocess crash（synvelis-library headless）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/designs/alib-07-import-cancel-design.md"
      - type: "file"
        path: "docs/plans/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/tasks/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/impl/ir/alib-07-import-cancel.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "docs/test-specs/alib-07-fault-matrix.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  derives-from:
    - "docs/specs/alib-07-fault-matrix.md"
  depends-on:
    - "docs/specs/alib-07-fault-matrix.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-07-fault-matrix.md"
    - "docs/designs/alib-07-fault-matrix-design.md"
---

# ALIB-07 slice 2 Spec — LIB-IMPORT-04 publish/commit 相 cancel + LIB-IMPORT-08 subprocess crash

Synvelis #91（ALIB-07 Asset Library Fault Matrix）の残 headless
sub-case を閉じる slice。先行 slice（PR #236、39 Pass）で stage 相
cancel までを固定した `synvelis-library` import pipeline について、
(a) publish 完了〜project commit 間の cancel 観測契約を確定し、
(b) publish 相 cancel を決定的に固定し、(c) 実 process crash からの
manifest resume を subprocess fixture で証明し、(d) Case ID 別証拠
matrix（`docs/test-specs/alib-07-fault-matrix.md`）の簿記を現行
baseline へ更新する。

## 目的と境界

- **目的**: Issue #91 の LIB-IMPORT-04「cancellation at each phase」
  のうち publish/commit 相を headless で固定し、LIB-IMPORT-08
  「Blob Published/ACK lost/manifest crash」の実 process crash 経路を
  Mock 結果と区別して記録する。
- **境界**: `crates/synvelis-library` の `src/import.rs` commit 相
  注入点・`tests/import.rs`・`tests/common/`・docs チェーンのみ。
  `synvelis-project` 他 crate、provider 実装本体、GUI、実
  network/device/OS credential store は対象外。
- **非昇格**: subprocess/Mock の成功を実 provider・実 durability・
  実 OS crash 保証と読み替えない。`NotRun`/`Unsupported`/
  `NotImplemented`/`Unknown` を Pass に昇格させない。

## Cancel 契約の確定（LIB-IMPORT-04 の判断）

`import_verified` の stage machine は
`Started → Staged → Verified → Published → Committed` であり、
協調 cancel は「well-defined points」（context.rs の契約）で観測
される。先行実装では publish_blob 成功後〜`controller.apply` までの
区間に checkpoint がなく、その間に着地した cancel は commit を
妨げなかった。

**決定: commit 直前に `cx.check_interrupts()` checkpoint を置く
（案 a）。** 「commit は通る」を仕様として追認しない。理由:

- Issue #91 は「cancellation at each phase」を要求し、Published→
  Committed は独立した stage 遷移である。publish 完了後に cancel が
  着地した場合、利用者の cancel 意図を黙って無視して commit するのは
  協調 cancel の契約に反する。
- checkpoint は `controller.apply` 直前の一点のみ。`apply` は
  in-memory の atomic 操作であり、開始後の cancel は観測不能かつ
  commit 済み状態を `Cancelled` と誤報するため、apply 成功後に
  checkpoint を置かない。apply〜manifest 書込み間の crash は既存の
  `applied_replay` が次回呼出で `Committed` へ reconcile する設計を
  そのまま使う。
- cancel が pre-commit checkpoint で観測された場合、manifest は
  `Published` に留まり、publish 済み blob は manifest 保護の下で
  orphan になり、同一 ImportId の retry が blob 再利用で commit へ
  収束する（既存の resume 意味論と整合）。

publish 相 cancel は `BlobPublisher` port の typed 終端
（`BlobPublishError::Interrupted` → `ImportError::Provider(
Cancelled)`）として既に分類経路を持ち、本 slice で決定的 test を
追加する。`StagedBlobPublisher` は copy/readback の各 chunk で
`check_interrupts` を呼ぶため、実 publisher では copy 中の cancel は
publish 相で既に観測される。残る隙間は publish tail
（seal/publish/dirsync）と pre-commit 区間であり、pre-commit
checkpoint がその両方を覆う。

## 受入条件（本 slice）

- [x] LIB-IMPORT-04: publish 相 cancel が typed `Cancelled` +
  manifest `Verified` 保持 + 同一 ImportId resume で固定される。
- [x] LIB-IMPORT-04: commit 相（pre-commit checkpoint）cancel が
  typed `Cancelled` + manifest `Published` 保持 + publish 済み blob
  温存 + resume が再 publish せず commit 収束することで固定される。
- [x] LIB-IMPORT-08: 実 subprocess crash（publish ACK 喪失相当・
  pre-commit crash 相当の 2 crash point）から、別 process が
  manifest + on-disk blob のみを根拠に resume し、ID 予約・
  冪等・blob 再利用が process 境界を越えて収束する。
- [x] `docs/test-specs/alib-07-fault-matrix.md` の baseline・
  PR 参照・Case ID 行が現行 SHA と本 slice の実測で更新される。
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv` が exit 0。

## 非対象（Issue #91 の残件として記録）

- LIB-MIG-02 — #86（PR #253）で main 済み。#91 は吸収しない。
- LIB-IMPORT-07・LIB-QOS-04 実 device/OS/GUI — NotRun（#89/#114）。
- 実 network/Git remote(HTTP/SSH)/実 WebDAV/OS credential store/
  実 SSRF/実 crash durability/Linux・macOS 実機 — NotRun/
  Unsupported（実環境）。
- remote LFS fetch（LIB-PIN-02 server 経路）— provider 実装未整備の
  ため NotRun。
