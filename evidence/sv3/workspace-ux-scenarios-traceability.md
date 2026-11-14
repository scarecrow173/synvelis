# Workspace UX Scenarios — 受入項目→シナリオ→仕様節 追跡表 (Issue #3)

作成: 2026-11-14 / branch: `docs/3-workspace-ux`
対象: `docs/specs/workspace-ux-scenarios.md`・
`docs/designs/workspace-ux-scenarios-design.md`・
`docs/test-specs/workspace-ux-scenarios.md`

## 1. WUX-AC → Requirement → Design 節 → Scenario → Issue §

| WUX-AC | spec 要件 | design 節（正本） | 関連 Scenario | Issue #3 対応 |
| --- | --- | --- | --- | --- |
| WUX-AC-01 | R1 | §5 Docking 操作仕様（WUX-DK-01〜10・§5.2 Preview・§5.3 rollback・§5.4 代替経路） | WUX-SCN-A/D | §2 Docking 操作 + §9-1 |
| WUX-AC-02 | R2 | §6 Window/View 閉鎖仕様（§6.2 Hidden/Discarded・§6.3 detached pool・§6.4 Shutdown） | WUX-SCN-A/D | §3 Window・View を閉じる操作 + §9-2 |
| WUX-AC-03 | R3 | §4.3 Inspector 状態図 + §7.1 遷移規則 + §7.2 選択変更時挙動表 | WUX-SCN-B/C | §4 Follow/Explore/Pinned + §9-3 |
| WUX-AC-04 | R4 | §8 Link Group/Linked Context（§8.1 UI・§8.2 同期範囲分離） | WUX-SCN-E | §5 Link Group と Linked Context + §9-4 |
| WUX-AC-05 | R5 | §9 Focus/Keyboard/Context Menu/IME/Plugin GUI 入力境界（§9.1 状態表・§9.2 Target 明示・§9.3 Tool 境界） | WUX-SCN-F | §6 キーボード・メニュー・フォーカス + §9-5 |
| WUX-AC-06 | R6 | §10 Layout 保存・復元（§10.1 3+1・§10.2 復元順序・§10.3 モニター欠落・§10.4 Undo 関係） | WUX-SCN-A/D | §7 レイアウトの保存・復元 + §9-6 |
| WUX-AC-07 | R7 | §11 代表シナリオ WUX-SCN-A〜F（手順・Expected Oracle・検証レーン） | 全件 | §8 代表シナリオ + §9-7 |
| WUX-AC-08 | R8 | §13 Owner 境界と検証レーン + §12 Decision Log D19/D20 | — | §9-8 Ruviform 境界 |
| WUX-AC-09 | — | 文書チェーン spec→design→plan→task→test-spec→ir/exp + 本 evidence | — | —（タスク固有） |
| WUX-AC-10 | R8 | design §11 検証レーン語彙（real-os = 現時点 NotRun）+ §15 Deferred + 本 verification 記録 | 全件 | §9 受入「未検証は Pass としない」+ POLICIES §2/§3 |

## 2. Issue #3 §別 → 設計カバレッジ

| Issue 節 | 内容 | 設計側の対応 |
| --- | --- | --- |
| §1 承認済み UX 方向性 (1-6) | 自由配置・予測可能な作用先・調査/編集分離・文脈継続・復旧可能性・連携は明示的 | design §3 操作分類原則・§4.5 Command 解決・§5〜§10 全般 |
| §2 Docking 操作 | 操作表 8 項目 + 安全条件 6 項目 | design §5.1 WUX-DK-01〜10・§5.2・§5.3・§5.4（メニュー 7 項目を全網羅） |
| §3 Window/View 閉鎖 | 4 操作区分 + detached pool + CloseRequested/Destroy 分離 | design §6.1〜§6.5・§4.2 Window 状態図 |
| §4 Inspector Follow/Explore/Pinned | 3 状態 + 遷移案 + ヘッダー UI | design §4.3 状態図・§7.1〜§7.3 |
| §5 Link Group/Linked Context | Group UI・同期例・分離・Zoom/Scroll 独立・通知 dedup | design §8.1〜§8.3・§4.5 |
| §6 キーボード/メニュー/フォーカス | 入力境界優先・Delete 規則・IME・Undo 系統・Context Menu Target・Plugin GUI | design §9.1 状態表・§9.2・§9.4 Undo 3 系統 |
| §7 Layout 保存/復元 | Default/Saved/Last Session・Project 分離・モニター欠落補正 | design §10.1〜§10.4 |
| §8 シナリオ A〜F | 手順概要 + 見るべき結果 | design §11 WUX-SCN-A〜F（Expected Oracle で具体化） |
| §9 検証/完了条件 | 8 チェック項目 | spec WUX-AC-01〜08 へ写像（§9-7 は WUX-AC-07、残りは各 AC） |
| §10 未決定事項 | 6 項目 | design §15 Deferred Design Concerns + spec Deferred Design Concerns |
| 2026-11-10 実行判断 | Worker が設計ドラフト〜review-protocol〜PR まで担当・新規トレードオフは分離 | design §12 D17〜D20（分離/未決定）・本タスクの実施形態 |

## 3. 既存 docs との整合確認

| 既存文書 | 確認内容 | 結果 |
| --- | --- | --- |
| `docs/designs/workspace-docking-state-architecture.md`（統合設計） | ViewInstanceId/WindowId/DockTree・状態正本・Command 契約・§5.1/§5.2/§6/§7.2/§7.3 の参照先として整合。UX 規範を重複定義せず前提参照に留めた | 整合（矛盾なし） |
| `docs/specs/ws-a-workspace-foundation-*.md` + test-spec | WS-A-01〜06 の headless 固定契約（View 独立性・Link Group・Command 解決・Detach/Rollback/Veto/Snapshot）を前提として参照。新規 UX 挙動の headless 検証は後続 slice と明記 | 整合 |
| `docs/specs/ed-a-editing-model-*.md` | Smart/Dedicated Tool の HitTest/Gesture 契約（ED-A-02/04/05）を Dock ジェスチャとの空間分離の根拠として参照 | 整合 |
| `docs/specs/sv-gpui-03-product-theme-workspace-presets.md` + `docs/designs/sv177-*.md` | Workspace Preset を Layout 3 概念とは別 schema として位置づけ（design §10.1）。monitor 補正は既存 `restore_or_default`/signature fallback を再利用し新規補正規則を定義しない（§10.3） | 整合 |
| `docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md` | #113（closed）の実装済み dock op 経路を「実装経路は存在するが操作面の UX 規範は未文書化」として位置づけ、本書がその正本であることを spec Why Now に明記 | 整合 |
| `docs/mockups/synvelis-daw.html` | 旧 HTML Mock は設計参考であり正式仕様ではない（Issue 2026-11-07 v2 明記）。本書は Mock に依存せず、Mock 更新は対象外（spec Out of Scope） | 整合（Mock 非依存を確認） |
| POLICIES.md §2/§3 | Required/Deferred/NotApplicable・Pass/Fail/NotRun/Unsupported の語彙。real-os/Plugin GUI 未検証を NotRun/Deferred として記録し、設計評価を実機 Pass に換算しない | 整合 |

## 4. 未決定事項の所在（Deferred）

| 項目 | Issue § | design 節 | spec 節 | 確定先 |
| --- | --- | --- | --- | --- |
| Drop zone 寸法・ドラッグ閾値・Split 最小サイズ | §10 | §15 | Deferred | 実機/UI PoC（#113/#114） |
| 閉じた Window 再表示 UI・復元履歴保持期間 | §10 | §15・§6.3 | Deferred | 後続 UX 検証 slice |
| Explore 履歴深さ・Follow 元選択変更時の通知表現 | §10 | §15・§7.1 | Deferred | #5 |
| Link Group 複数所属・タイムライン連携・詳細設定 UI | §10 | §15・§8.1 | Deferred | #3 §10・#11 |
| Layout Undo 履歴範囲・Workspace preset 細部 | §10 | §15・§9.4/§10.1 | Deferred | #177 運用確定後 |
| OS 別 Window ドラッグ・Focus・Plugin GUI 互換性 | §10 | §15・§9.1 | Deferred | #114/#176 実機 lane |
| 新規製品トレードオフ・契約破壊・定量 SLO・OS サポート宣言 | 2026-11-10 判断 | §12 D20 | Out of Scope | 人間決定待ち（承認を捏造しない） |
