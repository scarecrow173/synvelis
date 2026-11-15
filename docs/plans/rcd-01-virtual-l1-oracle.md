---
id: "PLAN-3ibVZ5odzisDWKe9oPD32D"
type: "plan"
status: "approved"
title: "RCD-01 Virtual L1 Oracle Plan"
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
  implements:
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  implemented-by:
    - "docs/tasks/rcd-01-virtual-l1-oracle.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-01-device-recording-headless-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
    - "docs/designs/rcd-01-virtual-l1-oracle.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
  verified-by:
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
---

# RCD-01 Virtual L1 Oracle Plan

## Scope split

単一 slice（Option A — synthetic-L1 最終 Oracle の確立）。Issue #115 の
残り gate（rf#628 fixture variant・rf#629 仮想 endpoint EXT-L3・physical・
GUI Option B）は本 slice 対象外で、Issue は open 継続。

## File map

| File | 変更 | 内容 |
| --- | --- | --- |
| `crates/synvelis-recording/tests/rcd01_oracle.rs` | added | 独立 oracle + scripted device + E2E fidelity/multi-track/fault test（7 test） |
| `crates/synvelis-recording/tests/common/mod.rs` | changed | 必要に応じた test-only helper（最終的に変更なしで済んだ場合は記録のみ） |
| `evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md` | added | headless evidence ledger（Env 表・Status 語彙・per-case Expected/Observed） |
| `evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md` | added | Required case 分類 matrix（synthetic-L1/rf#628/rf#629/physical/real-input/canonical blob） |
| `docs/specs|designs|plans|tasks|test-specs|impl/ir|impl/exp/rcd-01-virtual-l1-oracle.md` | added | doc chain（slug `rcd-01-virtual-l1-oracle`） |
| `docs/*/README.md` | generated | `doc-maintenance` Skill の手順で index 再生成（手編集禁止） |

## Dependency graph

```
spec → design → plan → task → test-spec → impl/ir → impl/exp
                 ↘ evidence/sv115 (ledger + case matrix)
```

## Risk register

| Risk | 対応 |
| --- | --- |
| oracle が `MockPcm` と構造を共有してしまう（self-reference 残存） | mixer・定数・bit window・f32 射影を意図的に別構造にし、module doc に明記。emission-log 検証で generator 再計算に依存しない構造にする |
| `RealDeviceRig::open` が `&dyn Device` を受けない | `tests/common/mod.rs:969` で `&[(&dyn Device, &InputStreamSpec)]` を確認済み。`DeviceRig` は `&FakeInputDevice` 専用なので `RealDeviceRig` を選択 |
| fault case の finalized outcome を `Partial` と誤認 | precedent（`fake_input_drop_fails_stop_and_preserves_verified_prefix`）に倣い、writer outcome=durable frontier `Complete`、commit 層 recoverable 分類は capture-fault marker が制御、として実装 |
| `verified_sha256` の独立再計算で sha2 API を誤る | `sha2` crate（既存 dep）で journaled prefix bytes を SHA-256 して `sha256:<64hex>` と照合 |
| canonical blob publish 脚を誤って Pass 転用 | `synvelis-player` 管轄として matrix に参照のみ明記し、本 slice で再実走しない |

## Rollback strategy

新規 test file + 新規 doc/evidence のみ。production `src/` 変更なし。
問題時は当該 commit を revert すれば既存 test・production に影響しない。

## Verification matrix

| 検証 | 種別 | 期待 |
| --- | --- | --- |
| `cargo test -p synvelis-recording --test rcd01_oracle` | 補助証拠 | 7 test pass |
| `mise run fmt-check` | 正規 | pass |
| `mise run check synvelis-recording` | 正規 | pass |
| `mise run lint synvelis-recording` | 正規 | pass（-D warnings） |
| `mise run test synvelis-recording` | 正規 | pass（非回帰含む） |
| `mise run test-rt` | 正規 | pass |
| `mise run check-ruviform-rev` | 正規 | pinned rev `fd1d4d5` 確認 |
| `mise run test-recording-real-input` | 正規 opt-in | device 不在なら loud SKIP = NotRun 記録 |
| `mise exec -- prek run --all-files` | 正規 | pass |
