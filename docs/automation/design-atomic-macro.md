# Atomic Macro vs Workflow 分離契約（AUTX-04/AUTX-05 境界）

一次正本: `crates/synvelis-gateway/src/prepare_engine.rs`（batch lowering）、
`crates/synvelis-gateway/src/commit.rs`（CommitGate）、`request.rs`
（`Consistency::ExpectedRevision`）。本書は分離境界と入出力契約のみを定義する。
新しい Transaction Engine・第二 Commit 経路・第二 Undo を作らない。

## 1. 用語と境界

| 概念 | 定義 | 実装の正本 |
| --- | --- | --- |
| Atomic Macro（Batch） | 単一 Project・単一 ExpectedRevision・単一 Commit・単一 Undo を持つ、ProjectMutation の有界な合成 | 既存 `synvelis.operation.batch` capability → `ProjectOp::Composite` への lower → GW-04 prepare → GW-06 commit |
| Workflow | 非 RT のコーディネータ。型付き Step（Query/Atomic Batch/Job Submit/Await/Result/If/Bounded Loop）を計画・実行・再開・取消する | AUTX-05 (#264) Plan + AUTX-06 (#265) Coordinator（将来 crate。Gateway ではない） |

分離の判定基準: **「単一 Project の原子的変更に収まるか」**。
収まらないもの（複数 Project 横断・非同期待機・条件分岐・外部 I/O・
Query/Async/RuntimeControl の混在）は Workflow へ送る。

## 2. Atomic Macro 契約（既存 batch の投影）

`synvelis.operation.batch`（`prepare_engine.rs`）が正本。

### Input 契約

| 項目 | 契約 |
| --- | --- |
| member 数 | 1〜64 件（空 batch は `batch requires at least one member` で拒否。上限は `PrepareBudget::default().max_ops = 64` / schema `max_len: Some(64)`、超過は `LimitExceeded`） |
| member domain | GW-04 の semantic-operation table（`ProjectMutation` domain のみ）に登録された capability の `NamedOperation` triple のみ。`Query`/`AsyncOperation`/`RuntimeControl`/外部 I/O を member に**含めない** |
| project | 単一 `ProjectId`（`OperationRequest.project` は 1 つ。複数 Project 横断は型で表現不能） |
| consistency | `ExpectedRevision { revision }` を 1 つ。member ごとの個別 revision は持たない |
| nested batch | **禁止**。member が `synvelis.operation.batch` を指す場合は `nested batch is not allowed` で typed 拒否（`prepare_engine.rs` の再帰検出） |
| batch target | batch 自体は target を取らない（`batch takes no target`）。project は `OperationRequest.project` の単一 `ProjectId` のみ |

### Output / 実行契約

- lower 結果は単一の `ProjectOp::Composite` → GW-04 が pinned snapshot 上で
  検証・効果導出 → `ProposalReady` → approval → `CommitGate` で
  **1 CAS（ExpectedRevision）・1 Commit・1 Undo**。
- batch 全体が 1 つの proposal/approval/apply に束ねられる。member 単位の
  部分 commit は存在しない。
- 失敗は全体失敗（all-or-nothing）。member index を含む typed error で
  失敗位置を報告してよいが、部分適用はしない。

### Unsupported（型付き拒否）→ Workflow 行き

| 要求 | 判定 | 行き先 |
| --- | --- | --- |
| member > 64 | `LimitExceeded`/`SchemaViolation` | Workflow（分割 plan） |
| nested batch | typed 拒否（既存） | 設計誤り。Workflow で明示的に逐次化 |
| 複数 Project | 型で表現不能 → `MalformedRequest`/`Validation` | Workflow（project ごとの batch step） |
| Query/Async/RuntimeControl/外部 I/O 混在 | domain 不一致で拒否 | Workflow の対応 step kind |
| 条件分岐・待機・retry | batch に表現なし | Workflow |
| 作成直後 ID 参照（後述 §3） | Input/Output 契約で規定 | scratch-ID 対応 or typed Unsupported |

## 3. 作成直後 ID 参照（create-then-reference）

batch 内で「member A が作成するリソースの ID を member B が参照する」
ケースの契約:

- **対応形（目標）**: GW-04 の scratch-ID/rewrite seam を拡張し、prepare 時に
  scratch 空間で採番した仮 ID を後続 member の参照へ rewrite し、commit 時に
  実 ID へ確定する。これは AUTX-04 (#263) の実装範囲。
- **未対応時の契約**: seam 未実装の間は当該 batch を **typed Unsupported**
  として拒否し、Workflow で「create → query で実 ID 取得 → 参照 step」へ
  分解することを正規経路とする。暗黙の ID 推測・順序依存の参照は許さない。
- 受入: 対応/未対応の両状態が Input/Output/Unsupported としてテスト可能で
  あること（spec R-6）。

## 4. Workflow 側の step kind との対応

| Workflow step kind | 下位で使う正本 | Atomic 性 |
| --- | --- | --- |
| `Query` | `OperationDomain::Query` capability（`Latest`/`ReadAt`/`FixedSnapshot`） | 読み取り。原子性不要 |
| `AtomicBatch` | `synvelis.operation.batch`（上記 §2） | 単一 Project/1 CAS/1 Undo |
| `JobSubmit` | `OperationDomain::AsyncOperation` capability → `JobPort` | 非原子。副作用は provider 側 |
| `Await` | job status 再照会（`ReadOperationResult`/`JobPort`） | 待機。副作用なし |
| `Result` | 完了 job/operation の結果取得 | 読み取り |
| `If` | plan 評価（typed condition。副作用なし） | — |
| `BoundedLoop` | plan 評価（上限必須。無制限 loop 禁止） | — |

**外部副作用（network/filesystem/provider 呼出）は Atomic Batch に入れない。**
入れると「commit したが外部副作用が不明」という回復不能状態を生む。
外部副作用は必ず Workflow の `JobSubmit`/`Await` step として plan に明示し、
Fault Matrix（[design-workflow-run.md](design-workflow-run.md)）の対象にする。

## 5. 禁止事項（再掲・強制）

- 新しい Transaction Engine・第二 CommitGate・第二 Undo 機構の作成禁止。
- Workflow Coordinator が Gateway/CommitFence/Project mutex を保持・再帰呼出
  しない（呼出は step 境界で逐次的に行う）。
- batch member の自己申告 effect を信用しない（GW-04 が導出）。
- CLI/Script が batch を独自に解釈・分割して直接 commit する経路を作らない。
  分割は Workflow plan の責務。

## 6. 検証可能な受入（AUTX-04/AUTX-05 向け）

- 65 member / nested batch / cross-project / Query 混在がすべて typed 拒否
  され、部分 commit が発生しないことを fake adapter で確認できる。
- 単一 batch が 1 proposal → 1 approval → 1 commit → 1 undo に対応する
  ことを journal/receipt で確認できる。
- scratch-ID seam の有無で create-then-reference の対応/Unsupported が
  切り替わり、Unsupported 時に Workflow 分解が唯一の正規経路であること。
