# SV-SCRIPT-01 ScriptApiModel・Luau stub/binding — Case Ledger（synvelis#271 headless slice）

Date: 2026-11-14。Slice: 新規 crate `synvelis-script` —
Gateway `CapabilityRegistry`/`SchemaRegistry` を read-only
正本とする versioned `RegistrySnapshot` capture、
`ScriptSurfaceSpec`（script 名→exact `CapabilityRef`）、
決定的 `ScriptApiModel` build、同一 model digest からの
`BindingSet`（wire-shaped `InvocationDraft` lower）と
`LuauStub`（`.d.luau` 形型定義）の双生成、Analyzer port
（definition-injection consumer adapter）。**本 slice は
headless の compile-time/golden 契約のみ**を対象とし、実
Luau runtime 実行・実 Luau.Analysis native FFI/実測
（ruviform#560）・consumer 結合（#272/#273/#274）は
NotRun/Deferred である。headless の Pass を runtime/実機の
Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/271
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `scarecrow173/sv271-script1`（worktree `sv271-script1`） |
| OS / CPU / FS | Windows 11 / Intel Core i9-9900K / NTFS（C:） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target（worktree `target` リンク先） |
| Registry input | in-memory `CapabilityRegistry`/`SchemaRegistry` fixture（`tests/common/mod.rs`）と serialized snapshot fixture（`tests/fixtures/registry_snapshot.json`）の双方。live Gateway 非依存 |

## 判定語彙

- **Pass**: 当該 SHA で検証が exit 0 で完了し、対象
  test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 Analyzer 結合）。
- **Deferred**: 別 Issue/別リースが Owner の項目
  （ruviform#560 の実 Analyzer、#272/#273/#274 consumer）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-script` | Pass（exit 0） |
| `mise run check synvelis-gateway` | Pass（exit 0） |
| `mise run lint synvelis-script` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-gateway` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-script` | Pass（56 test: analyzer 7・binding 13・golden 10・luau 9・model 17。doc-test 0） |
| `mise run test synvelis-gateway` | Pass（26 test binaries・433 test・0 failure。read-only 消費元の非回帰） |

補助証拠（canonical = 上記 mise task）: `cargo check
--locked --all-targets -p synvelis-script`・`cargo fmt
--all -- --check` の直接実行も同一結果。

## 受入条件 → case 対応表

### AC-1 同一 Snapshot からの同一 Model/Luau 型定義/Digest Golden

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| GLD-G01 決定的 model | 同一 snapshot+surface → 同一 canonical JSON・model digest | digest 一致 | Pass | `tests/golden.rs` determinism |
| GLD-G02 決定的 artifacts | 同一 model → 同一 binding/stub text・digest | digest 一致 | Pass | `tests/golden.rs`・`tests/luau.rs` |
| GLD-G03 fixture=live | serialized snapshot fixture と live capture が同一 model を生成 | digest 一致 | Pass | `tests/golden.rs` fixture/live equality |
| GLD-G04 serialization | snapshot/model/binding/stub の serde round-trip | round-trip 一致 | Pass | `tests/golden.rs`・`tests/model.rs` |

### AC-2 Binding↔型定義の Capability/Version/I-O 一致・Schema Drift 検出

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| DRF-G01 surface 一致 | binding/stub が同一 invocable public 集合・同一 capability pin・schema ref を投影 | `check_consistency` Pass | Pass | `tests/golden.rs`・`tests/luau.rs` |
| DRF-G02 model drift | `verify_against` が model digest 不一致を `StaleArtifact` で拒否 | typed error | Pass | `tests/binding.rs`・`tests/luau.rs` |
| DRF-G03 stub drift | stub digest/model digest 不一致を拒否 | typed error | Pass | `tests/luau.rs` `stub_verifies_against_model_and_detects_stale` |
| DRF-G04 binding drift | binding model digest 不一致を拒否 | typed error | Pass | `tests/binding.rs` |

### AC-3 型付き検証（Bool/Number/Integer/Enum/Array/Object/Optional/Id/TimeRange・Null/Absent/i64・Bad Time/Target）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| TYP-G01 全 variant 投影 | `ValueSchema` 全 variant が Luau 型へ写像 | 型 alias 生成 | Pass | `tests/luau.rs` |
| TYP-G02 i64 精度 | f64 範囲外 `Integer` は `SynvelisI64` 文字列、schema `Integer` 位置のみ strict decode、非数字 reject | `i64::MAX` decode・typed reject | Pass | `tests/binding.rs`・`tests/luau.rs` |
| TYP-G03 Null/Absent 区別 | required+`Optional` は `inner | nil`、optional field は `?` で absent を許容 | stub 型が区別 | Pass | `tests/luau.rs` |
| TYP-G04 target 検証 | contract shape・resource kind・tagged id・resource-set 非空・create parent/creates | `GatewayError` typed | Pass | `tests/binding.rs` target 系 |
| TYP-G05 consistency×domain | domain 別 legality（`Unconditional`/`ExpectedRevision`/`ReadAt`/`FixedSnapshot` 等） | typed error | Pass | `tests/binding.rs` consistency 系 |
| TYP-G06 schema 適合 | invoke 前 schema 形状検証・time-range domain/整数制約 | `GatewayError` typed | Pass | `tests/binding.rs` schema/time 系 |
| TYP-G07 wire 適合 | `InvocationDraft` が `WireRequest::parse` を通過 | parse 成功 | Pass | `tests/binding.rs` `verify_wire` |

### AC-4 Unknown/Retired/Disabled/Hidden の表示と実行許可の分離・Metadata Disclosure

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| VIS-G01 typed verdict | Unknown/UnsupportedVersion/Disabled/Planned/Retired/UnresolvedSchema/InvalidName が typed verdict | verdict 一致 | Pass | `tests/model.rs` |
| VIS-G02 実行許可分離 | 非 invocable verdict・Hidden は `assemble_call`/binding/stub を生成しない | `NotInvocable`/非投影 | Pass | `tests/binding.rs`・`tests/luau.rs` |
| VIS-G03 hidden 非漏洩 | Hidden entry は `list_public`・stub・completion に現れない | 非存在確認 | Pass | `tests/model.rs`・`tests/luau.rs`・`tests/analyzer.rs` |
| VIS-G04 reason 非開示 | `Disabled{reason}` の reason が stub/diagnostic に出ない | stub に reason 非含有 | Pass | `tests/model.rs` `disabled_reason_stays_out_of_stub`・`tests/luau.rs` |
| VIS-G05 authority 非投影 | model/stub/binding の公開面に authority 型・field が存在しない | 型/field 非存在 | Pass | `tests/model.rs` `model_carries_no_authority_types` |

### AC-5 Analyzer が Generated Definition から診断・completion を返す

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| ANZ-G01 未知 API | 未知 `synvelis<Name>` 呼出を diagnostic で報告 | diagnostic 返却 | Pass | `tests/analyzer.rs`（fake analyzer fixture） |
| ANZ-G02 誤 arity | 宣言と異なる引数個数を diagnostic で報告 | diagnostic 返却 | Pass | `tests/analyzer.rs` |
| ANZ-G03 completion | 公開 capability の候補を返し hidden/非 invocable を含まない | 候補一致 | Pass | `tests/analyzer.rs` |
| ANZ-G04 stale reject | source/stub revision ずれを stale typed error で拒否 | `verify_result_revision` | Pass | `tests/analyzer.rs` |
| ANZ-G05 unsupported/unavailable | `Unsupported`/`Unavailable` を typed error で fail closed | typed error | Pass | `tests/analyzer.rs` |
| ANZ-G06 権限非昇格 | analyzer 成功が Grant/実行権限を生まない（port が authority を保持しない） | 型に authority 非存在 | Pass | `tests/analyzer.rs`・`tests/model.rs` |

### AC-6 第二 Registry/独自認可/Job 正本なし・VM/GPUI 型を公開 Port に含まない

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| ARC-G01 依存方向 | `synvelis-script` は `synvelis-gateway` read API のみ消費、`synvelis-automation` 非依存 | Cargo.toml 依存面 | Pass | `crates/synvelis-script/Cargo.toml` |
| ARC-G02 公開面静的検査 | 公開型に VM/GPUI/authority 型が存在しない | fixture 照合 | Pass | `tests/model.rs` `model_carries_no_authority_types` |
| ARC-G03 invoke 非呼出 | crate は `Gateway::bind`/`invoke` を呼ばず draft まで | `assemble_call` は draft のみ返却 | Pass | `tests/binding.rs`（draft→`verify_wire` のみ） |

### Deferred（NotRun）

- 実 Luau runtime での script 実行・実 Luau.Analysis
  native FFI/実測 — ruviform#560 領域。
- Action Script lowering（#272）・Script Builder（#273）・
  Workflow Script（#274）の consumer 受入 — 統合受入
  ゲート。`ScriptApiModel`/digest 契約の consumer 適合は
  後続 slice。
- `mise run gw10` gateway ハーネス非回帰 — 任意実行枠の
  ため本 slice では NotRun（`check`/`lint`/`test`
  synvelis-gateway は Pass 済み）。
- `*-msrv` レーン — ユーザー指定により実行しない。
- 実 OS・GUI 結合・製品内実行 — 対象外。
