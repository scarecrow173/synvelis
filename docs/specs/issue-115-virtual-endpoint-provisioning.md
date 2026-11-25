---
id: "SPEC-sv115VirtualEndpointProvisioning"
type: "spec"
status: "proposed"
title: "Issue #115 RCD-01 Virtual Endpoint Provisioning Consumer Contract"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "docs/specs/issue-115-virtual-endpoint-provisioning.md"
      - type: "file"
        path: "docs/designs/issue-115-virtual-endpoint-provisioning.md"
      - type: "file"
        path: "docs/test-specs/issue-115-virtual-endpoint-provisioning.md"
      - type: "file"
        path: "docs/adr/issue-115-virtual-endpoint-provisioning-ownership.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/issue-115-virtual-endpoint-provisioning.md"
  depends-on:
    - "docs/specs/issue-115-virtual-endpoint.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/issue-115-virtual-endpoint-provisioning-ownership.md"
    - "https://github.com/scarecrow173/ruviform/blob/main/docs/designs/virtual-audio-endpoint-provisioning-interface.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-virtual-endpoint.md"
  derived-by:
    - "docs/designs/issue-115-virtual-endpoint-provisioning.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-115-virtual-endpoint-provisioning.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/629"
    - "https://github.com/scarecrow173/ruviform/issues/762"
    - "https://github.com/scarecrow173/ruviform/issues/1001"
  defers: []
  deferred-by: []
---

# Issue #115 RCD-01 Virtual Endpoint Provisioning Consumer Contract

## What Should Be Built

Ruviform が所有する step-level virtual-audio-endpoint provision/unprovision
primitive を、Synvelis が所有する manual consumer workflow から同一
`windows-2025` job 内で呼び出し、provisioned Ruviform virtual WASAPI endpoint
上で既存の `mise run test-recording-virtual-endpoint` を実走できるようにする。
本 spec はその後続 implementation slice の要件正本である。

この文書は 2026-11-25 の design/docs slice で作成された proposal であり、
provisioning script・workflow・evidence はまだ実装されていない。実装済みの
spec として扱ってはならない。既存 approved spec
`docs/specs/issue-115-virtual-endpoint.md` は consumer harness の要件正本で
あり、その Non-goals（driver install/provisioning、CI workflow/job 追加、
cross-repo artifact download、pin 変更の非実施）は既に merged 済みの harness
slice の履歴として維持する。本 spec はそれらを別 slice の設計対象として
扱うものであり、既存 harness の受入済み範囲を遡って変更しない。

## Acceptance Criteria

1. Ruviform 側 implementation slice が、driver build/test-sign/driver store
   staging/root-enumerated MEDIA devnode 作成/microphone consent provisioning/
   endpoint probe/cleanup を step-level primitive として提供する。interface は
   `https://github.com/scarecrow173/ruviform/blob/main/docs/designs/virtual-audio-endpoint-provisioning-interface.md`
   を正本とし、Synvelis へ install ロジックを複写しない。
2. Synvelis 側 implementation slice が `workflow_dispatch` 専用の manual
   workflow を追加し、同一 `windows-2025` x64 job 内で provision → endpoint
   readiness 確認 → `mise run test-recording-virtual-endpoint` → metadata/
   artifact upload → `if: always()` cleanup を実行する。reusable workflow job
   間で endpoint を受け渡す構成、signed driver package artifact を恒久 interface
   にする構成は採用しない。
3. consumer workflow の対象 SHA は checkout 後に `git rev-parse HEAD` で検証し、
   `SYNVELIS_RUN_REPO` / `SYNVELIS_RUN_SHA` へ注入する。dispatch input または
   workflow ref と checkout SHA が不一致なら harness を実行せず fail-closed で
   終了する。
4. provisioning source は Synvelis `Cargo.lock` の single resolved Ruviform
   revision に pin する。actual checkout revision・lock revision・driver package/
   cert/devnode/probe/cleanup record を artifact に残し、consumer RunMetadata
   と突合できるようにする。
5. `result=pass` は provisioned endpoint 上で consumer harness が出力した
   terminal RunMetadata に限る。upstream Ruviform Pass、job green、artifact の
  存在、cleanup 成功を Pass に転用しない。pair 不在は `NotRun`、candidate 存在
   下の oracle/open/stream/commit/reload 失敗は `Fail` として解釈する。
6. cleanup は harness の Pass/Fail/NotRun、test crash、artifact upload 失敗に
   関係なく `if: always()` で実行する。cleanup 結果は consumer verdict と
   分離して記録し、partial provisioning state の uninstall までを scope に含める。
7. raw run artifact は CI artifact に残し、canonical evidence は通常の PR で
   `evidence/sv115/` へ転記する。CI から repository へ直接 commit しない。
8. 後続 implementation は (i) Ruviform primitive extraction、(ii) Synvelis
   consumer workflow の 2 slice に分割し、既存 manual probe workflow
   `virtual-audio-driver-probe.yml` を退行させない移行戦略を取る。

## Non-goals

- 本 design/docs slice では script extraction、workflow 追加、PowerShell 変更、
  provisioning 実行、driver install、管理者権限を要する OS 設定変更を行わない。
- `workflow_call` reusable workflow、cross-repo artifact package download、
  signed driver binary の長期配布、production driver release を恒久 interface
  にしない。
- required PR gate・ci-summary gate・scheduled lane への接続を初期要件にしない。
- external audio driver、VB-CABLE/Voicemeeter、physical device、macOS/Linux
  endpoint、GUI Option B、fault injection、Punch/Monitor/Arm 全 case、
  cross-clock multi-device の implementation design には拡張しない。
- `evidence/sv115/` は後続 run の正規転記先を定義するだけであり、本 spec の
  作成時点では evidence ファイルを追加しない。
- `Cargo.toml`/`Cargo.lock`/ruviform pin・mise lane の変更は本 design/docs
  slice の scope 外。implementation slice で lock revision の前進が必要なら、
  workflow 実装へ混ぜず通常の依存更新として別途扱う。

## Acceptance coverage map

| acceptance 項目 | 正本 |
| --- | --- |
| 所有権境界 | `docs/designs/issue-115-virtual-endpoint-provisioning.md` / `docs/adr/issue-115-virtual-endpoint-provisioning-ownership.md` |
| reusable workflow・artifact package の不適理由 | `docs/adr/issue-115-virtual-endpoint-provisioning-ownership.md` |
| provision/unprovision interface | Ruviform `docs/designs/virtual-audio-endpoint-provisioning-interface.md` |
| 同一 job orchestration | `docs/designs/issue-115-virtual-endpoint-provisioning.md` |
| source pin 方針 | `docs/designs/issue-115-virtual-endpoint-provisioning.md` |
| RunMetadata/evidence 運用 | `docs/designs/issue-115-virtual-endpoint-provisioning.md` / `docs/test-specs/issue-115-virtual-endpoint-provisioning.md` |
| 後続 2 slice 分割 | `docs/designs/issue-115-virtual-endpoint-provisioning.md` |
