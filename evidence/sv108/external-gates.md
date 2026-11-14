# SF-05 external gates (Issue #108 / synvelis#108)

headless テストの成功を実機・実 GUI・実 plugin の Pass に
しないための外部 Gate 判定記録。POLICIES.md の
OS-L1/GUI-L2/EXT-L3・Required/Deferred/NotApplicable と
Pass/Fail/NotRun/Unsupported の語彙に従う。

| Gate | 対象 AC | 状態 | 理由 |
| --- | --- | --- | --- |
| Windows L2 実 GPUI Window での Signal Flow 描画・操作 | SF-UX-01/02/04/08 | **NotRun** | headless は `signal_flow_ui` read model + `UiOpsState`/`WorkspaceManager` の契約層のみ。実 window の layout・hit test・描画は #114 系の native/GUI 検証が必要 |
| 実 keyboard/IME/Plugin GUI focus event | SF-UX-04 | **NotRun** | `set_input_context` の API 遷移で契約は固定済み。実 OS の focus/IME/plugin window イベント経路は GUI-L2 |
| 実 plugin (VST3/AU/CLAP) の port 再構成・dynamic layout | SF-UX-03 | **NotRun** | `ProcessorBinding::Resolved` + `capability_revision` bump の Fixture のみ。実 plugin の scan/再構成・失敗時挙動は #176 系の EXT-L3 |
| 実 device での probe capture/monitor・PCM leakage 確認 | SF-UX-06 | **NotRun** | `sf05_probe_share.rs` は subscriber/lease/capture owner の headless 契約のみ。実 device の capture 重複・leakage は EXT-L3 |
| SF-UX-08（実 OS/GPUI Native lifecycle・HighDPI・A11y・IME） | SF-UX-08 | **Deferred** | Issue #108 の headless 範囲外。#114 系へ接続 |
| macOS/Linux 実 GUI | 全般 | **NotRun** | 3 OS 横断は Daily/Manual Full CI・手動検証の責務 |
| Headless PASS の製品受入への昇格 | 全般 | **Unsupported** | headless は契約層の証拠であり、GUI-L2/EXT-L3 の実施結果なしに製品受入としない |

## 判定ルール

- headless テストの green は「headless 契約の Pass」であり、
  実機・実 plugin・実 GUI の Pass を意味しない。
- `NotRun`/`Deferred`/`Unsupported` は「未実施」または
  「本タスク範囲外」を意味し、推測で Pass にしない。
- 後続: GUI-L2 は #114 系、実 plugin/device は #176 系の
  Issue/CI で実施。本台帳の headless Case をそれらの前提
  証拠として参照するが、代替しない。
