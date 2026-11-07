//! ALIB-01 conformance: Local Folder and Project Assets satisfy the
//! same `AssetProvider` catalog contract — browse/stat/pin/open with
//! stable identities, typed capabilities, verified streams.
//!
//! This is the test the issue asks for ("Local/Project Assetsの
//! list/stat/pin/readで同一正本とStable Identityを正しく示す"): the
//! same generic sequence runs against both baseline providers.

mod common;

use std::collections::BTreeSet;
use std::io::Read;
use std::sync::{Arc, Mutex};

use synvelis_library::*;
use synvelis_project::{
    Command, CounterIdGenerator, Project, ProjectController, ProjectId, ProjectOp,
};

use common::{TempTree, aref, browse_req, cx, pid};

/// The contract sequence every baseline provider must satisfy:
/// browse → stat each asset → pin each asset → `open_pinned` verifies
/// bytes end-to-end. Returns the locator→bytes table actually read.
fn exercise_contract(provider: &dyn AssetProvider, cx: &ProviderContext) -> Vec<(String, Vec<u8>)> {
    // 1. Capabilities are declared before use — every op used here is
    //    supported on both baseline providers.
    let caps = provider.capabilities(None);
    for cap in [
        &caps.browse,
        &caps.stat,
        &caps.range_read,
        &caps.fetch,
        &caps.checksum_hint,
    ] {
        assert!(cap.is_supported(), "capability must be declared");
    }
    assert!(caps.pin.is_supported());

    // 2. Browse the root: all entries are Assets, reachable, sorted.
    let page = provider.browse(&browse_req(50), cx).unwrap();
    assert_eq!(page.coverage, Coverage::Complete);
    let assets: Vec<_> = page
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Asset && e.availability == Availability::Available)
        .collect();
    assert!(!assets.is_empty(), "fixture must contain assets");

    // 3. Locators are stable: browse → stat → pin all address the same
    //    object by the same locator.
    let mut locators = BTreeSet::new();
    let mut fetched = Vec::new();
    for entry in &assets {
        assert!(locators.insert(entry.locator.clone()), "duplicate locator");
        let asset = aref(provider.id(), entry.locator.as_str());

        // 4. stat reports typed observed facts for the same locator.
        let stat = provider.stat(&asset, cx).unwrap();
        assert_eq!(stat.asset, asset);
        assert_eq!(stat.availability, Availability::Available);
        assert!(stat.facts.byte_len.value().is_some());
        assert!(stat.facts.kind.value().is_some());
        // Duration/BPM/key are never fabricated.
        assert_eq!(stat.facts.media.bpm.value(), None);

        // 5. pin binds a fetchable object with declared proof kind.
        let pin = provider.pin(&asset, cx).unwrap();
        assert_eq!(pin.asset, asset);
        match (&caps.pin, &pin.proof) {
            (PinCapability::ConditionalStrong, PinProof::ConditionalStrong { .. })
            | (PinCapability::ImmutableObject, PinProof::ImmutableObject { .. })
            | (PinCapability::ConditionalOrImmutable, _) => {}
            (cap, proof) => panic!("pin proof {proof:?} exceeds declared {cap:?}"),
        }

        // 6. open_pinned streams the bytes and verifies at EOF.
        let mut read = provider.open_pinned(&pin, cx).unwrap();
        assert!(matches!(
            read.claims.verification,
            VerificationPolicy::StreamVerified { .. } | VerificationPolicy::IdentityBound { .. }
        ));
        let mut bytes = Vec::new();
        read.reader.read_to_end(&mut bytes).unwrap();
        let receipt = read.completion.take().expect("terminal outcome").unwrap();
        assert_eq!(receipt.bytes_read, bytes.len() as u64);
        if let VerificationPolicy::StreamVerified { .. } = read.claims.verification {
            assert!(receipt.verified, "stream-verified fetch must verify");
            assert_eq!(receipt.digest, pin.digest_hint);
        }
        fetched.push((asset.locator.as_str().to_owned(), bytes));
    }
    fetched
}

#[test]
fn local_folder_satisfies_the_catalog_contract() {
    let tree = TempTree::new("contract-local");
    tree.write("media/a.wav", b"local-alpha");
    tree.write("media/b.txt", b"local-beta");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();

    let fetched = exercise_contract(&provider, &cx());
    let table: std::collections::BTreeMap<_, _> = fetched.into_iter().collect();
    assert_eq!(table["media/a.wav"], b"local-alpha");
    assert_eq!(table["media/b.txt"], b"local-beta");
}

#[test]
fn project_assets_satisfies_the_catalog_contract() {
    let tree = TempTree::new("contract-project");
    tree.write("blobs/dddd0000eeee5555", b"project-alpha");
    tree.write("blobs/ffff1111aaaa6666", b"project-beta");

    let mut c = ProjectController::new(
        Project::new(ProjectId::new(1), "p"),
        Box::new(CounterIdGenerator::new(1)),
    );
    for digest in ["dddd0000eeee5555", "ffff1111aaaa6666"] {
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::RegisterAsset {
                digest: digest.into(),
            },
        })
        .unwrap();
    }
    let blobs = LocalBlobDir::new(tree.join("blobs"));
    let provider = ProjectAssetsProvider::new(pid(2), Arc::new(Mutex::new(c)), blobs);

    let fetched = exercise_contract(&provider, &cx());
    // Every fetched locator is a tagged project AssetId, and every
    // byte sequence equals the published blob — the project aggregate
    // remains the single authority.
    let table: std::collections::BTreeMap<_, Vec<u8>> = fetched.into_iter().collect();
    assert_eq!(table.len(), 2);
    assert!(table.values().any(|b| b == b"project-alpha"));
    assert!(table.values().any(|b| b == b"project-beta"));
    assert!(table.keys().all(|k| k.starts_with("asset-")));
}
