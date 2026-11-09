---
id: "ADR-034UqvaPZtUL9Khx2bhVH3"
type: "adr"
status: "accepted"
title: "SV-GUI-04 slice2: close veto は末尾 live window のみ・guard query は app-global reasons を返す"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-04-02-close-veto.md"
    - "docs/adr/sv-gui-04-01-two-window-second-view.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/sv-gui-04-02-close-veto-design.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# ADR: SV-GUI-04 slice2 — close veto の適用範囲と guard query の責務分界

## Context and Problem Statement

`NativeShell::on_windowed` の `CloseRequested` は現行スライスでは
無条件 `Approve` で、shell.rs module doc が "unsaved-guard veto"
を設計上の hook として明記している。Issue #113 の close-cancel
受入を満たすには、未保存変更・録音中の session を window close
（特に app exit を伴う末尾 window の close）から守る必要がある。

一方で project/session/recording の truth は app-global であり
window に帰属しない — 残存 window がある限り session は生き続ける。
「どの close を veto するか」のポリシーと「guard 状態をどう
問い合わせるか」の機構をどこに置くかを決める必要がある。

## Decision Drivers

- Issue 受入「close-cancel で安全な復帰」「Window close while
  Recording → Domain Truth 不変」。
- 「quit 中の一部 veto」を fixture で実演できること
  （decision が window ごとに分岐し得る構造）。
- decision query 自体の失敗を安全側に倒せること
  （Window 生成失敗時の安全な復帰）。
- `syn.*` op surface の既存パターン（`attach_view` の
  `dispatch_window_intent` 同期 dispatch）との整合。
- window topology（live window 集合）の知識は shell/driver 側に
  あり、app/service 側は知らない — 責務分界を跨がないこと。

## Decision

1. **guard query は app-global reasons を返す read op**
   `syn.window.close_guard` とする。`SynvelisApp` が
   `unsaved_changes`（`live_revision` vs `saved().revision`、
   `saved` 無しで `live_revision > 0`）・`recording_active`
   （open recording transaction / `RecorderSlot::active()`）を
   reasons 配列で報告し、shell が `dispatch_window_intent` で
   同期問い合わせする。op 自体は window を知らず、topology を
   引数に取らない。
2. **veto は末尾 live window の close にのみ適用する**。
   `driver.windows()` が当該 window のみを含む場合に限り、
   reasons 非空なら `Veto`、空なら `Approve`。非最終 window の
   close は session が残存 window に保持されるため guard 状態に
   関わらず `Approve` — veto しても失われるものが無く、UX を
   不必要に阻害するだけだからである。quit 中も同じ規則が
   window ごとに評価されるため「一部 veto」が自然に発生する
   （処理順で先に approve された window は destroy、最後に
   処理された window が veto で生存）。
3. **query 失敗は fail-closed**。op error・Result 欠落・
   malformed payload は guarded 扱いとし、末尾 window の close
   を `Veto` に倒す。判定不能な破壊的操作を許可しない。
4. **`Defer` は使わない**。保存確認プロンプト等の非同期継続は
   後続 slice。veto は「閉じない」であり、明示的な破棄経路は
   既存の `syn.project.reload`/`new`/`open` が担う。

## Consequences

### Positive

- project/session/recording の喪失が起き得る唯一の経路
  （末尾 window close → app exit）だけを guard し、データ損失を
  構造的に防ぐ。
- quit の一部 veto が window 処理順に依らず決定的に発生し、
  fixture で検証可能。
- guard の判定材料（revision 比較・recording 状態）は app 層の
  authoritative 実装に一元化され、shell は topology と
  `CloseDecision` 写像だけを担う。
- query 失敗時も window が生き残る方向に倒れるため、壊れた
  decision 経路が session を暗黙破棄することはない。

### Negative

- dirty 状態で非最終 window を閉じると、その window の view/
  subscription は teardown される（project 自体は無傷）。
  window 固有の未保存 UI state を守る仕組みは無い — 現状
  view state は揮発性で永続化対象外のため実害なし。
- `RecorderSlot` は現状 stub（`active()` は常に false）のため、
  recorder veto は配線完了まで実効しない。open recording
  transaction（`session.control().recording()`）は実状態を読む。
- 「保存しますか?」の確認 UX は無く、veto された利用者は
  save/reload/new/open で明示的に状態を解決してから再度 close
  する必要がある。

## Considered Options

- **全 window close に無条件 veto（app-global guard のみ）**:
  実装は最も単純だが、残存 window が session を保持する限り
  失われるものが無い close まで阻害し、「quit 中の一部 veto」
  も発生しない（全 veto になる）。
- **op が `last_window` param を受けて最終 decision を返す**:
  policy を service 側に寄せると、window topology という
  shell/driver の知識を op contract に漏らす。guard query を
  純粋な状態問合せに留め、topology との合成を shell に置く
  方が責務分界として正しい。
- **`CloseDecision::Defer` + pending-close 管理**: 非同期の
  保存確認フロー向け。prompt UI が無い現 slice では close-pending
  状態を解決する経路が無く、複雑さに見合わない。

## References

- Spec: docs/specs/sv-gui-04-02-close-veto.md
- Issue: https://github.com/scarecrow173/synvelis/issues/113
