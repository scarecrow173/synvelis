---
id: "SPEC-034VphYPdMdjVaJWKkDJHg"
type: "spec"
status: "approved"
title: "SF-04 slice5 — probe subscriber/window ownership（session 層 lease・capture owner）"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
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
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/specs/sf-04-slice4-app-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-04-slice4-app-wiring.md"
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice5 — probe subscriber/window ownership（session 層 lease・capture owner）

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）の SF-PROBE-06 session 層
slice。slice 2 が確立した `ProbeTraceSession` の相関
（`ProbeSubscriptionKey → BTreeSet<ParameterTargetId>`）に、
**subscriber（Window/Inspector）単位の lease/refcount と capture
owner 追跡**を additive に追加し、「同じ Semantic Point を複数
Window/Inspector が監視しても Broker Capture が重複しない」
「Window Close / Window Move / Session stop / explicit Capture
Abort の ownership」を headless session 層で固定する。

具体的には `crates/synvelis-workspace/src/probe_trace.rs`
（`ControlSession` の probe API 面）に:

- subscriber 識別子として ruviform-ui の `ViewInstanceId`
  （window move/detach/reattach を跨いで安定な論理 view id）を
  採用し、lease 保持者を `ProbePrincipal`
  （`Session` | `View(ViewInstanceId)`）で表す。
- `probe_subscribe` — subscriber 付き購読。session がその key に
  live hold を持たない場合のみ `port.probe_request` を呼び
  （port 側 `ProbeSession::request` の key 冪等と合わせて Broker
  重複なし）、live hold がある場合は port を呼ばず lease と
  target 相関だけを追加する。
- `probe_unsubscribe` / `probe_release_subscriber` /
  `probe_release_all` — lease 解放。最後の lease が外れた key は
  `probe_remove` と同じ後処理（port remove + 相関 target へ
  `Missing` 記録 + lease/owner 掃除）を行い、他 subscriber や
  session 保持中の key・進行中 capture を巻き込まない。
- capture owner 追跡 — `probe_capture`（session 権限）と新規
  `probe_capture_for`（subscriber 権限）が開始した capture の
  owner を `ProbePrincipal` で記録し、`probe_abort_capture`
  （owner のみ）と `probe_capture_owner`（read）を提供する。
  owner の lease が外れて key が残る場合は owner を `Session`
  へ退避し、capture job 自体は継続する（Close Window は進行
  Job を勝手に Stop しない）。
- 既存 `probe_request`/`probe_set_mode`/`probe_capture`/
  `probe_stop_capture`/`probe_remove`/`probe_drain` の port 呼出
  順序・相関・戻り値は不変（`probe_request` は `Session`
  principal の lease を内部的に記録するのみで外部挙動は同一）。

## Why Now

Issue #107 の SF-PROBE-06 は「同じ Semantic Point を 2
Window/Inspector が監視しても Broker Capture 重複なし、
Window Close/Window Move/Session stop/explicit Capture Abort の
ownership」を要求する。現行の `ProbeTraceSession` は key→target
相関のみで subscriber/window 識別子を持たず、`probe_remove`/
`probe_stop_capture`/`probe_set_mode` が key に無条件で効くため、
共有購読中の一方の Window close を表現できない（契約:
「複数Inspector/Windowは購読者」「Close Windowは他Subscriberや
進行Jobを勝手にStopしない」「PCM Captureは明示Request/
Owner/Frame Budgetを伴う」）。Broker 側の dedup は
`synvelis-player` `ProbeSession::request` の key 冪等で済んで
おり、不足は session 層の lease/refcount と capture owner 追跡
のみ — 本 slice がその headless 中核を固定する。#108/#114 の
実 2-window GUI gate と後続の app 配線 slice がこの契約の上に
乗る。

## Users and Value

- #108 UI 実装者が、view（Window/Inspector）単位で probe 購読を
  開始・解放でき、「window を閉じても他の inspector の観測や
  進行中 capture が死なない」契約を session API 経由で得る。
- app/session 配線の後続 slice が、`syn.probe.*` wire op に
  subscriber 識別子を載せる際の session 側受け口を得る
  （wire op 化自体は SV#117 が app/src を占有中のため本 slice
  対象外）。
- session 層の headless unit test が、複数 subscriber・capture
  owner・各種解放経路の ownership を回帰固定する。

## Scope

### In Scope

- `crates/synvelis-workspace/src/probe_trace.rs`: subscriber
  lease/capture owner の state と additive API、`#[cfg(test)]`
  unit tests（`crates/synvelis-workspace/tests/` は別 Worker 占有
  のため src 内 unit test に置く — `ProbeTracePort` は trait で
  fake 容易）。
- `crates/synvelis-workspace/src/lib.rs`: 新規公開型の export
  追加のみ。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/impl
  record）。

### Out of Scope

- `crates/synvelis-workspace/tests/`（別 Worker 占有）。
- `crates/synvelis-app/`（SV#117 占有中）を含む他 crate —
  `syn.probe.*` wire op への subscriber 識別子追加・view close
  hook の app 配線は後続 Issue 側の消費。
- `ui/src/ipc.ts` の `syn.probe.*` OpName 追加（lease 外）。
- 実 2-window GUI 検証（#108/#114 gate）、SF-PROBE-05/07/08/09
  等の他残件、player/project 層の変更（port 契約・broker dedup
  は既存のまま消費するのみ）。
- `ControlSession` 以外の session 状態・永続化（probe state は
  persist しない既存設計を維持）。

## Requirements

- 同一 `ProbeSubscriptionKey`（session + Semantic Point +
  kind）への複数 subscriber 参加で Broker probe/capture が
  重複しない: session が live hold を持つ key への
  `probe_subscribe` は `port.probe_request` を呼ばず、port 側
  `ProbeSession::request` の冪等も二重の担保となる。
- `mode` は key の live hold が無い場合（初回 subscribe・
  `Stopped` binding の revival）にのみ適用され、既存 hold への
  参加は共有 binding の mode を変更しない（mode 変更は
  `probe_set_mode` の session 権限操作のみ — per-subscriber
  mode は契約に存在しない）。
- 「Close Windowは他Subscriberや進行Jobを勝手にStopしない」:
  `probe_release_subscriber`/`probe_unsubscribe` は対象
  subscriber の lease のみを外し、他 subscriber が残る key の
  port subscription・capture は継続する。最後の lease が外れた
  場合のみ port remove（capture は `Removed` segment end で
  閉じる — port 側既存契約）。
- 「Window Move」は `ViewInstanceId` が move/detach/reattach を
  跨いで安定なため session 層に遷移が存在しない — lease は
  view id に紐づき window 帰属に依存しない。
- 「Session stop」は `probe_release_all` が session の全
  subscription（session 保持・view 保持を問わず）を port
  remove する明示経路を提供する。
- 「explicit Capture Abortのownership」: capture 開始者を
  `ProbePrincipal` で記録し、`probe_abort_capture` は owner
  のみ受理する。owner でない subscriber の abort は
  `NotCaptureOwner` で fail-closed。session 権限の
  `probe_stop_capture`/`probe_remove` は従来どおり無条件の
  明示 abort として残す。
- owner の lease 解放で key が残る場合 capture は継続し owner
  は `Session` へ退避する（帰属は「最後に明示開始した者」の
  記録であり、job の生死は lease ではなく明示操作が決める）。
- missing は missing のまま: lease の無い subscriber の
  unsubscribe/abort/capture は `NotSubscriber` で fail-closed、
  port 失敗は `ProbeTraceError::Port` で伝播し局所状態を
  汚さない。
- 既存 API の外部挙動不変: `probe_request`/`probe_set_mode`/
  `probe_capture`/`probe_stop_capture`/`probe_remove`/
  `probe_drain` と `ProbeDrainReport`・`probe_targets`/
  `probe_subscriptions` の契約は維持する。

## Acceptance Criteria

- [ ] SF-PROBE-06 session 層: 同一 Semantic Point を 2
  subscriber（Window/Inspector 相当の `ViewInstanceId`）が
  監視しても `port.probe_request` は live hold 中に重複発行
  されず（fake port で request 回数・capture 重複なしを固定）、
  Window Close（`probe_release_subscriber`）/Window Move
  （id 安定性）/Session stop（`probe_release_all`）/explicit
  Capture Abort（`probe_abort_capture` の owner 限定）の
  ownership が `probe_trace.rs` 内 unit test で固定される。
- [ ] 旧 Acceptance "multiple inspector/windows" 句の session
  側証拠: 共有 key で片方の release が他 subscriber の
  subscription・capture・drain fan-out を止めないこと、最後の
  release でのみ port remove + `Missing` 記録されること。
- [ ] 既存 `probe_request`/`probe_remove` 系の unit/integration
  テスト（`sf04_s2_probe_trace.rs` 等）が無変更で green。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` が green。

## Deferred Design Concerns

- `syn.probe.*` wire op への subscriber 識別子追加と view close
  → `probe_release_subscriber` の app 配線 | `synvelis-app` は
  SV#117 が占有中・`ui/src/ipc.ts` は lease 外 | app 配線の
  後続 Issue（#108 系）| session 契約が wire 面の要求を満たすか
  は app 実装時に検証。
- 実 2-window GUI での capture dedup・close/move 観測 | 実
  Window/OS GUI は #108/#114 gate | gate 通過後 | headless
  session 層のみでは実 UI 寿命との整合は証明できない。
- subscriber 単位の mode/capture 個別制御（view ごとに違う
  mode を同一 key に要求する等）| key=binding は共有で mode は
  binding 属性 — 契約上 per-subscriber mode は存在しない |
  必要になった時点で port/契約拡張を検討 | 現状は共有 binding
  の mode を `probe_set_mode` で一括変更するのみ。
- subscriber ごとの target 相関（view A の target だけ観測を
  止める等）| 相関は key→target の session 級 fan-out で
  per-subscriber target lease は持たない | UI が target 単位の
  unsubscribe を要求した時点 | 現状は key 生存中 target は
  観測を受け続ける（bounded・honest）。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/107>
  （SF-PROBE-06、契約節「複数Inspector/Windowは購読者」
  「Close Windowは他Subscriberや進行Jobを勝手にStopしない」
  「PCM Captureは明示Request/Owner/Frame Budgetを伴い」）。
- 前提: SF-04 slice1（`ProbeSession`/`ProbeSubscriptionKey`・
  request 冪等）、slice2（`ProbeTracePort`/
  `ProbeTraceSession` 相関・`probe_drain` 単一 owner）、
  slice3（segment cut/epoch）、slice4（app 配線・
  `syn.probe.*` ops — 本 slice はその session 側受け口）。
- ruviform-ui `dock::ViewInstanceId` — 論理 view id（move/
  detach/reattach で安定、consumer mint、非 RT）。
- 本 spec は Issue #107 のうち SF-PROBE-06 の session 層
  headless 部分のみを対象とし、Issue は Open 継続が前提。
