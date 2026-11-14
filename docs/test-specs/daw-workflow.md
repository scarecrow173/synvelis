---
id: "TSPEC-034X9DglWzVt6ACpSlw4YC"
type: "test-spec"
status: "approved"
title: "標準 DAW 制作ワークフロー UX 仕様の文書契約検証"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/8"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/daw-workflow-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/daw-workflow-plan.md"
    - "docs/tasks/daw-workflow-task.md"
    - "docs/impl/ir/daw-workflow.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/daw-workflow-design.md"
  derived-by: []
  verifies:
    - "docs/specs/daw-workflow.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# 標準 DAW 制作ワークフロー UX 仕様の文書契約検証

## Purpose

`docs/specs/daw-workflow.md`（WF-R/WF-AC）と
`docs/designs/daw-workflow-design.md`（GA/WF-SCN/§7〜§10）
が Issue #8 の受入条件を **検証可能な形で**規定していること
を保証するテスト仕様。本書は (a) 本文書チェーン自体の
決定論的検証（doc_status/index/fmt）と、(b) 将来の実装が
満たすべき製品契約（OS-L1/GUI-L2/EXT-L3 の各レーンでの
受入 Oracle）を定義する。実機受入は #114/#176 lane に委譲
し、本書では文書契約と将来検証の枠組みのみを固定する。

## Feature

Gate A「Core DAW Creation」の標準制作ワークフロー —
Audio/MIDI Track 作成 → Import/Record → 非破壊編集 →
Mixer/Transport → Save/Reload → Offline WAV Export →
独立 Reader 検証 — が、AI・Signal Flow 診断・VCS・Remote
Browser を OFF/不通にした状態で完結する UX 契約。

## Rules

- **TS-R-01 文書網羅性**: Issue #8 の受入条件 7 項目が
  WF-AC-01〜07 に写像され、WF-AC-08（Keyboard/Gesture/
  Tool UX）・WF-AC-09（文書チェーン）・WF-AC-10（証拠の
  誠実さ）が追加要件として存在する。
- **TS-R-02 独立 Oracle**: Gate A backbone GA-01〜08 の
  各工程に、実装内部状態に依存しない Expected Oracle
  （ファイル存在・hash 不変・WAV ヘッダ/PCM sanity・
  独立 Reader decode 等）が定義されている。
- **TS-R-03 診断非依存**: WF-SCN-MIDI/AUDIO/MIX の spine
  が Signal Flow・Inspector・Version Diff・AI・VCS を
  要求しない。detour は文脈保持の任意遷移。
- **TS-R-04 Window/Dock 独立性**: Window/Dock 移動・
  View 開閉・Focus 変更が Recording/Transport/Project
  Revision/編集対象を変更しない（Navigate/Arrange 分類）。
- **TS-R-05 共通操作体系**: Track/Clip/MIDI Note/Take の
  選択・編集 Command・Undo が、Gesture 開始時に固定した
  対象 ID+Revision に一貫適用され、Conflict は拒否される。
- **TS-R-06 非破壊・回復**: Source/Asset は不変、編集は
  derived projection。録音ファイルは Staging→Journal→
  Finalize→Commit の回復可能ライフサイクルを持つ。
- **TS-R-07 Routing/Meter 分離**: Mixer の Routing 表示は
  Semantic Routing Model と一致し、通常 Channel Meter と
  明示的 Probe/Capture を区別する。Signal Flow OFF でも
  Mixer は成立する。
- **TS-R-08 Remote/VCS 耐性**: Remote Source 不通でも
  Local Import/録音/編集/保存/再生/Export が成立し、
  Missing は正直に表示される。VCS 非依存で Undo/Save/
  Load/Export/Transport が動作する。
- **TS-R-09 Keyboard/Gesture/Tool 安全**: 最小操作集合の
  各操作が入力コンテキストでゲートされ、Gesture は
  Preview→Atomic Commit→1 Undo、Esc は階層的 Cancel。
- **TS-R-10 証拠誠実性**: 実装・実機・実 GUI で未検証の
  項目は `NotRun`/`Deferred`/`Unsupported` として記録され、
  headless/設計評価を実機 Pass に換算しない
  （POLICIES.md §2/§3）。

## Examples

- **EX-1（WF-SCN-AUDIO 正常系）**: Audio Track 作成 →
  Local WAV Import → Clip trim/gain/fade → Transport 再生 →
  Mixer で Gain 調整 → Save → Reload → Offline WAV Export →
  独立 WAV Reader で decode 成功。全工程で Signal Flow/
  AI/VCS OFF。Oracle: 元ファイル hash 不変・復元等価性・
  WAV ヘッダ/PCM sanity。
- **EX-2（WF-SCN-MIDI 正常系）**: MIDI Track 作成 → Note
  入力/編集 → Plugin 割当 → 再生/Mixer → Save/Reload →
  Export → 独立検証。Oracle: Note 配置・Undo 復元・WAV
  decode。
- **EX-3（WF-SCN-MIX detour）**: 既存 Project Load → Mixer
  で Routing 調整 → （任意）Signal Flow で診断 → 文脈保持
  で Mixer に復帰 → Save/Reload → Export。detour 未実施でも
  spine は完結する。
- **EX-4（録音 crash recovery）**: 録音中に crash → 再起動
  → Journal から未完了 Take を検出 → Finalize 可能なら回復、
  不能なら Discard。完成済み Take は保全。
- **EX-5（Remote 不通）**: Remote Source 不通状態で Local
  Import → 編集 → Save → 再生 → Export が全て成功。Remote
  asset は Missing 表示で偽装しない。
- **EX-6（Window/Dock 移動）**: 録音中/再生中/編集中に
  Window/Dock を移動 → Recording/Transport/Project/
  編集対象が不変であること。
- **EX-7（文書契約）**: `doc_status lint` が新規文書に
  ついて blocking finding を出さない、`index apply` 後に
  index-membership が一致、`mise run fmt-check` が exit 0。

## Guarantees

- 文書成果物として: spec/design/plan/task/test-spec/impl
  (ir/exp) の front matter relations が双方向整合し、
  `evidence/sv8/` に WF-AC→仕様節→docs/Issue の台帳がある。
- 将来の実装に対して: GA-01〜08 の Oracle が OS-L1 で検証
  可能な論理条件として、GUI-L2/EXT-L3 で実機条件として
  定義されている。
- 全 WF-AC が「文書が検証可能に規定した」状態で Pass と
  記録され、実機未検証は NotRun/Deferred として分離されて
  いる。

## Non-goals

- 実 OS Window/実入力/実 Plugin/実 Device での動作検証
  （#114/#176 lane、NotRun）。
- Track/Clip/Timeline モデル・Tool/Snap 数値・Recording/
  Comping 詳細・Asset Browser 詳細の再定義（#10/#11/#12/#9
  が正本）。
- Signal Flow/Inspector/Version Diff/AI/VCS の個別仕様
  （Gate B/C Owner）。
- Keyboard の具体的キー割当の確定（#112、Deferred）。
- `crates/` 以下のコード実装・テスト実行。

## Risk

- 文書契約が実装時に検証不能な Oracle を含むと、Gate A 受入
  が形骸化する → Oracle は独立検証可能な形（ファイル/hash/
  decode/等価性）に限定し、曖昧な表現を避ける。
- detour 契約が「診断を強制しない」を弱めると、Gate A が
  Gate B/C 完了を待つ構造になる → spine と detour を工程表で
  明示的に分離済み。
- 証拠の誠実さが崩れると headless 成功が実機 Pass に誤換算
  される → POLICIES.md の Result 語彙を `evidence/sv8/` で
  強制し、Pass は実行済み検証のみに限定する。
