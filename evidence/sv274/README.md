# SV-SCRIPT-04 Script Editor — Case 台帳 (Issue #274 headless slice)

作成: 2026-11-14 / branch: `scarecrow173/sv274-script4`
対象: `crates/synvelis-automation/src/analyzer.rs`・
`crates/synvelis-workspace/src/script_editor.rs`・
`crates/synvelis-app/src/script_editor.rs`・
`crates/synvelis-theme/src/status.rs` と対応 test

語彙は POLICIES.md §2/§3 に従う。**headless/fake driver の Pass は
実機 Pass ではない**。実機未検証項目は Case ID 付きで
NotRun/Deferred として記録する。本スライスは
`crates/synvelis-script`・`crates/synvelis-gateway`・
`crates/synvelis-cli`・`assets/` を変更しない（read-only
consumer / 別リース / 範囲外）。

検証環境: Windows 11 (x86_64)、rustc は `mise` 管理
toolchain。canonical 検証は `mise run` レーン（PR 本文に記録）。

## 1. 受入条件 → Case → test → 結果

| Issue #274 受入行 | Case | 検証内容 | test / 証跡 | Expected | Observed | Result |
| --- | --- | --- | --- | --- | --- | --- |
| Synvelis公開CapabilityのLuau補完/型診断/構文診断/Unknown APIが実Editorで見える | SV274-AC-01 | Registry 解決 + port-level adapter が injected `DefinitionInjection` のみから unknown-api/wrong-arity/構文診断・prefix 補完を返し、document が行として保持、ViewSpec が tables として投影 | `synvelis-automation/tests/script_analyzer.rs` 全 12 件、`synvelis-workspace/tests/script_editor.rs` の `accepted_result_projects_rows_and_ready_status`、`synvelis-app/tests/svscript04_script_editor.rs` の `populated_state_projects_status_source_and_tables` | typed `Diagnostic`/`Completion` が `source_revision` 付きで返り、Hidden/Retired/Disabled は診断・補完のいずれにも「存在」として現れない | 28 headless tests pass。`synvelisHiddenOp`/`synvelisLegacyOp`/`synvelisOldOp`/`synvelisWrongVer` は unknown-api、補完候補に出ない | **Pass（headless）**。実 Editor 描画は SV274-NR-01 Deferred |
| Registryの更新・Version変更で生成型が更新され、Hidden/Retiredの漏洩やstale補完がない | SV274-AC-02 | model/stub digest pin と `SourceRevision` の二重照合で digest drift・revision 遅延を `StaleResult` reject。verdict 投影は `ScriptApiModel` 由来のみで Hidden 非投影 | `definition_digest_drift_rejects_in_flight_result`・`delayed_result_is_rejected_as_stale`・`api_projection_shows_verdicts_and_never_leaks_hidden`・`analyze_source_never_becomes_execution_authority` | digest drift / stale revision は破棄、verdict は invocable/disabled/retired/unsupported-version を区別、hiddenOp は行に出ない | 全 case pass。registry snapshot 更新→新 `DefinitionInjection`→pin 差分で reject を確認 | **Pass（headless）** |
| Source revision 遅延応答/IME/2Window/Focus/Undo/DPIをGUI Fixtureと実OS Gateで区別して検証する | SV274-AC-03 | 遅延非同期結果の consumer-side stale reject（F-14）を headless fixture で検証。IME/2Window/Focus/Undo/DPI/pixel は実 OS Gate | `delayed_result_is_rejected_as_stale`・`unsolicited_result_is_rejected`・`edit_bumps_revision_and_marks_dirty` | revision 不一致・非要求結果は `StaleResult`/`UnsolicitedResult` で破棄、edit で Dirty | 全 case pass（headless）。IME/2Window/Focus/Undo/DPI/pixel は **NotRun**（SV274-NR-01） | **Pass（headless）/ NotRun（実 OS Gate）** |
| 未提供AnalyzerはUnsupportedと表示し、Runtime自体の起動可否とは区別する | SV274-AC-04 | 未登録言語→`Unavailable`、未 advertise capability→`Unsupported`、contract version 不一致→`VersionMismatch` を registry が typed error で返し、document status が `Unavailable`/`Unsupported`/`Rejected` を区別表示 | `registry_unregistered_language_is_unavailable`・`registry_rejects_unadvertised_capabilities`・`registry_rejects_contract_version_mismatch`・`analyze_source_unregistered_language_is_typed_unavailable`・`apply_error_maps_typed_vocabulary` | silent skip/成功扱いなし。status は runtime 起動可否と別軸 | 全 case pass。Rhai/YAML 等未登録言語は `AnalyzerError::Unavailable`、`Unsupported{capability}` は status へ写像 | **Pass（headless）** |
| GPUI型/Native FFI/VM型を共通ViewSpecへ漏らさず、外部luau-lsp常駐・追加インストールなしで動く | SV274-AC-05 | `synvelis-app::script_editor` の公開 signature が `ruviform_ui::view` + `synvelis_workspace` 型のみ。analyzer は in-process port 契約 | `synvelis-app/tests/svscript04_script_editor.rs` 全 8 件 + `cargo check` の依存木確認 | GPUI/OS/Native/VM 型が spec/read model に出ない、外部プロセス不要 | 8 tests pass。spec は `ComponentSpec::{Label,Table,Button,Panel}` のみ、mount は `ViewHost`+`AttachEpochAllocator` | **Pass（headless）**。実 FFI bridge 結合は SV274-NR-02 Deferred |
| Mock・Real GPUI・実OSのEvidenceを分離しNotRunをPassとしない | SV274-AC-06 | 本台帳で headless Pass と実機 NotRun/Deferred を Case ID で分離 | 本ファイル §1/§3 | fake/headless と real GPUI/実 OS が別行・別 Case ID | 分離記録済み | **Pass（headless）** |

## 2. Case 詳細（headless 実施分）

| Case | crate::test | 状態 | 備考 |
| --- | --- | --- | --- |
| SV274-T-01 | `synvelis-automation::script_analyzer` 全 12 test | Pass | registry 解決行列・Luau adapter 診断/補完・headless 入口・非実行権限 |
| SV274-T-02 | `synvelis-workspace::script_editor` 全 8 test | Pass | document lifecycle・stale/digest reject・verdict 投影・ViewState serde |
| SV274-T-03 | `synvelis-app::svscript04_script_editor` 全 8 test | Pass | decode matrix・fail-closed spec・mount smoke |
| SV274-T-04 | `synvelis-theme` status module test | Pass | 新規 `StatusTone` token path・label 写像 |
| SV274-T-05 | `cargo fmt --all --check` + `cargo clippy --locked --all-targets`（対象 4 crate） | Pass | fmt 適用済み、warning 0（canonical `mise run` 結果は PR 本文） |

## 3. NotRun / Deferred 対応表

| Case | 内容 | 状態 | Required/Deferred | 後続 gate / 必要ファイル |
| --- | --- | --- | --- | --- |
| SV274-NR-01 | 実 GPUI Script Editor（テキスト編集 widget・IME・2Window・Focus・Undo・DPI・pixel）と実 OS 受入 | **NotRun** | Required（GUI-L2）だが本スライスは headless-first | synvelis#114/#267。`ComponentSpec` にテキスト編集 kind が無いため real lane で Ruviform 側 component 追加が前提になり得る |
| SV274-NR-02 | 実 Luau.Analysis Native Bridge（ruviform#560）結合・mlua 同一バイナリ・3OS Native Link/配布・外部 luau-lsp 非依存の実配布検証 | **NotRun** | Deferred（ruviform#560/#558/#559 lane gate） | ruviform 側 PoC の製品統合 slice。`LuauPortAnalyzer` の `AnalyzeRequest`/`AnalyzeResult`/`AnalyzerError` 契約はそのまま bridge 実装に使える形 |
| SV274-NR-03 | `synvelis-cli` への Script Analyzer verb 配線（headless CLI 入口の製品配線） | **NotRun** | Deferred（`synvelis-cli` は本タスク編集禁止） | 別 slice。`analyze_source`/`analyze_with_injection` は GUI 非依存の公開 API として提供済み |
| SV274-NR-04 | composition root（`SynvelisApp`/`NativeShell`）への Script Editor mount 配線・dock open 導線 | **NotRun** | Deferred（`crates/synvelis-app` の shell/composition は編集範囲外） | `mount_script_editor_view` + `decode_script_editor_command` + `ViewState.script_editor` + `ViewType::ScriptEditor` の呼出側配線 slice |
| SV274-NR-05 | `assets/themes/synvelis-{dark,light}.dtcg.json` への新規 `color.status-*` token 値追加 | **NotRun** | Deferred（`assets/` は repo root、編集範囲外） | `StatusTone::token_path` の新規 6 path に対応する `$value` を product document に追加する slice |
| SV274-NR-06 | `gpui-test` feature の GPUI Semantic scenario への Script Editor surface 追加 | **NotRun** | Deferred（spec builder/mount は headless test で検証済み） | `mise run check-app-gpui`/`lint-app-gpui`/`test-app-gpui` が必要になる後続 slice |
| SV274-NR-07 | Rhai analyzer（および Yaml workflow 言語）の実装 | **NotRun** | Deferred（本スライスは Luau port-level adapter のみ。未登録言語は typed `Unavailable` で fail-closed） | 言語別 analyzer slice。`AnalyzerRegistry` は登録のみで解決ロジックは言語非依存 |

## 4. 補足

- `synvelis-app` は `synvelis-gateway`/`synvelis-script` に
  依存しないため、app 層 test は decode/spec/mount のみを対象とし、
  projection/stale reject の oracle は `synvelis-workspace` 側
  test が `synvelis-script` 型を使って担保する（依存方向の維持）。
- analyzer 成功は実行権限を生まない: `ScriptApiModel` の
  `is_invocable` verdict と stub 内容は analysis 結果と独立に
  保たれ、Hidden/Retired/Disabled/Planned/Unknown/非対応
  version の API は診断・補完・verdict 行のいずれにも
  「呼べる」形では現れない。
- `StatusTone::of_label` が裸の `"information"`/`"warning"`/
  `"error"` を diagnostic tone へ写す変更は、同名 generic label
  の既存利用を grep で確認した上で追加（衝突なし）。
