# TIM-03 テンポ undo/redo レビュー

評価対象は履歴を通したテンポ source の同一性、下位 mixed plan の保持、実 ABI の非遡及 PCM、証跡の再現性。担当 worker が単独で作業する契約のため、レビューと validation を時点・役割で分けた。独立レビュアーを起動した、または Code Owner が承認したとは主張しない。

| Review pass | 指摘候補 | Validator の確認 | disposition |
| --- | --- | --- | --- |
| Control / correctness | 中間の baseline sync があると revision 単独の拒否条件を誤って通過し得る。 | 下位の test が、baseline を挟まない A→B も実行し、古い割り当てを current としないことを照合した。 | テストで検出。 |
| Adversarial / production | 計測の `Seek` が同期を再試行するため、通知漏れを PCM のみでは検出できない。 | operation 後、計測前の graph notification、current plan、実 `Applied` receipt を要求し、世代と位置の継続も照合した。 | 経路に同期順序の検査あり。 |
| Adversarial / PCM | 先行測定の click や PPQN 960 の既定値を数えてしまう。 | 3 種の期待値を独立計算した。全窓で一意の impulse を要求し、480 の実プランと欠落時の未設定を確認した。 | runtime と一致。 |
| Provenance | 旧ログでは OS の版・build と、子 runner が実際にロードした DLL が未確認だった。 | 2 件だけを正規 lane で再計測した。OS 版・build と対象 SHA を採取し、Windows の子プロセスで観測した DLL のパス・SHA-256 を入力 DLL と照合した。 | [再計測ログ](dynamic-tempo-undo-redo/dynamic-child-provenance.log) と同期した JSONL に反映。 |
| Static checks | 初回 player lint で needless borrow とテスト関数の長さを検出。 | 借用を修正し、期待 snapshot の構築を分離した。lint とテストを再実行した。 | 修正済み。 |

判定: ACCEPT WITH NON-BLOCKING NOTES。本 slice に確認済みの P0/P1 blocker は 0。Windows workspace `fmt-check` の path length 障害は未解消として[検証記録](dynamic-tempo-undo-redo-verification.md)に残した。GUI・EXT-L3・3OS および統合 CI の承認は含めない。
