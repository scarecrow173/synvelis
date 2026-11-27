# TIM-04 RCP-01 保存済み Plugin State の再読込

## 検証範囲

Windows OS-L1 の headless 契約テストとして、`PluginStateSession::save` の Receipt と、
同じディレクトリを開き直してからの Recipe / Artifact Eligibility を別々に検証した。
capture は固定 fixture、Plugin State の保存先は実ファイルを使う `DirectoryPluginStateStore`、
Artifact Store は生成 WAV を登録する `InMemoryArtifactStore` である。
製品コード、永続化 schema、RecipeKey、依存定義、`Cargo.lock` は変更していない。

- テスト: [`directory_state_reopen_preserves_quality_and_exact_eligibility`](../../crates/synvelis-render/tests/tim04_rcp01_state_projection.rs)
- 対象 SHA: `fc760b39d7e84685de979e543a08af4d8f9759b8`（base `bf0442cfad73c5102d137f3f1e20ac74d84860b3`）
- Ruviform resolved SHA: `511e1a7944c35797942ba065c092b9263067c48a`
- 実行環境（2026-11-28）: Windows 26H2 build `26300.9974` / x86_64、`rustc 1.99.1` (`x86_64-pc-windows-msvc`)、mise `2026.11.2`、既定 Cargo features
- 保存先: `std::env::temp_dir()` 配下のテスト専用ディレクトリ
- Blob の固定期待値: 改行なしの `tim04-reopen-state` を `printf 'tim04-reopen-state' | sha256sum` で独立に計算した `sha256:ffb02af46b451617039d38417b0b19b08af08ca192ef147ce3ff05644949d759`

## Receipt と再読込の観測結果

各ケースで Standard policy による保存が成功し、Receipt に manifest 名・digest と品質別の件数が記録された。
保存時のセッションを破棄した後、新しいストアの `HEAD` から manifest を読み、
Receipt と同じ名前・digest および fixture のプロジェクト ID・revision を確認した。
後続のケースも同じディレクトリに保存した。古い exact manifest が `HEAD` に残った場合や、
`Unsupported` に既存 Blob の digest を誤って割り当てた場合は、テストが失敗する。

| ケース | 保存時の Receipt | 再読込時の Plugin State | Recipe / Artifact Eligibility |
| --- | --- | --- | --- |
| `Quiescent`（停止・安定境界） | `Captured`、`exact: 1` | Blob bytes・固定 digest・`Quiescent` を確認 | `Resolved` に digest と品質を保持し、登録済み fixture Artifact は `EligibleExact` |
| `BestEffort`（処理中・安定境界なし） | `Captured`、`degraded: 1` | Blob bytes と digest は `Quiescent` と同一、品質は `BestEffort` | `Unknown(PluginStateDegraded)`、同じ Artifact を問い合わせても `Unknown` で候補なし |
| `Unsupported` | Blob 参照なし、`unsupported: 1` | digest を持たない `PluginStateProbeOutcome::Unsupported` | `Unknown(PluginStateDegraded)`、同じ Artifact を問い合わせても `Unknown` で候補なし |

`BestEffort` は既知の digest を保持するが、それを根拠に exact と判定しない。
`Unsupported` は Blob なしとして読み込み、`Unknown` から仮の digest や RecipeKey を作らない。
`Coordinated`、外部 Asset の未 pin、故障の型付け・整合性エラーについては既存テストを残し、今回のディレクトリ再読込テストには追加していない。

## ローカル検証

| コマンド | 終了コード | 結果 |
| --- | ---: | --- |
| `mise run fmt-check` | 1 | Fail（環境）。Windows `os error 206` により workspace 全体の rustfmt を起動できない。既存の [projection の記録](tim04-rcp01-projection.md#canonical-verification-lane) と同じ失敗 |
| `mise exec -- cargo fmt -p synvelis-render -- --check` | 0 | Pass（補助証拠）。変更クレートの整形を確認 |
| `mise run check synvelis-render` | 0 | Pass |
| `mise run lint synvelis-render` | 0 | Pass |
| `mise run test synvelis-render --test tim04_rcp01_state_projection` | 0 | Pass。8 件成功、失敗・無視なし |
| `mise run test synvelis-render` | 0 | Pass。107 件成功、失敗・無視なし |
| `mise exec -- prek run --all-files` | 0 | Pass。該当ファイルがない Windows ファイル名 hook のみ skip |

## 未実施の製品受入

| 項目 | 要求状態 | 結果・後続 |
| --- | --- | --- |
| Windows EXT-L3: 本番 Host での VST3/CLAP の Save / Reload / Offline / Crash、Blob 品質・State Hash・独立 PCM Oracle | Required | NotRun。この変更では正規 ABI Test Plugin をロードしない。製品受入は RF `#630` の fixture と本番 Host で別途判定する |
| Crash / Timeout / Permission / Blob Corruption の実環境試験、インデターミネート publish の Reconcile、Undo / History / Copy Restore、RT Active や形式別 capability の確認 | この変更の対象外 | NotRun。通常の保存・再読込と Recipe 判定を検証した結果を、復旧・実行状態の証拠にはしない |
| macOS / Linux OS-L1 | Required | NotRun。統合時に対象 SHA の Daily CI 証拠を確認する |
| AudioLens / RT / 精度の横断非回帰 | この変更のローカル検証対象外 | NotRun。統合時に対象 SHA と該当する必須ケースの検証証拠を確認する |

市販 Plugin の外部 Asset や非決定挙動は保証しない。形式的証明は実行していない。
ローカル検証は変更範囲の証拠に限る。統合検証は merge 後の main SHA の必須 CI 証拠で判定し、今回は手動 CI を起動していない。
