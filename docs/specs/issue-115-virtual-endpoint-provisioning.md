---
id: "SPEC-sv115VirtualEndpointProvisioning"
type: "spec"
status: "approved"
title: "Issue #115 RCD-01 Virtual Endpoint Provisioning Consumer Contract"
created: "2026-11-25"
updated: "2026-11-26"
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
      - type: "file"
        path: ".github/workflows/virtual-endpoint-consumer-acceptance.yml"
      - type: "file"
        path: "evidence/sv115/issue-115-virtual-endpoint-consumer-workflow-implementation.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/tests/virtual_endpoint.rs"
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

> Cross-repo link note: Ruviform 側文書への `blob/main` URL は、対応 PR merge 後の
> canonical location を示す。proposal 時点の main にはまだ存在しておらず、両 PR の統合後に
> 実在させる意図のリンクである。

## What Should Be Built

Ruviform が所有する step-level virtual-audio-endpoint provision/unprovision
primitive を、Synvelis が所有する manual consumer workflow から同一
`windows-2025` job 内で呼び出し、provisioned Ruviform virtual WASAPI endpoint
上で既存の `mise run test-recording-virtual-endpoint` を実走できるようにする。
本 spec はその後続 implementation slice の要件正本である。

この文書は 2026-11-25 の design/docs slice で作成され、2026-11-26 の
Slice B implementation で manual-only consumer workflow が実装された。
初回 provisioned manual run、artifact 検収、canonical run evidence 転記は
2026-11-26 の merge 後 follow-up gate で実施され、run `45797309944` attempt `1`
は consumer Fail だった。詳細は
`evidence/sv115/issue-115-virtual-endpoint-provisioned-consumer-run.md` を参照し、
本 spec の要件水準は run 結果で変更しない。既存 approved spec
`docs/specs/issue-115-virtual-endpoint.md` は consumer harness の要件正本で
あり、その Non-goals は merged 済み harness slice の履歴として維持する。
本 spec は provisioning orchestration を別 slice の要件として扱うものであり、
既存 harness の受入済み範囲を遡って変更しない。

## Acceptance Criteria

1. Ruviform 側 implementation slice が、driver build/test-sign/driver store
   staging/root-enumerated MEDIA devnode 作成/microphone consent provisioning/
   endpoint probe/cleanup を step-level primitive として提供する。interface は
   `https://github.com/scarecrow173/ruviform/blob/main/docs/designs/virtual-audio-endpoint-provisioning-interface.md`
   を正本とし、Synvelis へ install ロジックを複写しない。
2. Synvelis 側 implementation slice が `workflow_dispatch` 専用の manual
   workflow を追加し、同一 `windows-2025` x64 job 内で provision → endpoint
   readiness 確認 → `mise run test-recording-virtual-endpoint` → pre-cleanup
   metadata/artifact upload → `if: always()` cleanup → post-cleanup cleanup
   artifact upload を実行する。reusable workflow job 間で endpoint を受け渡す
   構成、signed driver package artifact を恒久 interface にする構成は採用しない。
3. consumer workflow の対象 SHA は checkout 後に `git rev-parse HEAD` で検証し、
   `SYNVELIS_RUN_REPO` / `SYNVELIS_RUN_SHA` へ注入する。dispatch input または
   workflow ref と checkout SHA が不一致なら harness を実行せず fail-closed で
   終了する。
4. provisioning source は Synvelis `Cargo.lock` の single resolved Ruviform
   revision に pin する。actual checkout revision・lock revision・driver package/
   cert/devnode/probe record は pre-cleanup consumer evidence artifact に残し、
   cleanup record は post-cleanup cleanup artifact に残して consumer RunMetadata
   と突合できるようにする。
5. `result=pass` は provisioned endpoint 上で consumer harness が出力した
   terminal RunMetadata に限る。upstream Ruviform Pass、job green、artifact の
   存在、cleanup 成功を Pass に転用しない。pair 不在は `NotRun`、candidate 存在
   下の oracle/open/stream/commit/reload 失敗は `Fail` として解釈する。
6. cleanup は harness の Pass/Fail/NotRun、test crash、artifact upload 失敗に
   関係なく `if: always()` で実行する。consumer evidence artifact の upload は
   cleanup より前に完了させ、cleanup 結果は consumer verdict と分離して
   post-cleanup artifact に記録する。partial provisioning state の uninstall
   までを scope に含め、test evidence が cleanup 成否へ依存しないようにする。
7. raw run artifact は CI artifact に残し、canonical evidence は通常の PR で
   `evidence/sv115/` へ転記する。CI から repository へ直接 commit しない。
   consumer evidence artifact と cleanup artifact は名前を分け、upload-artifact
   v4 の immutable artifact 制約と evidence 保存責務を混同しない。
8. implementation は (i) Ruviform primitive extraction、(ii) Synvelis
   consumer workflow の 2 slice に分割された。Slice B の workflow implementation
   は本 spec の対象であり、provisioned run evidence の採取と転記は merge 後
   follow-up gate とする。既存 manual probe workflow
   `virtual-audio-driver-probe.yml` を退行させない移行戦略を維持する。

## Non-goals

- 本 implementation slice では Ruviform source、PowerShell primitive、
  `Cargo.toml`/`Cargo.lock`、required PR gate、scheduled lane を変更しない。
  provision/unprovision script の実行は GitHub-hosted ephemeral runner 上の
  manual workflow に限定し、開発者 machine や persistent runner では行わない。
- `workflow_call` reusable workflow、cross-repo artifact package download、
  signed driver binary の長期配布、production driver release を恒久 interface
  にしない。
- required PR gate・ci-summary gate・scheduled lane への接続を初期要件にしない。
- external audio driver、VB-CABLE/Voicemeeter、physical device、macOS/Linux
  endpoint、GUI Option B、fault injection、Punch/Monitor/Arm 全 case、
  cross-clock multi-device の implementation design には拡張しない。
- `evidence/sv115/` への provisioned run 転記は merge 後 follow-up gate で行う。
  本 slice の evidence は workflow implementation の static/local validation に
  限定し、EXT-L3 Pass evidence ではない。
- `Cargo.toml`/`Cargo.lock`/ruviform pin・mise lane の変更は本 slice の scope 外。
  lock revision の前進が必要なら、workflow 実装へ混ぜず通常の依存更新として別途扱う。

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
