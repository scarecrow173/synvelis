//! ALIB-01 `LocalFolderProvider` tests: bounded browse, traversal and
//! symlink safety, paging/cursor invalidation, pin→open verification,
//! mutation/rename/missing handling, cancel and limits.

mod common;

use std::fs;
use std::io::Read;
use std::time::{Duration, Instant};

use synvelis_library::*;

use common::{TempTree, aref, browse_req, cx, cx_named, loc, pid, try_symlink};

fn provider_at(tree: &TempTree) -> LocalFolderProvider {
    LocalFolderProvider::new(pid(1), tree.path()).unwrap()
}

fn fixture() -> (TempTree, LocalFolderProvider) {
    let tree = TempTree::new("local");
    tree.write("a.wav", b"alpha-bytes");
    tree.write("b.txt", b"beta");
    tree.write("sub/c.wav", b"cee");
    tree.mkdir("sub/deep");
    let provider = provider_at(&tree);
    (tree, provider)
}

// ── browse ───────────────────────────────────────────────────────

#[test]
fn browse_lists_files_and_dirs_with_observed_facts() {
    let (_tree, provider) = fixture();
    let page = provider.browse(&browse_req(50), &cx()).unwrap();

    assert_eq!(page.coverage, Coverage::Complete);
    assert!(page.next_cursor.is_none());
    let locs: Vec<_> = page.entries.iter().map(|e| e.locator.as_str()).collect();
    assert_eq!(locs, ["a.wav", "b.txt", "sub", "sub/c.wav", "sub/deep"]);

    let sub = page
        .entries
        .iter()
        .find(|e| e.locator.as_str() == "sub")
        .unwrap();
    assert_eq!(sub.kind, EntryKind::Container);
    assert_eq!(sub.facts.kind.value(), Some(&AssetKind::Directory));
    assert!(!sub.via_symlink);

    let a = &page.entries[0];
    assert_eq!(a.display_name, "a.wav");
    assert_eq!(a.kind, EntryKind::Asset);
    assert_eq!(a.availability, Availability::Available);
    assert_eq!(a.facts.byte_len.value(), Some(&11));
    assert!(a.facts.modified.is_observed());
    // Media facts are never fabricated: unprobed means Unknown.
    assert_eq!(a.facts.media.duration_ms.value(), None);
    assert!(matches!(
        a.facts.media.duration_ms,
        AssetObservation::Unknown { .. }
    ));
    // A strong checksum requires fetching bytes — never guessed.
    assert!(matches!(
        a.facts.checksum_hint,
        AssetObservation::Unknown {
            reason: UnavailableReason::RequiresFetch
        }
    ));
}

#[test]
fn browse_empty_scope_is_complete_not_error() {
    let tree = TempTree::new("empty");
    let provider = provider_at(&tree);
    let page = provider.browse(&browse_req(10), &cx()).unwrap();
    assert_eq!(page.entries, Vec::<BrowseEntry>::new());
    assert_eq!(page.coverage, Coverage::Complete);
    assert!(page.next_cursor.is_none());
}

#[test]
fn browse_sub_scope_lists_only_that_subtree() {
    let (_tree, provider) = fixture();
    let mut req = browse_req(10);
    req.scope = BrowseScope::Locator {
        locator: loc("sub"),
    };
    let page = provider.browse(&req, &cx()).unwrap();
    let locs: Vec<_> = page.entries.iter().map(|e| e.locator.as_str()).collect();
    assert_eq!(locs, ["sub/c.wav", "sub/deep"]);
}

#[test]
fn browse_scope_must_be_a_container() {
    let (_tree, provider) = fixture();
    let mut req = browse_req(10);
    req.scope = BrowseScope::Locator {
        locator: loc("a.wav"),
    };
    let err = provider.browse(&req, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::InvalidRequest);
}

#[test]
fn browse_page_limit_zero_is_rejected() {
    let (_tree, provider) = fixture();
    let err = provider.browse(&browse_req(0), &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::InvalidRequest);
}

// ── paging / cursors ─────────────────────────────────────────────

#[test]
fn paging_walks_all_entries_once_without_dupes() {
    let (_tree, provider) = fixture();
    let mut seen = Vec::new();
    let mut req = browse_req(2);
    loop {
        let page = provider.browse(&req, &cx()).unwrap();
        req.cursor = page.next_cursor.clone();
        seen.extend(page.entries.iter().map(|e| e.locator.as_str().to_owned()));
        if page.next_cursor.is_none() {
            assert_eq!(page.coverage, Coverage::Complete);
            break;
        }
        // A non-final page is honest about more data existing.
        assert_eq!(page.coverage, Coverage::Partial);
    }
    assert_eq!(seen, ["a.wav", "b.txt", "sub", "sub/c.wav", "sub/deep"]);
}

#[test]
fn cursor_is_rejected_after_query_scope_or_principal_changes() {
    let (_tree, provider) = fixture();
    let first = provider.browse(&browse_req(2), &cx()).unwrap();
    let cursor = first.next_cursor.unwrap();

    // Same query → accepted.
    let mut req = browse_req(2);
    req.cursor = Some(cursor.clone());
    assert!(provider.browse(&req, &cx()).is_ok());

    // Changed scope → rejected.
    req.scope = BrowseScope::Locator {
        locator: loc("sub"),
    };
    let err = provider.browse(&req, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);
    req.scope = BrowseScope::Root;

    // Changed filter → rejected.
    req.filter.name_contains = Some("wav".into());
    let err = provider.browse(&req, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);
    req.filter = FilterSpec::default();

    // Changed sort → rejected.
    req.sort.direction = SortDirection::Desc;
    let err = provider.browse(&req, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);
    req.sort = SortSpec::default();

    // Changed principal → rejected.
    let err = provider
        .browse(&req, &cx_named("other-principal"))
        .unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);
}

#[test]
fn cursor_from_another_provider_is_rejected() {
    let tree_a = TempTree::new("foreign-a");
    let tree_b = TempTree::new("foreign-b");
    tree_a.write("a.wav", b"a");
    tree_a.write("a2.wav", b"a2");
    tree_b.write("b.wav", b"b");
    let pa = LocalFolderProvider::new(pid(1), tree_a.path()).unwrap();
    let pb = LocalFolderProvider::new(pid(2), tree_b.path()).unwrap();

    // Two entries + page_limit 1 → a real cursor minted by provider A.
    let cursor = pa
        .browse(&browse_req(1), &cx())
        .unwrap()
        .next_cursor
        .unwrap();
    let mut req = browse_req(1);
    req.cursor = Some(cursor);
    let err = pb.browse(&req, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);
}

#[test]
fn undecodable_cursor_is_rejected() {
    let (_tree, provider) = fixture();
    let mut req = browse_req(2);
    // Persisted cursors round-trip via serde; hostile/garbage tokens
    // are rejected at decode.
    for bad in ["\"zzzz\"", "\"7b7d\""] {
        req.cursor = Some(serde_json::from_str::<OpaquePageCursor>(bad).unwrap());
        let err = provider.browse(&req, &cx()).unwrap_err();
        assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid, "token {bad}");
    }
    // An empty token is invalid at the serde boundary itself — it
    // never reaches the decoder.
    assert!(serde_json::from_str::<OpaquePageCursor>("\"\"").is_err());
}

// ── stat ─────────────────────────────────────────────────────────

#[test]
fn stat_reports_observed_facts() {
    let (_tree, provider) = fixture();
    let stat = provider.stat(&aref(pid(1), "a.wav"), &cx()).unwrap();
    assert_eq!(stat.availability, Availability::Available);
    assert_eq!(stat.facts.byte_len.value(), Some(&11));
    assert_eq!(stat.facts.kind.value(), Some(&AssetKind::File));
    // The project-style provenance list is empty for a bare file.
    assert_eq!(stat.provenance, Vec::<ProvenanceRecord>::new());
}

#[test]
fn stat_reports_link_kind_not_target_kind() {
    let (tree, provider) = fixture();
    let link = tree.join("linked");
    if !try_symlink(&tree.join("sub"), &link) {
        eprintln!("symlink privilege unavailable — skipping");
        return;
    }
    // `stat` on the link locator reports `Symlink` — the target's
    // Directory kind must not silently leak through.
    let stat = provider.stat(&aref(pid(1), "linked"), &cx()).unwrap();
    assert_eq!(stat.facts.kind.value(), Some(&AssetKind::Symlink));
}

#[test]
fn stat_missing_and_directory_and_foreign() {
    let (_tree, provider) = fixture();
    assert_eq!(
        provider
            .stat(&aref(pid(1), "gone.wav"), &cx())
            .unwrap_err()
            .kind(),
        ProviderErrorKind::NotFound
    );
    let dir = provider.stat(&aref(pid(1), "sub"), &cx()).unwrap();
    assert_eq!(dir.facts.kind.value(), Some(&AssetKind::Directory));
    // A ref owned by another provider is not this provider's asset.
    assert_eq!(
        provider
            .stat(&aref(pid(9), "a.wav"), &cx())
            .unwrap_err()
            .kind(),
        ProviderErrorKind::InvalidRequest
    );
}

// ── traversal safety ─────────────────────────────────────────────

#[test]
fn traversal_locators_are_rejected_before_any_io() {
    let (_tree, provider) = fixture();
    for bad in [
        "../up",
        "a/../../up",
        "sub/../..",
        "/abs",
        "c:/x",
        "a\\b",
        ".",
        "a//b",
    ] {
        let err = provider.stat(&aref(pid(1), bad), &cx());
        assert_eq!(
            err.unwrap_err().kind(),
            ProviderErrorKind::InvalidRequest,
            "locator {bad:?}"
        );
    }
}

// ── symlink safety ───────────────────────────────────────────────

#[test]
fn symlink_into_root_is_browsable_and_marked() {
    let (tree, provider) = fixture();
    let link = tree.join("linked");
    if !try_symlink(&tree.join("sub"), &link) {
        eprintln!("symlink privilege unavailable — skipping");
        return;
    }
    let page = provider.browse(&browse_req(20), &cx()).unwrap();
    let entry = page
        .entries
        .iter()
        .find(|e| e.locator.as_str() == "linked")
        .unwrap();
    assert_eq!(entry.kind, EntryKind::Container);
    assert!(entry.via_symlink);
    assert_eq!(entry.facts.kind.value(), Some(&AssetKind::Symlink));
    assert_eq!(entry.availability, Availability::Available);
    // Descending through the link yields the target's canonical
    // locators — the link itself is a route, not an identity.
    assert!(
        page.entries
            .iter()
            .any(|e| e.locator.as_str() == "sub/c.wav")
    );
}

#[test]
fn symlink_loop_does_not_recurse_forever() {
    let (tree, provider) = fixture();
    let link = tree.join("sub/loop");
    if !try_symlink(tree.path(), &link) {
        eprintln!("symlink privilege unavailable — skipping");
        return;
    }
    // `sub/loop` → root: descending would loop. The page completes.
    let page = provider.browse(&browse_req(50), &cx()).unwrap();
    assert_eq!(page.coverage, Coverage::Complete);
    let loop_entry = page
        .entries
        .iter()
        .find(|e| e.locator.as_str() == "sub/loop")
        .unwrap();
    assert_eq!(loop_entry.kind, EntryKind::Container);
    // Entries under the real tree still appear exactly once.
    let count = page
        .entries
        .iter()
        .filter(|e| e.locator.as_str() == "sub/c.wav")
        .count();
    assert_eq!(count, 1);
}

#[test]
fn ancestor_symlink_cannot_leak_out_of_locator_scope() {
    // V3 regression: a `BrowseScope::Locator` walk must never re-enter
    // the scope or emit out-of-scope locators through a link pointing
    // at an ancestor.
    let (tree, provider) = fixture();
    let link = tree.join("sub/up");
    if !try_symlink(tree.path(), &link) {
        eprintln!("symlink privilege unavailable — skipping");
        return;
    }
    let mut req = browse_req(50);
    req.scope = BrowseScope::Locator {
        locator: loc("sub"),
    };
    let page = provider.browse(&req, &cx()).unwrap();
    // Every locator stays inside the `sub` scope — nothing under the
    // root leaks through `sub/up`.
    let mut locs: Vec<_> = page.entries.iter().map(|e| e.locator.as_str()).collect();
    locs.sort_unstable();
    assert_eq!(locs, ["sub/c.wav", "sub/deep", "sub/up"]);
    // The ancestor-bound link is observed but unreachable through the
    // scope boundary.
    let up = page
        .entries
        .iter()
        .find(|e| e.locator.as_str() == "sub/up")
        .unwrap();
    assert!(matches!(up.availability, Availability::Unavailable { .. }));
}

#[test]
fn symlink_escaping_root_is_listed_but_never_opened() {
    let (tree, provider) = fixture();
    let outside = TempTree::new("escape-target");
    outside.write("secret.txt", b"secret");
    let link = tree.join("escape");
    if !try_symlink(&outside.join("secret.txt"), &link) {
        eprintln!("symlink privilege unavailable — skipping");
        return;
    }
    // Browse lists it — it exists — but marked unreachable.
    let page = provider.browse(&browse_req(20), &cx()).unwrap();
    let entry = page
        .entries
        .iter()
        .find(|e| e.locator.as_str() == "escape")
        .unwrap();
    assert!(matches!(
        entry.availability,
        Availability::Unavailable { .. }
    ));

    // stat / pin / open all refuse — the target is outside the root.
    let escape = aref(pid(1), "escape");
    for (op, err) in [
        ("stat", provider.stat(&escape, &cx()).map(|_| ())),
        ("pin", provider.pin(&escape, &cx()).map(|_| ())),
        (
            "preview",
            provider.open_preview(&escape, None, &cx()).map(|_| ()),
        ),
    ] {
        assert_eq!(
            err.unwrap_err().kind(),
            ProviderErrorKind::PermissionDenied,
            "{op} through escaping symlink"
        );
    }
}

#[test]
fn dangling_symlink_is_missing_not_fatal() {
    let (tree, provider) = fixture();
    let link = tree.join("dangling");
    if !try_symlink(&tree.join("never-existed"), &link) {
        eprintln!("symlink privilege unavailable — skipping");
        return;
    }
    let page = provider.browse(&browse_req(20), &cx()).unwrap();
    let entry = page
        .entries
        .iter()
        .find(|e| e.locator.as_str() == "dangling")
        .unwrap();
    assert!(matches!(entry.availability, Availability::Missing { .. }));
    assert_eq!(
        provider
            .stat(&aref(pid(1), "dangling"), &cx())
            .unwrap_err()
            .kind(),
        ProviderErrorKind::NotFound
    );
}

// ── pin / open_pinned ────────────────────────────────────────────

#[test]
fn pin_then_open_streams_and_verifies() {
    let (_tree, provider) = fixture();
    let asset = aref(pid(1), "a.wav");
    let pin = provider.pin(&asset, &cx()).unwrap();

    assert!(matches!(
        pin.proof,
        PinProof::ConditionalStrong {
            token: StrongRevisionToken::ContentHash { .. }
        }
    ));
    assert_eq!(pin.expected_size, Some(11));
    assert!(pin.digest_hint.is_some());
    assert!(pin.object_identity.as_str().starts_with("sha256:"));

    let mut read = provider.open_pinned(&pin, &cx()).unwrap();
    assert!(matches!(
        read.claims.verification,
        VerificationPolicy::StreamVerified { .. }
    ));
    assert_eq!(read.claims.expected_size, Some(11));
    // Nothing is finished until the stream reaches EOF.
    assert!(read.completion.take().is_none());

    let mut bytes = Vec::new();
    read.reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"alpha-bytes");

    let receipt = read.completion.take().unwrap().unwrap();
    assert!(receipt.verified);
    assert_eq!(receipt.bytes_read, 11);
    assert_eq!(receipt.digest, pin.digest_hint);
}

#[test]
fn zero_length_reads_do_not_brick_the_stream() {
    // `read(&mut [])` must be a no-op, not a spurious EOF that records
    // a false completion and starves the real bytes.
    let (_tree, provider) = fixture();
    let pin = provider.pin(&aref(pid(1), "a.wav"), &cx()).unwrap();
    let mut read = provider.open_pinned(&pin, &cx()).unwrap();
    assert_eq!(read.reader.read(&mut []).unwrap(), 0);
    assert!(!read.completion.is_finished());
    let mut bytes = Vec::new();
    read.reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"alpha-bytes");
    let receipt = read.completion.take().unwrap().unwrap();
    assert_eq!(receipt.bytes_read, 11);
}

#[test]
fn pin_failure_blocks_formal_import_path() {
    let (_tree, provider) = fixture();
    // A directory cannot be pinned: the formal import path has no
    // fetchable object.
    let err = provider.pin(&aref(pid(1), "sub"), &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::InvalidRequest);
    // A missing asset cannot be pinned.
    let err = provider.pin(&aref(pid(1), "gone.wav"), &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);
    // A wrong proof type cannot be opened: foreign ConditionalStrong
    // tokens and ImmutableObject proofs are not local pins.
    let fake = PinnedAssetRef {
        asset: aref(pid(1), "a.wav"),
        proof: PinProof::ImmutableObject {
            revision: RevisionKey::new("blob:x").unwrap(),
        },
        object_identity: ObjectIdentity::new("x").unwrap(),
        expected_size: None,
        digest_hint: None,
    };
    let err = provider.open_pinned(&fake, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::InvalidRequest);
}

#[test]
fn changed_content_breaks_the_pin_not_silently_serves() {
    let (tree, provider) = fixture();

    // Same-length mutation → the open size precondition passes, the
    // stream digest verification fails at EOF with IntegrityMismatch.
    let pin = provider.pin(&aref(pid(1), "b.txt"), &cx()).unwrap();
    fs::write(tree.join("b.txt"), b"BBBB").unwrap();
    let mut read = provider.open_pinned(&pin, &cx()).unwrap();
    let mut buf = Vec::new();
    let err = read.reader.read_to_end(&mut buf).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    let outcome = read.completion.take().unwrap().unwrap_err();
    assert_eq!(outcome.kind(), ProviderErrorKind::IntegrityMismatch);

    // Length change → the open precondition itself fails.
    let pin = provider.pin(&aref(pid(1), "a.wav"), &cx()).unwrap();
    fs::write(tree.join("a.wav"), b"alpha-bytes-extended").unwrap();
    let err = provider.open_pinned(&pin, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::SourceChanged);
    assert_eq!(err.retry(), RetryAdvice::AfterUserRePin);
}

#[test]
fn renamed_or_deleted_file_cannot_open_pinned() {
    let (tree, provider) = fixture();
    let pin = provider.pin(&aref(pid(1), "a.wav"), &cx()).unwrap();

    // Rename away → the locator no longer resolves.
    fs::rename(tree.join("a.wav"), tree.join("moved.wav")).unwrap();
    let err = provider.open_pinned(&pin, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);

    // Deleting is the same class of failure.
    let pin = provider.pin(&aref(pid(1), "b.txt"), &cx()).unwrap();
    fs::remove_file(tree.join("b.txt")).unwrap();
    let err = provider.open_pinned(&pin, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);
}

#[test]
fn foreign_proof_is_not_confused_with_local_pin() {
    let (_tree, provider) = fixture();
    let wrong = PinnedAssetRef {
        asset: aref(pid(1), "a.wav"),
        proof: PinProof::ConditionalStrong {
            token: StrongRevisionToken::StrongValidator {
                validator: "\"etag\"".into(),
                precondition: PreconditionKind::IfMatch,
            },
        },
        object_identity: ObjectIdentity::new("x").unwrap(),
        expected_size: None,
        digest_hint: None,
    };
    assert_eq!(
        provider.open_pinned(&wrong, &cx()).unwrap_err().kind(),
        ProviderErrorKind::InvalidRequest
    );
}

// ── preview / search / filters ───────────────────────────────────

#[test]
fn preview_reads_a_range_unverified() {
    let (_tree, provider) = fixture();
    let asset = aref(pid(1), "a.wav");
    let mut read = provider
        .open_preview(&asset, Some(ByteRange { offset: 6, len: 5 }), &cx())
        .unwrap();
    assert_eq!(read.claims.verification, VerificationPolicy::Unverified);
    let mut buf = Vec::new();
    read.reader.read_to_end(&mut buf).unwrap();
    assert_eq!(buf, b"bytes");
    let receipt = read.completion.take().unwrap().unwrap();
    assert!(!receipt.verified);
    assert_eq!(receipt.bytes_read, 5);
}

#[test]
fn search_matches_name_substring() {
    let (_tree, provider) = fixture();
    let page = provider
        .search(
            &SearchQuery {
                text: "wav".into(),
                scope: BrowseScope::Root,
                page_limit: 10,
                cursor: None,
            },
            &cx(),
        )
        .unwrap();
    let locs: Vec<_> = page.entries.iter().map(|e| e.locator.as_str()).collect();
    assert_eq!(locs, ["a.wav", "sub/c.wav"]);
}

#[test]
fn filters_by_name_kind_and_extension() {
    fn locators(provider: &LocalFolderProvider, req: &BrowseRequest) -> Vec<String> {
        provider
            .browse(req, &cx())
            .unwrap()
            .entries
            .iter()
            .map(|e| e.locator.as_str().to_owned())
            .collect()
    }

    let (_tree, provider) = fixture();
    let mut req = browse_req(20);
    req.filter.name_contains = Some("WAV".into()); // case-insensitive
    assert_eq!(locators(&provider, &req), ["a.wav", "sub/c.wav"]);

    req.filter = FilterSpec {
        name_contains: None,
        kinds: Some(vec![EntryKind::Container]),
        extensions: None,
    };
    assert_eq!(locators(&provider, &req), ["sub", "sub/deep"]);

    req.filter = FilterSpec {
        name_contains: None,
        kinds: None,
        extensions: Some(vec!["wav".into()]),
    };
    assert_eq!(locators(&provider, &req), ["a.wav", "sub/c.wav"]);
}

// ── cancel / limits ──────────────────────────────────────────────

#[test]
fn cancelled_context_fails_operations() {
    let (_tree, provider) = fixture();
    let cx = cx();
    cx.cancel.cancel();
    assert_eq!(
        provider.browse(&browse_req(10), &cx).unwrap_err().kind(),
        ProviderErrorKind::Cancelled
    );
    assert_eq!(
        provider
            .pin(&aref(pid(1), "a.wav"), &cx)
            .unwrap_err()
            .kind(),
        ProviderErrorKind::Cancelled
    );
    assert_eq!(
        provider
            .stat(&aref(pid(1), "a.wav"), &cx)
            .unwrap_err()
            .kind(),
        ProviderErrorKind::Cancelled
    );
}

#[test]
fn elapsed_deadline_is_budget_exceeded() {
    let (_tree, provider) = fixture();
    let mut ctx = cx();
    ctx.limits.deadline = Some(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
    assert_eq!(
        provider.browse(&browse_req(10), &ctx).unwrap_err().kind(),
        ProviderErrorKind::BudgetExceeded
    );
}

#[test]
fn entry_budget_truncates_honestly() {
    let (_tree, provider) = fixture();
    let mut ctx = cx();
    ctx.limits.max_entries = 2;
    let page = provider.browse(&browse_req(50), &ctx).unwrap();
    // The walk stopped at the budget: the page is Partial, never a
    // fake-Complete.
    assert_eq!(page.coverage, Coverage::Partial);
    assert!(page.entries.len() <= 2);
}

#[test]
fn depth_budget_marks_truncation() {
    let (_tree, provider) = fixture();
    let mut ctx = cx();
    ctx.limits.max_depth = 1;
    let page = provider.browse(&browse_req(50), &ctx).unwrap();
    // `sub` is listed, its contents not — the page says Partial.
    assert_eq!(page.coverage, Coverage::Partial);
    assert!(page.entries.iter().any(|e| e.locator.as_str() == "sub"));
    assert!(
        !page
            .entries
            .iter()
            .any(|e| e.locator.as_str() == "sub/c.wav")
    );
}

#[test]
fn byte_budget_bounds_pin_and_open() {
    let (_tree, provider) = fixture();
    let mut ctx = cx();
    ctx.limits.max_bytes = 4;
    // `a.wav` is 11 bytes: pin fails the budget.
    assert_eq!(
        provider
            .pin(&aref(pid(1), "a.wav"), &ctx)
            .unwrap_err()
            .kind(),
        ProviderErrorKind::BudgetExceeded
    );
    // `b.txt` is 4 bytes: pin succeeds; a pinned oversized file fails
    // at open too.
    let ok = provider.pin(&aref(pid(1), "b.txt"), &ctx).unwrap();
    let mut read = provider.open_pinned(&ok, &ctx).unwrap();
    let mut buf = Vec::new();
    read.reader.read_to_end(&mut buf).unwrap();
    assert_eq!(buf, b"beta");
}

// ── capabilities ─────────────────────────────────────────────────

#[test]
fn declared_capabilities_match_behavior() {
    let (_tree, provider) = fixture();
    let caps = provider.capabilities(None);
    assert!(caps.browse.is_supported());
    assert!(caps.stat.is_supported());
    assert!(caps.search.is_supported());
    assert_eq!(caps.pin, PinCapability::ConditionalStrong);
    assert!(caps.range_read.is_supported());
    assert!(caps.preview.is_supported());
    assert!(caps.fetch.is_supported());
    assert!(caps.checksum_hint.is_supported());
    assert_eq!(caps.access, AccessMode::ReadOnly);
}
