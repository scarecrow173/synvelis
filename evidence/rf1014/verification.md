# rf1014 — formal verification policy reflection verification record

`scarecrow173/ruviform#1014` の cross-repo 反映のうち、Synvelis slice
の検証記録。本 slice は POLICIES・verify skill・formal verification
spec の整合を行う文書変更であり、製品コード・proof harness・runner・
mise lane の再設計は行わない。Journal/Authorization 検証の移行は
`scarecrow173/synvelis#659` が担う。

- Date: 2026-11-25
- OS/Arch: Windows 11 x86_64（MINGW64_NT-10.0-26300）
- Base SHA: `36b7da5495fe4bac565e63066524e71edb3539ba`（`origin/main`。
  着手前に `git fetch origin` 後の `git rev-parse origin/main` で確認）
- Branch: `scarecrow173/rf1014-sv-policy`
- PR head SHA: push 後の PR 本文に記録する。本文書はコミット内容に
  含まれるため、最終 head SHA を本文書内で自己参照しない。
- ローカル検証 = 変更範囲の検証。統合検証は merge 後の対象 SHA に
  対する必須 CI 結果が証拠（本記録はそれを代替しない）。

## canonical source と転記忠実性

1. `gh issue view 1014 -R scarecrow173/ruviform --json body --jq .body`
   で canonical source を再取得した。転記対象は `#### 共通原則` から
   `#### 開発工程・管理場所・責務` までの 6 節。
2. `POLICIES.md` §6.1〜§6.6 から見出し行を除いた非空行と、canonical
   source の対応する 6 節の非空行を Python で逐語比較した。結果は
   全 6 節 `MATCH` で `verbatim-body-check: PASS`（共通原則 10 行、
   検証粒度・境界 2 行、手法選定・契約分解 2 行、健全性・検証可能性
   2 行、コスト・証跡・同期 2 行、開発工程・管理場所・責務 4 行）。
3. 比較は rule の省略・追加を行わないことを確認するために実施した。
   編集上の差異は `## 6.` と `### 6.x` の構造化見出しだけであり、
   synvelis 固有の例・製品名・注記を共通本文へ混入していない。
   Synvelis 固有の適用は renumber 後の §7 に分離されている。

## renumber と参照確認

- 旧 `## 6. Synvelis固有の適用` は `## 7. Synvelis固有の適用` に
  renumber した。内容は変更していない。
- 修正した参照:
  - `docs/designs/rcp-render-recipe-design.md`: `POLICIES.md §6` →
    `POLICIES.md §7`
  - `docs/adr/rcp-00-render-recipe.md`: `POLICIES.md §1/§6` →
    `POLICIES.md §1/§7`
- 確認コマンド:
  `grep -RIn --include='*.md' -E 'POLICIES(\.md)?[^\n]{0,30}(§6|section 6|Section 6|6章)' .`
  （`apm_modules` を除く）。
- 修正しなかった参照:
  - `evidence/sv289/render-checkpoint-contract-trace.md` の
    `POLICIES.md` §1/§6 — 歴史 evidence であり、当時の参照として
    書き換えない。
  - `docs/adr/ed-a-...` の issue #11 §6、`docs/adr/pc-a-...` の #6
    §6、`docs/adr/vs-a-...` の #17 §6、各 design doc 自身の §6 —
    `POLICIES.md` の section 6 を指さないため変更しない。
  - `apm_modules/` 配下の §6 — 依存パッケージ内部の参照であり、
    lease 外かつ `POLICIES.md` の section 6 を指さない。

## AGENTS.md の編集是非

`AGENTS.md` は編集しなかった。`AGENTS.md:5` が Issue 起票・改訂、
設計、実装、テスト、レビュー、受入判定で `POLICIES.md` を読むことを
必須にしており、`AGENTS.md:7` が詳細の重複記載を禁じている。新しい
形式的検証原則は `POLICIES.md` §6 に正本として入ったため、既存の
必須参照 pointer が導線として足りると判断した。`AGENTS.md` には
Kani/proptest/TLA+/Lean/formal verification/昇格に関する記述がなく、
旧昇格路との矛盾も見つからなかった。

## docs/specs/README.md の扱い

全 index 再生成は行わなかった。`docs/specs/formal-verification.md`
の front matter title から固定 layer framing を取り除いたため、
`SPEC-sv601frmlvrf0slice1` の 1 行だけを手動で title/status/file に
整合させた。status は `proposed` のままである。既存の無関係な行や
index drift には触れていない。

## formal-verification.md / verify-synvelis SKILL.md の reframe

- front matter title と intro: 「D 層」「property-based testing →
  bounded model checking → 将来の TLA+/Lean 昇格」「第一弾」という
  固定 layer/昇格 framing を除去し、rf#1014 / `POLICIES.md` §6 を
  上位ポリシー、本文書を Issue #601 の implementation spec、記載 lane
  を issue-specific lane と明示した。再設計・移行は
  `scarecrow173/synvelis#659` に委譲することを明記した。
- Lane 構成: `test-formal` / `test-formal-kani` の「第一層」「第二層」
  表現を、property-test lane と bounded-proof lane という実行事実の
  表現へ改めた。
- Model 対 Product: 将来の昇格を既定経路として期待する表現を、
  TLA+/Lean 等の model-based technique を Property へ選ぶ場合の
  条件付き記述へ改めた。
- 判定語彙: 実行 verdict と保証種別・対象・同期状態・失敗原因の独立
  軸を補った。既存 verdict 語彙は rename していない。環境欠如を
  `not_run` + `environment_blocked` で記録する正直な posture は保存した。
- 「昇格経路」節: 固定順序ではない tool-fit 候補評価として reframing
  し、Kani/TLA+/Lean/proptest 継続の候補記録は保存した。実際の
  再設計は `scarecrow173/synvelis#659` に委譲する旨を明記した。
- 末尾の証跡 practice: 「昇格先にも適用する」を「Property に対して
  選定された検証手法にも適用する」へ改めた。
- verify skill: Issue #601 lane が固定 promotion path ではない boundary
  note、実行 verdict と assurance の分離、現行 runner の失敗原因分類が
  限定的であること、変更影響評価・再検証・検出能力確認・証跡同期の
  procedure hook を追加した。原則本文は転記せず、`POLICIES.md` §6 と
  `docs/specs/formal-verification.md` への navigation に留めた。

保存した既存内容: 契約表、proptest/Kani の明示的 bound、仮定、
Model 対 Product の区別、環境 provisioning、mutation-fail 証跡規則、
正直な NotRun posture、coverage 記録。これらは削除せず、reframe で
整合させた。

矛盾確認: `proofs held`、`proptest → Kani → TLA+ → Lean` を肯定する
指示、「D 層」「第一層」「第二層」「昇格経路」「昇格候補」「将来の
昇格」「昇格先」「昇格は保留」を対象 2 文書で grep し、有効な指示
として残っていないことを確認した。`POLICIES.md` §6 に「固定的な
proptest → Kani → TLA+ → Lean の昇格路を廃止する」という canonical
本文の否定形が残るのは意図どおりである。
`docs/specs/formal-verification.md` には固定昇格路ではない旨の否定形
だけが残る。

## 検証結果

| Command / check | OS | Exit | Verdict | 備考 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows x86_64 | 0 | Pass | docs/policy-only 変更の軽量 hygiene。Rust 差分なし |
| `mise exec -- prek run --all-files` | Windows x86_64 | 0 | Pass | trailing whitespace / EOF / YAML / TOML / mixed line ending / BOM / private key / actionlint 全 pass。auto-fix 差分なし |
| canonical 6 節逐語比較（Python） | Windows x86_64 | 0 | Pass | `verbatim-body-check: PASS` |
| `git diff --check` | Windows x86_64 | 0 | Pass | whitespace error なし（補助確認） |
| Workspace Build/Test | — | — | NotApplicable | 文書・ポリシーのみの変更。`POLICIES.md` §5 により必須外 |
| `mise run test-formal` | — | — | NotRun | コマンド参照の smoke は任意。製品コード・proof・lane 不変のため未実施 |
| `mise run test-formal-kani` | — | — | NotRun | manual/opt-in かつ環境依存。製品コード・proof・lane 不変のため未実施 |
| GUI-L2 / EXT-L3 / 3OS / Full CI | — | — | NotApplicable/NotRun | 文書変更に新たな物理機材・GUI ゲートは不要。統合検証は merge 後対象 SHA の CI 証拠 |

## 範囲確認と残件

- 変更は `POLICIES.md`、`.agents/skills/verify-synvelis/SKILL.md`、
  `docs/specs/formal-verification.md`、`docs/specs/README.md` の 1 行、
  `docs/designs/rcp-render-recipe-design.md`、
  `docs/adr/rcp-00-render-recipe.md`、本 evidence に限定した。
  `crates/`、`tools/`、`mise.toml`、`.github/`、`Cargo.toml`、
  `Cargo.lock`、他 skill、他 evidence は変更
  していない。
- 固定階層・専用 DSL・独自フレームワーク・全 proof 必須 CI は追加して
  いない。製品コード・公開 API・永続化形式・個別 proof/Stub/Bound/
  assume・既存 RT/精度/互換性要件も変更していない。
- `mise.toml` の `D-layer` 表記は既存 lane metadata であり、本 slice
  では変更しなかった。再設計は `scarecrow173/synvelis#659` に委譲する。
- `docs/specs/formal-verification.md` の front matter は `created:
  2026-11-23` が旧 `updated: 2026-11-21` より後という pre-existing の
  日付順序不整合だった。本 slice は `updated` を 2026-11-25 に更新した
  が、`created` の訂正は metadata 判断の範囲外として行わなかった。
- docs index は生成物であり無関係な drift が残る可能性がある。本 slice
  は `SPEC-sv601frmlvrf0slice1` の 1 行だけを整合させ、全 index 再生成
  は Coordinator の統合整備に委ねる。

## 関連リンク

- `scarecrow173/ruviform#1014`: 共通原則の canonical Issue
- `scarecrow173/synvelis#659`: Journal/Authorization 検証再設計・移行 Issue
- PR: push 後に PR 本文へ記録
