---
id: "PLAN-uaNPrumAjiAnEaagqm9BgJ"
type: "plan"
status: "completed"
title: "Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 実装計画"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/blob.rs"
      - type: "file"
        path: "crates/synvelis-player/src/verified_import.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/verified_import.rs"
      - type: "file"
        path: "docs/plans/issue-86-verified-import-wiring-plan.md"
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
        path: "docs/plans/README.md"
  implements:
    - "docs/specs/issue-86-verified-import-wiring.md"
  implemented-by:
    - "docs/tasks/issue-86-task-verified-import-wiring.md"
  depends-on:
    - "docs/designs/issue-86-verified-import-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
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

# Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 実装計画

## Scope split

単一 slice。`synvelis-player` の blob 解決権威 + consumer 入口 +
`synvelis-render` の call site 更新 + headless chain test。
`import_local`・library・project は不変。

## File map

| File | 変更 |
| --- | --- |
| `crates/synvelis-player/src/blob.rs` | `open_blob_verified` + `VerifiedBlob` — digest-kind 分岐の単一権威（新規） |
| `crates/synvelis-player/src/resolver.rs` | `open_verified` の blob 解決を `open_blob_verified` へ置き換え |
| `crates/synvelis-player/src/verified_import.rs` | `import_verified_local` + `VerifiedImportRequest`/`Outcome`（新規） |
| `crates/synvelis-player/src/error.rs` | `ResolveError::UnsupportedDigest` + `VerifiedImportError` 追加 |
| `crates/synvelis-player/src/lib.rs` | `blob`/`verified_import` module 宣言 + re-export |
| `crates/synvelis-player/Cargo.toml` | `synvelis-library` + `sha2` 依存追加 |
| `crates/synvelis-render/src/prepare.rs` | `open_source` の blob 解決を `open_blob_verified` へ置き換え |
| `crates/synvelis-player/tests/verified_import.rs` | DoD(2) chain test（新規） |
| `Cargo.lock` | 依存追加に伴う再生成 |

## Dependency graph

`player::blob`（project の `AssetDigest` + import の flat/blob 規則 +
sha2）→ `resolver` / `render::prepare` が利用。
`player::verified_import`（library `import_verified` + `player::blob` +
`ProjectOp::SetSourceState`）→ tests / 将来の app 配線が利用。
依存方向 `player → library → project`・`render → player` で非循環。

## Risk register

- `Opaque` digest の fail-closed 化で既存 source が解決不能になる
  可能性 → `>= Verified` に達する source の digest は全て
  `import_local`(CRC)/`import_verified`(sha256) 由来であり、Opaque は
  そもそも解決不能だった。workspace test で非回帰確認。
- `resolver` の全件読み → 同一 fd 化でメモリ使用量減 + verify-then-open
  gap 解消。decode 失敗分類（`Corrupt`）は不変。
- decode 検証が重い場合の import 時間 → fixture サイズの headless 用途
  で問題なし。巨大 file は library の byte budget が先に拒否。
- `provider_id` 定数化で manifest の provider identity が全 import で
  同一になる → manifest pin 照合は `object_identity`+`asset` で行われ
  provider 実体は ephemeral のため問題なし。

## Verification matrix

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv`（canonical）
- 新規 test `crates/synvelis-player/tests/verified_import.rs`:
  - digest-kind 3 分岐（sha256 解決・crc 非回帰・opaque fail-closed）
  - pin → import_verified → receipt → SetSourceState → prepare +
    HeadlessDriver sample 一致
  - undo → StaleRevision → rebase → manifest 予約 ID 収束
  - DirectoryBundleStore save → Saved HEAD → load → 再生
  - 中断 job `inspect_imports` → resume → commit → 再生
- `import_local` 20 call sites 非回帰は `l1_e2e` 等の既存 test で確認
