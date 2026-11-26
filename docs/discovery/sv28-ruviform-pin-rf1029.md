---
id: "DISC-rf1029m7Vx2Qp9Lw4Tz8Kn6Ys3H"
type: "discovery"
status: "resolved"
title: "SV#28 Ruviform pin 更新 discovery（87a7ba8f → 0479fa62 / rf#1029）"
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
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv28-ruviform-pin-rf1029.md"
    - "docs/designs/sv28-ruviform-pin-rf1029.md"
  verifies: []
  verified-by: []
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "evidence/sv28/rf1029/ruviform-0479fa62-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 discovery（rf#1029）

## 概要

dispatch 時点に再測定した ruviform `origin/main` HEAD
`0479fa62e77e669a7bf84320d3d5418ffb949fc6` を candidate とし、synvelis
`Cargo.lock` の ruviform revision `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`
からの 17 commits を評価した。candidate は rf#1029 stereo capture contract の
merge commit そのものであり、`git merge-base --is-ancestor` では同値として
ancestor 条件を満たす。

range の consumer-visible な中心変更は 2 件ある。1 件目は rf#1029 の
virtual capture endpoint stereo preserve contract で、synvelis#115 の
`in=1 out=2` Fail を upstream 側で解消する候補である。2 件目は rf#1026 の
WASAPI input forward gap 分類で、同一 capture epoch 内の forward gap が
`DeviceError::Xrun` に続いて `DeviceError::ClockDiscontinuity` として報告される。

## 実測値

| 項目 | 値 |
| --- | --- |
| synvelis base / `origin/main` | `34e69fb27774f41f6aded1a24320be90ffd7477a` |
| before ruviform rev | `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` |
| candidate ruviform rev | `0479fa62e77e669a7bf84320d3d5418ffb949fc6` |
| candidate short8 | `0479fa62` |
| range | `87a7ba8f..0479fa62` |
| commit count | 17 |
| lockfile ruviform sources | 36（before/after とも） |
| rf#1029 ancestor 確認 | Pass（candidate と同値） |
| upstream 全体 diff | 31 files, +4,320 / −544 |
| upstream `Cargo.toml` 差分 | なし |

## 依存木交差

upstream の `crates/` 変更は `ruviform-audio-device`、
`ruviform-io-structured`、`ruviform-ui` の 3 crate に限定される。synvelis
lockfile に存在するのは `ruviform-audio-device` と `ruviform-ui` で、
`ruviform-io-structured` は依存木外である。driver、workflow、docs、evidence、
tool script の差分は Cargo dependency の直接入力ではないが、virtual endpoint
capability として評価した。

## 主要変更と採否

| upstream 変更 | consumer 影響 | 本 slice の判断 |
| --- | --- | --- |
| rf#1029 / PR #1032 stereo capture preserve contract。capture format range/default/jack metadata を stereo 対応へ修正し、mono capture silent pass 防止の oracle を upstream に追加 | synvelis#115 の `in=1 out=2` Fail の upstream 原因を修正する。synvelis の stereo 要求 oracle は変更しない | **採用対象の中核**。pin に取り込む。canonical EXT-L3 再 run は後続 slice |
| rf#1026 WASAPI input forward gap 分類。forward gap は `Xrun` + `ClockDiscontinuity`、backward jump/reset と flag-only は `ClockDiscontinuity` のみ | synvelis `input_error_callback` は `Xrun` を input discontinuity、`ClockDiscontinuity` を epoch discontinuity へ既に写像する。Fault latch は冪等で、2 連続 report は両 bit を立てる | **適応なしで採用**。入力欠損と位置 epoch 再同期を別 bit で保持する現行分類は製品契約と矛盾しない |
| rf#1009 xrun injection/harness。virtual audio driver copy/injection、probe workflow、PowerShell controller、upstream harness を追加 | synvelis 依存木の直接 API ではない。synvelis workflow/tool は追加しない | **未採用・候補**。fault matrix 採用可否は後続 slice |
| rf#1015 structured proof redesign。`ruviform-io-structured` の CSV limit decision core、`ruviform-ui` の `valid_op` byte predicate 再構成、Kani proof | `ruviform-io-structured` は依存木外。`valid_op` は公開 signature と grammar を維持する内部再構成で、synvelis app/UI 非回帰は canonical lane で確認する | **pin の一部として採用**。proof 自体は synvelis に転用しない |
| driver `rvf-virtual-audio`、`.github`、upstream tools/docs/evidence | Cargo compile 入力外。virtual endpoint capability と upstream evidence として評価 | **capability 差分として記録**。upstream provisioned Pass を consumer Pass へ転用しない |

## WASAPI input contract の consumer 評価

synvelis の `input_error_callback` は `DeviceError::Xrun` を
`FAULT_INPUT_DISCONTINUITY`、`DeviceError::ClockDiscontinuity` を
`FAULT_EPOCH_DISCONTINUITY` へ写像する。rf#1026 の forward gap は両方を順に
報告するため、1 回の gap で両 bit が立つ。これは重複副作用ではなく、capture
loss（xrun）と position epoch の再同期（clock discontinuity）という別の事実を
表す。両 bit は fail-stop 条件であり、後続 callback は従来どおり無効化される。

したがって `crates/synvelis-recording/src/input.rs` と
`crates/synvelis-recording/tests/input_adapter.rs` の変更は不要と判断した。
stereo multi-track requirement、capture channel-width oracle、fault 分類の意味は
弱めない。

## lockfile 期待差分

scoped `cargo update` 後の `Cargo.lock` 差分は 36 source fragment の
`-` / `+` 72 行だけで、無関係な crates.io version drift、新規 package、
依存リスト変更はない。`Cargo.toml` に恒久 `rev` は書かない。

## 採用条件

lease 内適応の要不要判断が完了し、必須 canonical lane が pass すれば採用する。
synvelis#115 canonical EXT-L3 再 run、provisioned endpoint、manual workflow、
cross-consumer tuple、3OS main CI adjudication は本 slice では `NotRun` または
後続 slice とする。Issue #28 は close しない。
