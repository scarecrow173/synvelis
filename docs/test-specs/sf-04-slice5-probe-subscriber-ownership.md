---
id: "TSPEC-034Vpt0mvci6gGc0CDalrl"
type: "test-spec"
status: "approved"
title: "SF-04 slice5 受入テスト仕様 — probe subscriber/window ownership（session 層 lease・capture owner）"
created: "2026-11-10"
updated: "2026-11-11"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
    - "docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice5 受入テスト仕様 — probe subscriber/window ownership（session 層 lease・capture owner）

## Purpose

SF-04 slice5（Issue #107 / SF-PROBE-06 の session 層部分）が、
同一 semantic point を複数 view/inspector が監視しても port
subscription を重複させず、Window Close・Window Move・
Session stop・明示 Capture Abort の ownership を fail-closed
に固定することを、`probe_trace.rs` 内 `#[cfg(test)]` unit
tests（fake `ProbeTracePort`）で保証する。実 GUI・wire op・
port/player 実装は対象外。

## Feature

Probe subscriber lease と capture owner を持つ
`ProbeTraceSession` が、複数 `ViewInstanceId` subscriber の
共有 subscription を正しく参照カウントし、owner 認可された
capture abort と session stop の全解放を行う。

## Rules

- **R1 — first-holder request / later-holder dedup**: 同一
  `(session_id, point, kind)` key への `probe_subscribe` は、
  live lease が無い場合のみ `port.probe_request` を呼ぶ。
  lease が既にある場合は port を呼ばず lease・target 相関のみ
  追加し、共有 binding の mode を変更しない。
- **R2 — last-lease removal**: lease が空になる解放のみ
  `port.probe_remove` + `Missing` 記録を行う。他 principal が
  残る解放は局所 lease のみ除去し port を呼ばない。
- **R3 — owner-only abort**: `probe_abort_capture` は
  `capture_owners[key] == View(subscriber)` の場合のみ
  `port.probe_stop_capture` を呼ぶ。それ以外は
  `NotCaptureOwner`、lease 非保持は `NotSubscriber` で
  fail-closed。
- **R4 — owner lease 喪失の退避**: capture owner の view が
  lease を失っても key が他 principal に保持される限り
  capture は継続し、owner は `Session` に退避する。
- **R5 — session 権限の無条件性**: `probe_capture`/
  `probe_stop_capture`/`probe_remove`/`probe_set_mode`/
  `probe_release_all` は lease 保持者に関わらず session が
  明示実行でき、owner 記録・lease revoke を正しく同期する。
- **R6 — fail-closed**: port 操作失敗時は局所 lease/owner/
  相関を変更しない（`Err` 伝播のみ）。lease/owner の無い
  subscriber 操作は `NotSubscriber`/`NotCaptureOwner`。
- **R7 — Window Move 不変性**: `ViewInstanceId` は move/
  detach/reattach で不変のため session 層に move 遷移はなく、
  別 view の release は当該 view の lease のみに影響する。
- **R8 — 既存 API 不変**: `probe_request`/`probe_set_mode`/
  `probe_capture`/`probe_stop_capture`/`probe_remove`/
  `probe_drain` と `ProbeDrainReport` の外部挙動は不変
  （`Session` lease/owner 記録の追加のみ）。

## Examples

- **E1 dedup + fan-out**: view A・view B が同一 point+kind に
  subscribe → `probe_request` port call は 1 回、`leases[key]`
  = `{View(A), View(B)}`、`by_view` 両方向、両 target が
  `targets[key]` に相関。drain update が両 target に
  fan-out する。
- **E2 partial release**: A のみ `probe_release_subscriber` →
  port remove なし、B の lease・capture・観測が継続。B の
  最後の release で `probe_remove` 相当 + `Missing` 記録。
- **E3 capture ownership**: B が `probe_capture_for` →
  `capture_owner=View(B)`。A の `probe_abort_capture` は
  `NotCaptureOwner`。B の abort は `port.probe_stop_capture`
  に届き owner 消去。B の lease 喪失（A が残る）で owner が
  `Session` に退避し capture 継続。
- **E4 session stop**: `probe_release_all` で全 lease key が
  port remove され `by_view`/`capture_owners` が空になる。
- **E5 port failure atomicity**: `probe_subscribe`/`probe_remove`
  系で port が `Err` を返した場合、lease/owner/相関は subscribe
  前の状態を保持し retry 可能。
- **E6 legacy protection**: `probe_request` で作った key は
  `Session` lease を持ち、view の subscribe→release で
  道連れ remove されない。

## Guarantees

- `leases ⊆ targets` の key 集合関係、`by_view` が `leases`
  の View member の正確な逆引き、空集合 entry を残さない、
  `capture_owners` の key は必ず `leases` に存在 — 全ての
  公開 API 呼出後に維持される。
- port 呼出順序: `probe_request`（初回/revival のみ）→
  lease/相関登録、`probe_remove`（最終解放/session 明示のみ）
  → Missing 記録 → lease/owner 掃除。
- `ProbeReleaseReport` が release pass の `released`/`removed`
  を正直に報告し silent drop しない。

## Non-goals

- 実 2-window GUI・`ViewRegistry` との結合・`syn.probe.*`
  wire op（#108/#114・app slice）。
- port/player/project 層の変更、broker dedup 再設計。
- per-subscriber mode・per-subscriber target lease・
  probe state の永続化。
- port 直叩き（session API 以外の `ProbeTracePort` 呼出）に
  対する防御 — 契約外行為。

## Risk

lease/owner の atomicity が崩れると「window close が他者の
subscription/capture を殺す」「capture abort の権限昇格」
という SF-PROBE-06 の中核違反になる。dedup が崩れると broker
capture 重複（RT 負荷・観測破壊）に直結する。これらを unit
test で固定しない限り、app 配線後にのみ顕在化する不可逆的な
ownership 欠陥となる。
