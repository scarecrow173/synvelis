<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/branding/logo-dark.svg">
    <img src="assets/branding/logo.svg" alt="Synvelis" width="440">
  </picture>
</p>

# Synvelis

**Synvelis（シンヴェリス）** は、作曲・録音・編集・ミキシング・サウンドデザインを統合する、Rust製の汎用デジタルオーディオワークステーション（DAW）を目指すプロジェクトです。

> **音の流れが見える。創作の可能性が広がる。**  
> ※ コンセプトを検討するための暫定的なメッセージです。

**開発状況：主要設計ベースライン（v0.3〜v0.7.1）合意済み・実装準備段階。** 現在のリポジトリは開発基盤のテンプレートから作成されており、DAWとして動作するアプリケーションはまだありません。**設計の合意とコード/PoCの実装・試験完了は別**です。

## Vision

Synvelisが目指すのは、通常の制作作業を直感的に進められ、必要なときには音声・MIDI・ルーティング・パラメータ制御の**構造と関係性を理解できるDAW**です。

従来型のタイムライン、ミキサー、プラグイン画面を制作の中心に置きつつ、信号経路や制御関係を必要な粒度で確認できる体験を検討しています。

- **Create — 制作する：** 作曲から書き出しまでを、一貫した制作フローで行う。
- **Understand — 理解する：** トラック、バス、プラグイン、オートメーションなどのつながりを把握する。
- **Refine — 調整する：** 意図しない挙動の発見や設定の改善を、把握可能な情報に基づいて進める。

「すべてをノードベースにする」のではなく、**必要な構造を必要なときに見せること**を重視します。

## Design principles

以下は製品設計の基本原則として検討しているものです。

| 原則 | 意味 |
| --- | --- |
| **Connected** | トラック、信号経路、制御元、影響先の関係を把握できる |
| **Transparent** | 設定、観測結果、推定、観測できない状態を区別する |
| **Flexible** | 特定の制作ジャンルやワークフローを強制しない |
| **Reliable** | リアルタイム処理の安定性、非破壊編集、データ保護を重視する |

### 構造と変化の理解

制作環境の状態を調べるときに、次の問いへ答えられることを目指します。

- **Where：** どのトラック、バス、ノード、パラメータで起きたか。
- **What：** どの値や信号の状態が変わったか。
- **Why：** 既知の制御元や接続関係は何か。
- **How：** どの経路・順序・変換を通じて適用されたか。

第三者製プラグインの内部処理など、ホストから観測できない情報まで説明できることは保証しません。

## UI / UX direction

標準的なDAW操作と、必要に応じて展開できる構造表示を組み合わせます。

1. **通常表示：** Arrangement、Timeline、Mixer、Pluginなどの制作画面。
2. **関係表示：** 選択対象の入力元・出力先・制御元・影響先の強調。
3. **構造表示：** Bus、Send、Sidechain、エフェクトチェーンなどの信号経路。
4. **詳細・診断表示：** 取得可能な制御値、処理状態、レイテンシや負荷などの情報。

Signal Flowをノードとして表示する案と、選択対象の関連情報を示すContext Inspectorを検討しています。これらは**初期UIの方向性**であり、最終仕様ではありません。

**Workspaceの必須要件:** Viewのドッキング・分離、複数ウィンドウ・複数モニター対応を初期設計に含めます。レイアウトと画面間状態の統合方針は[Workspace / Docking / Multi-Window 統合設計](docs/designs/workspace-docking-state-architecture.md)を参照してください（設計段階・未実装）。

### Interactive UI mockup

[**Synvelis DAW UI Concept — HTMLモックアップ**](docs/mockups/synvelis-daw.html)

Arrangement / Timeline、Mixer、Signal Flow、Automation、Context Inspectorの関係を検討するための**インタラクティブなデザイン資料**です。旧仮称「Auvane」で作成した初期案の表記を「Synvelis」に更新したものです。

**実際のDAW機能を提供するアプリケーションではありません。** 再生、録音、メーター、波形、パラメーター値は視覚的なデモであり、音声エンジンやプロジェクトの永続化には接続されていません。UI構成や挙動も今後変更する可能性があります。

GitHubのファイル画面ではHTMLソースとして表示されます。実際に操作する場合はリポジトリをクローンし、`docs/mockups/synvelis-daw.html` をブラウザで開いてください（ロゴは `assets/branding/logo-dark.svg` を参照します）。


## Planned capabilities

長期的には次の制作領域を扱う方針です。実装済み機能の一覧ではありません。

| 領域 | 検討している機能 |
| --- | --- |
| 制作・編集 | マルチトラック、非破壊オーディオ編集、MIDIノート編集 |
| 録音・再生 | オーディオ録音、ソフトウェア音源、Transport |
| ミキシング | Mixer、Bus、Send / Return、Sidechain |
| 制御 | Automation、Modulation、パラメータ関係の表示 |
| 拡張 | オーディオプラグイン、将来的なスクリプティング |
| 仕上げ | オフライン書き出し、マスタリング向けの制作フロー |

初期バージョンから全機能を揃えるのではなく、**小さな制作ワークフローを端から端まで成立させる**方法で段階的に開発する予定です。

## Architecture direction

Synvelisは、汎用マルチメディアフレームワーク **[Ruviform](https://github.com/scarecrow173/ruviform)** を利用する**独立したDAWアプリケーション**として構想しています。

- **Synvelisの責務（合意済み）：** プロジェクト、トラック、クリップ、Take/Comp、Project PPQN設定（既定960）、制作操作、DAW固有のUI・状態管理・保存形式。
- **Ruviformの責務（合意済み）：** Rational Beat/TempoMap、汎用Graph/Media、オーディオ入出力、DSP、RT処理、Plugin Host、Probe/UI基盤などの再利用可能な能力。
- **境界（合意済み）：** SynvelisのSemantic ProjectとRuviformのGraph IR/Audio Execution、Timed Event、Plugin Adapter、Media Output、Observabilityを分離。具体API・新crate・RT所有方式はPoCで確定する。

RuviformにcrateやAPIが存在することは、SynvelisのDAW要件への適合を意味しません。実装と契約を確認しながら統合方針を決めます。

関連する **[AudioLens](https://github.com/scarecrow173/audio-lens)** は独立した音声解析ツールであり、Synvelisへの統合は現在の方針ではありません。

## Approved architecture baselines（2026-11-01）

製品コンセプトから個別設計・Ruviform汎用基盤・PoCまでの参照経路は[Synvelis #22 — 全体設計・トレーサビリティ索引](https://github.com/scarecrow173/synvelis/issues/22)を正本とし、Ruviformの移行管理は[ruviform #192](https://github.com/scarecrow173/ruviform/issues/192)を参照します。

| 設計 | 正本 |
| --- | --- |
| Project / Asset / Persistence v0.3 | [Synvelis #17](https://github.com/scarecrow173/synvelis/issues/17) / [設計書](docs/designs/project-asset-persistence-architecture.md) |
| Playback / Mixer / Graph v0.4 | [Synvelis #18](https://github.com/scarecrow173/synvelis/issues/18) / [設計書](docs/designs/playback-mixer-generic-execution-graph-architecture.md) |
| Musical Time / Variable PPQN | [Synvelis #19](https://github.com/scarecrow173/synvelis/issues/19) / [設計書](docs/designs/configurable-ppqn-musical-time-architecture.md) |
| Transport / MIDI / Automation / Plugin v0.5 | [Synvelis #20](https://github.com/scarecrow173/synvelis/issues/20) / [設計書](docs/designs/time-transport-midi-automation-plugin-integration-architecture.md) |
| Media / Import / Export / UI / Diagnostics v0.6 | [Synvelis #21](https://github.com/scarecrow173/synvelis/issues/21) / [設計書](docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md) |
| Ruviform Target Architecture v0.7.1 | [Synvelis #22](https://github.com/scarecrow173/synvelis/issues/22) / [設計書](docs/designs/synvelis-driven-ruviform-target-architecture-v07.md) |

Workspace/Recording/Asset Library等の詳細Issueは#22の索引から辿れます。現在の主要な未解決PoCは[Ruviform #190 Transport/Plugin Block](https://github.com/scarecrow173/ruviform/issues/190)、[#189 Variable PPQN実装](https://github.com/scarecrow173/ruviform/issues/189)、[#191 PCM WAV Output](https://github.com/scarecrow173/ruviform/issues/191)です。

## Development roadmap

以下は**検討中の順序**です。日程や完成範囲を確約するものではありません。

1. **Product definition：** 製品価値、主要ワークフロー、対象ユーザーと非目標の明確化。
2. **Architecture validation：** プロジェクトモデル、音声処理、表示の整合性を最小の垂直スライスで検証。
3. **Initial DAW：** マルチトラック再生、基本編集、ミキシング、プロジェクト保存などを順次統合。
4. **Practical alpha：** MIDI、音源、録音、プラグイン、Undo/Redo、書き出しなどを整備。
5. **Experience & reliability：** 高度なルーティング、制御関係の表示、安定性・互換性を強化。

## Getting started (development foundation only)

現在はアプリケーションコードのビルド手順ではなく、共通開発環境の初期化手順だけを提供しています。

前提：`git` と [mise](https://mise.jdx.dev/) がインストールされていること。また、このリポジトリへのアクセス権が必要です。

```sh
git clone https://github.com/scarecrow173/synvelis.git
cd synvelis

mise install
mise run setup
```

`mise run setup` はAPM依存関係の復元、`prek`のフック導入、リポジトリローカルのGit LFS設定を行います。必要に応じて、`mise exec -- prek run --all-files` で設定済みのフックを実行できます。

現時点ではRustのアプリケーションワークスペース、ビルド、テスト、CIの定義はありません。これらは実装方針の確定後に整備します。初回セットアップ後に生成される`mise.lock`と`apm.lock.yaml`は、内容を確認してからコミットする運用です。

## Repository layout

| ファイル | 内容 |
| --- | --- |
| `README.md` | プロジェクト概要と現時点の方向性 |
| `assets/branding/` | ロゴ（明色・暗色）とシンボルのSVGアセット |
| `docs/mockups/synvelis-daw.html` | DAW UIのインタラクティブなデザインモックアップ |
| `AGENTS.md` | 開発・設計・作業時の共通契約 |
| `mise.toml` | 開発ツールとセットアップタスク |
| `apm.yml` | AIエージェント関連の依存設定 |
| `.pre-commit-config.yaml` | Gitフック設定 |
| `.gitattributes` / `.gitignore` | テキスト・LFS・除外設定 |
| `.worktreeinclude` | worktree間で共有するローカル設定の指定 |

DAW本体のソースコードは未着手ですが、正式に合意した設計書は`docs/designs/`に保存されています（上記索引参照）。

## License and name

**ライセンスは未決定です。** 現時点で本リポジトリに`LICENSE`ファイルはありません。公開・再利用・再配布に関する条件は、ライセンス決定後に明記します。

「Synvelis」は *Synthesis*（合成・創造）と *Visibility*（可視性）を組み合わせる意図で採用したプロジェクト名です。商標・類似名称・ドメインの調査は完了していません。
