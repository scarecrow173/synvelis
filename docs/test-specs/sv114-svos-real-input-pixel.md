---
id: "TSPEC-SV114SVOSREALINPUT00001"
type: "test-spec"
status: "approved"
title: "SV-OS-01 real input/pixel vertical slice verification matrix (synvelis#114)"
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
    - "docs/specs/sv114-svos-real-input-pixel.md"
    - "docs/designs/sv114-svos-real-input-pixel.md"
    - "evidence/sv114/sv-os-real-input-pixel-verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv114-svos-real-input-pixel.md"
    - "docs/designs/sv114-svos-real-input-pixel.md"
  derived-by: []
  verifies:
    - "docs/specs/sv114-svos-real-input-pixel.md"
  verified-by: []
  references: []
  defers: []
---

# SV-OS-01 real input/pixel vertical slice verification matrix

## Rules

- SVRIP-01: real-only augmentation は `real-os-scenario` featureの `sv_os_real` targetで compile/lint できる。
- SVRIP-02: input opが `Injected` でない場合は verdictを `Unsupported` または `NotRun` にし、Passにしない。
- SVRIP-03: captureが `Captured` かつ `pixel_count > 0` の場合だけ pixel Pass候補とし、`Unsupported`/`NotRun`/未知 outcomeを Passにしない。
- SVRIP-04: Fake/GpuiSimulated laneの semantic verdict、matrix projection、catalog required capabilitiesを弱めず、GUI-L2へ昇格させない。
- SVRIP-05: SV-OS-01以外の real input/pixel capabilityは Deferredのまま、native dockは NotApplicableのままとする。
- SVRIP-06: Workerは interactive desktopで実 windowを開く real laneを実行しない。Worker evidenceは compile/static/headless verificationに限る。
- SVRIP-07: canonical verificationは mise task、cargo直接実行は補助証拠と明記する。

## Worker verification matrix

| Lane / command | Kind | Expected evidence |
| --- | --- | --- |
| `mise run fmt-check` | canonical format | exit 0。長い worktree pathで Windows process command limitに達する場合は、scope内fileへの rustfmt/`cargo fmt -p synvelis-app -- --check` を補助証拠として記録する |
| `mise run check synvelis-app` | canonical static | exit 0 |
| `mise run lint synvelis-app` | canonical lint | exit 0 |
| `mise run test synvelis-app` | canonical headless | Fake catalog testsを含め exit 0 |
| `mise run test-app-gpui` | canonical gpui-test | GpuiSimulated catalog testsを含め exit 0 |
| `mise run check synvelis-app --features real-os-scenario` | canonical real-lane static | exit 0。real windowは開かない |
| `mise run lint synvelis-app --features real-os-scenario` | canonical real-lane lint | exit 0。real windowは開かない |

## Coordinator interactive gate（Workerでは実行しない）

interactive Windows desktop上で、必要な OS automation 権限と desktop sessionがあることを確認したうえで、次を実行する。

```sh
SYNVELIS_SV114_REAL_EVIDENCE=evidence/sv114/sv-os-real-run.jsonl \
mise exec -- cargo test -p synvelis-app --locked --features real-os-scenario --test sv_os_real -- --nocapture
```

PowerShell/cmdでは環境変数の設定方法を合わせる。確認点:

1. SV-OS-01 rowの `observed.real_input_pixel.ops.pointer-move/mouse-down/mouse-up` がすべて `tag=injected`。
2. `command.actions` が `project/save` 1件と一致し、receiptが harness-posted `Applied` として記録される。
3. `settled_observation` が settle後の component stateを持ち、`product_state_oracle` が独立 service oracleを含む。
4. `capture.outcome=captured`、backend/width/height/scale factor/pixel countがあり、`pixel_count > 0`。
5. SV-OS-01 verdictが Passの場合でも、detailが IME/DPI/monitor/native dock/production wiringを claimしていない。
6. input/captureが `Unsupported`/`NotRun`/`EnvironmentBlocked` の場合は、該当 verdictが Passに昇格していない。
7. SV-OS-02〜05は既存 semantic/focus/geometry laneとして記録され、input/pixel augmentationを主張しない。

## Fail/NotRun の扱い

scope内の fmt/lint/test失敗は修正して再検証する。interactive desktop不在、OS automation denied、backend capture unsupportedは Failではなく環境/能力別の `NotRun`/`Unsupported` として記録する。一方、injection済みで command/receipt/pixel observationが欠ける場合は製品またはharness契約違反として Failであり、`environment_blocked`へ書き換えない。
