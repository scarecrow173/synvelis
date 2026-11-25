---
id: "DESIGN-034jU54Fg1eMsmUlQArX7T"
type: "design"
status: "approved"
title: "TIM-04 Plugin State Snapshot — real ABI consumer-run design"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-04-real-abi-state.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-04-plugin-state-snapshot-consistency-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-04-plugin-state-snapshot-consistency-design.md"
    - "https://github.com/scarecrow173/ruviform/issues/630"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-04-real-abi-state.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/630"
  defers: []
  deferred-by: []
---

# TIM-04 Plugin State Snapshot — real ABI consumer-run design

## Context

TIM-04 の headless/mock slice は `PluginStatePort`・
`PluginStateSession`・`DirectoryPluginStateStore`・quality policy を
main に実装済みである。本 slice はそれらを変更せず、ruviform#630 の
VST3/CLAP Test Plugin 実バイナリを consumer 側から駆動して、同じ
production contract が実 ABI bytes に対して成立するかを測定する。

実装上の制約が 1 つある。ruviform pinned rev
`20dc41f71c47674b3e51d1eb0921124b94f8bfaf` の VST3/CLAP host wrapper
は thread-affine object を持ち、`PluginFactory::create_sendable` を実装
しない。typed runtime attach に必要な endpoint capable instance を作れない
ため、`PluginHost::prepare_endpoint` は `HostError::EndpointUnavailable`
を返す。Coordinator 決裁に従い、本 slice はこの事実を `Unsupported` の
発見として記録し、PCM mutation は同じ production `PluginHost` cell の
非 RT `process` 経路で駆動する。`ProcessorConnection` が通ったかのような
証拠にはしない。

## Components

### 1. Artifact resolution

`tests/tim04_real_abi_state.rs` は test 内の小さな resolver で cdylib を
解決する。順序は次のとおり。

1. explicit path override（`SYNVELIS_TIM04_TEST_VST3_PATH` /
   `SYNVELIS_TIM04_TEST_CLAP_PATH`。sv#176 との互換入力として
   `RUVIFORM_PLUGIN_DOCK_TEST_VST3` / `RUVIFORM_PLUGIN_DOCK_TEST_CLAP`
   も受理）。
2. 統合 test binary と同じ `target/<profile>/deps/` の un-hashed artifact。
3. profile root の artifact。
4. `deps/<prefix><stem>-*<suffix>` の hashed dep-artifact glob。

解決した file が存在しない、または読めない場合は test を fail させる。
成功時は path・file name・SHA-256 を全 case の `environment.artifact` に
記録する。ruviform resolved rev は workspace `Cargo.lock` から読み、
`environment.ruviform_resolved_rev` に入れる。

### 2. Host composition and lifecycle

fixture load は `Vst3Library::load` / `ClapLibrary::load` を使い、
`factories()` の各 factory を `PluginHostProcessors::product` に登録する。
`product` は production `PluginHost` を内包する adapter であり、test は
`host()` が返す同じ shared cellだけを使う。コマンド列は次のとおり。

```text
Scan
Instantiate(plugin_id)
Initialize
Activate(ProcessConfig { sample_rate: 48000, min_frames: 1, max_frames: 64 })
StartProcessing
... test-specific processing/capture ...
StopProcessing
Deactivate
Destroy
```

`Scan` の descriptor から `test_stateful`・`test_stateful_single`・
`test_fault` を名前で選択し、plugin id/name/vendor/version を evidence に
残す。`test_fault` の fault parameter は arm しない。

### 3. Typed endpoint capability probe

各 format の最初の stateful instance に対し、activated/processing 状態で
`PluginHost::prepare_endpoint(handle, EndpointBounds { max_frames: 64,
channels: 2 })` を呼ぶ。現行 fixture では factory capability を
`create_sendable().is_some()` で併せて記録し、VST3/CLAP とも `false`、
endpoint probe は `EndpointUnavailable` となる。evidence には error kind と
発生地点 `PluginHost::prepare_endpoint` を記録する。

この結果は typed graph/PCM attach capability matrix の `Unsupported` で
ある。manual host path はその代替成功ではなく、state snapshot consumer
oracleを成立させるための production host 経路である。ruviform 側に
`create_sendable` capable wrapper が実装された場合は、typed runtime 経路を
別途 consumer-run に昇格できる。

### 4. PCM driver

`test_stateful` の DSP law は `out[c][n] = in[c][n] * gain`。test は
64 frame、2 channel、left `0.5`・right `-0.25` の固定入力を使う。
`HostInput`/`HostOutput` に test 内の erased slice view を渡し、
`PluginHost::process` を呼ぶ。parameter mutation は frame offset 0 の
`EventKind::ParamValue`（`ParamId(0)`、normalized `0.375`）であり、
fixture の linear lawで plain gain `0.75` になる。

期待 PCM は次のとおり。

- mutation 前: left `0.5`、right `-0.25`
- mutation block: left `0.375`、right `-0.1875`
- initial state restore 後: left `0.5`、right `-0.25`

PCM hashと先頭値を evidence に記録する。PCM driver は RT callback 内で
動かすものではなく、非 RT test が production host の process verb を直接
駆動する。state save・FS IO・artifact load を RT callback に入れない。

### 5. RFST v2 independent parser

fixture state blobを adapter の返り値として信用せず、test 内 parser で
次を検査する。

```text
RFST v2 block (20 bytes):
[u32 LE magic "RFST"][u32 LE version = 2][f64 LE value][u32 LE fnv1a32(core 16 bytes)]
```

format ごとの外側 layout は次のとおり。

- VST3 distributed `test_stateful`: adapter frame
  `[u32 comp_len][component RFST][u32 ctrl_len][controller RFST]`。
  component value は plain gain、controller value は normalized gain。
  現行の長さは 48 bytes。
- VST3 `test_stateful_single`: 同じ frame で controller length が 0。
  現行の長さは 28 bytes。
- CLAP `test_stateful`: raw 20-byte RFST stream。value は plain gain。

processor-side parameter event は component/plain state を `0.75` に変える。
VST3 distributed の controller payload は controller の最後の normalized
stateを保持し、RT event pathで書き換わらないため、mutation 後も
normalized `0.5` を示す。parserはこの違いを別々に記録し、controller
normalized を processor mutation の証拠と混同しない。

blob SHA-256と `PluginStateOutcome::Captured.blob_digest` の hex を照合
する。save→restore→save の restored blob は initial blobと byte-identical
でなければならない。

### 6. Project/session binding

`PluginStateSession::save` は semantic project routingを見るため、test は
既存 TIM-04 test と同じ最小 project（AddTrack → AddTrackStrip →
InsertProcessor）を作る。`PluginRef` は scan descriptorの id/versionから
作り、`PluginStateHost::bind` には manual lifecycleで得た `PluginHandle`
を使う。`test_fault` は statelessなので Strict saveの projectには混ぜず、
capability matrixでは個別に `PluginStateHost::capture` を呼ぶ。

save requestは `CaptureMode::Coordinated`・`StateQualityPolicy::Strict`。
stateful pluginは capture前に `StopProcessing` して stable boundaryを作る。
そのため qualityは `Coordinated` となり、Strict gateを通る。

### 7. Capability matrix

matrixは次の axisで `PluginStateHost::capture` を実行する。

- format: VST3/CLAP
- plugin: `test_stateful`、VST3のみ `test_stateful_single`、stateless
  `test_fault`
- lifecycle: Processing/Stopped
- mode: `Coordinated`/`Quiescent`/`BestEffort`

期待は lifecycle observationに基づく。

| lifecycle | requested mode | expected achieved |
| --- | --- | --- |
| Processing | any | `BestEffort` |
| Stopped stable | `Coordinated` | `Coordinated` |
| Stopped stable | `Quiescent` | `Quiescent` |
| Stopped stable | `BestEffort` | `Quiescent`（実測品質を過小申告しない） |

`test_fault` は全組合せで `PluginStateCaptureError::Unsupported` を期待
する。これは失敗ではなく state capability の正直な `Unsupported` verdict
である。

### 8. Corrupt/missing restore

real ABI `test_stateful` を Strict saveした後、blob fileに対して次を行う。

1. 1 byte flip: restoreはblob read後の SHA-256 digest mismatchを
   `Failed(Corrupt)` として記録する。digest precheckで失敗するため、
   plugin restoreへは到達しない。
2. file delete: restoreは `Failed(Missing)` を記録する。

この caseは store/manifestの実 file layoutを使うため、in-memory storeでは
なく `DirectoryPluginStateStore` を使う。

### 9. Evidence schema

`SYNVELIS_TIM04_REAL_ABI_EVIDENCE_DIR` が指定された場合だけ、caseごとに
`<case>.jsonl` を書く。相対 pathは workspace root基準に解決する。1行JSON
の必須 fieldは `case`・`driver`・`verdict`・`detail`・`observed`・
`environment`。`environment` は OS/arch・`environment_kind: test-plugin`・
`isolation: in-process/l0`・format・artifact file name/path/SHA-256・
ruviform resolved rev・plugin identityを持つ。evidence未指定でもtestの
断言は同じであり、証拠出力の有無で成功条件を変えない。

## Failure handling

- artifact missing / ABI load failure / required descriptor missing: test
  failure。必要なら evidenceにも `Fail` を書いてから panicする。
- endpoint attach: `EndpointUnavailable` は現行 capabilityとして
  `Unsupported`。別 errorや endpoint mint成功は `Fail`。
- stateless capture: `Unsupported` は expected capability verdict。
  stateful pluginが `Unsupported` を返した場合は `Fail`。
- RFST parse・length・checksum・digest・PCM mismatch: `Fail`。
  fixture bytesへ consumer側を合わせ込まない。ただし VST3 adapter frameは
  ruviformの公開state bridge契約として独立parserで扱う。
- corrupt/missing blob: typed outcomeが `Corrupt`/`Missing` 以外なら
  `Fail`。

## Deferred concerns

- ruviform側の `create_sendable` capable VST3/CLAP wrapper、typed runtime
  endpoint経由の PCM consumer-run。Coordinatorへ発見として報告し、
  ruviform follow-up issueの要否は Coordinatorが判断する。
- L1 out-of-process crash/timeout/permission fault matrix。
- Offline renderへの実 plugin組込みと `TIM04-RCP-01` の end-to-end受入。
- 製品 registry/discovery、実 GUI/Native Editor、市販 plugin、AudioLens
  cross-gate、3OS横断。
