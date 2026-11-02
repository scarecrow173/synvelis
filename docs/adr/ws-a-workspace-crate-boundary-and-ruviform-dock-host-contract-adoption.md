---
id: "ADR-034QwWcA5mHzwXYyFiDTTi"
type: "adr"
status: "accepted"
title: "WS-A workspace crate boundary and Ruviform dock/host contract adoption"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/36"
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
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# WS-A workspace crate boundary and Ruviform dock/host contract adoption

## Context and Problem Statement

Synvelis の Workspace 基盤 (Issue #36 headless PoC) は、(a) Workspace/
View/Link/Command のセッション状態を扱う新しい責務領域であり、
(b) ruviform #181/#182 の Dock/Window 契約を消費する側である。
どのクレートにこの責務を置き、ruviform 契約型をどう取り込み、
永続化形式をどう分離するかを決める必要がある。

## Decision Drivers

* Project 正本 (`synvelis-project`) は永続化される制作データの集約で
  あり、セッション scoped の View/Layout/Focus 状態を混入させたくない。
* Dock Tree・Window 契約・Layout snapshot の再実装は ruviform #182 と
  重複する。逆に Synvelis 固有型を ruviform へ持ち込んではいけない。
* headless PoC のため `ruviform-ui-webview` (wry/tao adapter) に依存
  できない — `ruviform-ui` の contract crate のみ使える。
* AGENTS.md の Rust ワークスペース規約: 依存方向は一方向、ruviform
  依存は既定ブランチ追従の git 依存 (rev は Cargo.lock で記録)。

## Considered Options

* Option 1: 新クレート `synvelis-workspace` を `crates/` 配下に追加し、
  `synvelis-project` + `ruviform-ui` のみに依存させる。Workspace 固有
  envelope (`WorkspaceSnapshot`) でレイアウト/View/Link 設定を保存。
* Option 2: `synvelis-project` に workspace module を追加する。
* Option 3: `recording-poc` 型の別 PoC bin クレートを作り、
  workspace 状態をその内部に閉じ込める。

## Decision Outcome

Chosen option: "Option 1", because Workspace 状態はセッション scoped で
永続 Project 集約と寿命が異なり、独立クレートにすることで責務境界と
依存方向 (synvelis-workspace → synvelis-project) が明確になる。Dock
機構は ruviform `ruviform-ui::dock`/`window` の契約型をそのまま消費
し、View/Selection/Link/Command は Synvelis 側で実装する — 設計書 §8
の責務分界と一致する。

### Consequences

* Good, because Project 集約が session UI 状態で汚染されず、Workspace は
  headless で独立にテストできる。ruviform `DockWorkspace` /
  `WindowHandle` / `LayoutSnapshot` をそのまま再利用し、Dock Tree 再実
  装・DOM 移送・Window 管理の自前実装が不要。
* Good, because `ruviform-ui-webview` 非依存のまま headless CI で検証
  できる (real-GUI lane へ後続委譲)。
* Bad, because ルート `Cargo.toml` の `members`/`workspace.dependencies`
  と crates 索引が増え、ruviform git 依存の rev 更新に追従管理が必要。
* Bad, because `WindowId` が host-issued (session scoped) なため、永続
  レイアウトでは snapshot の window 順序で Window key を対応付ける
  独自 envelope が必要になる。

### Confirmation

* `synvelis-workspace` が `synvelis-project` + `ruviform-ui` + serde
  系のみに依存し、逆方向依存・webview adapter 依存がない
  (`cargo tree` / `mise run check` で確認)。
* 全 ruviform git source が `Cargo.lock` で同一 revision (#181/#182 を
  含む origin/main 祖先) に解決される。
* Workspace/View/Link/Command 型が ruviform crate に出現しない。

## Implementation Plan

* Affected paths: `Cargo.toml` (members + workspace deps),
  `crates/synvelis-workspace/**` (新規), `Cargo.lock` (再生成)。
* Patterns to follow: `synvelis-project` の stable ID / Operation /
  ProjectController 契約、`crates/*/README` は無し (本リポジトリは
  クレート README を持たない)、`#[non_exhaustive]`・`must_use`・
  `missing_docs` lint 準拠。
* Patterns to avoid: Project Store の二重実装、Dock Tree の再実装、
  Session 状態の Project snapshot 混入、RT 経路からの workspace 参照。
* Dependencies or configuration: `ruviform-ui` git 依存追加 + 全
  ruviform source を #181/#182 含有 revision へ更新。

## Verification

* [ ] `mise run check` / `mise run lint` / `mise run test` が全 PASS。
* [ ] `Cargo.lock` の全 `ruviform-*` source が同一 git revision で
  `#181/#182` merge commit を祖先に持つ。
* [ ] `synvelis-workspace` の依存一覧に `ruviform-ui-webview` /
  `wry` / `tao` が存在しない。

## Pros and Cons of the Options

### Option 1

* Good, because 責務境界・依存方向・永続化分離が既存規約と整合し、
  後続の real-GUI 統合でも差し替え点が WorkspaceManager の host port
  に限定される。
* Bad, because crate/member/lockfile/索引の boilerplate が増える。

### Option 2

* Good, because クレート数を増やさず Project 型へ直接アクセスしやすい。
* Bad, because 永続集約に session scoped の UI 状態が混入し、
  save/reload の対象と Workspace 状態の寿命が衝突する。Project
  Controller への逆依存も生じやすい。

### Option 3

* Good, because PoC が本体ワークスペースを汚染しない。
* Bad, because WS-A は PoC bin ではなく本番用の Workspace 基盤クレート
  として成立させる前提であり (Issue #2 §Synvelisで設計・実装する範囲)、
  bin 内部実装では library として再利用できない。

## More Information

- 設計正本: `docs/designs/workspace-docking-state-architecture.md`
- ruviform `crates/ruviform-ui/src/dock.rs` / `window.rs` (PR #240/#237)
- `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
