# Acceptance Ledger — synvelis#114 受入条件 8 行の分類

Date: 2026-11-14. Slice: SV-GUI-05 real-OS case-definition freeze +
acceptance ledger (docs/evidence only — **no real-OS, real-GUI, or
native-plugin lane was executed**). Every row below is transcribed
verbatim from Issue #114「受入条件」(2026-11-13 現行正本) and
classified into exactly one of:

- **merged-evidence pointer** — evidence already merged on main;
  referenced by path only, not re-asserted or upgraded here.
- **NotRun** — required evidence does not exist yet; this slice does
  not produce it.
- **external Owner** — acceptance depends on another repo/issue/CI
  lane outside this slice's authority.

No row is `Pass`/`accepted` as a result of this slice
(test-spec Rule SVT-FRZ-04). Case-level freeze:
`evidence/sv114/sv-os-case-definitions.md`.

## 受入条件（Issue #114 現行正本からの転記）

- AC1: SVT-01〜09のProject/Transport/Undo/Save/Job/Window/Plugin Panelの製品State/ReceiptをFake Driver + GPUI Test seam/Headless UI Scenarioで検証し、実Service/Mock Serviceの不変条件を区別する。
- AC2: Windows/macOS/Linux既存CIのNative Build/Link、GPUI/Component/Headless UI/RT/Project/CLIの対象FeatureとCase別Run URLを照合する。
- AC3: 2Window Ownership/Focus/Close/Undo/Save、stale generation、Plugin Editor Session/NativeSurfaceSlotのL1契約はRuviform #243/#313/#388/#389のFake/Fixtureと独立製品Oracleで比較する。
- AC4: Ruviform #219/#294・SV #28の候補SHAに紐づくGUI/PCM/RT/Project/AL f64の独立結果を記録する。
- AC5: 実GPUI Window/Input/Pixel、IME/DPI/Monitor、実VST3/CLAP/AU Dock/実Plugin hang（PD-01〜07）、実Audio/MIDI等のL2/L3は現行DoDから除外。利用OS/Pluginが具体化した時だけ#176と共にActivationし、Fake Passを実GUI/Native Dock Passに昇格させない。
- AC6: Windows L2/L3（利用中の機能のみ）— WindowsでSynvelis GUIを使用する場合、実GPUI WindowでProject/Transport/Undo/Save/2Window/Focus/ReceiptをL2実走する。実Plugin GUI Native Dockを使用する場合は#176とPD-01〜07の必要Format/操作のみL3。既知のDComp不表示は当該能力の未達。
- AC7: macOS/Linux L1のみ — 既存CIのNative Build/Link、Headless/RT/必要Featureと製品Oracleを対象SHAで検証し、L2/L3は利用時までDeferred。
- AC8: 非回帰/証拠 — Windowsでも未使用Device/Plugin形式/Providerは一律に要求しない。必要な実走がNotRun/Failなら機能単位で未受入。各OS/Owner/CI/実機証拠を分離する。

## Ledger

| AC | 分類 | 根拠・pointer / Owner / pending |
| --- | --- | --- |
| AC1 | merged-evidence pointer | `evidence/sv114/svt-headless-l1.md`（SVT-01〜09 被覆 map・実 Service/Mock 不変条件の区別）・`crates/synvelis-app/tests/svt_product_e2e.rs`（10 tests）・`crates/synvelis-app/tests/svt_product_e2e_gpui.rs`（`GpuiSimulated`）。Headless L1 範囲の証拠は merge 済み。real-OS 側の残件は AC6/SV-OS-01〜05 が別途 `NotRun` で保持。 |
| AC2 | NotRun（pending: Coordinator CI 証拠工程） | Windows/macOS/Linux 既存 CI の Native Build/Link・対象 Feature・Case 別 Run URL 照合は、merge 後対象 SHA の CI 証拠として未収集。`l1-contract-comparison.md` §5 が本照合を Coordinator の証拠工程と明記。Job green のみでは個別 Case の Pass を示さない（POLICIES.md §3）。関連: `.github/workflows/ci.yml`・`daily-ci.yml`、Issue #377（`docs/specs/ci-os-l1-native-gpui.md`、gpui-test 3OS OS-L1 補完・status proposed）。 |
| AC3 | merged-evidence pointer | `evidence/sv114/l1-contract-comparison.md` — 2Window Ownership/Focus/Close/Undo/Save・stale generation/attach epoch・Plugin Editor Session/NativeSurfaceSlot の L1 契約を Ruviform #243/#313/#388/#389 の Fake/Fixture と独立製品 Oracle で比較済み（merge 済み）。同書 §3 は plugin surface を `Unsupported`（製品層で未配線）と正直に記録しており、本 ledger はそれを昇格させない。実 OS/実 plugin 受入は外部 Owner（ruviform#313/#388/#389、synvelis#176）に残る。 |
| AC4 | 外部 Owner + NotRun | Ruviform #219/#294・Synvelis #28 の候補 SHA に紐づく GUI/PCM/RT/Project/AL f64 の独立結果は、Consumer 横断 CI 証拠の収集が前提で本スライスでは未実施。Owner: ruviform#219/#294（cross-consumer gate）、synvelis#28（共通基盤追従）、対象 SHA の CI（daily/full）。 |
| AC5 | NotRun（Deferred — 外部 Owner: synvelis#176 / ruviform#313/#314/#388/#389） | 実 GPUI Window/Input/Pixel・IME/DPI/Monitor・実 VST3/CLAP/AU Dock・実 Plugin hang（PD-01〜07）・実 Audio/MIDI の L2/L3 は現行 DoD から除外済みであり、本スライスも実行しない。Activation 条件（利用 OS/Plugin の具体化 + #176）は `sv-os-case-definitions.md` SV-OS-05/06/09 と spec の Deferred Design Concerns に固定。Fake Pass を実 GUI/Native Dock Pass に昇格させない規則は test-spec SVT-FRZ-02/03 が保持。 |
| AC6 | NotRun | Windows L2（実 GPUI Window での Project/Transport/Undo/Save/2Window/Focus/Receipt）と L3（#176・PD-01〜07 の必要 Format/操作のみ）は、実 OS Scenario Runner・opt-in mise 入口・実機/CI 実行環境が未整備のため未実走。ケース定義は `sv-os-case-definitions.md` SV-OS-01〜05・09 に freeze 済み。既知の DComp 不表示は当該 capability の未達として扱う（Issue #114 AC6 本文どおり）。Owner: synvelis#114（実施）/ ruviform#313（共通 OS Input Driver）。 |
| AC7 | NotRun（pending: 対象 SHA の CI 証拠） | macOS/Linux L1（既存 CI の Native Build/Link・Headless/RT/必要 Feature・製品 Oracle の対象 SHA 検証）は daily/full CI の実走証拠待ち。L2/L3 は利用要件まで Deferred（POLICIES.md §2）。gpui-test feature の 3OS OS-L1 補完は Issue #377（`docs/specs/ci-os-l1-native-gpui.md`）が別正本。Owner: CI lane + Coordinator 証拠工程。 |
| AC8 | NotRun（証拠 separation は本 ledger + case definitions で freeze） | 「未使用 Device/Plugin 形式/Provider を一律要求しない」「必要な実走が NotRun/Fail なら機能単位で未受入」「OS/Owner/CI/実機証拠を分離する」は `sv-os-case-definitions.md`（評価列分離・NotApplicable の扱い）と本 ledger の分類規則として固定済み。各 OS/Owner/CI/実機の実測証拠自体は AC2/AC4/AC6/AC7 の pending に従属し `NotRun` のまま。 |

## 集計

- merged-evidence pointer: AC1, AC3（2 行）
- NotRun: AC2, AC5, AC6, AC7, AC8（5 行）
- 外部 Owner（+ NotRun）: AC4（1 行）
- `Pass`/`accepted`: 0 行（本スライスは分類の freeze のみ）

## Pending / 後続工程

- merge 後対象 SHA の CI Run URL・Case 別照合 → Coordinator の
  証拠工程（`l1-contract-comparison.md` §5、AC2/AC7）。
- 実 OS Scenario Runner / opt-in mise 入口の整備 → 実 OS/Plugin
  が具体化した時（AC5/AC6、synvelis#176・ruviform#313/#314）。
- SV-OS-09 の plugin binary/format/SHA・PD-01〜07 展開 →
  synvelis#176。
- SVT-10（実 Backend E2E）→ 実 Backend 準備後の別受入。
- AUTX #267 AUTX-GUI-01〜06 との同一 Case ID 接続 → #267
  側の実施時に `sv-os-case-definitions.md` の SV-OS-03/05/06
  と照合。
