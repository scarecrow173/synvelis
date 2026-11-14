# Trust / OS Boundary 契約（AUTX-02/AUTX-03 境界）

一次正本: `crates/synvelis-gateway/src/request.rs`（`PrincipalKind`/
`OriginKind`/`AdapterSeal`/`CallerIdentity`）、`authorization.rs`
（`AuthorizationStage`/`PolicyEvaluator`）、`grant.rs`（`Grant`/
`ApprovalPolicy`/`Validity`/`PolicyEpoch`）、`proposal.rs`
（Approval/Apply 状態機械）。

## 1. 認証モデル（既存 Gateway の投影）

```
PrincipalKind: Human | Agent
OriginKind:    ManualUi | ExternalAdapter | TestFixture
AdapterSeal:   crate 内部のみ発行可能な unforgeable token
CallerIdentity: Principal + OriginKind + AdapterId（seal 経由でのみ生成）
```

- `CallerIdentity` は wire から parse されない。trusted adapter が
  `AdapterSeal` を使って生成し、`Gateway::bind(wire, caller)` に渡す。
- `AdapterId` は session key を兼ねる（`GrantBinding::Session`）。
  transport が複数 session を多重化する場合は session ごとに別 `AdapterId`
  を発行する（GW-08）。
- wire 引数に principal/authority を自己申告するフィールドがあれば
  `SelfDeclaredAuthority` で拒否（`parse_wire`）。

## 2. CLI セッション種別と Principal の分離

CLI は「どの OS ユーザーが起動したか」「TTY があるか」「環境変数」でなく、
**adapter が attest した session 種別**で `OriginKind`/`PrincipalKind` を決める。

| CLI セッション | OriginKind | PrincipalKind | 根拠 |
| --- | --- | --- | --- |
| 人間が対話的に操作するローカル CLI | `ExternalAdapter`（新設 headless adapter） | `Human` | adapter が trusted local session を attest した場合のみ |
| 無人/CI/エージェント起動の CLI | `ExternalAdapter` | `Agent` | 人間の対話的 session が存在しない |
| 将来の外部 MCP transport | `ExternalAdapter` | `Agent`（または attested Human） | GW-08 の channel kind 判定 |
| GUI 内蔵 adapter | `ManualUi` | `Human` | 既存 `ManualUi` 経路 |
| 契約テスト | `TestFixture` | （fixture） | 本番経路では発行しない |

**重要**: `OriginKind`/`PrincipalKind`/`AdapterSeal` の発行権限は
**adapter 実装（trusted host 側）にのみ存在**し、CLI 引数・環境変数・
TTY の有無はそれを生成しない。

## 3. 「Human 承認が引数で生成されない」契約

以下は**いずれも単独または組合せでも Human 承認・Human 権限を生成しない**:

- `--as human` / `--user human` 等の自己申告引数
- `--yes` / `-y` / `--non-interactive` 等の確認省略フラグ
- TTY の有無（TTY があっても adapter が Human を attest しなければ Agent）
- 環境変数（`SYNVELIS_*` 等）による principal 主張
- stdin からの指示文字列

Human 承認（Approval receipt）が必要な apply は、**信頼された UI/対話経路**
（GW-05 の ApprovalPolicy::ApproveToApply → trusted user decision）でのみ
発行される。CLI・Script・Workflow・Agent は approval receipt を**要求**
できるが**発行**できない。receipt は proposal/changeset/principal/project/
revision に束縛され、1 回の apply で消費される（`Consumed`）。

## 4. 無人/CI/AI の権限境界

- 無人・CI・AI セッションは `PrincipalKind::Agent` として振る舞い、
  `ApprovalPolicy` が要求する範囲でのみ Grant を得る。
  - `Suggest`: proposal 準備のみ。apply は拒否。
  - `ApproveToApply`: apply ごとに trusted approval が必要。
  - `DelegatedApply`: Grant の `GrantLimits`（`apply_budget`/`job_budget`）と
    `Validity`（`expires_at`/`revoked`）の範囲内で apply 可。** proactive
    activation 権限ではない**（AHG-007）。
- Grant は `PolicyEpoch` に紐づき、revoke/expiry/epoch 変更は各 step と
  commit 境界で再評価される（fail-closed）。
- Grant/Token/Approval/human authority を Script 定義・durable Workflow plan・
  CLI 引数に**保存・埋め込みしない**（[design-script.md](design-script.md) §5）。

## 5. アダプタの責務と domain truth の分離

| 層 | 所有するもの | 所有しないもの |
| --- | --- | --- |
| CLI adapter | wire serialize、session 種別の attest、correlation record、機械可読出力 | capability 定義、認可判定、job 実行状態、project 変更 |
| GUI adapter | 同上（`ManualUi` origin） | 同上 |
| MCP adapter | 同上（`ExternalAdapter`、channel kind 判定） | 同上 |
| Workflow coordinator | Run/Step 状態、journal、resume cursor | project/job/grant の第二正本（参照のみ） |
| Gateway | capability/schema/trust/outcome の正本 | adapter の session 管理 |

接続の生死（GUI close・MCP disconnect・CLI process 終了）は**実行の所有権を
変更しない**。Run/Job/Proposal の状態は接続を超えて存続し、resume・
cancel request・結果再照会が可能である（[design-workflow-run.md](design-workflow-run.md) §6）。

## 6. 検証可能な受入（AUTX-02/AUTX-03 向け）

- `--as human`/`--yes`/TTY/環境変数のみでは `PrincipalKind::Human` の
  `CallerIdentity` が生成されない（adapter 側試験）。
- 無人 CLI session が `Agent` principal で `ApprovalRequired` を受け取り、
  script/CLI から approval を捏造できないこと。
- `AdapterSeal` を持たない外部 crate/ wire payload から `CallerIdentity` を
  構築できないこと（型レベルで保証済み、再確認）。
- Grant revoke/expiry が workflow の各 step・commit 境界で再評価され、
  stale grant での apply が `Denied`/`ApprovalInvalid` になること。
