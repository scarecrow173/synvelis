//! sv#572: `port_capability_generation` must move on a
//! same-`capability_revision` rebind to a different plugin identity.
//! The pre-fix fingerprint summed only `capability_revision` values,
//! so `Resolved(A, rev:3)` → `Resolved(B, rev:3)` — a plugin update
//! that keeps the capability revision — left the generation
//! unchanged and an intent authored under plugin A could stage
//! against plugin B's view without `StalePortCapability`. These pins
//! lock the identity-mixed contract: the fingerprint covers each
//! binding's plugin identity (`descriptor`/`version` on the
//! processor, `plugin`/`advertised` on the port), not the revision
//! alone — and a state-identical rebind still leaves the generation
//! untouched.

use synvelis_project::*;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply_op(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

/// A project with one track strip ready for a sidechain-capable
/// processor insert (three stamped slots: in / out / sidechain).
fn strip_project() -> (ProjectController, StripId) {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply_op(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (c, strip)
}

fn plugin_ref(descriptor: &str, version: &str, capability_revision: u64) -> PluginRef {
    PluginRef {
        descriptor: descriptor.into(),
        version: version.into(),
        capability_revision,
    }
}

fn insert_processor(
    c: &mut ProjectController,
    strip: StripId,
    binding: ProcessorBinding,
) -> ProcessorId {
    apply_op(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding,
            sidechain_input: true,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn rebind(c: &mut ProjectController, proc: ProcessorId, binding: ProcessorBinding) {
    apply_op(
        c,
        ProjectOp::SetProcessorBinding {
            processor_id: proc,
            binding,
        },
    );
}

// ── sv#572 AC1: same-revision rebind moves the generation ────────

#[test]
fn same_revision_different_descriptor_rebind_moves_generation() {
    let plugin_a = plugin_ref("vendor.a", "1.0.0", 3);
    // The residual gap `synvelis#571`'s port re-stamp cannot close:
    // the stamp updates, but a revision-only fingerprint sees the
    // same sum either way.
    let plugin_b = plugin_ref("vendor.b", "1.0.0", 3);
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, ProcessorBinding::Resolved(plugin_a));
    let before = port_capability_generation(c.live());

    rebind(&mut c, proc, ProcessorBinding::Resolved(plugin_b));

    assert_ne!(
        port_capability_generation(c.live()),
        before,
        "a rebind to a different plugin identity must move the \
         generation even when capability_revision is unchanged"
    );
}

#[test]
fn same_revision_different_version_rebind_moves_generation() {
    // `version` never reaches the port stamp — only the processor
    // binding carries it — so a version bump at a fixed descriptor
    // and revision is the case port-side identity alone cannot see.
    let plugin_a = plugin_ref("vendor.a", "1.0.0", 3);
    let plugin_b = plugin_ref("vendor.a", "2.0.0", 3);
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, ProcessorBinding::Resolved(plugin_a));
    let before = port_capability_generation(c.live());

    rebind(&mut c, proc, ProcessorBinding::Resolved(plugin_b));

    assert_ne!(
        port_capability_generation(c.live()),
        before,
        "a version change at identical descriptor/revision must \
         still move the generation"
    );
}

#[test]
fn unresolved_descriptor_rebind_moves_generation() {
    // The last-known identity is part of the authored view too:
    // re-pointing an unresolved processor at a different descriptor
    // re-stamps its ports (`Unresolved.plugin`) and must move the
    // generation.
    let (mut c, strip) = strip_project();
    let proc = insert_processor(
        &mut c,
        strip,
        ProcessorBinding::Unresolved {
            descriptor: "vendor.a".into(),
        },
    );
    let before = port_capability_generation(c.live());

    rebind(
        &mut c,
        proc,
        ProcessorBinding::Unresolved {
            descriptor: "vendor.b".into(),
        },
    );

    assert_ne!(
        port_capability_generation(c.live()),
        before,
        "an Unresolved→Unresolved rebind to a different descriptor \
         must move the generation"
    );
}

#[test]
fn identical_rebind_keeps_generation() {
    // The generation is a pure function of persisted state — a
    // rebind to an identical `PluginRef` produces an identical
    // project and must not invalidate authored intents.
    let plugin_a = plugin_ref("vendor.a", "1.0.0", 3);
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, ProcessorBinding::Resolved(plugin_a.clone()));
    let before = port_capability_generation(c.live());

    rebind(&mut c, proc, ProcessorBinding::Resolved(plugin_a));

    assert_eq!(port_capability_generation(c.live()), before);
}

// ── sv#572 AC2: a pre-rebind intent rejects StalePortCapability ──

#[test]
fn intent_authored_before_same_revision_rebind_rejects_stale() {
    let plugin_a = plugin_ref("vendor.a", "1.0.0", 3);
    let plugin_b = plugin_ref("vendor.b", "1.0.0", 3);
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, ProcessorBinding::Resolved(plugin_a));
    let authored_generation = port_capability_generation(c.live());

    // An intent that only touches plugin-free structure — the
    // rejection must come from the generation check, not the edit.
    let intent = RoutingEditIntent {
        operation_id: "op".into(),
        expected_port_capability_generation: authored_generation,
        edits: vec![RoutingEdit::SetMute {
            strip: StripRef::Existing(strip),
            muted: true,
        }],
        approved_conversion: ExplicitConversionPolicy::None,
    };

    // Sanity: authored against the live view, the intent stages.
    let mut ids = CounterIdGenerator::new(c.live().id_watermark);
    routing_preflight(c.live(), &intent, &mut ids).unwrap();

    rebind(&mut c, proc, ProcessorBinding::Resolved(plugin_b));
    let live_generation = port_capability_generation(c.live());
    assert_ne!(live_generation, authored_generation);

    let mut ids = CounterIdGenerator::new(c.live().id_watermark);
    let err = routing_preflight(c.live(), &intent, &mut ids).unwrap_err();
    assert_eq!(
        err,
        RoutingEditError::StalePortCapability {
            expected: authored_generation,
            actual: live_generation,
        }
    );

    // The commit path collapses to the same typed rejection.
    let err = c
        .apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::Routing { intent },
        })
        .unwrap_err();
    assert!(
        err.to_string().contains("routing-stale-capability"),
        "{err}"
    );
}
