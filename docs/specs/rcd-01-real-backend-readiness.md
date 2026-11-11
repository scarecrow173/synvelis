---
id: "SPEC-gut3XVRAIUtMHADNZBNFHC"
type: "spec"
status: "proposed"
title: "RCD-01 実Audio Device 到達準備 — stamp placement fallback + real backend 配線（synvelis#115）"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
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
    - "SPEC-034V2ghdXlh1m9UFtt5ks1"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "TASK-LRVMzVtzazw5gBnDcKYhLk"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 実Audio Device 到達準備 — stamp placement fallback + real backend 配線（synvelis#115）

## What Should Be Built

`crates/synvelis-recording` の入力 adapter を、実 audio device
（`ruviform-audio-device`、cpal backend、ruviform#251 で pin
`b02942c2` に含有済み）へ到達可能な状態にする。2 slice 構成:

- **Slice A — stamp placement fallback**: `InputAdapter` の data
  callback は現状 `info.frame_position: None` を即 `FAULT_INPUT_DISCONTINUITY`
  で fail-stop するが、実 cpal stream は常に `frame_position: None` を返す。
  port 契約の代替経路 `InputCallbackInfo::capture_stamp()` → `InputStamp`
  → `ClockAnchor` + `map_stamp_to_frame`（`ruviform-media-core`、RT-safe
  純粋整数演算）を配線し、`frame_position` 非報告時の配置を stamp 検証
  付きの累積 frame count 配置へ fallback させる。実機実測で capture
  stamp の timestamp jitter（sub-ms・±数 frame 相当）が判明したため、
  block 毎の独立 `map_stamp_to_frame` ではなく、初回 stamp のみを seed
  に使い配置は累積 count で行い、以後の stamp は連続性・ドリフトの
  検証に使う jitter 許容設計とする（詳細は design 文書）。
- **Slice B — real backend 配線 + opt-in lane**: ルート
  `[workspace.dependencies]` に `ruviform-audio-device` を追加し、
  `synvelis-recording` に composition root（`src/real_input.rs`:
  host 列挙 → F32 交渉 → `InputSession` 結線）と opt-in 実機 lane
  （`tests/real_input.rs` + `mise.toml` task）を追加する。

## Why

実 cpal input stream は `frame_position` を報告しない（port 契約上
「未対応は `None`、捏造禁止」）。現行 adapter はこれを fail-stop とするため、
実 device を繋ぐと初回 callback で必ず fault になる。一方で backend は
capture timestamp（ADC capture instant）を `InputTimestamp::capture`
として報告し得るため、`capture_stamp()` → `map_stamp_to_frame` の
sanctioned 経路で frame 位置を導出できる。fallback が無ければ実 device
到達は不可能であり、fallback を入れなければ RCD-01 の実機受入 gate
（実録音・fidelity 実測・multi-device 相関・xrun/長時間）に進めない。

なお実機検証（Windows/WASAPI・Audient iD4、44.1 kHz F32 stereo）で、
cpal backend の capture stamp が sub-ms の timestamp jitter
（連続 block 間隔 10.019 ms / 10.006 ms、独立写像で 443/444/439 vs
期待 441 frame）を持つことが判明した。PCM block 録音は sample-exact
な連続 frame 位置を要求するため、jitter を配置へ反映する設計は
`FAULT_INPUT_DISCONTINUITY` を誘発する。これを受けて配置は累積
frame count で導出し、stamp は初期 seed と連続性/ドリフト検証に
限定する設計へ改訂した（coordinator 判断済み）。また cpal WASAPI
adapter が `IAudioClock::GetPosition` の device frame position を
取得しながら破棄している上流欠落を確認し、ruviform#565 を起票した
（供給されれば `frame_position` 経路が本線となる）。

## Acceptance Criteria（本 slice 分・Issue #115 転記）

- [ ] 実 device 接続に必要な契約が headless test で検証される:
  stamp placement fallback の 5 種 `MapRejection` → `FAULT_*` 写像、
  `frame_position` 併存時の経路選択、両方欠如時の fail-stop 維持、
  timestamp jitter 下での累積配置による sample-exact 連続性、
  stamp 逆行・進行乖離・累積ドリフトの fail-stop、same-clock
  domain の既存強制との整合。
- [ ] real backend composition root（host 列挙 → F32 交渉 →
  `InputSession` 結線）が実装され、opt-in 実機 lane のコードと
  SKIP/未使用経路が compile/lint/test で検証される。実機 lane は
  Windows/WASAPI（Audient iD4）で実行し、jitter 下でも fault 無しで
  録音経路が成立することを確認済み。
- [ ] 実機 gate 行（実録音受入・Raw capture fidelity 実測・
  multi-device 実機相関・xrun/removal/長時間・証拠リンク）は本 slice
  では実行不可のまま NotRun/Unsupported として Issue 本文との対応を
  PR に明記。

## Constraints

- RT callback（data/error callback）で allocation/dealloc・blocking・
  I/O・panic を行わない。stamp→frame 変換は `map_stamp_to_frame` の
  純粋整数演算のみ。
- `InputTimestamp::callback` を capture stamp として代用しない
  （buffering latency 分だけ配置がずれる）。callback 回数から
  `frame_position` を合成しない。foreign timestamp base を再ラベル
  しない — これらは port 契約が明示的に禁じる。
- `InputAdapter`/`InputSession` の既存公開 signature を維持する
  （`synvelis-app` 等の consumer が利用中）。
- 既定の `mise run test`/`check`/`lint` は hardware 非依存のまま
  （opt-in lane は `real-device` feature + `#[ignore]` + 専用 task）。
- `ruviform-media-core` は src で使うため dev-dependency から
  通常 dependency へ昇格する（dep 追加理由を PR に記録）。
- 公開 enum は `#[non_exhaustive]` 方針を維持。MSRV 1.89、
  edition 2024、workspace lint 規約に従う。
