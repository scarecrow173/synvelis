---
id: "ADR-yEJFG31sy193FI2AeehiqF"
type: "adr"
status: "accepted"
title: "X — Finalized raw media の canonical publication 境界・commit digest 接続・playback 無変更"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/26"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  depends-on:
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
    - "docs/adr/l1-player-slice-boundaries.md"
    - "docs/adr/l2-render-slice-boundaries.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
    - "docs/discovery/x-recorded-verified-source-playback-export-discovery.md"
  derives-from:
    - "docs/discovery/x-recorded-verified-source-playback-export-discovery.md"
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# X ADR — Finalized raw media の canonical publication 境界

## Status

Accepted (2026-11-02)

## Context and Problem Statement

録音 media は `staging/<session>/streams/stream-NNNN.pcm` の container-less
raw `f32le-interleaved` で、`Source.content_digest` には raw PCM の fold
digest が登録される。一方 resolver (`synvelis_player::resolver::prepare`・
`synvelis_render::prepare::open_source`) は `media_root/blobs/<digest>` に
symphonia-decodable な blob があることを要求する。両者が出会う点が無い
ため、commit 済み録音 Source は playback/export 経路を通過できない。

制約: 録音固有 playback path・file-format 特例・第二 Project Store は
禁止 (#26)。`SourceId` 等の stable ID を再採番しない。`import_local`
(importer-copy shortcut) で誤魔化さない。

## Decision Drivers

- 同一 verified-source 境界を録音 Source でも通す (generic 維持)。
- Finalized 以外の media が resolve しないことを構造的に保証。
- raw fold digest (journal/forensics) と canonical blob digest の
  provenance を両立。
- crash/再実行でも冪等・fail-closed。

## Considered Options

- **Option 1 (採用)**: finalize→commit 間に canonical publication 工程を
  追加。`synvelis-player::publish` が durable frontier の raw PCM を F32
  WAV へ encode → `content_digest` → `blobs/<digest>` 原子公開 →
  `canonical.json` receipt 原子公開。`build_payload` が整合 receipt を
  `StreamEntry.digest` に採用 (+`raw_digest` provenance)。playback 側
  無変更。
- **Option 2**: resolver/render に raw-PCM fallback decode を追加。
  media contract を拡張する file-format 特例であり禁止事項に抵触。
- **Option 3**: recording 側で Source を独自登録 (commit 経路を分岐)。
  第二登録経路 = ID/provenance 分断、applied_commits 冪等性と不整合。
- **Option 4**: 録音 media を最初から WAV container で書く (writer 変更)。
  RT writer の write path に container serialize/seek を混入させ、
  journal/checkpoint 契約 (append-only chunk) を壊す。却下。

## Decision Outcome

Option 1 を採用。canonical publication は media-store 側の工程であって
playback path の特例ではない: 公開後の録音 blob は import blob と完全に
同一の形・同一の digest・同一の verified-source 検査を通る。

境界配置: **encode/publish は `synvelis-player`** (blob layout・
`content_digest`・`publish_atomic` を所有; `ruviform-io-wav` を新規依存化)。
**receipt 読取りは `synvelis-recording::commit`** (manifest 構成の責務)。
`canonical.json` の schema/path helper は `synvelis-recording::staging`
(session-dir layout の責務) に置く。依存方向は `player → recording` の
既存方向のみで cycle を作らない。

### Consequences

- Good: resolver/lowerer/render/export に録音固有コードが一行も入らない;
  全経路が L1/L2 と同一コード。canonical digest が commit manifest の
  `identity.content_hash` と `applied_commits` に入り、commit identity が
  canonical media に確定される。
- Good: receipt 不在 = raw fold digest のまま (VS-D 挙動・テスト互換)。
- Bad: blob bytes の再計算コスト (raw PCM 全読 + encode) — 非 RT 工程で
  許容。canonical.json と blob の二世代管理は staging lifecycle の
  責務に委譲。
- channels>2 は typed error (capability gap)。

### Confirmation

- `prepare_manifest` の fingerprint 照合により、未適用 manifest は
  canonical 有無の変化で自動再構築される (digest が fingerprint 入力)。
- 適用済み manifest は registry authority で不変 (stable ID 保護)。
- `canonical.json` は blob 公開後にのみ公開 (fail-closed ordering)。

## Implementation Plan

1. `CanonicalSourceRef` (staging schema + `canonical_rel`/`load_canonical`) +
   `StreamEntry.raw_digest` (serde default) + `build_payload` digest 選択。
2. `synvelis-player::publish`: `publish_recorded(area, media_root)`
   → `PublishReport{published,reused,skipped}` / `PublishError`。
3. X-01..X-05 headless 受入 test。
4. impl record + 検証記録。

## Verification

`mise run fmt-check` / `check` / `lint` / `test` 全 PASS + X 系 test が
`MockPcm` と bit-exact 一致することを確認。unrun capability gap は
impl record に記録する。

## Pros and Cons of the Options

### Option 1 (採用)

- Pros: generic 境界を維持、fail-closed、冪等、provenance 明示。
- Cons: encode コスト + receipt artifact が 1 種増える。

### Option 2

- Pros: 録音 media をそのまま resolve 可能。
- Cons: playback path に file-format 特例が残り、raw bytes では container
  検証が構造的に不可能 (canonical 化の必然性が消えるだけで安全側に
  倒れない)。#26 禁止事項に抵触。

### Option 3 / 4

- Pros: なし (Option 3) / container を直書きできる (Option 4)。
- Cons: ID/冪等分断、または RT writer 契約の破壊。

## More Information

- `docs/designs/x-recorded-verified-source-playback-export-design.md`
- `docs/specs/x-recorded-verified-source-playback-export.md`
- `docs/adr/vs-d-commit-identity-manifest-single-mutation.md`
