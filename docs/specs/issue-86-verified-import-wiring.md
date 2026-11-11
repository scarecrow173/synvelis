---
id: "SPEC-rNbdNqOT10RavAY4Hj9RKJ"
type: "spec"
status: "approved"
title: "Issue #86 DoD(2) — import_verified consumer 入口 + digest-kind blob 解決（synvelis-player slice）"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/24"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/blob.rs"
      - type: "file"
        path: "crates/synvelis-player/src/verified_import.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/verified_import.rs"
      - type: "file"
        path: "docs/specs/issue-86-verified-import-wiring.md"
      - type: "file"
        path: "docs/designs/issue-86-verified-import-wiring-design.md"
      - type: "file"
        path: "docs/plans/issue-86-verified-import-wiring-plan.md"
      - type: "file"
        path: "docs/tasks/issue-86-task-verified-import-wiring.md"
      - type: "file"
        path: "docs/test-specs/issue-86-verified-import-wiring.md"
      - type: "file"
        path: "docs/impl/ir/issue-86-verified-import-wiring-slice.md"
      - type: "file"
        path: "evidence/issue-86-verified-import-wiring/verification-ledger.md"
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
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements: []
  implemented-by:
    - "docs/designs/issue-86-verified-import-wiring-design.md"
    - "docs/plans/issue-86-verified-import-wiring-plan.md"
  depends-on:
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-86-verified-import-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #86 DoD(2) — import_verified consumer 入口 + digest-kind blob 解決

## What Should Be Built

Issue #86 の残実装 DoD(2): 「Import Receipt→ProjectController CAS/Undo→
Saved HEAD/Recovery→Offline Playback の実結合」を headless で閉じる。

ALIB-02（PR #220）で `synvelis-library::import_verified`（streaming stage
machine・sha256-only commit gate・rebase/inspect/gc_orphans）は merge
済みだが、consumer は library tests のみ。`synvelis-player` 側には
2 つのギャップがある。

### (a) digest-kind blob 解決

`synvelis-player::resolver::open_verified` と
`synvelis-render::prepare::open_source` は同一ロジックの重複実装で、
ともに flat `blobs/<digest>` + CRC32 fold 検証のみを解決する。
`sha256:<hex>` digest を持つ source は `blobs/sha256:<hex>` を探して
`MissingMedia` になる（Windows では `:` がファイル名として不正）ため、
`import_verified` で commit した source は現在**再生不能**。

`AssetDigest::parse(digest).kind()` で分岐する blob 解決 helper を
`synvelis-player` に**単一権威**として実装し、両 call site をそれへ
移す（render は既に player 依存のため helper を再利用する）。

- `DigestKind::Sha256` → `<media_root>/blobs/sha256/<64hex-lowercase>`
  + SHA-256 再検証（`sha2` 直接利用。library の `stream_digest` は
  `pub(crate)` のため）。
- `DigestKind::Crc32Fold` → flat `<media_root>/blobs/<digest>` +
  従来の forward+reverse CRC32 fold 再検証（非回帰）。
- `DigestKind::Opaque` → fail-closed。verified content address でない
  digest は再生 gate を通しても blob 解決できない型付き失敗とする。

digest 計測と demuxer open は同一 `File` を通す（verify-then-open gap を
閉じる — render 側が既に採る方式に player resolver も揃う）。

### (b) player 側 consumer 入口

任意 path の単一ファイルを `import_verified` へ通す player 入口:

```text
canonicalize(file) → LocalFolderProvider(parent-dir 境界) pin
  → ImportIntent { import_id: draw_raw_id(), expected_project_revision:
    live_revision(), source: pin, target }
  → import_verified（ImportStore/LocalBlobDir/StagedBlobPublisher）
  → published blob を digest-kind helper 経由で decode 検証
    （demuxer → PCM codec gate → SourceReader → EOF まで decode）
  → ProjectOp::SetSourceState で Imported → Verified へ前進
```

`import_verified` は byte-integrity のみを検証し `SourceState::Imported`
で commit するため、media decode 検証 + state 前進は consumer の責務
（library の doc 契約どおり）。再生 gate は `>= Verified` のため
`Verified` で止める。`Committed` まで進めると旧経路
（`import_local` = 1 command = 1 undo step）に対して追加の command /
undo step が増えて粒度が変わるため、本 slice は **`Verified` 止め**とする
（undo 粒度差の詳細は下記「Undo 粒度の変更」を参照）。

## Acceptance Criteria

- `sha256:` digest の source が blob 解決され再生できる
  （digest-kind 分岐。`Crc32Fold` 旧経路は非回帰・`Opaque` は
  fail-closed）。
- `import_verified` → `ProjectController`（`expected_project_revision`
  CAS）→ `SetSourceState`→`Verified` → `resolver::prepare` +
  `HeadlessDriver` で fixture sample 一致再生する連鎖を headless test
  で検証する。
- Undo → 再 import の manifest 予約 ID 収束、`DirectoryBundleStore`
  save → Saved HEAD → 新 controller `load` → 再生、中断 job の
  `inspect_imports` → resume → commit → 再生を検証する
  （Issue DoD(2) の CAS/Undo・Saved HEAD/Recovery→Playback）。
- 既存 lane 全 Pass・`import_local` 経路（20 test call sites）非回帰。
- `Verified` 止めの undo 粒度差・`import_local` 温存・app 層配線対象外
  を spec/PR に明記する。
- RT / 実 device / 実 GUI は対象外（Pass 扱いしない）。

## Undo 粒度の変更（明示）

旧経路 `import_local` は import 全体を 1 `Operation`（`CommitImport`,
source は `Committed` で登録）として commit するため undo 1 回で
import が消える。新経路は `CommitVerifiedImport`（`Imported` 登録）
+ `SetSourceState`(→`Verified`) の **2 command = 2 undo step** になる。
state 前進を 1 step 追加するのは byte-integrity commit と media
decode 検証の責務分離の当然の帰結であり、再生 gate（`>= Verified`）を
満たす最小の前進として `Verified` 止めを採用する。`Committed` 一致を
旧経路と揃えるとさらに 1 command 増えるため本 slice では進めない。

## Non-goals

- `import_local` 本体とその 20 test call sites の変更（v5-compat spec
  の non-goal とおり legacy consumer として温存）。
- `synvelis-app` / `synvelis-workspace` / `synvelis-gateway` 層の配線
  （UI 経路・app Service op は別 Issue 範囲）。
- `synvelis-library` / `synvelis-project` の変更（digest 分類層・stage
  machine・manifest/rebase/inspect/gc は merge 済み）。
- RT engine・実 audio device・実 GUI の検証（対象外、Pass 扱いしない）。
- DoD(1) v5 移行（merge 済み・本 slice と独立）。

## Dependency note

`synvelis-player` に `synvelis-library`（`provider`/`import`/`refs`）と
`sha2`（sha256 blob verify）を依存追加する。依存方向は
`player → library → project` で非循環（library は player/recording に
依存しない）。`synvelis-render` は player 経由で推移的に利用する。
