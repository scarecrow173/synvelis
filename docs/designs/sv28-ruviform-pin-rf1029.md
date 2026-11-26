---
id: "DESIGN-rf1029k8Qw3Vx7Lm2Np6Ty4Rs9Zd"
type: "design"
status: "approved"
title: "SV#28 Ruviform pin 更新設計（87a7ba8f → 0479fa62 / rf#1029）"
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
      - type: "file"
        path: "crates/synvelis-app/frontend/dist"
  implements:
    - "docs/specs/sv28-ruviform-pin-rf1029.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv28-ruviform-pin-rf1029.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-rf1029.md"
    - "docs/specs/sv28-ruviform-pin-rf1029.md"
  derived-by:
    - "docs/test-specs/sv28-ruviform-pin-rf1029.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-rf1029.md"
  references:
    - "Cargo.toml"
    - "Cargo.lock"
    - "crates/synvelis-recording/src/input.rs"
    - "tools/check-ruviform-rev.mjs"
    - "tools/sync-ruviform-ui.mjs"
    - "evidence/sv28/rf1029/ruviform-0479fa62-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新設計（rf#1029）

## 概要

`Cargo.lock` の ruviform 36 source を `0479fa62e77e669a7bf84320d3d5418ffb949fc6`
へ scoped に更新し、rf#1029 stereo capture preserve contract を consumer が
利用できる状態にする。WASAPI input error contract の精緻化は upstream 側の
report 分類であり、synvelis の fault 写像は既に両 variant を扱うため、
production source/test の適応は行わない。

## Context

- ruviform git 依存は `main` 追従で、解決 revision は `Cargo.lock` が保持する。
- rf#1029 は synvelis#115 の初回 provisioned consumer run で観測された
  `in=1 out=2` Fail に対応する upstream 修正である。
- rf#1026 は forward gap を `Xrun` と `ClockDiscontinuity` の 2 report に分類する。
  これは WASAPI input の runtime contract 差分であり、compile API diff ではない。
- synvelis の `input_error_callback` は `Xrun` を input discontinuity、
  `ClockDiscontinuity` を epoch discontinuity へ既に写像する。`FaultLatch` は
  bit ごとの冪等 latch で、2 report を 1 回の副作用として潰さない。

## Scope

### In Scope

- scoped `cargo update` による `Cargo.lock` 更新。
- `mise run sync-ui-deps` による vendor 再同期。
- WASAPI input contract の consumer 影響評価と、必要な場合の lease 内最小適応。
- 必須 canonical lane 実行と evidence/doc chain 記録。

### Out of Scope

- `Cargo.toml`、`mise.toml`、tools、CI、docs index。
- `crates/synvelis-recording/tests/virtual_endpoint.rs` の変更。
- rf#1009 fault injection matrix、provisioned endpoint 実走、manual workflow。
- cross-consumer tuple、3OS main CI adjudication。

## Components and Boundaries

- **lockfile boundary**: `Cargo.lock` の git source fragment だけが変わる。
  `cargo update` の対象を ruviform package に限定し、無関係依存 drift を入れない。
- **recording input boundary**: upstream error report を consumer fault bit へ
  写像する責務は `input_error_callback` に集中する。forward gap の 2 report は
  input loss と epoch re-anchor の両事実を保持し、下流で片方を無かったことにしない。
- **virtual endpoint oracle boundary**: stereo requirement と channel-width
  oracle は synvelis consumer が所有する。upstream evidence は再評価を始める
  根拠にはなるが、consumer Pass の代替にはしない。
- **generated vendor boundary**: `ui/vendor/ruviform` と frontend dist は生成物で、
  gitignore 済みのまま commit しない。

## Data and Control Flow

```text
dispatch-time ruviform origin/main = 0479fa62…
  -> scoped cargo update
  -> Cargo.lock 36 source fragments
  -> sync-ui-deps -> ui/vendor/ruviform/REV == candidate
  -> compile/lint/test lanes
  -> diff review + verification evidence
  -> adopt or rollback
```

## 設計判断

| 判断 | 内容 | 根拠 |
| --- | --- | --- |
| candidate 固定 | `0479fa62…` を dispatch 時点 candidate とする | rev-watch を終了可能にし、rf#1029 merge commit と一致させる |
| scoped update | `cargo update -p ruviform-media-core --precise 0479fa62…` で同一 git source を原子更新する | lock diff を ruviform source fragment だけに限定できる |
| recording 適応なし | `input.rs` / `input_adapter.rs` を変更しない | 現行写像は両 error variant を正しい fault bit へ独立に写像し、forward gap の 2 report を隠さない |
| stereo oracle 非変更 | `virtual_endpoint.rs` を変更しない | upstream contract の導入は consumer oracle を弱める理由にならない |
| rf#1009 不採用 | xrun injection workflow/tool を synvelis に追加しない | pin 追従と fault matrix 採用は別責務である |
| proof 転用なし | rf#1015 の Kani proof を synvelis に移植しない | consumer 非回帰の確認対象であり、proof 採用 slice ではない |

## Non-Functional Constraints

- canonical verification は mise task。cargo/git/node 直接実行は補助証拠と明記する。
- 生成物出力先は変更しない。
- rollback は未コミットなら scoped `cargo update -p ruviform-media-core --precise 87a7ba8f…` を優先する。
- 失敗・未実施を Pass と書かない。

## Risks and Trade-offs

| リスク | 対応 |
| --- | --- |
| forward gap で両 fault bit が立つことを重複報告と誤解する | evidence に upstream report 順序と consumer bit 意味を記録する |
| upstream stereo preserve Pass を consumer Pass と誤認する | synvelis#115 EXT-L3 再 run を `NotRun` / 後続 slice と明記する |
| upstream moving target | candidate freeze と再測定値を evidence/PR に明記する |
| 実機・GUI gate 未実施の誤解 | NotRun/Deferred を lane 表と PR に分離記録する |
