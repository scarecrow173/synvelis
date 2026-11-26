---
id: "DESIGN-b6Vd3Nx9Qm2Lp8Rt5Yw7Ks"
type: "design"
status: "approved"
title: "SV#28 Ruviform pin 更新設計（20dc41f7 → 87a7ba8f）"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
  changes:
    added: []
    modified:
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/specs/sv28-ruviform-pin-87a7ba8f.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-87a7ba8f.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-87a7ba8f.md"
    - "docs/specs/sv28-ruviform-pin-87a7ba8f.md"
  derived-by:
    - "docs/test-specs/sv28-ruviform-pin-87a7ba8f.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-87a7ba8f.md"
  references:
    - "Cargo.toml"
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "evidence/sv28/ruviform-87a7ba8f-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新設計

## 概要

`Cargo.lock` の ruviform 36 source を `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`
へ scoped に更新し、`Plugin::buses` の型変更に対して test fixture の意味を
保つ適応だけを行う。production 設計変更、新 API 採用、CI/workflow 追加は
行わない。

## Context

- 既存規約では `Cargo.toml` の ruviform git 依存は `main` 追従で、解決
  revision は `Cargo.lock` が保持する。
- `check-ruviform-rev` は lockfile source 件数・単一 SHA・UI vendor REV の
  一致を検証する。
- 今回の破壊的変更は bus enumeration availability を正直に表す upstream API
  変更であり、synvelis の production source は直接 `.buses()` を呼ばない。
  compile 影響は test fixture の trait impl に限定されるが、real ABI oracle
  では upstream adapter が stereo main input/output を report するようになった
  ため、fail-closed 経路の期待を bus incompatibility から endpoint
  unavailability へ更新する必要がある。

## Scope

### In Scope

- scoped `cargo update` による `Cargo.lock` 更新。
- 36 fixture の `fn buses(&self) -> Option<&[BusDescriptor]>` 適応。
- `crates/synvelis-app/tests/tim03_real_abi_process_oracle.rs` の pin 起因
  oracle 適応（範囲拡張許可済み。assertion/comment/adapter failure oracle のみ）。
- `mise run sync-ui-deps` による vendor 再同期。
- 必須 canonical lane 実行と evidence/doc chain 記録。

### Out of Scope

- `Cargo.toml`、`mise.toml`、`crates/*/src/`、tools、CI、docs index。
- `ParamNotice` / Input widget / provisioning primitive / uitest seam の
  consumer 採用。
- 実機、実 GUI、provisioned endpoint、cross-consumer tuple。

## Components and Boundaries

- **lockfile boundary**: `Cargo.lock` の git source fragment だけが変わる。
  `cargo update` の対象を ruviform package に限定し、無関係依存 drift を
  入れない。
- **fixture boundary**: bus table を返す fixture は declared table なので
  `Some(...)`。empty static table は `Some(&[])` であり `None` と区別する。
  `None` は unavailable の意味で、本 fixture 群には該当しない。
- **host projection boundary**: upstream `ruviform-plugin-host` が
  `Some` / `None` を既存公開 `BusReport::{Reported,Unavailable}` へ投影する。
  synvelis 側はこの enum を消費するだけで、追加の abstraction を作らない。
- **generated vendor boundary**: `ui/vendor/ruviform` は `sync-ui-deps` の
  生成物。gitignore 済みで commit しない。

## Data and Control Flow

```text
dispatch-time ruviform origin/main = 87a7ba8f…
  -> scoped cargo update
  -> Cargo.lock 36 source fragments
  -> fixture buses() adaptation (Some(...))
  -> sync-ui-deps -> ui/vendor/ruviform/REV == candidate
  -> canonical mise lanes -> evidence / PR verification
```

## 設計判断

| 判断 | 内容 | 根拠 |
| --- | --- | --- |
| candidate 固定 | `87a7ba8f…` を dispatch 時点 candidate とする。後続 upstream 前進は追わない | rev-watch を終了可能にし、評価と検証の対象を一意にする |
| scoped update | `cargo update -p ruviform-media-core --precise 87a7ba8f…` を中心に ruviform package のみ更新 | 無関係依存 drift を排除し、lock diff を 72 行に限定 |
| fixture 適応 | 36 件すべて `Some(...)` | すべて declared static table。`None` は意味論を変えるため不適切 |
| production 非変更 | `crates/*/src/` は変更しない | 直接 `.buses()` 参照はなく、compile 破壊は test fixture に限定 |
| ParamNotice 不採用 | wire v9 type/API は pin に含まれるが consumer 実装は追加しない | additive。採用は automation/UI 設計を要し pin slice の責務外 |
| Input widget 不採用 | `ruviform-ui` の additive API を vendor 経由で同期するだけ | 製品 UI 採用は別 slice |
| provisioning primitive 不採用 | ruviform #1022 の script primitive を workflow に組み込まない | synvelis #115 Slice B の前提であり、manual/provisioned run は本 slice の明示的非対象 |

## Non-Functional Constraints

- canonical verification は mise task。cargo 直接実行は補助証拠と明記。
- 生成物出力先は変更しない。
- rollback は未コミットなら scoped `cargo update -p ruviform-media-core --precise 20dc41f7…` を優先する。
- 失敗・未実施を Pass と書かない。

## Risks and Trade-offs

| リスク | 対応 |
| --- | --- |
| `Some(&[])` と `None` の意味混同 | fixture ごとの table 実体を確認し、全件 declared table のため `Some` に限定 |
| wire v9 の version mismatch | host/runner/test plugin を同一 lock から build し、`test-tim03-l1` で確認 |
| upstream moving target | candidate freeze と後続 slice を evidence/PR に明記 |
| GUI/実機 gate 未実施の誤解 | NotRun/Deferred を lane 表と PR に分離記録 |
