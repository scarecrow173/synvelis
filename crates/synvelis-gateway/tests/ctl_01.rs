//! CTL-01 (issue #121): gateway typed-resource surface for the new
//! `ControlOwner` variants — `Strip`/`Send`/`Processor` refs parse,
//! enumerate, and project scope facts.

use synvelis_gateway::request::{ResourceKind, ResourceRef};
use synvelis_gateway::snapshot::{attributes, enumerate_kind};
use synvelis_project::{
    ChainId, ChannelStrip, ConnectionKind, PortBinding, PortDirection, PortOwner, PortRole,
    ProcessorChain, Project, ProjectId, RoutingConnection, RoutingConnectionId, SemanticPort,
    SemanticPortId, SendConfig, SendId, SendTap, SignalDomain, StripId, StripOwner, Track, TrackId,
};

/// A project carrying one track strip, one send, and no processors —
/// the minimum routing state the new refs project over.
fn seeded() -> (Project, StripId, SendId) {
    let mut p = Project::new(ProjectId::new(0), "p");
    let track = TrackId::new(10);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t".into(),
            lane_ids: Vec::new(),
        },
    );
    let strip = StripId::new(11);
    let chain = ChainId::new(12);
    p.routing.chains.insert(
        chain,
        ProcessorChain {
            id: chain,
            processors: Vec::new(),
        },
    );
    let pin = SemanticPortId::new(13);
    let pout = SemanticPortId::new(14);
    for (id, direction) in [(pin, PortDirection::Input), (pout, PortDirection::Output)] {
        p.routing.ports.insert(
            id,
            SemanticPort {
                id,
                owner: PortOwner::Strip(strip),
                direction,
                role: PortRole::Main,
                domain: SignalDomain::Audio,
                binding: PortBinding::Internal,
                channels: synvelis_project::routing::DEFAULT_PORT_CHANNELS,
            },
        );
    }
    p.routing.strips.insert(
        strip,
        ChannelStrip {
            id: strip,
            owner: StripOwner::Track(track),
            chain,
            port_ids: vec![pin, pout],
        },
    );
    let send = SendId::new(15);
    let conn = RoutingConnectionId::new(16);
    let master_in = {
        let master = &p.routing.buses[&p.routing.master];
        let mstrip = &p.routing.strips[&master.strip];
        mstrip
            .port_ids
            .iter()
            .find(|id| p.routing.ports[id].direction == PortDirection::Input)
            .copied()
            .unwrap()
    };
    p.routing.connections.insert(
        conn,
        RoutingConnection {
            id: conn,
            source: pout,
            destination: master_in,
            kind: ConnectionKind::Send(send),
            enabled: true,
            channel_map: None,
        },
    );
    p.routing.sends.insert(
        send,
        SendConfig {
            id: send,
            connection: conn,
            tap: SendTap::PostFader,
            gain_target: None,
        },
    );
    (p, strip, send)
}

#[test]
fn strip_send_processor_refs_roundtrip_through_parse_and_kind() {
    for (kind_name, id, kind) in [
        ("strip", StripId::new(1).to_tagged(), ResourceKind::Strip),
        ("send", SendId::new(2).to_tagged(), ResourceKind::Send),
        (
            "processor",
            synvelis_project::ProcessorId::new(3).to_tagged(),
            ResourceKind::Processor,
        ),
    ] {
        let r = ResourceRef::from_parts(kind_name, &id).unwrap();
        assert_eq!(r.kind(), kind);
        // The kind serializes to its kebab wire name and parses back.
        let wire = serde_json::to_value(kind).unwrap();
        assert_eq!(ResourceKind::parse(wire.as_str().unwrap()).unwrap(), kind);
        // Kind-typed expect agrees.
        assert_eq!(ResourceRef::expect(kind, &id).unwrap(), r);
    }
    // The kind comes out of the id's own tag, not the declared name —
    // mismatches surface as WrongResourceKind.
    assert!(ResourceRef::from_parts("strip", &SendId::new(4).to_tagged()).is_err());
    assert!(ResourceRef::expect(ResourceKind::Send, &StripId::new(4).to_tagged()).is_err());
    assert!(ResourceKind::parse("strip").is_ok());
    assert!(ResourceKind::parse("bus").is_err());
}

#[test]
fn enumerate_and_scope_facts_cover_the_new_routing_refs() {
    let (p, strip, send) = seeded();
    // Two strips: the seeded track strip plus the bootstrapped
    // master's.
    assert_eq!(
        enumerate_kind(&p, ResourceKind::Strip),
        vec![
            ResourceRef::Strip(strip),
            ResourceRef::Strip(p.routing.buses[&p.routing.master].strip),
        ]
    );
    assert_eq!(
        enumerate_kind(&p, ResourceKind::Send),
        vec![ResourceRef::Send(send)]
    );
    assert_eq!(enumerate_kind(&p, ResourceKind::Processor), Vec::new());

    // A track strip's scope owner is its track; a bus strip's is the
    // project (the resource model carries no bus ref).
    let attrs = attributes(&p, &ResourceRef::Strip(strip)).unwrap();
    assert_eq!(attrs.kind, ResourceKind::Strip);
    assert_eq!(attrs.owner, Some(ResourceRef::Track(TrackId::new(10))));
    let master_strip = p.routing.buses[&p.routing.master].strip;
    let attrs = attributes(&p, &ResourceRef::Strip(master_strip)).unwrap();
    assert_eq!(attrs.owner, Some(ResourceRef::Project(p.project_id)));

    let attrs = attributes(&p, &ResourceRef::Send(send)).unwrap();
    assert_eq!(attrs.kind, ResourceKind::Send);
    assert_eq!(attrs.owner, Some(ResourceRef::Project(p.project_id)));

    // Unknown ids resolve to `None` — no fabricated facts.
    assert!(attributes(&p, &ResourceRef::Strip(StripId::new(999))).is_none());
    assert!(
        attributes(
            &p,
            &ResourceRef::Processor(synvelis_project::ProcessorId::new(999))
        )
        .is_none()
    );
}
