---
id: "IMPL-034VpPQEAhdWyygKYS7mb9"
type: "impl"
status: "in-progress"
title: "SF-04 slice5 probe subscriber window ownership"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
      - type: "file"
        path: "docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md"
      - type: "file"
        path: "docs/plans/sf-04-slice5-probe-subscriber-ownership-plan.md"
      - type: "file"
        path: "docs/tasks/sf-04-slice5-task1-probe-subscriber-ownership.md"
      - type: "file"
        path: "docs/test-specs/sf-04-slice5-probe-subscriber-ownership.md"
      - type: "file"
        path: "docs/impl/ir/sf-04-slice5-probe-subscriber-window-ownership.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/probe_trace.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sf-04-slice5-task1-probe-subscriber-ownership.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
    - "docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md"
    - "docs/test-specs/sf-04-slice5-probe-subscriber-ownership.md"
    - "docs/plans/sf-04-slice5-probe-subscriber-ownership-plan.md"
    - "docs/impl/ir/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/impl/ir/sf-04-slice2-controlsession-probe-observation-trace-wiring.md"
    - "docs/impl/ir/sf-04-slice3-probe-clock-epoch-discontinuity-capture-segmentation.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-04 slice5 probe subscriber window ownership

## Summary

Issue #107 (SF-04) slice 5: SF-PROBE-06 の session 層 headless
部分を実装した。`ProbeTraceSession` に subscriber lease /
capture owner の所有権モデルを追加し、同一 semantic point を
複数 window/inspector が監視しても `ProbeTracePort` への
request/remove/capture が重複・誤停止しないことを unit test
で固定した。subscriber 識別子は consumer-minted の
`ruviform_ui::dock::ViewInstanceId` を採用し、lease は
`ProbePrincipal::{Session, View}` で表現する。既存 API
（`probe_request`/`probe_remove`/`probe_capture`/
`probe_stop_capture`/`probe_set_mode`/`probe_drain`）の外部
挙動は不変で、`probe_request` は `Session` lease を伴う。
window close 相当の `probe_release_subscriber`、明示 abort の
`probe_abort_capture`（owner-only）、共有 key の owner 喪失時の
`Session` への retreat を実装。`#[cfg(test)]` fake port で
11 件の unit test を追加し全件 pass。

## Context

`docs/tasks/sf-04-slice5-task1-probe-subscriber-ownership.md`
に基づく。上流は spec/design/test-spec/plan の SF-04 slice5
文書チェーンと slice1–3 の IR。slice2 で確立した
`ProbeSubscriptionKey → targets` correlation には subscriber /
window 識別子がなく、`probe_remove`/`probe_stop_capture` が key
に無条件だったため、共有購読中の一方の window close を表現
できなかった。本 slice はその session 層中核（lease/refcount +
capture owner 追跡）のみを対象とし、wire op 化・app 配線・
実 2-window GUI は後続 Issue（#108/#114 gate、SV#117 占有中の
app/src）に委ねる。

## Implementation

### Changes

- `crates/synvelis-workspace/src/probe_trace.rs`:
  - `ProbePrincipal`（`#[non_exhaustive]` public enum:
    `Session` / `View(ViewInstanceId)`）を追加。
  - `ProbeTraceError` に `NotSubscriber` / `NotCaptureOwner`
    variant を追加。
  - `ProbeReleaseReport`（released keys の公開 read model）を追加。
  - `ProbeTraceSession` に `leases`
    （`ProbeSubscriptionKey → BTreeSet<ProbePrincipal>`）、
    `by_view`（`ViewInstanceId → BTreeSet<ProbeSubscriptionKey>`
    の逆引き index）、`capture_owners`
    （`ProbeSubscriptionKey → ProbePrincipal`）を追加。
    `targets` correlation は remove 後も保持し stopped-binding
    revival を可能にするため `leases ⊆ targets`（等号ではない）。
  - `probe_request`: 成功時に `Session` lease を付与（外部挙動不変）。
  - `probe_subscribe`: live lease が無い key のみ
    `port.probe_request` を呼び、live lease があれば port を
    呼ばず target correlation と view lease のみ追加。
    2 人目以降の `mode` は共有 binding を変更しない
    （mode 変更は `probe_set_mode` のみ）。port 失敗時は
    local state を一切変更しない（request 先行・commit 後置）。
  - `probe_unsubscribe`: view lease を持たない key は
    `NotSubscriber`。最後の lease 解放時のみ shared
    `remove_key` 経由で `port.probe_remove` → correlated
    targets へ `ProbeObservation::Missing` を同期記録 →
    全 lease / `by_view` / capture owner を失効
    （`targets` は保持）。port 失敗時は local state を
    untouched のまま retry 可能。
  - `probe_release_subscriber`: 1 view が保持する全 key を
    解放（window close 相当）。未登録 view は冪等に空 report。
    port 失敗で中断しても残りは retry 可能。
  - `probe_release_all`: principal を問わず全 held key を解放し
    removals を report。
  - `probe_capture`: 成功後に `Session` を capture owner として記録。
  - `probe_capture_for`: view lease を要求し、port capture 成功後に
    のみ `View(subscriber)` を owner 記録。失敗時 owner は空のまま。
  - `probe_abort_capture`: owner-only。lease 無し →
    `NotSubscriber`、別 owner / session owner / 対象外 →
    `NotCaptureOwner`。一致する view owner のみ
    `port.probe_stop_capture` を呼ぶ。port 失敗時 owner は不変。
  - `probe_stop_capture`: 従来どおり session-authorized。
    port stop 成功後にのみ owner を clear。
  - capture owner の view が lease を手放しても他 holder が残る
    場合は capture job を継続し owner を `Session` に retreat
    （`AlreadyCapturing` で上書きしない）。
  - read API: `probe_capture_owner` / `probe_lease_holders` /
    `probe_subscriber_keys`。
  - `remove_key`（private）: `port.probe_remove` 先行 →
    `Missing` 同期記録 → 全 view holder の `by_view` cleanup
    （空 entry 削除）→ lease/owner 失効 → `targets` 保持。
    `probe_remove`（legacy）もこの helper を共有。
  - `#[cfg(test)]` fake port（`ProbeTracePort` trait 実装）+
    11 件の unit test を同ファイル末尾に追加。
- `crates/synvelis-workspace/src/lib.rs`: 新規 public 型の export
  追加のみ。
- `docs/` slice5 文書チェーン 6 件 + 各 index README 更新。

### Decisions During Implementation

- **subscriber 識別子に `ViewInstanceId` を採用**: opaque
  `ProbeSubscriberRef` を新設せず、既存の consumer-minted・
  非 0・`Copy`/`Ord`/serde 対応・move/detach/reattach で安定な
  `ruviform_ui::dock::ViewInstanceId` を使う。window/inspector の
  論理寿命と一致し、RT 非依存・project/session port 境界を
  越えない識別子として適切（Ruviform source で直接確認済み）。
- **port-first atomicity**: subscribe の初回 request・
  unsubscribe/release の remove・capture/abort の port call は
  すべて local commit より先。port 失敗時に lease/owner/
  correlation が中途半端に残らず retry 可能な fail-closed。
- **`leases ⊆ targets`（等号にしない）**: `targets` は remove
  後も保持して stopped-binding revival に使うため、key 集合は
  lease 側が真部分集合になり得る。空 lease/`by_view` entry は
  残さない hygiene を helper で保証。
- **`Session` lease による legacy 保護**: `probe_request` は
  `Session` principal の lease を伴うため、view が自分の lease を
  解放しても session 所有の共有購読は `probe_remove` されない。
  session-authorized ops（`probe_set_mode`/`probe_stop_capture`/
  `probe_remove`/`probe_release_all`）は無条件のまま。
- **capture owner retreat**: owner view の lease 喪失時に他 holder
  が残れば job は継続し owner は `Session` へ退く。capture job を
  誤停止しない SF-PROBE-06 契約（close は他 subscriber/job を
  勝手に止めない）に一致。
- **`#[allow(clippy::too_many_arguments)]` on `probe_subscribe`**:
  `probe_request` と同じ flat 引数形状（point/kind/target/mode/
  segment 系）を踏襲する additive API で、引数 struct 化は
  既存 API との一貫性を崩すため、repo 先例（flat arg pack への
  justified allow）に倣い理由コメント付きで局所許可。

## Related Experiments

### Adopted

<!-- none -->

### Rejected

<!-- none -->

## Validation

- `mise run fmt-check`: pass（merge + vendor 同期後に再確認、exit 0）
- `mise run check`（cargo check --workspace --all-targets）: pass（exit 0）
- `mise run lint`（cargo clippy --workspace --all-targets --
  -D warnings）: pass（exit 0）
- `mise run test`（cargo test --workspace --locked）: pass（exit 0、
  再実行）。初回実行では `synvelis-library` webdav test 2 件
  （`malformed_chunked_body_fails_typed` /
  `cross_origin_redirect_never_reaches_other_origin`）が Windows
  socket 由来の flake（os error 10054 / TransientIo vs
  InvalidRequest）で fail。単独再実行・`mise run test` 再実行・
  `mise run test-msrv`（rust 1.89.0、webdav 42/42 pass）の
  いずれでも pass し、本変更（synvelis-workspace + docs のみ）
  と無関係の既存 flaky と判断。
- `mise run test-rt`（cargo test --workspace --locked rt_）: pass（exit 0）
- `mise run test-msrv`（rust@1.89.0 cargo test --workspace
  --locked）: pass（exit 0）
- 補助証拠（canonical = mise）: `cargo test -p
  synvelis-workspace` で新規 11 件を含む unit test 全件 pass、
  `cargo check -p synvelis-workspace --all-targets` pass。
- review-protocol: doc チェーン review（実装前）と実装 review
  （実装後）の 2 回実施。いずれも CONFIRMED P0/P1 なしで ACCEPT
  （設計段階で invariant 矛盾と unsubscribe atomicity の 2 件を
  発見・修正済み）。

## Risks

- `ViewInstanceId` は consumer（UI/dock 層）が mint するため、
  session 層は「同一 view からの再 subscribe = 同一 lease
  保持」として扱う。view が誤って同一 ID を使い回した場合の
  誤 dedup は consumer 側契約の範囲（Ruviform が再利用しない
  ことを source で確認済み）。
- lease は session-only で永続化しない — session 再生成で
  probe 購読は張り直しになる（slice2 以降の契約どおり）。
- webdav flaky は本 slice の変更と無関係だが、`mise run test`
  の canonical 結果に影響し得る — 再実行結果を PR に記録する。

## Follow-ups

- #108/#114: 実 2-window GUI gate — app/UI 層が
  `probe_subscribe`/`probe_release_subscriber`/
  `probe_capture_for`/`probe_abort_capture` を消費する配線
  （SV#117 が app/src 占有中のため本 slice 範囲外）。
- `ui/src/ipc.ts` の `syn.probe.*` OpName 追加と wire op 化 —
  後続 Issue 側の消費。
- SF-PROBE-07 `align_observation_points` 消費、SF-PROBE-05
  pool/channel fault injection、slice-3 残 transport 契約拡張、
  SF04-RCP-01、SF-PROBE-08/09 は本 slice 範囲外。
