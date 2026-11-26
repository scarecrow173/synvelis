---
id: "DISC-mQ7vN2xR9tL4pK8sJ6hG3d"
type: "discovery"
status: "resolved"
title: "SV#28 Ruviform pin 更新 discovery（20dc41f7 → 87a7ba8f）"
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
    - "docs/specs/sv28-ruviform-pin-87a7ba8f.md"
    - "docs/designs/sv28-ruviform-pin-87a7ba8f.md"
  verifies: []
  verified-by: []
  references:
    - "Cargo.lock"
    - "tools/check-ruviform-rev.mjs"
    - "evidence/sv28/ruviform-87a7ba8f-diff-review.md"
  defers: []
  deferred-by: []
---

# SV#28 Ruviform pin 更新 discovery

## 概要

dispatch 時点で再測定した ruviform `origin/main` HEAD
`87a7ba8ff3fbe6d4502bea27d152d7f74c105899` を candidate とし、
synvelis `Cargo.lock` の ruviform pin `20dc41f7` からの 71 commits を
評価した。対象 range は fast-forward で、synvelis 依存木に入る破壊的変更は
`Plugin::buses` / `ErasedPlugin::buses` の戻り値変更
（`&[BusDescriptor]` → `Option<&[BusDescriptor]>`）である。

candidate 固定後に upstream `origin/main` はさらに前進した
（2026-11-26 再読取では `0fb97d1b…`）。本 slice は dispatch 時点で
再解決した candidate `87a7ba8f` を固定し、移動目標を追わない。

## 実測値

| 項目 | 値 |
| --- | --- |
| synvelis base / `origin/main` | `01c99ea2b2f1b2509c9208b8d31c66fb6b3c7008` |
| before ruviform rev | `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` |
| candidate ruviform rev | `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` |
| candidate short8 | `87a7ba8f` |
| range | `20dc41f7..87a7ba8f` |
| commit count | 71 |
| lockfile ruviform sources | 36（before/after とも） |
| upstream `crates/` diff | 35 files, +7,685 / −543 |

## 依存木交差

upstream 変更 crate は 13。synvelis lockfile 依存木に存在するのは
`ruviform-audio-device`、`ruviform-audio-plugin-bridge`、
`ruviform-audio-test`、`ruviform-plugin-clap`、`ruviform-plugin-core`、
`ruviform-plugin-export`、`ruviform-plugin-host`、
`ruviform-plugin-test-clap`、`ruviform-plugin-test-vst3`、
`ruviform-plugin-vst3`、`ruviform-ui` の 11。
`ruviform-io-archive` と `ruviform-midi-device` は lockfile に無く、
直接 compile 影響はない。

## 主要変更と採否

| upstream 変更 | consumer 影響 | 本 slice の判断 |
| --- | --- | --- |
| ruviform #1020: `Plugin::buses` / `ErasedPlugin::buses` が `Option<&[BusDescriptor]>` へ変更。`None` = unavailable、`Some(&[])` = declared none | synvelis test fixture 36 件が旧 signature。production `src/` の直接参照なし。real ABI oracle では in-process adapter が stereo main input/output を report するようになり、旧 `Reported([])` 前提が崩れた | **適応して採用**。declared static table を返す fixture は `Some(...)` へ修正し、real ABI oracle は現行 host contract に合わせて endpoint-unavailable fault を検証する |
| wire v9 / ruviform #912: `ParamNotice`、`drain_param_notices`、rescan/clear/preset/gesture notification | additive。synvelis consumer grep 0 件 | **未採用・候補**。pin 追従に必要な compile 適応を超えるため後続 slice |
| ruviform #999: `ComponentSpec::Input`、`InputSpec`、`InputValueSpec`、`A11yRole::TextBox` / `SpinButton` | additive。`ComponentSpec` は `#[non_exhaustive]`。synvelis 画面採用なし | **未採用・候補**。UI vendor は同一 rev へ同期するが製品 UI 採用は後続 |
| ruviform #630 系: wire tag 29/30 export-side L1 dispatch、VST3/CLAP focus helper、test plugin/host path 更新 | host と L1 runner は同一 lock から build。`test-tim03-l1` で artifact 経路を検証 | **pin の一部として採用**。新規 consumer workflow は追加しない |
| ruviform #1022: virtual endpoint provisioning PowerShell primitive | synvelis #115 Slice B の前提。本 slice では provisioning/manual run なし | **未採用・候補**。primitive の存在のみ記録 |
| `ruviform-midi-device` Windows.Devices.Midi2 probe、`ruviform-io-archive` weak-FS cleanup | synvelis lockfile 依存木外 | **非影響**。capability 差分として記録 |
| `ruviform-audio-device` / `audio-test` / `plugin-bridge` | 変更は upstream tests のみ | compile/test 非回帰を canonical lane で確認 |

## bus enumeration の意味論

synvelis の 36 fixture はすべて static bus table（`BUSES`、
`STEREO_BUSES`、`STATE_BUSES`、`MONO_IN_BUSES`、または保持済み
`self.buses`）を返す実装で、declared table を表す。したがって
`Some(...)` への適応が正しく、`None`（unavailable）へ変えるべき
fixture はない。host 側の公開 `BusReport` は従来どおり
`Reported` / `Unavailable` で、`tim03_l1_process_oracle.rs` と
`tim03_real_abi_process_oracle.rs` は新しい `BusReport::Unavailable`
分岐を既に扱う。

## lockfile 期待差分

scoped `cargo update` 後、`Cargo.lock` 差分は 36 source fragment の
`-` / `+` 72 行のみ。無関係な crates.io package の version drift、
新規 package、依存リスト変更はない。

## 採用条件

lease 内適応が完了し、必須 canonical lane が pass すれば採用。
失敗・未実施は pass と書かない。外部実機、cross-consumer tuple、
3OS CI、Windows GUI-L2/EXT-L3 は本 slice では `NotRun` / 後続 slice。
