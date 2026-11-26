# CTL-02 canonical persistence verification evidence

## Scope

- Task: synvelis#122 CTL-02 canonical persistence slice
- Branch: `feat/122-canonical-persistence`
- Base SHA: `175b7b0d621f59e8b89cf0e5a0acfc784bbee940`
- Target SHA: `29271c20ef965a31747fdc70fa4d5cd0fb0e9b81`（implementation commit。後続 commit は docs/evidence のみ）
- OS: Microsoft Windows NT 10.0.26300.0 (`x86_64`)
- Canonical verification entrypoint: mise tasks. Direct cargo output, where used, is auxiliary evidence only.
- 対象は headless `synvelis-project` persistence contract。GUI / EXT-L3 / 3OS / provisioned runner はこの slice の範囲外。

## Fixture observations

`mise run test synvelis-project --test ctl_02`（exit 0）で次を観測した。

1. `v15_snapshot_normalizes_legacy_internal_specs_without_rewriting_authored_state`
   - v15 body の非 canonical master fader / strip mute / send-level target が migration 後に canonical `kind`/`mapping`/`time_scope`/`capability: None` へ揃う。
   - `name`/`base`/target ID/owner/role が保持される。
   - free-form target と plugin capability target は untouched。
   - automation lane target 参照、send `gain_target` binding、`id_watermark` は変わらない。
   - 同一 v15 bytes の 2 回 load projection が一致する（deterministic migration）。
2. `v16_snapshot_rejects_each_noncanonical_internal_spec_field`
   - v16 body で `kind`/`mapping`/`time_scope`/`capability` を 1 field ずつ外すと、それぞれ load oracle が `Error::Validation` で fail-closed。
   - error message に target id・owner・role・外れた field 名が含まれる。
3. `fresh_and_v9_migrated_projects_pass_the_canonical_internal_spec_oracle`
   - fresh project と v9→v16 migration が canonical oracle を pass する。
4. 既存 CTL-02 非回帰
   - strip seeding、first-use `Pan`、send create atomicity、send gain target adoption/rebind seal release、canonical write/adoption rejection を含む 21/21 pass。

## Command results

| Command | Result | Exit | Evidence |
| --- | --- | ---: | --- |
| `mise run fmt-check` | Fail（環境依存） | 1 | `cargo fmt --all -- --check` が `ファイル名または拡張子が長すぎます。 (os error 206)` で rustfmt usage を表示して終了。workspace 全体の all-package 起動が Windows path/argv 長に阻害された。変更差分の format violation ではない。 |
| `mise exec -- cargo fmt --package synvelis-project -- --check` | Pass（補助証拠；canonical = mise） | 0 | 変更対象 package の rustfmt check。 |
| `mise run check synvelis-project` | Pass | 0 | `cargo check --locked --all-targets -p synvelis-project` 完了。 |
| `mise run lint synvelis-project` | Pass | 0 | `cargo clippy --locked --all-targets -p synvelis-project -- -D warnings` 完了。 |
| `mise run test synvelis-project --test ctl_02` | Pass | 0 | 21 passed / 0 failed。 |
| `mise run test synvelis-project` | Pass | 0 | 656 passed / 0 failed / 58 test suites（doc-tests 2 pass を含む）。 |
| `mise run test-rt` | Pass | 0 | `cargo test --workspace --locked rt_` 完了。filter に合致した 171 tests passed / 0 failed（336 test suite entries）。 |
| `mise run check-headless-deps` | Pass | 0 | `no GUI framework deps in 12 headless crates`。 |

## Acceptance criteria mapping

- AC1（Fader/Send/Pan/Mute/Solo の GUI/Automation/Save/Undo で 1 Stable Parameter 正本 ほか）: **部分対応**。Save/Reload persisted spec の一意性を migration + oracle で検証した。GUI/Automation/Undo 全体と Send orphan/Undo の追加実測は後続。既存 send create atomicity test は非回帰。
- AC2（新旧 Graph/Plugin capability/FrameBase/Clock で誤配送ゼロ、Pending/Blocked/Observed 表示分離）: **NotRun / 範囲外**。実 plugin binding、SourceMap/ControlTrace、graph generation、GUI surfacing はこの slice に含まない。
- AC3（Channel mismatch 承認待ち、Host/Plugin/Realtime/Offline/2Window、AudioLens 非回帰）: **NotRun / 範囲外**。approval flow、実 realtime/offline/2window、AudioLens lane は後続 gate。

## Notes

- docs index（`docs/**/README.md`）は Worker が触らない。Coordinator が統合時に整備する。
- ローカル検証は変更範囲の検証であり、統合検証は merge 後 main SHA に対する必須 CI 結果で確認する。
