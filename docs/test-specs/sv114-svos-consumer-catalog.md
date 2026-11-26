---
id: "TSPEC-SV114SVOSCONSUMER000001"
type: "test-spec"
status: "approved"
title: "SV-OS consumer catalog verification matrix (synvelis#114)"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
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
  related:
    - "docs/specs/sv114-svos-consumer-catalog.md"
    - "docs/designs/sv114-svos-consumer-catalog.md"
    - "evidence/sv114/sv-os-consumer-catalog.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv114-svos-consumer-catalog.md"
    - "docs/designs/sv114-svos-consumer-catalog.md"
  derived-by: []
  verifies:
    - "docs/specs/sv114-svos-consumer-catalog.md"
  verified-by: []
  references: []
  defers: []
---

# SV-OS consumer catalog verification matrix

## Rules

- SVC-01: SV-OS-01〜05 の executable entries が catalog declarationに存在し、case id/scenario id/required/spec extras/run functionが追跡できる。
- SVC-02: Fake laneとGpuiSimulated laneは同じ entriesを実行し、driver/capabilityを正直に記録する。
- SVC-03: matrix projection は declarationだけを使い、実行や verdict昇格をしない。
- SVC-04: real laneは interactive preflightに fail-closedし、利用不可なら全件NotRunとする。
- SVC-05: real lane成功時もsemantic Command/Receipt/view-state証拠とphysical OS input/pixel/IME/native dock証拠を分離する。
- SVC-06: SV-OS-06〜09 executable case、SV-OS-09 native dock、PD-01〜07は追加しない。
- SVC-07: canonical verificationはmise task、cargo直接実行は補助証拠と明記する。

## Matrix

| Lane / command | Kind | Expected evidence |
| --- | --- | --- |
| `mise run test synvelis-app` | canonical headless | default target内で Fake catalog testsが Pass |
| `mise run test-app-gpui` | canonical gpui-test | GpuiSimulated catalog testsが Pass |
| `mise exec -- cargo test -p synvelis-app --features real-os-scenario --test sv_os_real` | opt-in auxiliary | interactive時: RealOs rows。非interactive時: 全件NotRun |
| `mise run fmt-check` / `check` / `lint` / `build` | canonical static/build | exit 0 |
| `mise run check-app-native` / `lint-app-native` | canonical native | exit 0 |
| `mise run check-app-gpui` / `lint-app-gpui` | canonical gpui static | exit 0 |
| `mise run check-headless-deps` / `check-ruviform-rev` | canonical dependency/pin guard | exit 0。本 slice の net diff は pin/Cargo.lock を変更しない（main 取込分は upstream 差分） |
| `mise exec -- prek run --all-files` | canonical hooks | exit 0 |

## Case verdict matrix

| Case | Headless Fake | GPUI Simulated | Real OS | Native Plugin |
| --- | --- | --- | --- | --- |
| SV-OS-01 | semantic Passを許可 | semantic Passを許可 | real window上のsemantic verdict。full L2 acceptanceではない | NotApplicable |
| SV-OS-02 | semantic Passを許可 | semantic Passを許可 | 同上 | NotApplicable |
| SV-OS-03 | semantic Passを許可 | semantic Passを許可 | 同上 | NotApplicable |
| SV-OS-04 | semantic Passを許可 | semantic Passを許可 | 同上 | 実device出力は別EXT-L3 |
| SV-OS-05 | semantic Passを許可 | semantic Passを許可 | 2window+focus observation必須。focus未観測ならNotRun | NotApplicable |
| SV-OS-06〜09 | 本sliceでは追加しない | 同左 | 同左 | 同左 |

## Fail/NotRun の扱い

scope内のfmt/lint/test失敗は修正して再検証する。scope外・環境起因と確定したものだけ証拠つきでFail/NotRunとする。real laneのinteractive desktop不在は失敗ではなく `NotRun + environment_blocked`である。逆にreal sessionが成立しcase bodyが契約違反を示した場合はFailであり、environment_blockedへ書き換えない。
