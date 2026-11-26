---
id: "TSPEC-sv115VirtualEndpointProvisioning"
type: "test-spec"
status: "approved"
title: "Issue #115 RCD-01 Virtual Endpoint Provisioning 検証仕様"
created: "2026-11-25"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "docs/test-specs/issue-115-virtual-endpoint-provisioning.md"
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

> Cross-repo link note: Ruviform 側文書への `blob/main` URL は、対応 PR merge 後の
> canonical location を示す。proposal 時点の main にはまだ存在しておらず、両 PR の統合後に
> 実在させる意図のリンクである。

## Purpose

`docs/specs/issue-115-virtual-endpoint-provisioning.md` と
`docs/designs/issue-115-virtual-endpoint-provisioning.md` の設計契約を、後続
implementation slice が満たすべき検証規則へ落とす正本である。

2026-11-26 の Slice B implementation で consumer workflow は実装された。
初回 provisioned manual run は 2026-11-26 に実施され、run `45797309944`
attempt `1` は consumer Fail だった。詳細は
`evidence/sv115/issue-115-virtual-endpoint-provisioned-consumer-run.md` に記録する。
stereo preserve 修正後 rerun `45834341661` attempt `1` でも、stereo format は
観測できたが `output timestamp regressions: 9` により consumer Fail だった。
詳細は `evidence/sv115/issue-115-virtual-endpoint-stereo-consumer-rerun.md` に
記録する。この文書に書く provisioned case がすべて Pass したとは読まない。
static/local validation、初回 provisioned run、stereo preserve 修正後 rerun、
後続の canonical Pass run を分けて記録する。

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
5. harness process crash、metadata 欠損、consumer evidence artifact 欠損、
   SHA/provenance mismatch は consumer case verdict を確定できない。run は
   証拠不備として扱い、既存の Pass/Fail/NotRun を上書きしない。cleanup
   artifact の欠損は運用 evidence の欠損であり、この rule の consumer verdict
   不確定事由とは区別する。
6. cleanup status は独立して記録する。cleanup error を consumer harness の
   Fail に混同しないが、runner residue として運用失敗には記録する。
7. CI は repository へ evidence を direct commit しない。artifact を一次記録、
   通常 PR での `evidence/sv115/` 転記を canonical evidence とする。

## Guarantees

後続 implementation は次を保証しなければならない。

- provision、consumer test、pre-cleanup metadata/artifact upload、cleanup、
  post-cleanup cleanup artifact upload が同一 `windows-2025` job 内で実行される。
- consumer evidence artifact upload は cleanup より前に置かれ、test evidence が
  cleanup の成否・hang に依存しない。
- test crash・test failure・artifact upload 失敗の後でも cleanup step が
  `if: always()` で実行される。
- `SYNVELIS_RUN_REPO` / `SYNVELIS_RUN_SHA` は checkout 後検証済みの値から
  注入され、手入力や upstream metadata をそのまま転記しない。
- `ruviform_lock_rev` と `ruviform_checkout_rev` が一致しない run は
  acceptance evidence に使えない。
- partial provisioning state も cleanup 対象になり、devnode/driver/cert/
  consent/service の residue が cleanup state へ記録される。
- consumer evidence artifact に terminal RunMetadata、provision state、driver
  package manifest、stdout/stderr、run URL 再現に必要な runner/tool 情報が
  含まれる。cleanup state は別の post-cleanup artifact に含める。

## Case matrix

| ID | 条件 | 実行方法（後続実装） | 期待判定 | acceptance 解釈 |
| --- | --- | --- | --- | --- |
| VP-S1 | workflow YAML・権限・step order | `mise run lint-actions`、静的 review | provision→test→pre-cleanup metadata/artifact upload→`if: always()` cleanup→post-cleanup cleanup artifact upload が同一 job | workflow 未 merge / provisioned run 未実施時は NotRun |
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
| VP-E1 | artifact/evidence 転記 | provisioned run 後の通常 PR | `evidence/sv115/` に RunMetadata JSON・CI URL・revision・provisioning record が残る。cleanup record は取得できれば併記するが、欠損しても consumer verdict を変えない | canonical evidence 完成 |
| VP-R1 | Ruviform manual probe 非回帰 | Ruviform parity manual run | 既存 stage vocabulary・artifact・fault cases が観測可能 | primitive extraction の acceptance |

## Pass evidence minimum

`VP-C2` を acceptance evidence に使うには、少なくとも次が同一 run の
consumer evidence artifact または同一 evidence 転記内で突合できる必要がある。
cleanup artifact は運用確認の参照先であり、consumer Pass の成立条件にはしない。

- terminal RunMetadata JSON本体。`repo=scarecrow173/synvelis`、対象 `sha`、
  `level=ext-l3`、`environment_kind=virtual-os-endpoint`、`oracle`、
  `result=pass`。
- CI run URL、artifact 名、artifact retention。
- provision state JSON。terminal status、source repo/revision、driver package
  manifest、cert thumbprint/notAfter、devnode instance id、endpoint pair
  candidate、mic consent record。
- `ruviform_lock_rev` と `ruviform_checkout_rev`。

cleanup state JSON の terminal status、residue/leftover、service restart 結果は
運用 evidence として残す。cleanup artifact が欠損した場合はその事実を記録するが、
consumer harness の terminal RunMetadata が満たす Pass/Fail/NotRun を書き換えない。

## Local validation for the implementation foundation

Slice B implementation の canonical local validation は次である。

- `mise run lint-actions`
- `mise run fmt-check`
- `mise run check synvelis-recording`
- `mise run lint synvelis-recording`
- `mise run test synvelis-recording`
- `mise run lint-recording-real-input`
- `mise run test-recording-virtual-endpoint`
- `mise run check-ruviform-rev`
- `mise exec -- prek run --all-files`

`test-recording-virtual-endpoint` が endpoint 不在 host で exit 0 でも、terminal
RunMetadata が `result=not-run` / `NOT_PROVISIONED` なら EXT-L3 Pass ではない。
workflow YAML の static confirmation と local harness の NotRun は
`evidence/sv115/issue-115-virtual-endpoint-consumer-workflow-implementation.md`
に記録する。初回 provisioned manual run、artifact 検収、canonical evidence 転記は
`evidence/sv115/issue-115-virtual-endpoint-provisioned-consumer-run.md` に記録し、
stereo preserve 修正後 rerun は
`evidence/sv115/issue-115-virtual-endpoint-stereo-consumer-rerun.md` に記録する。
いずれの consumer Fail も Pass に読み替えない。
