# Playback / Mixer / Generic Execution Graph — 検証記録 (Issue #18)

作成: 2026-11-13 / branch: `scarecrow173/sv18-playback-graph`
workspace: `C:\Users\akiyoshi\orca\workspaces\synvelis\sv18-playback-graph`
OS: Windows（MINGW64）/ toolchain: rustc（mise 管理）

## 1. 実行した検証

| # | コマンド | 目的 | 結果 |
| --- | --- | --- | --- |
| V1 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply --dir docs/{specs,designs,plans,tasks,test-specs,impl/ir}` | 触れた dir の index 再生成 | **7 dir 実行・6 written・0 blocked**。`docs/impl/exp` は `.jsonl` のみ index 対象の既存規約のため `.md` experiment record は index されず差分なし（`0 written`、sv10 と同一規約） |
| V2 | `node .agents/skills/doc-status/scripts/doc_status.js lint --type {spec,design,plan,task,test-spec} --json` | 新規文書の front matter/relation 検査 | 初回 playback-graph 起因 finding を修正後 **0 件**。修正内容: (a) spec `traceability-missing-upstream` → `derives-from` に architecture baseline + issue を追加、(b) plan `plan-missing-test-spec-evidence` → test-spec を `verifies`→`verified-by` へ移動、(c) test-spec `broken-relation-link`×2 → `defers` の prose 文字列を除去、(d) test-spec `inconsistent-reciprocal-relation` → design `derived-by` に test-spec を追加 |
| V3 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | IR 監査 | **0 件**（playback-graph 起因 finding なし） |
| V4 | `node .agents/skills/doc-status/scripts/audit_docs.js --type all --json` | 文書 audit | **0 件**（playback-graph 起因 finding なし） |
| V5 | `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus playback-graph --json` | 文書 graph router | **status: blocked**（`current: probe`、`edgeId: null`、pre-existing `artifact-graph`/`broken-relation` — repo baseline、範囲外。case-ledger §4 に記録） |
| V6 | `mise run fmt-check` | canonical format 検査（docs slice） | **exit 0 / Pass**（`cargo fmt --all -- --check`） |
| V7 | review-protocol（spec+design+chain 全体、cooperative/adversarial 分離 pass → validate → judge） | 文書チェーンレビュー | **1 サイクル・verdict: ACCEPT WITH NON-BLOCKING NOTES**。CONFIRMED P0/P1 blocker なし。確認した非 blocker: (a) spec/design の `verified-by` 逆リンクは test-spec 側 `verifies` で宣言済み・lint 非要求、(b) EX-015→`rcd_04_rt_comp_pcm` は最も対応が弱い写像で「Pass(部分)」で限定済み、(c) EX-001 の「過去記録」表記は fresh 計測でなく既存 fixture 対応の設計判断であることを明示 |

## 2. Baseline（既存・本タスク範囲外）の finding

| 項目 | 内容 | 扱い |
| --- | --- | --- |
| `doc-driven-dev-graph` route | `status: blocked`、pre-existing `artifact-graph`/`broken-relation` 多数（automation-modulation-design.md 等の無関係文書の `defers` が docs パスとして解決不能） | repo baseline。本タスクで修正しない。case-ledger §4 に記録 |
| `doc_status lint` repo 全体 | 既存文書の warning/error が baseline として残る | 既存 baseline。playback-graph 起因は V2 で 0 件に修正済み |

## 3. NotRun / Deferred レーン（POLICIES.md §2/§3 語彙）

| レーン | 状態 | 理由・後続 Owner |
| --- | --- | --- |
| headless fixture 照合（EX→既存 test 写像） | **Pass（設計レビュー + 既存 fixture 対応確認）** | 本 slice は docs のみ。既存 `sf03_*`/`tim*`/`l1_e2e`/`l2_e2e`/`x_e2e`/`sf04_*`/`rcd_04_*` fixture との対応を case-ledger に記録。新規 test 実行は実装 slice の責務 |
| design-review（文書チェーン静的レビュー） | **Pass** | V2/V3/V4/V7 + review-protocol |
| 実 OS audio device / 実 PCM device | **NotRun** | synvelis#114・EXT-L3・ruviform#294 |
| 実 VST3/CLAP plugin・native plugin dock | **NotRun** | EXT-L3・synvelis#114・tim-04・ruviform plugin host・synvelis#101 |
| GUI automation・Signal Flow UI・timeline | **NotRun/Deferred** | synvelis#114・#108/#112 |
| 実 MIDI device backend | **NotRun** | ruviform#253・tim-02 |
| 実 RT latency/underrun・device epoch restart 実測 | **NotRun** | ruviform#219/#294・synvelis#294 |
| 実 WAV finalize・実 storage durability | **部分** | headless reopen verify あり・実 durability fault は外部 gate（synvelis#84/#330） |
| macOS/Linux lane | **Deferred** | POLICIES.md — 使用要件未発生 |
| Ruviform arbitrary-port DAG 完成度 | **NotRun** | upstream 受入 ruviform#184/#308/#309・#294 |
| stem export 個別 job | **Deferred** | L2 は master のみ・stem は別 Issue |
| SF-ACT-07 preview vs export policy | **Deferred** | policy 未規定・後続 Issue |
| CI（merge 後対象 SHA） | **NotRun pending** | ローカル検証は変更範囲の検証。統合検証は merge 後 main SHA の必須 CI 証拠（Coordinator 確認） |

## 4. 判定まとめ

- 文書チェーン 7 文書 + `evidence/sv18/` 2 件が存在し、Issue #18 の
  EX-001〜020 が test-spec case・既存 Synvelis/Ruviform test・独立
  oracle・現在の観測状態・Required/Deferred/NotApplicable・
  Pass/Fail/NotRun/Unsupported に漏れなく写像される
  （case-ledger 参照）。
- `mise run fmt-check` exit 0。`doc_status lint`/`audit_impl_record`/
  `audit_docs` の playback-graph 起因 finding は 0 件。
- review-protocol: 1 サイクル、CONFIRMED P0/P1 blocker なし、
  verdict ACCEPT WITH NON-BLOCKING NOTES（V7 参照）。
- 実装・実 OS 検証の完了は一切主張しない。後続 lane は
  synvelis#104〜109・#114・#84/#330・ruviform#184/#219/#253/#294/
  #308/#309 に委譲。
