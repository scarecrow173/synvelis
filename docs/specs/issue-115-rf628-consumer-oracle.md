---
id: "SPEC-4s5cb4YaV5NS7O0krrfQcg"
type: "spec"
status: "approved"
title: "Issue #115 rf#628 consumer oracle — synvelis#115 slice"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
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
  implemented-by:
    - "docs/designs/issue-115-rf628-consumer-oracle.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 consumer oracle — synvelis#115 slice

## 背景

`synvelis#115`（RCD-01 実 Audio Device 録音統合）の仮想優先置換 DoD は
「`scarecrow173/ruviform#628` 固定 PCM/Clock fixture を Synvelis 本番
Recorder で記録し独立 Byte/Sample Oracle・複数 Track・
Clock/Queue/Recovery/VerifiedPrefix を検証する」ことを要求する。
先行 slice（`rcd-01-virtual-l1-oracle`）は test-local の独立 oracle
device で synthetic-L1 を確立したが、`rf#628-fixture` 列は pinned
ruviform に fixture が未収録のため NotRun のまま残っていた。

ruviform `abb9f372d3ee95ee824b3c4f7dc68e8083ce2774` で
`crates/ruviform-audio-test`（`pipeline/` 含む）が main に収録され、
`ScriptedBackend`/`PcmScenario`/`StreamFault`/`VirtualClock`/独立
`oracle` module が利用可能になった。本 slice はこれを Synvelis 側
consumer oracle として本番 `InputSession`/`InputAdapter` →
`StreamWriter` → staging media/journal → `commit_session` →
save/reload 経路へ通し、case matrix の `rf#628-fixture` 列を実測で
埋める。

## スコープ

- ruviform pin を `abb9f372` へ更新し、`ruviform-audio-test` を
  workspace dependency + `synvelis-recording` の dev-dependency として
  追加する（lockfile-primary 契約維持・manifest に `rev` を書かない）。
- `tests/rf628_oracle.rs` — SV-1〜SV-5:
  - SV-1 stereo PCM E2E（3 segment scenario・commit→reload・digest 連鎖）
  - SV-2 multichannel + 2 mono tap 分離
  - SV-3 same-clock 2 device 別 seed + clock domain mismatch 拒否
  - SV-4 PCM scenario gap → Xrun → fail-stop verified prefix
  - SV-5 scripted fault（FramePosition/Error/OpenFaults/vanish）
- `tests/rf628_oracle_rt_alloc.rs` — SV-6: scripted callback 経路の
  no-alloc/no-dealloc RT 契約（`stats_alloc` 単一計測テスト規約）。
- `evidence/sv115/` の case matrix `rf#628-fixture` 列・ledger 行・
  本 slice の検証記録を実測結果で更新する。

## 非スコープ（正直な境界）

- 実 OS audio device・WASAPI callback・GUI-L2・physical hardware・
  `scarecrow173/ruviform#629` 仮想 endpoint EXT-L3 — 本 slice 対象外、
  各 lane は現行 verdict のまま NotRun/NotRequired。
- SV-9 device-injected MIDI・SV-10 transport MIDI mapping・
  project persistence/timebase — 別 slice / Deferred。
- `test-recording-real-input` opt-in HW lane — 変更しない。
- Issue #115 は open 継続（PR は `Refs #115`、`Closes` 禁止）。

## 受入条件（本 slice）

- `rf#628-fixture` lane の SV-1〜SV-6 が deterministic-backend
  headless lane として実測 Pass し、証拠が case matrix/ledger/検証記録に
  記録される。
- scripted-backend の Pass を実 OS device/GUI/physical の Pass に
  転用しない。
- production `src/` 変更なし（test-only seam も不要だった）。
