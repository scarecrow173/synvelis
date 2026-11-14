# Script 統合契約（SV-SCRIPT-01〜04 / Ruviform #553/#558〜560 境界）

一次正本: `crates/synvelis-gateway/`（Action/Workflow が収束する先）、
Issue #271〜#274・Ruviform #553/#558/#559/#560。本書は Script が Gateway・
Plan・Artifact の各境界で何をしてよいかを定義する。

## 1. Script の位置づけ

- **Action Script** = ある Action（`CapabilityRef`）の一実装。script が
  独自 capability を登録・実行するのではなく、**型付き引数を Gateway 呼出へ
  lower する recipe** を生成する。
- **Workflow Script** = ある WorkflowDefinition の一実装。script が独自の
  実行機構を持つのではなく、**型付き Workflow Plan** を生成する。
- いずれも script 自体は authority を持たない。script の出力は常に
  「Gateway が検証する typed な呼出/plan」であり、直接 Project を mutate
  しない。

## 2. `ScriptActionDefinition` フィールド（確定）

| フィールド | 型/意味 | 制約 |
| --- | --- | --- |
| `script_id` | 安定識別子 | version と組で一意 |
| `version` | script 版 | source digest と整合必須 |
| `language_id` | `yaml` / `luau` / `rhai` 等 | backend registry が解決 |
| `backend_requirement` | 必要な backend 能力 | 未導入 backend は fail-closed |
| `source_digest` | source の content digest | version/実行時 source と照合 |
| `input_schema` | `SchemaRef` | 対象 Action の input schema と互換必須 |
| `output_schema` | `SchemaRef` | 対象 Action の output schema と互換必須 |

**source/version drift 拒否**: `source_digest` が登録時と実行時で異なる、
`version` が未知、または `language_id`/`backend_requirement` が解決不能な
場合は typed error で拒否し、**推測で旧版・別 backend にフォールバックしない**。

## 3. Script → Typed Plan → Gateway の収束

```
script source (yaml/luau/rhai)
  → language-neutral typed port で parse/評価
  → Invocation Document / WorkflowDefinition（型付き中間表現）
  → Plan Compiler → 型付き Plan / OperationRequest 相当
  → Gateway::bind → Gateway::invoke（GUI/MCP/CLI と同一経路）
```

- script は `OperationRequest` を**直接構築しない**。trusted caller
  （`CallerIdentity`/`AdapterSeal`）の発行は adapter の責務であり、
  script は caller を名乗れない。
- script が生成する呼出は通常の wire request と同じ検証（schema/target/
  consistency/availability）を通る。script だからといって検証を緩めない。
- Action Script が生成するのは「対象 Action への typed 引数」であり、
  Action 本体の効果・権限を拡大しない（`EffectSet`/`PermissionSet`/
  `DataAccessSet` は capability descriptor が上限）。

## 4. 言語・backend の境界（YAML/Luau/Rhai）

| 言語 | 役割 | backend | 状態 |
| --- | --- | --- | --- |
| YAML | 宣言的 Workflow/Action 定義 | yaml parser（外部 VM なし） | Required |
| Luau | 型付き script（標準 Luau） | Luau VM（RF#558 PoC: Rhai と同一バイナリ共存） | Required（PoC 合格後） |
| Rhai | 型付き script | Rhai VM（RF#559: Luau+Rhai 共存 PoC） | **PoC 限定・未正式サポート** |
| Lua 5.4 | — | — | **除外**（標準 Luau を使う。Lua 5.4 互換は保証しない） |

契約:
- YAML/Luau/Rhai は**同一の型付き WorkflowDefinition/Plan**へ収束する。
  言語ごとに別の実行意味論・別の正本を持たない。
- Luau/Rhai の typed port は**言語中立**であり、VM 内部型・DAW 固有型・
  VM ポインタを公開 API 越しに交換しない。
- **異種 Workflow 参照**（Luau script が Rhai で書かれた workflow を参照する
  等）は Plan 上の参照として表現し、**VM 内部状態を共有・復旧しない**。
  参照先は typed definition の digest/version で pin する。
- **Analyzer 独立**: script analyzer（RF#560 の native Luau Analyzer/FFI
  等）の成功は静的検査の通過であり、**Grant・Approval・実行許可を意味しない**。
  Analyzer と Runtime は独立した Owner/境界を持つ。
- Rhai は RF#559 の共存 PoC が目的であり、**正式サポートを約束しない**。
  `backend_requirement` で Rhai 必須の script は backend 未導入環境で
  fail-closed される。

## 4.5 Script Batch 規則（AUTX-04 との接続）

script が生成する batch 呼出は [design-atomic-macro.md](design-atomic-macro.md)
の既存 `synvelis.operation.batch` 契約にそのまま lower される。script 層の
追加規則:

| 要件 | 契約 |
| --- | --- |
| member 数 | **2〜64** の `ProjectMutation` member。1 件は batch 不要（単体 Action 呼出へ）、65 件以上は拒否 |
| nested batch | script が batch を batch の member にすることを禁止（gateway 側でも拒否済み） |
| cross-project | 単一 Project のみ。複数 Project は Workflow plan へ分解 |
| Query/Async/RuntimeControl/外部 I/O 混在 | atomicity を保証できない組合せは拒否し Workflow へ |

## 5. Script に許されないもの

- Grant・Token・Approval receipt・human authority の script 内への保存・
  埋め込み・発行（script は authority を運ばない）。
- 直接の Project mutation・CommitGate/Project mutex の呼出・保持。
- 第二 Capability Registry・第二 Job truth・第二 Project writer の実装。
- `script run` / `--script` 等の独立実行面の作成。**CLI は
  `action run --file <script>` / `workflow run --file <definition>` のみ**
  を公開し、script を第一級の実行単位にしない。
- VM 内部状態・生ポインタ・DAW 内部型の typed port 越えの受け渡し。

## 6. 検証可能な受入（SV-SCRIPT-01〜04 / RF 向け）

- `ScriptActionDefinition` の全フィールドが schema として検証され、
  source/version drift が typed 拒否される。
- YAML/Luau/Rhai の同一 WorkflowDefinition が同一 Plan へ lower され、
  同一の Coordinator/Gateway 経路で実行される（fake adapter で契約試験）。
- 異種言語参照が Plan 上の digest pin で解決され、VM 状態共有がないこと。
- Analyzer 成功のみでは `Denied`/`ApprovalRequired` が変わらないこと
  （Grant 非生成の確認）。
- `action run --file`/`workflow run --file` 以外の script 実行面が
  存在しないこと（CLI surface の検査）。
- Rhai backend 未導入で `backend_requirement: rhai` の script が
  fail-closed されること。Lua 5.4 source が拒否されること。
