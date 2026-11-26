# TIM-04 RCP-01 persisted state projection evidence summary

本ファイルは、Issue #128 の `TIM04-RCP-01` のうち persisted plugin-state projection slice の証拠を要約する。対象は `synvelis-project` の保存済み manifest/store を `synvelis-render` の RCP-01 recipe resolution と RCP-02 artifact cache lookup eligibility へ fail-closed に接続する headless contract である。実 plugin の Offline render 実行、real ABI capability、GUI、3OS 最終受入の証拠ではない。

## 環境

- OS/Arch: Windows / x86_64
- ruviform resolved rev: `0479fa62e77e669a7bf84320d3d5418ffb949fc6`
- plugin-state manifest schema: `PLUGIN_STATE_SCHEMA_VERSION = 1`
- store backend: `InMemoryPluginStateStore`（`synvelis-project` の保存契約を使う deterministic fixture）
- projection input: store HEAD / explicit manifest / manifest bytes
- driver: `crates/synvelis-render/tests/tim04_rcp01_state_projection.rs`
- machine-readable rows: `evidence/sv128/tim04_rcp01_state_projection.jsonl`

## 証拠生成コマンド

```sh
SYNVELIS_TIM04_RCP01_EVIDENCE_DIR=evidence/sv128 mise exec -- cargo test --locked -p synvelis-render --test tim04_rcp01_state_projection -- --test-threads=1
```

結果は exit code 0、7 passed / 0 failed だった。cargo 直接実行は evidence 生成の補助証拠であり、canonical lane ではない。canonical lane の結果は PR 本文の Verification に記録する。

## Projection matrix verdict

| case | verdict | observed |
| --- | --- | --- |
| `captured-Coordinated` | Pass | blob bytes digest を検証した evidence が `RecipeResolution::Resolved` に入る |
| `captured-Quiescent` | Pass | blob bytes digest を検証した evidence が `RecipeResolution::Resolved` に入る |
| `captured-BestEffort` | Pass | exact-ineligible な typed `Unknown` になり、BitExact/Exact eligibility に昇格しない |
| `unsupported` | Pass | digest を捏造せず typed `Unknown` になる |
| `failed-timeout` | Pass | blob-less failure kind を保持して typed `Unknown` になる |
| `missing-evidence` | Pass | valid manifest に record がない場合だけ `PluginStateUnknown` になる |
| `missing-blob` | Pass | persisted `Captured` record が参照する blob がない場合は projection error になり、exact evidence にならない |
| `artifact-lookup-non-resolved` | Pass | verified artifact candidate が存在しても non-`Resolved` resolution では `LookupOutcome::Unknown`、control では `EligibleExact` |
| `real-plugin-offline-render` | NotRun | `OfflineRenderSupport::Verified` capability がなく、この slice の対象外 |

test 側では `Failed` kind として `CaptureFailed`、`Timeout`、`PermissionDenied`、`Crashed`、`Corrupt`、`Unavailable`、`Missing` の全件を `UnknownCause::PluginStateDegraded.failure_kind` に保持した typed `Unknown` へ写像することを確認した。integrity test では HEAD absence、invalid manifest name、HEAD/manifest/blob read failure、malformed JSON、schema version mismatch、duplicate processor、manifest digest mismatch、missing/corrupt blob を projection construction error として確認した。

## Fail-closed oracle

- `Coordinated` / `Quiescent` だけが exact-eligible evidence になる。
- `BestEffort` は digest が正しくても exact-eligible にしない。
- `Unsupported` / `Failed` は blob を持たないため、empty/zero/fake digest を作らない。
- missing/corrupt/unreadable blob、manifest digest mismatch、schema mismatch、duplicate processor は projection error であり、空 evidence の成功と混同しない。
- non-`Resolved` resolution を artifact lookup に渡した場合、candidate を列挙せず `LookupOutcome::Unknown` にする。

## Canonical compatibility

`PluginStateEvidence { digest, quality }` の serialized shape、`RECIPE_SCHEMA_VERSION = 1`、`CANONICALIZATION_VERSION = 1` を変更していない。`Unsupported` / `Failed` の詳細は resolution-time の typed unknown にとどめ、canonical recipe bytes に入れていない。

## Canonical verification lane

ローカル検証は変更範囲の検証であり、merge 後の main SHA に対する統合検証とは区別する。

| command | exit code | verdict / observed |
| --- | ---: | --- |
| `mise run fmt-check` | 1 | scope-out tooling failure。`cargo fmt --all -- --check` が Windows の command-line length 制限で rustfmt 起動前に `os error 206`。変更 crate限定の `mise exec -- cargo fmt -p synvelis-render -- --check` は exit 0（補助証拠） |
| `mise run check synvelis-render` | 0 | Pass |
| `mise run lint synvelis-render` | 0 | Pass |
| `mise run test synvelis-render` | 0 | Pass。106 passed / 0 failed / 0 ignored。`tim04_rcp01_state_projection` は 7 passed / 0 failed |
| `mise run check synvelis-workspace` | 0 | Pass。直接下流の exhaustive match 互換を確認 |
| `mise run test-rt` | 0 | Pass。filter `rt_` で 171 passed / 0 failed / 0 ignored（3474 filtered out） |
| `mise run check-headless-deps` | 0 | Pass。`no GUI framework deps in 12 headless crates` |
| `mise run check-ruviform-rev` | 1 | scope-out existing stale vendor。ui vendor REV `20a24a66ee90f4d462224391e78c10c93f4f31ba` != lockfile `0479fa62e77e669a7bf84320d3d5418ffb949fc6`。本 slice では `sync-ui-deps` も pin bump も行わない |

`fmt-check` の失敗は rustfmt/cargo-fmt が workspace 全 target を1つの rustfmt command lineへ渡す Windows 制限に一致する。これは `rust-lang/rustfmt#6934` として upstream tracking がある既知問題であり、format diff 検査の失敗ではない。package-scoped rustfmt では変更 crateが pass している。

## Non-claims / 残件

- 実 plugin Offline render、real ABI capability、typed runtime process oracle、製品 registry/discovery を Pass と主張しない。
- plugin-originated PermissionDenied を実測 Pass にするには ruviform fixture/HostError capability が必要である。
- 実 plugin を組み込んだ Offline render には ruviform `OfflineRenderSupport::Verified` capability が必要である。
- GUI/Native Editor、AU、市販 plugin、物理機材、AudioLens cross-gate、3OS 最終受入は実施していない。
- docs index（`docs/**/README.md`）はこの slice では更新せず、Coordinator が統合時に整備する。
