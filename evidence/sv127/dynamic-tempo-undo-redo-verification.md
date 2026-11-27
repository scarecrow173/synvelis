# TIM-03 テンポ undo/redo の検証記録

対象は `tim03-dynamic-tempo-undo-redo-s1`。Windows x86_64 で、既存の `ProjectSession` 履歴操作に限り、下位の control と実 VST3/CLAP の両方を検証した。本番コード、依存 pin、mise タスクは変更していない。

## 対象 SHA と依存

| 対象 | SHA・件数 |
| --- | --- |
| PR #686: production musical transport | `82095ea5c9545345ff193156525243bd8ca0fcc4` |
| PR #687: representative refresh | `0954a19f74711dfe4b4b54dcdcf87f2ba992a940` |
| PR #688: frame/boundary matrix | `de3dcdd46e122077f437323030d7e501b9c36547` |
| PR #689 / `origin/main`: base | `ab678e935fbdf7ec61781d448fdb067ec5de8d8e` |
| 実行対象（テスト実装のコミット） | `d6368e0fbc97eb6284d6f198e8d954462ddf403f` |
| Ruviform pin（変更なし） | `511e1a7944c35797942ba065c092b9263067c48a`、`Cargo.lock` の source 36 件 |

## 履歴と計測の範囲

| 操作 | project / tempo revision の前後 | 操作位置・状態 | 復元・追加される内容 | VST3 / CLAP の click |
| --- | --- | --- | --- | --- |
| A を追加 | `9→10 / 2→3` | tick 4,820 / Paused | tick 9,600 に 75.000 BPM | install のみ |
| 1 回目の undo | `10→11 / 3→2` | tick 4,820 / Paused | baseline | `[640]` / `[640]` |
| redo | `11→12 / 2→3` | tick 4,832 / Playing | A | `[672]` / `[672]` |
| 2 回目の undo | `12→13 / 3→2` | tick 4,832 / Playing | baseline | install のみ |
| 分岐 B を追加 | `13→14 / 2→3` | tick 4,832 / Playing | tick 9,600 に 110.000 BPM、A とは異なる | `[622]` / `[622]` |

操作位置は最後の node `TransportSpan` の終了時点。再生中は `Seek` だけで編集 tick より前に戻し、RT 適用と観測 span を確認した。全操作で `retroactive: false`。プロジェクト全体の比較はテンポ以外の変更を拒否する。分岐後の redo は何も復元しない。

追加・undo・redo は `ProjectSession` 経由。graph build attempt の通知と、変化しない routing generation を確認した。通知だけでプランが更新され、後続の `Seek` より前に対応する plan generation の `Applied` receipt を得た。更新中の transport generation、playing/paused、予想どおりの位置進行も確認した。`control_apply_transport` の明示的な再同期を通知の証拠には使っていない。

下位テストは session 通知の証拠ではない。`ProjectController` の revision `7→12` と tempo revision `9→10→9→10→9→10` を操作ごとに照合し、`RoutingControlDispatch` の明示 sync で mixed plan を置換した。MIDI と parameter event の各位置、順序、payload、source sequence、任意の generation binding を保ち、refresh error は空。別の直接 A→B 比較では、同一 revision でも古い A は current ではなく、置換したプランだけが B のコンパイル済みマップ割り当てを共有した。

## 独立 PCM 判定

計測位置は tick 11,518、四分音符境界は tick 11,520。どちらも編集 tick 9,600 より後にある。host/project PPQN 480 の 2 ticks を adapter PPQN 960 の 4 ticks に投影する。分子は `4 * 48,000 * 60,000 = 11,520,000,000`。分母は milli-BPM × 960。商を切り上げて lookahead `2 * 256 = 512` frames を加える。

| source | milli-BPM | 分母 | 独立期待値 |
| --- | ---: | ---: | ---: |
| baseline（tick 1 の tempo） | 93,750 | 90,000,000 | `128 + 512 = 640` |
| A | 75,000 | 72,000,000 | `160 + 512 = 672` |
| B | 110,000 | 105,600,000 | `110 + 512 = 622` |

各 5 block の PCM に bit-exact 0.5 の click が 1 個だけあり、左右は一致し、それ以外は無音だった。lookahead 内の試料は正例に数えない。無音・pass-through・click の重複ではこの判定を満たせない。無音だけの block の代替処理と通常の無音は PCM 上では区別しない。実行経路は production composition → `PluginNode` → out-of-process L1 → `test_instrument`。direct endpoint process、呼び出し元の `TransportInfo`、test-only seed、fake plan install は使わない。host の実プランは 480、960 は投影先に限る。PPQN 不整合・欠落の下位テストも既定 PPQN を補わない。

## 証跡の同期

[JSONL](dynamic-tempo-undo-redo.jsonl) の ABI 行 2 件は [最終正規 lane](dynamic-tempo-undo-redo/dynamic-distinct-fixtures.log)、下位と負例の 2 件は [正規 player test](dynamic-tempo-undo-redo/test-player.log) の stdout から抽出する。app lane は player テストバイナリを実行しないため、下位証跡を app lane の結果とは扱わない。player test の `RUST_TEST_NOCAPTURE=1` と `RUST_TEST_THREADS=1` は stdout 収集用。両 lane の `TIM03_VERIFIED_SHA` は実行対象のコミット。fixture の入力には runner の sibling とは別のビルド済み `debug/deps` DLL を指定し、両方のサイズ・SHA-256 を照合した。最終 log に指定値を記録している。既定解決で行った前段の run は、同一ファイルへ解決したため最終 JSONL には使っていない。

```sh
mise exec -- node evidence/sv127/dynamic-tempo-undo-redo/sync-jsonl.mjs --check \
  d6368e0fbc97eb6284d6f198e8d954462ddf403f
```

## 正規コマンドの結果

すべて Windows x86_64。再実行分を含むコマンド、exit code、raw log は [canonical-results.tsv](dynamic-tempo-undo-redo/canonical-results.tsv) に記録する。ローカルの実行結果は変更範囲の検証であり、統合検証には merge 後の対象 SHA の CI が必要。

| 正規コマンド | exit | 結果 |
| --- | ---: | --- |
| `mise run fmt-check` | 1 | Infrastructure failure、`os error 206` |
| `mise run check-ruviform-rev` | 0 | Pass（生成物を同期後） |
| `mise run check synvelis-player` | 0 | Pass |
| `mise run lint synvelis-player` | 0 | Pass（初回 lint 指摘を修正後） |
| `mise run test synvelis-player` | 0 | Pass |
| `mise run check synvelis-app` | 0 | Pass |
| `mise run lint synvelis-app` | 0 | Pass |
| `mise run test-tim03-dynamic-musical-transport` | 0 | Pass（既存 2 件、追加 2 件） |
| `mise run test synvelis-app` | 0 | Pass（runner build 後） |
| `mise run test-tim03-frame-boundary-matrix` | 0 | Pass |
| `mise run test-tim03-transport` | 0 | Pass |
| `mise run test-rt` | 0 | Pass |
| `mise run check-headless-deps` | 0 | Pass |
| `mise exec -- prek run --all-files` | 0 | Pass（raw log の末尾を正規化後） |

初回の `check-ruviform-rev` は生成済み UI vendor の `20a24a66…` と lock の `511e1a79…` が不一致だった。Coordinator の承認後、既存の生成物を同じボリュームの worktree 外バックアップへ移動して保持し、`mise run sync-ui-deps` で既存 pin に復元した。再検査は exit 0。依存ファイルに差分はない。

`mise run fmt-check` は Windows の workspace path 列挙で `os error 206` により exit 1。Infrastructure failure として扱い、成功へ読み替えない。`mise exec -- cargo fmt --package synvelis-player --package synvelis-app -- --check` は exit 0 の補助証拠（canonical = mise）。

player lint の初回（exit 101）はテストの needless borrow と関数長が原因。修正後の正規 lint を合格とし、初回 log は診断用に残した。

prek の初回（exit 1）は ABI raw log の余分な末尾改行だけを正規化した。JSONL に抽出する行は変化していない。開発時の `dynamic-first.log` にある SHA は未コミットテストの親を指すため、実行対象の証拠には使わない。

## 検証境界

- Runtime Verification: Windows L1 の対象 runtime lane は Pass。静的検証の workspace `fmt-check` は環境起因で未合格。GUI runtime drive は NotApplicable。
- Formal Verification: NotApplicable / NotRun。本 slice の形式証明、モデル検査、Kani harness は追加していない。Issue `#601` の D-layer lane は本 TIM-03 の証明ではない。
- NotRun: full dynamic edit matrix、automation/capability/RT/fault/state/crash の残行列、GUI/EXT-L3、macOS/Linux、3OS CI、AudioLens cross-consumer。Issue 全体の完了ではない。
- Unsupported: 本 slice の対象操作にはなし。対象外の機能に Unsupported または Pass を推定しない。
- docs index は変更・再生成していない。Coordinator が統合時に整備する。
