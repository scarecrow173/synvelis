---
id: "SPEC-xzgW9NkNQVvC0xmEChXHi6"
type: "spec"
status: "approved"
title: "AI-Native Hybrid DAW Baseline — Intentional Usage・Rights・Shared Project・Agent Direction 設計契約 (synvelis#44)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/44"
  changes:
    added:
      - type: "file"
        path: "docs/specs/ai-native-baseline.md"
      - type: "file"
        path: "docs/designs/ai-native-baseline-design.md"
      - type: "file"
        path: "docs/plans/ai-native-baseline.md"
      - type: "file"
        path: "docs/tasks/ai-native-baseline-task.md"
      - type: "file"
        path: "docs/test-specs/ai-native-baseline.md"
      - type: "file"
        path: "docs/impl/ir/ai-native-baseline.md"
      - type: "file"
        path: "docs/impl/exp/ai-native-baseline.md"
      - type: "file"
        path: "evidence/sv44/ai-native-baseline-case-ledger.md"
      - type: "file"
        path: "evidence/sv44/ai-native-baseline-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/product-value-core.md"
    - "docs/specs/workspace-model.md"
    - "docs/specs/context-inspector.md"
    - "docs/specs/timeline-editing.md"
    - "docs/specs/project-model.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/musical-time.md"
    - "docs/specs/transport-integration.md"
    - "docs/specs/media-render.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/designs/gw-02-authorization-policy-design.md"
    - "docs/designs/midi-source-note-editing-design.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/44"
  derived-by:
    - "docs/designs/ai-native-baseline-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/ai-native-baseline.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
---

# AI-Native Hybrid DAW Baseline — Intentional Usage・Rights・Shared Project・Agent Direction 設計契約 (synvelis#44)

## What Should Be Built

Issue #44 の canonical 設計契約: Synvelis を「通常の手動制作と
AI による制作を同一 Project 上で共存させる AI-Native Hybrid
DAW」として成立させるための、**合意済み製品・セキュリティ・
Gateway 不変条件**（AIB-001〜007 / AHG-001〜011）を固定し、
Grant / Principal / Scope / Approval / ChangeSet・AI Off 既定・
Suggest / Approve-to-Apply / Delegated Apply の責務分離・
ProjectController Single Writer・RT 安全性・通常 DAW 独立経路を
一枚の契約として記述する。AIB/AHG の合意済み受入と、Issue が
「後続議論で決めること」「未決定の詳細設計論点」として保持する
未合意事項を区別し、前者だけを受入条件・証拠写像の対象とする。

上流 baseline は Issue #44 本文の Approved product/design
baseline（2026-11-03 / 2026-11-04 / 2026-11-09 / 2026-11-10 の
追補を含む）。本 slice は **docs/ 設計契約 + evidence 台帳のみ**。
Command Gateway の型・実装詳細は #45（型付き契約）と #46
（GW-01〜10 実装計画）が Owner であり、本 spec はそれらが継承
すべき不変条件を定義する。実外部 MCP Client/Provider・ユーザー
承認 UI・GPUI/OS/Privacy の製品受入は #46/#114 の別 Gate。
設計完了を実装・実 OS・実機受入とみなさない。

## Why Now

- Issue #44 は AIB-001〜007（製品・権利・安全性の基盤不変条件）
  と AHG-001〜011（Pluggable Hybrid / Gateway / MCP / 3 Mode /
  Explicit Target / Grant / Scope / Apply Mode / Validity /
  受入）を Approved baseline として保持するが、canonical
  spec/design/plan/task/test-spec/impl の 7 文書鎖と evidence
  台帳が未整備。
- GW-01〜10 の headless 実装（`crates/synvelis-gateway`、
  `crates/mcp-poc`）と `mise run gw10` の G0〜G6/GTV PASS
  証拠は main に存在するが、「合意済み設計のどの不変条件が
  どの fixture で被覆され、何が未被覆か」を一枚の契約表に
  固定する必要がある。headless PASS を製品受入・実機受入に
  繰り上げないための evidence 語彙も必要。
- #45（Gateway/認可/Capability 詳細型契約）と #46（MCP/
  Permission UX/RT/AI OFF 製品受入）は本 Issue の不変条件を
  継承する後続 Owner であり、境界を明示しないと責務が
  重複・欠落する。
- AI Gateway は製品 Gate A/B/C に対する**独立した第四軸**で
  あり、通常 DAW Gate A の受入を AI 有効化に依存させない
  こと、および逆に AI Gate の成功判定が ProjectController/
  Permission/Approval/Recovery/Security の受入を省略しない
  ことを、文書上で固定する必要がある。

## Users and Value

- 実装者（#45/#46 およびその配下の実装 Issue Owner）:
  Grant/Principal/Scope/Approval/ChangeSet・3 Mode・AI Off の
  判断基準と Owner 境界。
- Reviewer: 「設計合意」「headless fixture 成功」「製品受入」
  「実機/外部 Provider 受入」を区別できる evidence 語彙と、
  AIB/AHG 合意済み事項と未決定事項の分離。
- Ruviform Consumer 調整: Synvelis 固有の AI/DAW 意味論を
  Ruviform 汎用 Gateway/認証 Provider へ重複実装しない境界。
- 将来の利用者: AI を使わない通常 DAW 利用が Grant/Provider
  接続なしに完結し、AI を使う場合も意図しない推論・外部送信・
  Project 変更が起きないことの設計根拠。

## Scope

### In Scope

- AI-Native Hybrid DAW の製品位置づけ: Manual / AI の対等な
  共存、AI なしで完結する通常 DAW（AIB-001）。
- Intentional AI Usage: AI 既定 OFF、明示的利用意思・許可なしの
  推論/生成/AI 解析/AI 起因の外部データ送信の禁止、「利用可能」
  と「実行許可」の区別、無許可 AI 自動フォールバック禁止、
  処理内容・対象範囲・Provider/モデル・ローカル/外部実行・
  外部送信の確認・制御、AI 無効時の既存 Project 手動利用の
  非妨害（AIB-002）。
- 明示指示先行と将来の Proactive の権限分離: 自発的提案と
  自発的 Project 変更・外部送信は別権限、停止・取消可能
  （AIB-003）。
- 共通 Command/検証/変更履歴: typed Semantic Command/Controller
  経路、Stable Target ID・Expected Project Revision、
  Preview/Validation/Apply/Undo/Redo/Semantic Diff、
  「適用済み状態」と「提案・生成のみの状態」の区別
  （AIB-004）。
- 権利・来歴・透明性: AI 使用許可と生成物の利用条件・権利関係
  の区別、把握可能範囲での追跡、Unknown/Unverified/
  Unavailable の明示、観測事実/申告/推定/未確認の非混同、
  AI 関連記録自体の秘匿設計（AIB-005）。
- AI 境界の技術的強制と RT 安全性: UI 表示だけに依存しない
  共通許可/ポリシー境界、モデル実行・ネットワーク・外部
  サービス・生成 Asset 検証/登録・Agent Job の nonRT 扱い、
  RT Thread での無期限待機/I/O/推論禁止、未検証生成物の
  正規 Source 無断公開禁止、保証範囲の限定（AIB-006）。
- モデル非依存・Provider 交換可能性: ローカル/クラウド・
  生成/解析・テキスト/MIDI/Audio の能力差を扱える設計、
  特定モデル/事業者への永続依存を製品原則にしない
  （AIB-007）。
- Pluggable Hybrid: Synvelis の権威保持（DAW 正本・
  Capability/Command Gateway・利用許可・Context 公開範囲・
  ChangeSet 検証/適用）、手動 UI/外部 MCP Agent/将来の内蔵
  Agent の共通意味的操作契約、交換可能な Agent Reasoning
  Runtime、外部 Agent への Job 所有・実行許可・結果検証・
  登録・Project 状態整合の委譲禁止（AHG-001）。
- MCP Server 初期連携: 内部型付き Gateway の Adapter としての
  位置づけ、明示有効化・権限確認、接続/Tool 一覧/推論・
  Project 読取/適用の別概念化、信頼境界での接続主体特定、
  Context の限定的 Projection・権限付き操作・独立 Job Handle
  （AHG-002）。
- Gateway 論理契約: Capability/Invocation/Authorization/
  Context/Mutation/Execution の 6 要素（AHG-003）。
- 3 Apply Mode の初期提供: Suggest（提案のみ・Apply 権限
  なし）/ Approve-to-Apply（特定 ChangeSet の承認に結び付けた
  適用）/ Delegated Apply（明示 Grant 範囲内の個別承認なし
  適用）。Execution Grant と Approval Policy の別契約化、
  Delegated Apply ≠ Proactive、3 Mode すべての正常・異常系
  E2E（AHG-004/AHG-009）。
- 既存設計整合: ProjectController Single Writer の維持、
  別 Project Writer/別 Undo 基盤の禁止、通常 UI
  Command/Selection/Focus 契約の維持、Ruviform Generic
  Runtime の無変更利用、F0/E2E-L/R/X の暗黙変更禁止
  （AHG-005）。
- Explicit Target Resolution: Selection/Inspection Focus/
  Active Window/View/現在選択からの暗黙対象推測禁止、
  Command 発行前の Project ID・Typed Stable Resource ID・
  時間範囲/Expected Project Revision への解決、Grant 対象の
  発行時明示 ID/Scope への結合、UI 選択変更による Grant 対象
  の自動変更禁止（AHG-006）。
- Execution Grant/Policy 論理型: Authentication/Identity・
  Execution Grant・Activation Authority・Approval Policy の
  別契約化、論理 Grant 属性（Grant ID・Schema/Policy
  Revision・Issued By・Principal Binding・Project ID・
  Capability 集合・Resource Scope・Data Access・Activation・
  Approval・Validity/Revocation・Limits）、信頼境界での
  Principal 確認、Default Deny/Least Privilege、Grant 許可と
  AI 処理開始許可の独立、情報開示/モデル実行/外部送信/
  Project 編集の非一括権限化と保証境界の明示（AHG-007）。
- Resource Scope/操作効果評価: Typed Stable ID 基準、親子関係
  からの権限の無条件継承禁止、固定 Scope（Exact ID 集合・
  発行時 Snapshot 固定集合）と制約付き動的 Scope（明示条件
  評価・新規対象の暗黙無制限許可禁止）の初期設計対象化、
  新規リソース作成と既存対象編集の区別、移動操作の変更前後
  所属・時間範囲評価、Project 複製での Grant 自動移譲禁止、
  半開時間範囲と Musical Time/Source Native Time の区別、
  Validator 導出の構造的 Read/Create/Write/Delete・外部副作用
  と音響的波及の区別、不明/検証不能副作用の Delegated Apply
  自動許可禁止、Plugin 不透明 State/未検証 Media の
  Unsupported/Unknown 表現（AHG-008）。
- Apply Mode/Approval 評価: Suggest/Approve-to-Apply/
  Delegated Apply の個別契約、Approve-to-Apply の
  ChangeSet・Project・Principal・Expected Revision・期限への
  承認結合と改竄・再利用拒否、3 Mode 共通の
  Validation/Revision・Grant 再評価/ProjectController Single
  Writer/Undo/Semantic Diff/Provenance 迂回禁止、権限上
  Permit でも Semantic Validation 失敗時は Commit しない、
  複数 Grant の無条件合成による増権禁止、Proposal/Apply/
  Context/Job の各境界での認可（AHG-009）。
- Validity/Revocation/Commit 順序: Task Scoped 初期基本・
  Session Scoped/Persistent Project Scoped の表現対象化・
  無期限無制限の既定禁止、ユーザー失効・Agent 自己増権・
  無許可再委譲の禁止、失効・期限切れ・対象移動/削除・
  Policy 変更の評価、失効確定と Project Commit 最終認可の
  確定順序（TOCTOU 対策）、Job 取消/中断要求と完了済み
  外部送信・生成・Project Commit 取消の区別、Request ID/
  Idempotency Key による二重適用防止、最小必要監査情報
  （AHG-010）。
- 受入基準: AHG-011 の合意済み受入 8 件と、実装後見直しを
  許容しても暗黙に緩和しない不変条件の固定。
- AI Gateway の独立第四軸: Gate A/B/C との独立関係、AI Gate
  未完了でも A の制作・Playback/Recording/Undo/Export を
  壊さないこと、A の状態と独立に AI Gate を成功判定する
  場合も必要な ProjectController/Permission/Approval/
  Recovery/Security 受入を省略しないこと。
- AIB-001〜007/AHG-001〜011 と未決定事項の区別、#45/#46 への
  不変条件継承、case 写像・Owner・Fixture・Oracle・
  Pass/Fail/Unsupported/NotRun の evidence 台帳化。

### Out of Scope

- 実装コード（`crates/`、`Cargo.toml`、`Cargo.lock`、
  `mise.toml`、CI、Ruviform）。Command Gateway の型と実装は
  #45/#46 が Owner。
- GUI 実装・実 device/plugin・実 OS 受入・実外部 MCP
  Client/Provider・ユーザー承認 UI の製品受入（GUI-L2/
  EXT-L3、#46/#114 の別 Gate）。
- Ruviform 汎用 Gateway/認証 Provider の Synvelis 専用としての
  重複実装。推論 Backend・汎用許可機構・非 RT 処理・Media
  変換等の共通化は実需・適合性を検証して別途判断。
- 通常 DAW 利用への AI Grant/Provider 接続の強制。
- 「後続議論で決めること」「未決定の詳細設計論点」に列挙された
  未合意事項の確定（Grant 設定 UI・Context 取得対象・Privacy/
  Retention・Provider 契約・Agent 自律性レベル・具体 Budget
  値・MCP Tool Schema/Transport・各受入試験の実証結果等）。
- 特定 AI モデル/Provider/学習データの権利適法性の認定、
  生成物の著作権・商用利用権・「AI 不使用」の無条件保証、
  AI の既定実行・自動提案、Synvelis の DAW 固有意味論の
  Ruviform への移動。
- 既存 F0/E2E-L/R/X 実装順・受入 Gate・Ruviform 移行判断の
  暗黙変更。

## Requirements

Issue #44 の合意済み不変条件を AIB-001〜007 / AHG-001〜011
として転記・構造化する。各項目の現在の被覆状況は
`evidence/sv44/ai-native-baseline-case-ledger.md` を正本とし、
本 spec は要求のみを定義する。実装 Owner は #45（型契約）・
#46（MCP/Permission UX/RT/AI OFF 製品受入）および #23 配下の
実装 Issue であり、本 spec 自体は docs/ 設計契約 slice の
要求を定義する。

- **AIB-001 Manual/AI の対等な共存、AI なしで完結する DAW**:
  録音・再生・通常編集・MIDI・Mix・Export・Save/Load 等の
  通常 DAW ワークフローは Synvelis 管理下の AI 機能なしに
  成立する。手動編集と AI 編集は同一 Semantic Project 正本に
  作用し、AI 専用の別 Project 正本や通常検証を迂回する編集
  経路を作らない。AI 生成物も通常の編集可能な Asset/Clip/
  MIDI 等の制作対象として扱い、可能な範囲で通常編集・非破壊
  編集を継続できる。
- **AIB-002 Intentional AI Usage**: Synvelis 管理下の AI は
  初期状態で無効。明示的な AI 利用の意思・許可なしに推論・
  生成・AI 解析・AI 起因の外部データ送信を開始しない。
  「利用可能」「UI を開いた」状態と特定 AI 処理の「実行許可」
  を区別し、パネル表示・対象選択・通常 DAW 操作を暗黙の推論
  実行許可と解釈しない。通常アルゴリズムの失敗を理由に無許可
  で AI へ自動フォールバックしない。AI による処理内容・対象
  範囲・利用 Provider/モデル（判明する範囲）・ローカル/外部
  実行・外部送信の有無・対象データをユーザーが確認・制御
  できるようにする。AI を無効にしても既存 Project の手動利用・
  再生・保存・書き出しを不当に妨げない。
- **AIB-003 明示指示先行・将来の自発性は許可範囲に限定**:
  初期の AI 実行はユーザーの明示指示を起点とし、指示から
  合理的に導かれる必要な複数工程は許可された処理・データ範囲
  で扱う。将来の proactive/agent 動作は事前の明示的有効化と
  権限指定（対象・可能な操作・情報アクセス・実行条件/期間
  等）を要し、停止・取消可能。「自発的に提案できる」ことと
  「自発的に Project を変更・外部送信できる」ことは別権限。
  AI の制作上の判断を高度化しても最終的な制作上の制御・採否
  はユーザーに残す。
- **AIB-004 共通 Command/検証/変更履歴**: AI は UI の疑似
  クリックや Project Store の直接書換えを標準経路とせず、
  手動編集と共通の typed Semantic Command/Controller 契約を
  利用する。Stable Target ID・Expected Project Revision 等で
  対象・競合を検証し、権限外・旧 Revision・不整合な変更を
  暗黙適用しない。適切な粒度で Preview・Validation・Apply・
  Undo/Redo・Semantic Diff・失敗時の保護を可能にする。大規模
  な変更は分離した候補/Snapshot での比較を検討する。Project
  に実際に適用された状態と AI が提案・生成しただけの状態を
  区別し、Project 保存・VCS Commit・Engine Applied 等を
  同義にしない。
- **AIB-005 権利・来歴・透明性**: AI の使用許可、AI 生成物の
  利用条件・権利関係、実際の著作権上の適法性は別概念。
  Synvelis が把握できる範囲で AI 利用有無・生成/変換種別・
  Provider/モデル/バージョン・入力参照・生成物 ID・適用履歴・
  判明している条件/制限・外部送信の範囲を追跡できる設計を
  目指す。不明なモデル情報・外部素材の生成経緯・プラグイン
  内部処理・ライセンス/学習データ等は Unknown/Unverified/
  Unavailable として扱い、根拠なく「AI 不使用」「権利クリア」
  と保証しない。観測された事実・ユーザー/Provider の申告・
  AI による推定・未確認情報を混同しない。AI 関連の記録自体が
  秘密情報になり得るため、Prompt や送信素材の保存範囲・
  可視性・保持/削除方針は別途設計する。
- **AIB-006 AI 境界の技術的強制と RT 安全性**: AI 利用制御を
  UI 表示だけに依存させず、Synvelis 管理のモデル呼出し・
  Context 取得・生成・外部送信を通る共通の許可/ポリシー境界
  を設ける方向で設計する（具体 API 未決）。モデル実行・
  ネットワーク・外部サービス・生成 Asset の検証/登録・Agent
  Job は非 RT で扱い、Audio RT Thread で無期限待機・I/O・
  推論を行わない。生成した Audio 等は既存の Verified Media と
  Project Commit の境界に従い、未検証の生成物を正規 Source
  として無断公開しない。第三者プラグインや外部から取り込ま
  れた素材の内部 AI 利用は Synvelis が完全に観測・禁止できる
  とは限らず、「Synvelis 管理下で AI 処理を実行しない保証」
  とシステム全体・外部素材まで含む「AI 不使用保証」を区別
  する。
- **AIB-007 モデル非依存・Provider 交換可能性**: ローカル/
  クラウド、生成/解析、テキスト/MIDI/Audio 等の異なる能力を
  将来的に扱える設計を目指す。特定のモデル・事業者への永続
  依存を製品原則にしない。ただし具体的な Provider 抽象・
  機能差・権限差・コストの扱いは未決定。モデル選択や外部
  接続の有無が通常の DAW 制作を暗黙に変質させない。
- **AHG-001 Pluggable Hybrid**: Synvelis は DAW 正本・
  Capability/Command Gateway・ユーザーの AI 利用許可・
  Context の公開範囲・ChangeSet の検証と適用の権威を保持。
  手動 UI・外部 MCP Agent・将来の内蔵 Agent は共通の意味的
  操作契約を利用できるようにし、手動 DAW 操作を AI 有効化に
  依存させず、ProjectController の Single Writer/Undo/
  Revision 等の既存契約を再利用する。LLM・Prompt・Tool-
  Calling 反復・推論・計画などの Agent Reasoning Runtime は
  交換可能な責務として分離し、Synvelis 内部に固定の Agent
  Framework を必須にしない。DAW Job の所有・実行許可・結果の
  検証・登録・Project 状態整合は外部 Agent 任せにしない。
- **AHG-002 MCP Server 初期連携**: 初期連携方式に MCP Server
  を採用し、Synvelis 内部の型付き Gateway を公開する Adapter
  として扱う。MCP を内部の Project Model/Command 契約の正本
  としない。AI 利用・外部 Agent 接続は明示有効化・権限確認を
  要し、接続可能であること・Tool 一覧にあること・AI 推論・
  Project 読取・適用を許すことはそれぞれ別概念。MCP 接続/
  呼出し主体は信頼できる接続境界で特定し、外部入力の自己申告
  をユーザー本人の許可とみなさない。MCP Tool は Project の
  生データを無制限に公開せず、Context の限定的 Projection・
  権限付き操作・独立した長時間 Job Handle を基本とする。
  具体 Transport/認証/Tool 一覧は後続設計。
- **AHG-003 Gateway 論理契約**: Capability（操作 ID・入力/
  出力 Schema・対象種別・効果・必要権限・実行方式/利用可否）、
  Invocation（操作要求 ID・対象 Project/Stable Target ID・
  Expected Revision・引数・必要な冪等性キー。認証済み
  Principal/Origin/Grant は信頼境界で付与）、Authorization
  （Capability・Resource Scope・Execution Grant・Approval
  Policy・AI 実行・外部送信許可の検証）、Context（読取専用
  Snapshot/許可された Projection・Revision・観測/推定/不明の
  区別）、Mutation（Proposal→Validation/Preview→（必要に
  応じて Approval）→ProjectController Apply。通常 Undo/
  Semantic Diff に接続）、Execution（長時間 nonRT Job の
  進捗・取消・結果・Receipt。Result Ready/Project Committed/
  Engine Applied/Saved/VCS Committed を混同しない）の 6 要素。
- **AHG-004 3 Apply Mode の初期提供**: Suggest（Agent による
  提案/Preview。適用は別のユーザー操作。Agent に Apply 権限を
  付与しない）、Approve-to-Apply（提案単位で Synvelis 側が
  ユーザー承認を確認し、承認内容を再検証して適用。特定
  ChangeSet と承認の範囲のみ）、Delegated Apply（事前に
  ユーザーが許可した Principal/Project/対象/操作/期間・条件
  内で個別承認なしの適用。Scope と Grant の内側のみ）。
  3 モードは初期 MCP 連携から実際に動作させ、Delegated Apply
  を後続実装へ先送りしない（初期対象操作は限定してよい）。
  Execution Grant（誰が何を・どの範囲で・いつまで許されるか）
  と Approval Policy（実行ごとに承認が要るか）を別契約とする。
  Delegated Apply ≠ Proactive AI — 自動適用権限は AI が自主的
  に作業を開始する許可にならず、明示指示または別途明示的な
  開始権限が必要。3 モードすべてで Validation・Stable ID/
  Revision 確認・Undo・Semantic Diff・Provenance・権限失効
  確認・RT 安全性を省略せず、Agent が自分の Grant を増権
  できない。
- **AHG-005 既存設計との整合と実装境界**: Single Writer は
  従来どおり Synvelis ProjectController（#17）。Gateway は
  別の Project Writer・別の Undo 基盤を作らない。Synvelis の
  通常 UI Command/Selection/Focus（#2/#8）を維持し、Agent
  経由の入力は同じ検証された編集契約へ収束させる。Ruviform
  の Generic Runtime はそのまま利用し、MCP や Agent 意味論を
  自動的に共通基盤へ移さない。共通機能の抽出は全 Consumer
  要求と回帰を確認して別途判断する。既存 F0/E2E-L/R/X を
  変更したり、Issue の作成・追記を実装着手と解釈したりしない。
- **AHG-006 Explicit Target Resolution / Gateway は UI 状態
  非依存**: 共通 Capability/Command Gateway は Selection・
  Inspection Focus・Active Window・Active View・現在選択中等の
  可変な UI 状態から実行対象を暗黙推測しない。UI は独自の
  ViewState/Selection を持ってよいが、Command 発行前に
  Project ID・Typed Stable Resource ID・必要な時間範囲/
  Expected Project Revision へ解決する。MCP/内蔵 Agent/CLI/
  Test も同じ明示的対象指定の Command 契約へ収束する。複数
  Window/View があるため「現在の選択」は一意とは限らず、
  選択情報を外部に提供する場合は任意・別権限の明示的な
  Context API として扱い、Context 取得は Apply 権限ではない。
  Grant の対象は発行時の明示 ID・Scope に結び付け、UI で
  選択対象が変わっても Grant の対象は自動変更されない。既存
  Stable ID の所属関係が変化した場合は定義した構造条件を
  再評価する。MCP の接続状態や Agent の会話状態を Project の
  正本や UI Focus の代理にせず、明示 Handle/ID と Revision で
  状態を参照する。
- **AHG-007 Execution Grant/Policy の論理型と責務**:
  Authentication/Identity（検証済み Principal）・Execution
  Grant（可能な Capability・対象・情報アクセス・制限）・
  Activation Authority（誰が・どの Task/許可済み Proactive
  条件で AI 処理を開始できるか）・Approval Policy（個別承認が
  必要か）を別の契約とする。論理 Grant 属性: Grant ID・
  Schema/Policy Revision・Issued By・信頼境界で確認した
  Principal Binding・Project ID・明示的 Capability 集合・
  Resource Scope・Data Access・Activation・Approval・
  Validity/Revocation・Limits（具体 Rust 型・永続化構造は
  後続設計）。MCP クライアント認証済み≠Project アクセス許可
  済み。外部 Agent が自己申告する Principal/Task/「承認済み」
  は信頼せず、Synvelis 自身が認証・許可・承認の正本を保持
  する。Default Deny/Least Privilege — 新 Capability は既存
  Grant で暗黙に許可せず、通常 DAW UI の操作は AI 用 Grant に
  依存させず従来の CommandRouter/Controller 契約を保つ。Grant
  の許可と AI 処理の開始許可は独立で、Delegated Apply は
  Proactive 起動の許可ではない。明示 Task または別途明示許可
  された Proactive 起動がなければ AI は動作開始できない。
  Grant は外部情報開示/モデル実行/外部送信/Project 編集を同一
  権限として束ねず、外部 Agent へ情報を渡した後のモデル利用・
  保持は Synvelis が完全には検証できないため保証境界を明示
  する。
- **AHG-008 Resource Scope/操作効果の評価原則**: 対象指定は
  Typed Stable ID 基準で、Project→Track→Clip→Note 等の親子
  関係から権限を無条件継承しない。Asset/Source 等の共有参照
  や Plugin/Routing は単純なツリーで表せない。固定 Scope
  （Exact ID 集合・発行時 Snapshot に基づく固定集合）と制約
  付き動的 Scope（例: 特定 Track に現存する特定 Resource
  Kind・指定時間範囲/数量上限等）を初期から設計・実装対象と
  し、動的 Scope は明示条件で評価し新たに追加された対象が
  暗黙に無制限許可されないよう制約する。新規リソース作成は
  既存対象の編集と区別し、許可された親・Resource Kind・配置
  範囲を検証する。移動操作は変更前と変更後の所属・時間範囲
  を考慮する。Project 複製で Grant を別 Project へ自動移譲
  しない。Resource Scope に必要に応じて半開時間範囲を含め、
  Project Musical Time（Rational Beat 等）と Source Native
  Time を混同せず意図しない Quantize を発生させない。Agent
  の申告 Target だけでなく Synvelis 側の Command Validator が
  導出した構造的 Read/Create/Write/Delete・外部副作用を許可
  範囲に照合し、音響的な波及（例: Track Gain が Master 出力へ
  影響）と構造的な書き込み先は区別する。不明な/検証不能な
  副作用を Delegated Apply で自動許可せず、Plugin の不透明
  State や未検証 Media は Unsupported/Unknown 等を正しく表現
  する。
- **AHG-009 3 Apply Mode/Approval の評価**: Suggest は Agent
  が Proposal/Preview までで Apply 権限はなく、ユーザーが
  通常 UI で別途適用することは可能。Approve-to-Apply は
  Synvelis が特定の ChangeSet・Project・Principal・Expected
  Revision・期限に結び付けた承認を確認した後に適用し、
  変更案改竄・承認再利用を拒否する。Delegated Apply は
  ユーザーが事前に許可した Principal/Project/Resource
  Scope/Capability/期限・上限の内側で個別承認なしに適用可能
  とし、初期 MCP 連携でも実装対象とし Suggest/Approve-to-
  Apply と 3 モードすべての正常・異常系を E2E で検証する。
  3 モードとも Proposal/Validation・Apply 時の Revision/Grant
  再評価・ProjectController Single Writer・Undo/Semantic
  Diff/Provenance を迂回せず、権限上 Permit でも Project
  Controller の Semantic Validation に失敗する場合は Commit
  しない。複数 Grant を無条件に合成して増権せず、Proposal の
  認可/Apply の認可/Context や Job の認可を各境界で実施する。
- **AHG-010 Validity/Revocation/Commit の順序保証**: Task
  Scoped を初期基本にし、Session Scoped・Persistent Project
  Scoped も初期の Grant 設計の表現対象とするが、無期限・無制限
  を既定にしない。Persistent Grant は Project 保存物から別
  ユーザーへ自動移譲しない。ユーザーは Grant を失効でき、
  Agent は自己増権・無許可再委譲できず、失効・期限切れ・
  対象移動/削除・Policy 変更を評価する。TOCTOU 防止として
  失効確定と Project Commit の最終認可には確定順序を設け、
  失効が先に確定すれば後続 Commit を拒否し、Commit が先に
  確定した変更は失効だけで自動 Undo しない。単なる事前
  Check＋後段 Check だけで原子性を主張しない。非 RT の Job
  取消/中断要求と既に完了した外部送信・生成・Project Commit
  の取消を区別し、Project Revision/Save/Engine Applied/VCS
  Commit は別状態。Request ID/Idempotency Key を利用し、通信
  切断・Retry で二重適用を発生させない。監査は Principal/
  Grant/Task/ChangeSet/Commit 結果等の最小必要情報を記録し、
  Prompt/素材全文の無制限保存を前提としない。
- **AHG-011 受入基準と実装後の変更管理**: 初期 MCP 連携の
  最小対象は限定した MIDI 編集でよいが、初期から 3 モードを
  実際に提供し、最低限「受入基準」節の 8 件を自動試験/E2E
  対象とする。実装後の使い勝手に基づく見直し（Grant 設定
  UI・対象選択支援・確認ダイアログ・Grant 表示/操作導線・
  時間/回数上限の既定値・承認提示の粒度/バッチ UI・Error
  文言・操作の省力化・Scope 表現の利便性・内部型名/API 形状/
  最適化等）は Issue/設計変更として明示し、必要に応じて
  Migration・互換テストを行う。ただし意図しない AI 実行・
  外部送信禁止、UI 状態非依存・明示 ID、本人確認/最小権限、
  Delegated Apply と Proactive の分離、3 モード初期提供、
  Grant 取消と Commit の順序保証、ProjectController 単一
  Writer、非破壊性・履歴・RT 安全性、通常 DAW 経路の独立、
  全 Consumer 互換性は暗黙に緩和しない不変条件とする。
- **AI-GATE 独立第四軸**: AI Gateway は独立した第四軸であり、
  A の利用に AI 有効化・外部接続・Grant/Approval を要求しない。
  既存の AI Off 既定・明示指示・Execution Grant・
  ProjectController Single Writer・MCP 初期 3 Mode・RT 安全性
  を維持する。GW-01〜10 の headless 実装済み事実と、GUI での
  利用・MCP 実運用/Provider/すべての権限 UX の製品受入を区別
  する。AI Gate が未完了でも A の制作・Playback/Recording/
  Undo/Export を壊さず、A の状態と独立に AI Gate を成功と
  判定する場合も必要な ProjectController/Permission/Approval/
  Recovery/Security 等の受入は省略しない。

## Acceptance Criteria

Issue #44 の受入条件（AHG-011 合意済み受入 8 件）を AIB-AC-01
〜08 として転記・対応付ける。各 AIB/AHG 要件の現在の被覆状況は
`evidence/sv44/ai-native-baseline-case-ledger.md` を正本とし、
本 spec は要求のみを定義する。

- [ ] **AIB-AC-01** 未認証 Caller/Grant なし/他 Project・Scope
  外の対象/未許可 Media Read・外部送信が拒否される設計・
  写像・証拠が存在する。
- [ ] **AIB-AC-02** UI Selection の変更で既存 Invocation
  Target/Grant の対象が変わらない設計・写像・証拠が存在する。
- [ ] **AIB-AC-03** Track の権限が Clip・Asset へ無条件継承
  されず、固定 Scope・制約付き動的 Scope が定義どおり評価
  される設計・写像・証拠が存在する。
- [ ] **AIB-AC-04** Grant 範囲外への Note 移動・新規 Clip
  作成・Resource 所属変更・Unknown Effect が安全に判定される
  設計・写像・証拠が存在する。
- [ ] **AIB-AC-05** Suggest で Agent Apply 不可、Approve-to-
  Apply は該当 ChangeSet 承認のみ、Delegated Apply は Grant
  の内側のみ自動適用される設計・写像・証拠が存在する。
- [ ] **AIB-AC-06** Delegated Apply のみで Proactive 起動
  できない設計・写像・証拠が存在する。
- [ ] **AIB-AC-07** Grant 失効/Project Revision 競合/承認済み
  ChangeSet 改竄/同一 Request 再送が安全に処理され、意図
  しない二重適用や権限外 Commit がない設計・写像・証拠が
  存在する。
- [ ] **AIB-AC-08** AI/MCP 無効時にも通常 DAW 操作・既存
  Project Save/Playback/Export が継続できる設計・写像・証拠が
  存在する。

## Deferred Design Concerns

Issue #44 が「後続議論で決めること」「未決定の詳細設計論点」
として保持する未合意事項。**これらは現行の不変条件に対する
独立した受入 Gate ではなく**、実装前の設計課題・将来候補で
ある。確定は #45（型契約）・#46（実装計画/製品受入）または
後続設計 Issue が Owner。

- AI 使用許可の粒度（操作ごと/セッション/Project）・権限の
  期限・失効と同意 UI の具体形 | 未決定 | #46/#114 | 既定値を
  実測なしに確定し、後で緩和する誤り。
- Context の取得対象・機密データ保護・外部送信範囲・Privacy/
  Retention/ログ方針 | 未決定 | #45/#46 | 未許可 Media/Prompt
  全文の無制限保存を前提にする誤り。
- 学習済み解析と生成 AI の区分・第三者 Plugin への表示/制御
  能力・「AI 不使用」の正確な保証範囲 | 未決定 | #44 継続議論・
  #46 | 観測不能な外部 AI 利用を保証したと誤読する誤り。
- AI 編集 ChangeSet/Preview/Apply/競合解決/Undo・長時間 Job
  中の Revision 変化の詳細 | #45 型契約待ち | #45/#46 |
  headless 契約を製品 UX 完成と誤認する誤り。
- 生成 Asset の Provenance Schema・モデル/ライセンス情報の
  Unknown 表現・権利確認 UI | 未決定 | #45/#46 | Unknown を
  確定値で埋める誤り。
- Model Provider と Capability 契約・ローカル/クラウド切替・
  コスト・制限・障害時挙動 | 未決定 | #45/#46 | Provider 接続を
  通常 DAW 利用へ強制する誤り。
- Agent の自律性レベル・提案と実行の権限分離・実行予算・
  中断・人間の介入の詳細 | 未決定 | #45/#46 | Delegated Apply を
  Proactive と混同する誤り。
- 最初の最小 AI E2E PoC/失敗系テスト/他 Consumer 互換テストと
  既存実行計画への接続 | #23/#46 | #46 | F0/E2E 計画を暗黙変更
  する誤り。
- Grant/Approval/ChangeSet/Context/Job の具体 Rust Schema・
  認証方式・動的 Scope の安全な評価効率・Policy と
  ProjectController の線形化手法・Approval Receipt の保持/
  失効・永続 Grant と監査ログの保存/秘匿・具体 Budget 値・
  UI・MCP Tool Schema と Transport・各受入試験の実証結果 |
  未確定 | #45/#46 | 本 spec の不変条件を実装済み API と
  誤認する誤り。

## Sources and Relations

- Issue #44 本文（AIB-001〜007・AHG-001〜011・合意済み受入
  8 件・未決定論点・Owner 境界・検証方法・非対象）。
- `docs/designs/gw-01-gateway-contract-design.md`・
  `docs/designs/gw-02-authorization-policy-design.md`（GW-01/
  GW-02 の headless 契約設計 — 本 spec の不変条件を継承する
  実装側設計）。
- `docs/impl/ir/gw-01-gateway-contract.md`〜`gw-10-gateway-e2e-
  test-harness-evidence-gates.md`（GW-01〜10 の実装記録 —
  headless 実装済み事実の正本）。
- 確定済み縦断契約: `docs/specs/product-value-core.md`（#1 —
  Gate A/B/C）・`workspace-model.md`（#2）・`context-inspector.md`
  （#5）・`timeline-editing.md`（#11）・`project-model.md`（#10）・
  `project-persistence.md`（#17）・`playback-graph.md`（#18）・
  `musical-time.md`（#19）・`transport-integration.md`（#20）・
  `media-render.md`（#21）・`vcs-02-semantic-diff-changeset.md`
  （#7）・`ed-a-editing-model-*.md`（#38）。
- 関連 Issue: synvelis #1/#2/#5/#7/#8/#10/#11/#17/#18/#19/#20/
  #21/#22/#23/#38/#44/#45/#46/#52/#53〜#62/#83/#114、
  ruviform #192/#219/#294、audio-lens #127。
