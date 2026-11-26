---
id: "SPEC-rf1029p4Lx8Qm2Nv7Ty5Ks1Wd9Zc"
type: "spec"
status: "approved"
title: "SV#28 Ruviform pin 更新仕様（87a7ba8f → 0479fa62 / rf#1029）"
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
  implements: []
  implemented-by:
    - "docs/designs/sv28-ruviform-pin-rf1029.md"
  depends-on:
    - "docs/discovery/sv28-ruviform-pin-rf1029.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-rf1029.md"
  derived-by:
    - "docs/designs/sv28-ruviform-pin-rf1029.md"
    - "docs/test-specs/sv28-ruviform-pin-rf1029.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-rf1029.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "evidence/sv28/rf1029/ruviform-0479fa62-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新仕様（rf#1029）

## Purpose

synvelis の ruviform pin を `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`
から `0479fa62e77e669a7bf84320d3d5418ffb949fc6` へ前進させ、rf#1029 stereo
capture preserve contract を consumer lockfile に取り込む。lockfile を唯一の
revision 正本とし、synvelis の stereo capture oracle や fault 分類を弱めない。

## Feature

- Feature: Ruviform 継続追従 pin update slice。
- 対象: `Cargo.lock`、`ui/vendor/ruviform` 生成物、frontend build 生成物、
  本 slice の docs/evidence。
- 非対象: `Cargo.toml` / `mise.toml`、production source、CI/workflow、docs
  index、provisioned endpoint 実走、cross-consumer tuple。

## Rules

| # | Rule |
| --- | --- |
| R1 | candidate は dispatch 時点に再解決した ruviform `origin/main` HEAD `0479fa62e77e669a7bf84320d3d5418ffb949fc6` とする |
| R2 | candidate は rf#1029 merge commit `0479fa62e77e669a7bf84320d3d5418ffb949fc6` 自体またはその descendant でなければならない |
| R3 | `Cargo.toml` に恒久 `rev` を書かず、`Cargo.lock` を scoped `cargo update` でのみ更新する |
| R4 | 36 件の ruviform lockfile source は同一 SHA `0479fa62e77e669a7bf84320d3d5418ffb949fc6` を指す |
| R5 | lockfile 差分は ruviform source fragment の必要最小限とし、無関係依存を更新しない |
| R6 | WASAPI input forward gap の 2 report を重複副作用として隠さず、`Xrun` と `ClockDiscontinuity` の consumer 分類が製品契約に合うかを評価する |
| R7 | synvelis の stereo multi-track requirement、capture channel-width oracle、fault oracle を弱めない |
| R8 | rf#1009 xrun injection/harness、provisioned endpoint 実走、manual workflow、cross-consumer tuple は本 slice で採用・実行しない |
| R9 | vendor は `mise run sync-ui-deps` で lockfile revision へ同期し、生成物を commit しない |
| R10 | 必須 canonical lane の結果を exit code とともに記録し、NotRun/Deferred を Pass と混同しない |

## Examples

- 正常系: `check-ruviform-rev` が `resolved 0479fa62… (36 lockfile sources, lockfile-primary, REV match)` を返し、各 lane が終了コード 0。
- 異常系: lockfile に無関係な依存更新が混入した場合は採用せず、scoped update 手順をやり直す。

## Guarantees

- pin は lockfile-primary で単一 SHA。
- upstream stereo capture contract を含む revision を lockfile に固定する。
- upstream provisioned Pass を synvelis consumer Pass に転用しない。
- rollback 手順と未実行理由を evidence に残す。

## Non-goals

- synvelis#115 canonical EXT-L3 再評価の実 run。
- `mise run test-recording-real-input` と `mise run test-recording-virtual-endpoint`。
- rf#1009 fault injection matrix の synvelis 採用。
- ruviform #219/#294・AudioLens #127 への tuple 提出、3OS main CI、Windows GUI-L2/EXT-L3、実機/実 GUI/市販 plugin。
- Issue #28 の close。

## Risk

- forward gap で `FAULT_INPUT_DISCONTINUITY` と `FAULT_EPOCH_DISCONTINUITY` が同時に立つため、fault bit の意味を evidence で明確化する。fail-stop 自体は従来契約であり、verified prefix 方針を変えない。
- upstream main が dispatch 後に前進するリスクは candidate freeze で局所化し、次回 rev-watch slice に委譲する。
