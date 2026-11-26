---
id: "SPEC-w8Qp2Lm5Xc7Nv4Rb1Ty6Ks"
type: "spec"
status: "approved"
title: "SV#28 Ruviform pin 更新仕様（20dc41f7 → 87a7ba8f）"
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
      - type: "file"
        path: "crates/synvelis-app/tests/pc_c4_04_app_composition.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_b.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements: []
  implemented-by:
    - "docs/designs/sv28-ruviform-pin-87a7ba8f.md"
  depends-on:
    - "docs/discovery/sv28-ruviform-pin-87a7ba8f.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv28-ruviform-pin-87a7ba8f.md"
  derived-by:
    - "docs/designs/sv28-ruviform-pin-87a7ba8f.md"
    - "docs/test-specs/sv28-ruviform-pin-87a7ba8f.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv28-ruviform-pin-87a7ba8f.md"
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "evidence/sv28/ruviform-87a7ba8f-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新仕様

## Purpose

synvelis の ruviform pin を `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`
から `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` へ前進させ、upstream
差分で必要となる最小限の consumer 適応を行う。lockfile を唯一の revision
正本として維持し、新規 upstream feature の製品採用は行わない。

## Feature

- Feature: Ruviform 継続追従 pin update slice。
- 対象: `Cargo.lock`、pin 起因 compile 適応を必要とする 36 件の test
  fixture、bus enumeration 意味変化を検証する real ABI test oracle、
  `ui/vendor/ruviform` 生成物。
- 非対象: `Cargo.toml` / `mise.toml`、production source、CI/workflow、
  docs index、cross-consumer tuple、実機/provisioned endpoint 実走。

## Rules

| # | Rule |
| --- | --- |
| R1 | candidate は dispatch 時点に再解決した ruviform `origin/main` HEAD `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` とする。後続の upstream 前進は本 slice の移動目標にしない |
| R2 | `Cargo.toml` に恒久 `rev` を書かず、`Cargo.lock` を scoped `cargo update` でのみ更新する |
| R3 | 36 件の ruviform lockfile source は同一 SHA `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` を指す |
| R4 | lockfile 差分は ruviform source fragment の必要最小限とし、無関係依存を更新しない |
| R5 | `Plugin::buses` 適応は fixture の意味に合わせる。declared static table を返す fixture は `Some(table)` とし、機械的に `None` へ落とさない |
| R6 | `ParamNotice`、Input widget、provisioning primitive、virtual endpoint 実走、GUI/Editor workflow は本 slice で採用しない |
| R7 | vendor は `mise run sync-ui-deps` で lockfile revision へ同期し、生成物を commit しない |
| R8 | 必須 canonical lane の結果を exit code とともに記録し、NotRun/Deferred を Pass と混同しない |

## Examples

- 正常系: `check-ruviform-rev` が `resolved 87a7ba8f… (36 lockfile sources, lockfile-primary, REV match)` を返し、compile/lint/test lane が終了コード 0。
- 異常系: lockfile に無関係な依存更新が混入した場合は採用せず、scoped update 手順をやり直す。

## Guarantees

- pin は lockfile-primary で単一 SHA。
- bus table を declare する fixture の意味を変えない。
- pin 追従を超える新機能採用を本 slice に混入させない。
- rollback 手順と未実行理由を evidence に残す。

## Non-goals

- ruviform #912 `ParamNotice` の DAW automation/live UI 採用。
- ruviform #999 Input widget の製品 UI 採用。
- synvelis #115 Slice B の provisioning consumer workflow。
- cross-consumer tuple（Ruviform #219/#294、AudioLens #127）、3OS main CI、Windows GUI-L2/EXT-L3、実機/実 GUI/市販 plugin。
- Issue #28 の close。

## Risk

- wire v8→v9 は additive command/reply を含むが、synvelis は `WIRE_VERSION` を直接参照しない。L1 runner/host が同一 lock から build されることを `test-tim03-l1` で確認する。
- upstream が dispatch 後に前進するリスクは candidate を固定して記録し、次回 rev-watch slice に委譲する。
