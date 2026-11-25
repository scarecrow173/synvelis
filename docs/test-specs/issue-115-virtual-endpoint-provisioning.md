---
id: "TSPEC-sv115VirtualEndpointProvisioning"
type: "test-spec"
status: "proposed"
title: "Issue #115 RCD-01 Virtual Endpoint Provisioning 検証仕様"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "docs/test-specs/issue-115-virtual-endpoint-provisioning.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/issue-115-virtual-endpoint.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-115-virtual-endpoint-provisioning.md"
    - "docs/designs/issue-115-virtual-endpoint-provisioning.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-115-virtual-endpoint-provisioning.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-115-virtual-endpoint-provisioning.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/blob/main/docs/designs/virtual-audio-endpoint-provisioning-interface.md"
  defers: []
  deferred-by: []
---

# Issue #115 RCD-01 Virtual Endpoint Provisioning 検証仕様

## Purpose

`docs/specs/issue-115-virtual-endpoint-provisioning.md` と
`docs/designs/issue-115-virtual-endpoint-provisioning.md` の設計契約を、後続
implementation slice が満たすべき検証規則へ落とす正本である。

現時点では provisioning primitive も consumer workflow も実装されていない。
この文書に書く lane や case は実装済み test を装わない。検証は後続
implementation slice で初めて実行可能になる。

## Rules

1. Pass の権威は consumer harness の terminal RunMetadata に限定する。job
   conclusion、provision script exit code、artifact の存在、upstream Ruviform
   Pass、cleanup 成功は Pass に転用しない。
2. provision state `ok` と endpoint pair 候補が無い run では consumer harness
   を実行しない。実行しなかった場合の acceptance は Pass でも Fail でもなく、
   未達の NotRun として扱う。
3. provision state `ok` で endpoint pair 候補があるのに consumer harness が
   `NOT_PROVISIONED` を返した場合は、provisioning readiness か harness の
   endpoint 判定に矛盾がある。証拠を acceptance に使わず、原因を切り分ける。
4. Ruviform candidate pair が存在し、open/start/callback/capture/oracle/commit/
   save/reload のいずれかが失敗した場合は consumer case verdict を Fail とする。
5. harness process crash、metadata 欠損、artifact 欠損、SHA/provenance mismatch
   は consumer case verdict を確定できない。run は証拠不備として扱い、既存の
   Pass/Fail/NotRun を上書きしない。
6. cleanup status は独立して記録する。cleanup error を consumer harness の
   Fail に混同しないが、runner residue として運用失敗には記録する。
7. CI は repository へ evidence を direct commit しない。artifact を一次記録、
   通常 PR での `evidence/sv115/` 転記を canonical evidence とする。

## Guarantees

後続 implementation は次を保証しなければならない。

- provision、consumer test、metadata/artifact upload、cleanup が同一
  `windows-2025` job 内で実行される。
- test crash・test failure・artifact upload 失敗の後でも cleanup step が
  `if: always()` で実行される。
- `SYNVELIS_RUN_REPO` / `SYNVELIS_RUN_SHA` は checkout 後検証済みの値から
  注入され、手入力や upstream metadata をそのまま転記しない。
- `ruviform_lock_rev` と `ruviform_checkout_rev` が一致しない run は
  acceptance evidence に使えない。
- partial provisioning state も cleanup 対象になり、devnode/driver/cert/
  consent/service の residue が cleanup state へ記録される。
- artifact に terminal RunMetadata、provision state、cleanup state、driver
  package manifest、stdout/stderr、run URL 再現に必要な runner/tool 情報が
  含まれる。

## Case matrix

| ID | 条件 | 実行方法（後続実装） | 期待判定 | acceptance 解釈 |
| --- | --- | --- | --- | --- |
| VP-S1 | workflow YAML・権限・step order | `mise run lint-actions`、静的 review | provision→test→metadata/artifact→`if: always()` cleanup が同一 job | 未実装時は NotRun |
| VP-S2 | Ruviform lock rev extraction | workflow dry run または unit script test | `Cargo.lock` から single rev を抽出。零/複数/malformed は fail-closed | 不成立なら acceptance evidence 不可 |
| VP-P1 | provision 成功 | manual workflow run | provision terminal status `ok`、endpoint pair candidate、state/artifact manifest あり | test 実行条件成立。これだけでは Pass ではない |
| VP-P2 | provision absent/blocked/error | manual workflow run | consumer test は実行せず、provisioning evidence を upload | acceptance 未達。consumer Fail にしない |
| VP-C1 | endpoint 不在環境で harness 単独実行 | `mise run test-recording-virtual-endpoint` | terminal metadata `result=not-run` / `NOT_PROVISIONED`、lane exit 0 | NotRun。現行 dev host の挙動 |
| VP-C2 | provisioned endpoint で harness Pass | manual workflow run | terminal metadata `result=pass`、repo/sha/level/environment/oracle/evidence が一致 | EXT-L3 Pass evidence 候補 |
| VP-C3 | candidate pair ありで oracle/open/stream/commit/reload 失敗 | manual workflow run | terminal metadata `result=fail` または harness 非零終了 | consumer Fail。再実行で上書きしない |
| VP-C4 | harness crash で terminal metadata 欠損 | fault ではなく実事故として観測 | artifact に log は残るが case verdict 不明 | 証拠不備。Pass/Fail/NotRun に転用しない |
| VP-C5 | `SYNVELIS_RUN_SHA` と checkout HEAD 不一致 | static/fault review | harness 実行前に fail-closed | run invalid 相当の証拠不備 |
| VP-C6 | `ruviform_lock_rev` と checkout rev 不一致 | static/fault review | acceptance evidence 不可を明示 | 証拠不備。product Fail にしない |
| VP-X1 | test failure 後 cleanup | manual workflow run | cleanup step が実行され、devnode/driver/cert/consent residue が state に記録される | cleanup verdict は独立 |
| VP-X2 | cleanup partial/failure | manual workflow run または fault review | cleanup status `error`、residue detail、consumer verdict と別記録 | 運用失敗。consumer Pass/Fail を変えない |
| VP-E1 | artifact/evidence 転記 | provisioned run 後の通常 PR | `evidence/sv115/` に RunMetadata JSON・CI URL・revision・provisioning/cleanup record が残る | canonical evidence 完成 |
| VP-R1 | Ruviform manual probe 非回帰 | Ruviform parity manual run | 既存 stage vocabulary・artifact・fault cases が観測可能 | primitive extraction の acceptance |

## Pass evidence minimum

`VP-C2` を acceptance evidence に使うには、少なくとも次が同一 artifact または
同一 evidence 転記内で突合できる必要がある。

- terminal RunMetadata JSON本体。`repo=scarecrow173/synvelis`、対象 `sha`、
  `level=ext-l3`、`environment_kind=virtual-os-endpoint`、`oracle`、
  `result=pass`。
- CI run URL、artifact 名、artifact retention。
- provision state JSON。terminal status、source repo/revision、driver package
  manifest、cert thumbprint/notAfter、devnode instance id、endpoint pair
  candidate、mic consent record。
- cleanup state JSON。terminal status、residue/leftover、service restart 結果。
- `ruviform_lock_rev` と `ruviform_checkout_rev`。

## Local validation for this design/docs slice

本 slice は文書のみを変更する。実施する canonical validation は次である。

- `mise exec -- prek run --files docs/specs/issue-115-virtual-endpoint-provisioning.md docs/designs/issue-115-virtual-endpoint-provisioning.md docs/test-specs/issue-115-virtual-endpoint-provisioning.md docs/adr/issue-115-virtual-endpoint-provisioning-ownership.md`
- `git diff --name-only` / `git status --short` による lease 外差分の確認

これらは設計文書の構文・整形と lease の確認であり、provisioning の実走証拠
ではない。workflow actionlint、cargo lane、manual provisioned run は後続
implementation slice の対象である。
