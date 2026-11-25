---
id: "ADR-sv115VirtualEndpointProvisioningOwnership"
type: "adr"
status: "proposed"
title: "Issue #115 virtual endpoint provisioning ownership — Ruviform step-level primitive と Synvelis consumer orchestration"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "docs/adr/issue-115-virtual-endpoint-provisioning-ownership.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-115-virtual-endpoint-provisioning.md"
  implemented-by:
    - "docs/designs/issue-115-virtual-endpoint-provisioning.md"
  depends-on:
    - "docs/specs/issue-115-virtual-endpoint.md"
    - "docs/designs/issue-115-virtual-endpoint.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-115-virtual-endpoint-provisioning.md"
    - "https://github.com/scarecrow173/ruviform/blob/main/docs/designs/virtual-audio-endpoint-provisioning-interface.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/762"
    - "https://github.com/scarecrow173/ruviform/issues/1001"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# ADR: Issue #115 virtual endpoint provisioning ownership

> Cross-repo link note: Ruviform 側文書への `blob/main` URL は、対応 PR merge 後の
> canonical location を示す。proposal 時点の main にはまだ存在しておらず、両 PR の統合後に
> 実在させる意図のリンクである。

## Status

Proposed — 2026-11-25 design/docs slice の decision record。実装済みまたは
承認済みの ADR ではない。後続 implementation slice の merge 時に status を
見直す。

## Context and Problem Statement

Synvelis PR #654 で consumer virtual-WASAPI EXT-L3 harness は main に入ったが、
現行開発 host には Ruviform Cable endpoint pair がなく、既存 evidence は
`NotRun — NOT_PROVISIONED` である。Ruviform の manual probe workflow は
driver build/test-sign/install/devnode/probe/cleanup を同一 ephemeral
`windows-2025` job で実施できることを示したが、その logic は workflow inline
monolith であり、Synvelis から step-level に再利用できない。

必要なのは、endpoint の lifetime が有効な同一 runner 上で、Synvelis の対象
SHA を provisioned endpoint で実行し、consumer harness の terminal RunMetadata
を evidence に残すことである。同時に、Ruviform の共通基盤責務と Synvelis の
consumer acceptance 責務を混ぜない必要がある。

## Decision Drivers

- devnode、driver store、cert、microphone consent、service restart は OS の
  runner-local state であり、job をまたいで共有できない。
- driver provisioning 契約は Ruviform driver implementation と一緒に進化する。
- consumer acceptance は Synvelis の独立 oracle と RunMetadata 解釈で判定する。
  upstream Pass や job green を転用できない。
- self-signed cert は現行 workflow で 2 日失効、artifact retention は 30 日と
  短命であり、artifact package を恒久 interface にできない。
- Synvelis の Ruviform 依存は manifest default branch 追従だが、再現性の
  guard は `Cargo.lock` の single resolved revision にある。
- CI write 権限や cross-repo secret を増やさない方がよい。

## Considered Options

- **Option A（採用）: Ruviform-owned script pair + Synvelis-owned same-job
  orchestration**。Ruviform が provision/unprovision script pair を所有し、
  Synvelis manual workflow が同一 `windows-2025` job の step から呼ぶ。
- **Option B: install logic を Synvelis workflow へ複写**。却下。driver build、
  signing、devnode、consent、cleanup の不変条件を Ruviform の driver 実装から
  切り離して二重保守する。
- **Option C: Ruviform workflow 内で Synvelis consumer test を実行**。却下。
  Ruviform が特定 consumer の checkout・受入 oracle・evidence 解釈に依存し、
  共通基盤から consumer への依存方向を逆転させる。
- **Option D: reusable workflow**。却下。`workflow_call` callee は別 job であり、
  provisioned devnode/endpoint を同一 runner OS state として consumer job と
  共有できない。callee 内で consumer test まで実行すれば Option C と同じ逆依存
  になる。
- **Option E: signed driver package artifact を受け渡す**。却下。cert は 2 日
  失効、artifact は期限つきで、cross-repo download token/trust、source revision
  照合、signature 再検証を consumer 側の恒久運用に追加する。acceptance の
  再現性より debug transfer に適する。
- **Option F: external/self-hosted runner を先に調達**。却下。rf#762 の open
  scope として残るが、Ruviform 独自 driver を GitHub-hosted ephemeral runner
  で試す本件の必要条件ではない。外部 runner 調達待ちに consumer EXT-L3 実走を
  遅らせる理由にはならない。

## Decision Outcome

Chosen option: **Option A**。

Ruviform は step-level provision/unprovision primitive を所有する。初期
interface は composite action ではなく PowerShell script pair とし、machine-
readable state JSON を契約の中心に置く。primitive の提供範囲は driver build/
test-sign/install/devnode/mic consent/probe/cleanup である。

Synvelis は manual consumer workflow を所有し、同一 `windows-2025` job で
provision → readiness 確認 → `mise run test-recording-virtual-endpoint` →
pre-cleanup metadata/artifact upload → `if: always()` cleanup → post-cleanup
cleanup artifact upload を orchestrate する。consumer evidence artifact は cleanup
より前に確定させ、cleanup artifact と分離する。source revision は Synvelis
`Cargo.lock` resolved revision に pin し、checkout 後に検証した Synvelis SHA を
`SYNVELIS_RUN_SHA` へ注入する。

acceptance verdict は consumer harness の terminal RunMetadata を権威とする。
Ruviform の upstream Pass、workflow green、artifact 存在、cleanup 成功を Pass
に転用しない。evidence は CI artifact を一次記録とし、通常の PR で
`evidence/sv115/` へ転記する。

## Consequences

### Positive

- driver provisioning の変更が Ruviform の implementation と一緒に保守される。
- Synvelis は独立 oracle と target SHA の証拠を失わずに endpoint 環境を再利用
  できる。
- endpoint の lifetime を同一 job に閉じ込められるため、reusable workflow の
  runner isolation 問題を避けられる。
- public repo 期は anonymous HTTPS を第一経路にでき、新しい write credential
  を増やさない。

### Negative

- consumer job 内で WDK install/build/driver build を行うため実行時間が長い。
- primitive extraction merge 後も、Synvelis `Cargo.lock` が古い revision を
  指す間は新 primitive を使えない。lockfile の前進が別の依存更新として必要に
  なる。
- script pair を呼ぶ consumer workflow は state JSON parse の責務を持つ。
  composite action に隠蔽するより初期 workflow の記述が増える。
- mic consent registry と audio service restart を伴うため、ephemeral runner
  限定と cleanup/restore 契約を外すと運用リスクが大きい。

### Neutral

- rf#762 は close せず narrow/partial supersede として扱う。Ruviform 独自
  driver の GitHub-hosted 経路は本設計で代替できるが、external driver/license/
  persistent self-hosted runner の検討は残る。
- 本 ADR は docs index を更新しない。index は統合時に整備する。

## Revisit Conditions

- GitHub-hosted runner が testsigned driver load を許可しなくなった場合。
- WDK/driver build を含む manual job が実測で実用的な時間に収まらない場合。
- primitive interface が複数 consumer から使われ、state JSON parse の重複が
  composite action wrapper の追加コストを上回る場合。
- Ruviform の private 化または credential model の変更で anonymous checkout
  が使えなくなった場合。
- provisioned consumer run が安定し、scheduled/opt-in lane または required
  evidence gate への接続を別 ADR で検討する場合。

## Confirmation

本 design/docs slice では implementation を行わない。確認対象は、5 文書が
acceptance 7 項目・open questions・rf#762 関係・後続 2 slice 分割をカバーし、
指定 prek と lease 確認が実行されることである。provisioned endpoint での
consumer EXT-L3 Pass/Fail/NotRun は後続 implementation slice の manual run で
確認する。
