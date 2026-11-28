# TIM-03 実 ABI のオートメーション値変換

Windows の本番ルーティングで、Ruviform の実 VST3/CLAP `test_gain` に渡したオートメーション値を PCM で検証した。既存の normalized `0.375` を残し、形式ごとに `0.625` の独立ケースを追加した。製品コード、依存 pin、共有テスト部品は変更していない。Refs #127。

## 検証対象

- テスト実装 SHA: `4123d7ac36ca16d2e5c809a931e28d08b9ac87c7`（base `7e6a19f2f460f5156c49329c1353df63a991540d`）
- Ruviform の `Cargo.lock` 解決 SHA: `511e1a7944c35797942ba065c092b9263067c48a`
- 実行環境: Windows 11 Pro 26H2、version `10.0.26300`、build `26300.9974`、x86_64。Rust `1.99.1`、`x86_64-pc-windows-msvc`

各テストは新しいプロジェクトと L1 子 runner を起動する。`ProjectOp::WriteAutomationRegion` で同じ時刻のキー値だけを変え、`control_apply_transport` から本番の `L1 OutOfProcessSpec` を通す。ターゲットは processor gain (`param_id=0`)、線形範囲 `0..=2`、既定値 `1.0`。48 kHz、256 frames/block、tick 1 の 125 BPM、loop `[385,641)`、seek 513、automation media frame 545 は固定した。control ledger は `call_start=512`、`frame_offset=32`、物理 frame 544 を記録し、次のウィンドウは offset 128 の loop wrap を記録した。

## PCM 判定

入力は左 `+0.25`、右 `-0.25`。fixture の線形範囲から独立に定めた plain gain をそれぞれに掛け、各ブロックの左右 256 サンプルすべてを許容差 `1e-6` で比較した。出力 report の gain から期待値を逆算していない。

| 形式 | Normalized | 独立期待 gain | 期待・観測 PCM (左, 右) | 一致ブロック (0 起点) |
| --- | ---: | ---: | --- | --- |
| CLAP / VST3 | 0.375 | 0.75 | `(+0.1875, -0.1875)` | 4–11 |
| CLAP / VST3 | 0.625 | 1.25 | `(+0.3125, -0.3125)` | 4–11 |

4 ケースともブロック 0–1 は lookahead の無音、2 は unity、3 は遷移で、いずれも一致に数えなかった。ブロック 4 に加え、後続にも全サンプルが許容差内に入るブロックを求めた。`0.625` をそのまま gain にした PCM `(+0.15625, -0.15625)` や unity `(+0.25, -0.25)` ではこの条件を満たせない。normalized を plain に流用した値と全サンプルが一致するブロックは 4 ケースともなかった。

## 子 runner の DLL

`mise run test-tim03-transport -- <test-name> --exact` で 4 ケースを個別に再実行した。Windows の `Get-Process.Modules` で各子のモジュールを列挙し、`NtQueryInformationProcess` でテストプロセスを親とする起動系列をたどり、該当の mise PID まで照合した。`SYNVELIS_TIM03_L1_PLUGIN_DIR` は未設定だった。モジュール一覧に現れたパスの SHA-256 は、`target/debug/deps` から明示した検証 fixture と runner 隣の DLL の実行前後の SHA-256 に一致した。ステージ済み DLL の存在だけでは実 ABI の合格としない。

| 形式 | 子 PID (0.375 / 0.625) | 実ロード先 (worktree 相対) | DLL SHA-256 |
| --- | --- | --- | --- |
| CLAP | 18500 / 25268 | `target/debug/ruviform_plugin_test_clap.dll` | `f795b69476df4b03721fb473557941f480fa60d47cb2a2c80c76ef081c7740a1` |
| VST3 | 60560 / 46016 | `target/debug/ruviform_plugin_test_vst3.dll` | `cb05cb54c8d6c57daa01ae095dd52d81c3af62a00a38addf5993a169ac3a748d` |

[JSONL](abi-automation-gain.jsonl) に、各ケースの PCM 分類と先頭サンプル、実測時刻、実行環境、対象 SHA、モジュールの絶対パス、検証 fixture との照合結果を記録した。無指定の正規コマンドと個別再実行で、PCM 観測は一致した。

## ローカル検証コマンド

各コマンドは同じ Windows 環境で実行した。`check-ruviform-rev` の初回は生成済み vendor の REV `20a24a66ee90f4d462224391e78c10c93f4f31ba` が pin と不一致だった。旧ディレクトリを保持し、`mise run sync-ui-deps` で無追跡の生成物だけを同期して再検証した。`Cargo.lock` と pin は変更していない。

| コマンド | Exit | 結果 |
| --- | ---: | --- |
| `mise run fmt-check` | 1 | Fail: Windows の `os error 206` |
| `mise exec -- cargo fmt -p synvelis-app -- --check` | 0 | Pass: package の補助検証 |
| `mise run check synvelis-app` | 0 | Pass |
| `mise run lint synvelis-app` | 0 | Pass |
| `mise run test synvelis-app` | 0 | Pass: 498 件 |
| `mise run check-ruviform-rev` | 1 → 0 | 初回 Fail: REV 不一致。同期後に Pass |
| `mise run test-tim03-transport` | 0 | Pass: 4 件 |
| `mise run test-tim03-transport -- <test-name> --exact` | 0 | Pass: 4 ケースを個別実行、子 DLL の SHA-256 一致 |
| `mise exec -- prek run --all-files` | 0 | Pass |

## 未実施と判定範囲

この `Pass` は Windows 上の実バイナリ 2 形式、固定条件の 4 ケースに限る。fixture の判定はブロック境界の有限な PCM 観測であり、任意の sample offset でのプラグイン側適用、同時イベントやフレーム境界の全行列、RT/Offline/Crash/State の受入を示さない。`test_gain` 自体は transport 情報を消費しない。

macOS/Linux の OS-L1、Windows の GUI-L2 と本スライス以外の EXT-L3、AudioLens の f64 / Native PCM 非回帰は `NotRun`。他の fixture・形式や既存 CI の結果には転用しない。手動 CI は起動していない。ローカル実行は変更範囲の検証であり、統合検証には merge 後の対象 SHA の Daily CI 証跡が必要。
