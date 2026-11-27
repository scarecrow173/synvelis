---
id: "TESTSPEC-fbS127nQw7Ln9KrTyVzE"
type: "test-spec"
status: "approved"
title: "TIM-03 frame/boundary matrix"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
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
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 frame/boundary matrix — Test Spec

## Headless oracle

- `tim03_process_boundary.rs`: valid `W×B` cellで exact span/physical length sequence。offset 0はhead applyでinterior cutなし。interior Bは `[0,B),[B,W)`。W=100/B=100はtyped invalid。
- `tim03_request_plan.rs`: fresh node/session/controlで wired exact matrix。queue residueに依存しない。
- param eventは process-start-relative frame offset 0/1/middle/last-in-callを使い、same-position eventsを `(position_ticks, source_sequence)`順で確認する。
- `tim03_loop_boundary.rs` / `tim03_driver_loop_boundary.rs`: 100/128/256 geometryで end-aligned/interior/head-cascade loop wrapを区別する。

## Headless simultaneous ordering matrix

`tim03_req_simultaneous_loop_seek_tempo_param` は W=100/128/256 × valid B=0/1/100/last を fresh wired session/nodeで実行する。300-tick loopに対して head初期位置を `300-B` とし、natural loop wrap・target Seek・tempo-map付きPlanInstallを同じ boundaryへ収束させる。target Seekは `307` で post-seek loop collapse後 position 7 となり、wrap collapse位置0と区別できる。exact process lengthsは B=0で `[W]`、interior Bで `[B,W-B]`、3つのsame-position param eventsは post-seek position 7から process-start-relative offsetで exactly once deliveryし、source_sequence順とsame-param last-winsを固定する。plan-owned musical transportは tempo map revision 1と明示PPQN 480を使い、target call startで bpm_milli=120000/ppqn=480を観測する。W=100/B=100は valid range外なので対象外であり、clampしない。

## Production real ABI oracle

`tim03_l1_frame_boundary_matrix.rs` は VST3/CLAP両format、W=100/128/256で production composition seamから out-of-process L1 runnerの test_instrumentを駆動する。seek-derived positive、natural loop positive、tempo revision positiveを独立 click lawで判定し、amplitude 0.5、stereo identical、non-click silenceを要求する。

- lookahead latency = `2 × activated max_frames`（この seamでは W）
- initial/pass-through/duplicate/freshness外 blockは positive evidenceに数えない
- arbitrary interior command installationは unsupported
- param/frame offset primaryは independent observability不足のため NotRun。test_gain laneは secondary corroborationに留める

## Canonical lane

`mise run test-tim03-frame-boundary-matrix` は runner build、player matrix integration tests、VST3/CLAP real ABI integration testを `--test-threads=1`で束ねる。
