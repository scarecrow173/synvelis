---
id: "TSPEC-sv115VirtualEndpointRcd01"
type: "test-spec"
status: "approved"
title: "Issue #115 RCD-01 Consumer Virtual-WASAPI EXT-L3 検証仕様"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/virtual_endpoint.rs"
    modified:
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md"
    - "evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-115-virtual-endpoint.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-115-virtual-endpoint.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
---

# Issue #115 RCD-01 Consumer Virtual-WASAPI EXT-L3 検証仕様

## Purpose

`docs/specs/issue-115-virtual-endpoint.md` の acceptance criteria を、
`crates/synvelis-recording/tests/virtual_endpoint.rs` と mise lane の検証へ
落とす正本。endpoint 実在有無と harness 実装正しさを混同しない。

## Test matrix

| ID | 対象 | 実行 lane | Expected | 判定 |
| --- | --- | --- | --- | --- |
| VE-U1 | Ruviform Cable stem matcher | `mise run test synvelis-recording` | `Ruviform Cable Input/Output` と `Ruviform Virtual Audio Device` 表記揺れが同一 stem へ候補化される | feature 非依存 pure test |
| VE-U2 | 非対象 endpoint reject | 同上 | VB-CABLE・Voicemeeter・物理 device・`Ruviform Microphone` を候補化しない | feature 非依存 pure test |
| VE-U3 | RunMetadata round-trip | 同上 | Case Schema v1 vocabulary が JSON round-trip し、`virtual-os-endpoint`/`ext-l3`/`not-run` を保持する | feature 非依存 pure test |
| VE-U4 | NotRun/Fail 分類 | 同上 | pair 0 件は `NotProvisioned`、pair 1 件以上は `Candidates` に分類される | feature 非依存 pure test |
| VE-N1 | endpoint 不在 fail-closed | `mise run test-recording-virtual-endpoint` | pair 不在なら terminal metadata `result=not-run` / `NOT_PROVISIONED` を出し、test/lane は exit 0 | endpoint 不在環境で必須 |
| VE-E1 | EXT-L3 round-trip | 同上（endpoint provisioning 済み環境） | output callback→capture→Recorder→commit→save→reload が通り、marker/RMS/latency/journal/digest oracle が Pass | endpoint 不在環境では NotRun |
| VE-R1 | stale probe 現行化 | `mise run lint-recording-real-input` / compile | real-input lane が deterministic PCM fidelity の権威を virtual endpoint harness へ分離したことを正直に出力する | 非回帰 |

## Oracle detail

### 信号内容 oracle（EXT-L3）

- impulse marker は amplitude 0.8・100ms 周期。
- capture mono tap ごとに marker 8 個以上、平均間隔 100ms±10%、最初の marker
  latency 1 秒以内、非 finite sample なし、RMS/level ratio が許容帯内。
- output callback は channel 数一致、timestamp 非回退、fill error なし、十分な
  callback/frame 数を要求する。
- render stimulus と capture PCM の bit-exact 一致は要求しない（OS SRC 許容）。

### 記録媒体 oracle

- 各 mono stream の journal spans が session range を被覆し、source frames が
  連続、CRC/integrity 検証が通る。
- finalized media byte length は frame 数×f32 bytes と一致する。
- `verified_sha256` は staging media verified prefix bytes から `sha2` で独立
  再計算した値と一致する。
- commit manifest entry、Project `Source.content_digest`、`Asset.digest` が
  save→reload 後も一致する。
- reload 後 recommit は `AlreadyApplied`。

## Fail-closed vocabulary

- `HOST_ENUMERATION_FAILED`: host/device enumeration 自体が失敗。NotRun。
- `NOT_PROVISIONED`: Ruviform stem pair が 0 件。NotRun。
- `CANDIDATE_FAILED`: pair 候補は存在するが open/start/callback/oracle/
  commit/reload が失敗。Fail。
- endpoint 名一致は証拠ではなく、open/start/stream 観測済み run のみ Pass。

## Canonical commands

1. `mise run fmt-check`
2. `mise run check synvelis-recording`
3. `mise run lint synvelis-recording`
4. `mise run test synvelis-recording`
5. `mise run lint-recording-real-input`
6. `mise run test-recording-virtual-endpoint`
7. `mise exec -- prek run --all-files`

cargo/cargo-clippy/cargo-test の直接実行は補助証拠であり、canonical は mise。
