//! ALIB-01 contract tests for the pure provider types: identity,
//! references, pins, capabilities, observations, errors, registry.

mod common;

use std::time::SystemTime;

use sha2::Digest as _;
use synvelis_library::*;

use common::{aref, loc, pid};

// ── identity ─────────────────────────────────────────────────────

#[test]
fn provider_id_tagged_roundtrip_and_kind_check() {
    let id = pid(0xdead_beef);
    let tagged = id.to_tagged();
    assert_eq!(tagged, "provider-000000000000000000000000deadbeef");
    assert_eq!(ProviderId::from_tagged(&tagged).unwrap(), id);
    // A project-kind tag is not a provider id — typed IDs never mix.
    assert!(ProviderId::from_tagged("asset-000000000000000000000000deadbeef").is_err());
    assert!(ProviderId::from_tagged("provider-zz").is_err());
    // Serde uses the tagged form too.
    let wire = serde_json::to_string(&id).unwrap();
    assert_eq!(serde_json::from_str::<ProviderId>(&wire).unwrap(), id);
}

#[test]
fn opaque_locator_validation() {
    assert!(OpaqueLocator::new("").is_err());
    assert!(OpaqueLocator::new("a\0b").is_err());
    assert!(OpaqueLocator::new("x".repeat(4097)).is_err());
    let l = loc("dir/file.wav");
    assert_eq!(l.as_str(), "dir/file.wav");
    // Serializable + storable: the locator is opaque but round-trips.
    assert_eq!(
        serde_json::from_str::<OpaqueLocator>("\"dir/file.wav\"").unwrap(),
        l
    );
}

#[test]
fn opaque_text_types_validate_on_deserialize() {
    // The validation bound must hold at the serde boundary too — a
    // derived transparent Deserialize would bypass `new()`'s checks.
    assert!(serde_json::from_str::<PrincipalId>("\"\"").is_err());
    assert!(serde_json::from_str::<CredentialKey>("\"\"").is_err());
    assert!(serde_json::from_str::<RevisionKey>("\"\"").is_err());
    assert!(serde_json::from_str::<ObjectIdentity>("\"\"").is_err());
    assert!(serde_json::from_str::<OpaquePageCursor>("\"\"").is_err());
    assert!(
        serde_json::from_str::<OpaquePageCursor>(&format!("\"{}\"", "x".repeat(4097))).is_err()
    );
    // Valid payloads still round-trip.
    let cursor: OpaquePageCursor = serde_json::from_str("\"deadbeef\"").unwrap();
    assert_eq!(cursor.as_str(), "deadbeef");
}

#[test]
fn opaque_credential_debug_redacts_context() {
    let cred = OpaqueCredential::new(
        CredentialKey::new("key-1").unwrap(),
        "session-handle-secret-state",
    );
    let debug = format!("{cred:?}");
    assert!(!debug.contains("session-handle-secret-state"));
    assert!(debug.contains("<redacted>"));
}

#[test]
fn provider_identity_is_distinct_from_project_ids() {
    // Same raw u128, different types: ProviderId cannot be assigned
    // to or compared with synvelis_project::AssetId.
    let provider = pid(1);
    let asset_id = synvelis_project::AssetId::new(1);
    assert_ne!(provider.to_tagged(), asset_id.to_tagged());
    // Same kind-tag separation holds in the wire form.
    assert!(ProviderId::from_tagged(&asset_id.to_tagged()).is_err());
}

// ── references / pins ────────────────────────────────────────────

#[test]
fn content_digest_tagged_form() {
    let digest = ContentDigest::new(DigestAlgorithm::Sha256, &sha2::Sha256::digest(b"abc"));
    let tagged = digest.to_string();
    assert!(tagged.starts_with("sha256:"));
    assert_eq!(ContentDigest::parse(&tagged).unwrap(), digest);
    assert!(ContentDigest::parse("md5:0123").is_err());
    assert!(ContentDigest::parse("sha256:nothex").is_err());
    // Wrong length hex.
    assert!(ContentDigest::parse(&format!("sha256:{}", "ab".repeat(4))).is_err());
}

#[test]
fn pinned_ref_serde_roundtrip() {
    let pin = PinnedAssetRef {
        asset: aref(pid(7), "a.wav"),
        proof: PinProof::ConditionalStrong {
            token: StrongRevisionToken::ContentHash {
                digest: ContentDigest::new(DigestAlgorithm::Sha256, &[0xabu8; 32]),
            },
        },
        object_identity: ObjectIdentity::new("sha256:abab").unwrap(),
        expected_size: Some(12),
        digest_hint: Some(ContentDigest::new(DigestAlgorithm::Sha256, &[0xabu8; 32])),
    };
    let wire = serde_json::to_string(&pin).unwrap();
    let back: PinnedAssetRef = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, pin);

    let immutable = PinnedAssetRef {
        proof: PinProof::ImmutableObject {
            revision: RevisionKey::new("blob:aaaa").unwrap(),
        },
        ..pin.clone()
    };
    let back: PinnedAssetRef =
        serde_json::from_str(&serde_json::to_string(&immutable).unwrap()).unwrap();
    assert_eq!(back, immutable);
}

#[test]
fn weak_evidence_has_no_pin_proof_variant() {
    // The contract has no way to express a weak pin: no mtime/path/
    // weak-etag variant exists on PinProof. This test pins that shape —
    // the enum's two variants are ImmutableObject and ConditionalStrong.
    let wire = serde_json::to_value(PinProof::ImmutableObject {
        revision: RevisionKey::new("commit:00").unwrap(),
    })
    .unwrap();
    assert_eq!(wire["proof"], "immutable-object");
    let wire = serde_json::to_value(PinProof::ConditionalStrong {
        token: StrongRevisionToken::StrongValidator {
            validator: "\"strong-etag\"".into(),
            precondition: PreconditionKind::IfMatch,
        },
    })
    .unwrap();
    assert_eq!(wire["proof"], "conditional-strong");
}

// ── capability ───────────────────────────────────────────────────

#[test]
fn capability_unsupported_carries_typed_reason() {
    let cap = Capability::Unsupported {
        reason: UnsupportedReason::RequiresIndex,
    };
    assert!(!cap.is_supported());
    let wire = serde_json::to_value(&cap).unwrap();
    assert_eq!(wire["state"], "unsupported");
    assert_eq!(wire["reason"]["reason"], "requires-index");
    assert!(Capability::Supported.is_supported());
}

#[test]
fn pin_capability_orders_strengths() {
    assert!(
        !PinCapability::Unsupported {
            reason: UnsupportedReason::NotSupportedByBackend,
        }
        .is_supported()
    );
    assert!(PinCapability::ConditionalStrong.is_supported());
    assert!(PinCapability::ImmutableObject.is_supported());
    assert!(PinCapability::ConditionalOrImmutable.is_supported());
}

// ── observation ──────────────────────────────────────────────────

#[test]
fn observations_separate_observed_inferred_unknown_stale() {
    let observed = AssetObservation::observed(42u64, ObservationOrigin::Filesystem);
    assert!(observed.is_observed());
    assert_eq!(observed.value(), Some(&42));

    let inferred: AssetObservation<u64> = AssetObservation::Inferred {
        value: 120,
        method: InferenceMethod("bpm-grid".into()),
        at: ObservedAt(SystemTime::UNIX_EPOCH),
    };
    assert!(!inferred.is_observed());
    assert_eq!(inferred.value(), Some(&120));

    let unknown: AssetObservation<u64> = AssetObservation::unknown(UnavailableReason::NotProvided);
    assert_eq!(unknown.value(), None);

    let stale: AssetObservation<u64> = AssetObservation::Stale {
        value: 9,
        at: ObservedAt(SystemTime::UNIX_EPOCH),
        reason: StaleReason::CachedSnapshot,
    };
    assert_eq!(stale.value(), Some(&9));
    assert!(!stale.is_observed());
}

#[test]
fn media_facts_default_is_never_fabricated() {
    fn assert_not_provided<T>(field: &AssetObservation<T>) {
        assert!(matches!(
            field,
            AssetObservation::Unknown {
                reason: UnavailableReason::NotProvided
            }
        ));
    }
    let facts = MediaFacts::all_unknown();
    assert_not_provided(&facts.duration_ms);
    assert_not_provided(&facts.sample_rate);
    assert_not_provided(&facts.channels);
    assert_not_provided(&facts.container);
    assert_not_provided(&facts.codec);
    assert_not_provided(&facts.bpm);
    assert_not_provided(&facts.key);
}

// ── errors ───────────────────────────────────────────────────────

#[test]
fn error_kind_maps_to_io_kind_and_stays_typed() {
    let err = ProviderError::new(ProviderErrorKind::IntegrityMismatch, "digest drift");
    assert_eq!(err.kind(), ProviderErrorKind::IntegrityMismatch);
    let io: std::io::Error = err.into();
    assert_eq!(io.kind(), std::io::ErrorKind::InvalidData);
    // The typed error survives as the source — readers can downcast.
    let typed = io
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ProviderError>())
        .unwrap();
    assert_eq!(typed.kind(), ProviderErrorKind::IntegrityMismatch);

    for (kind, io_kind) in [
        (ProviderErrorKind::NotFound, std::io::ErrorKind::NotFound),
        (
            ProviderErrorKind::PermissionDenied,
            std::io::ErrorKind::PermissionDenied,
        ),
        (
            ProviderErrorKind::Cancelled,
            std::io::ErrorKind::Interrupted,
        ),
        (
            ProviderErrorKind::SourceChanged,
            std::io::ErrorKind::InvalidData,
        ),
    ] {
        let io: std::io::Error = ProviderError::new(kind, "x").into();
        assert_eq!(io.kind(), io_kind, "{kind:?}");
    }
}

#[test]
fn retry_advice_is_independent_from_kind() {
    let err = ProviderError::new(ProviderErrorKind::TransientIo, "flaky")
        .with_retry(RetryAdvice::After(Some(std::time::Duration::from_secs(2))));
    assert_eq!(err.kind(), ProviderErrorKind::TransientIo);
    assert_eq!(
        err.retry(),
        RetryAdvice::After(Some(std::time::Duration::from_secs(2)))
    );
    // Stale pins advise a user re-pin by default.
    assert_eq!(
        ProviderError::new(ProviderErrorKind::SourceChanged, "x").retry(),
        RetryAdvice::AfterUserRePin
    );
    // A stale cursor is recovered by re-browsing, never re-pinning.
    assert_eq!(
        ProviderError::new(ProviderErrorKind::CursorInvalid, "x").retry(),
        RetryAdvice::Never
    );
    assert!(ProviderError::unsupported("search", "test").is_unsupported());
}

// ── registry ─────────────────────────────────────────────────────

#[test]
fn registry_resolves_descriptor_and_provider() {
    use std::sync::Arc;

    let tree = common::TempTree::new("registry");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut registry = ProviderRegistry::new();
    registry
        .register(
            ProviderDescriptor {
                id: pid(1),
                kind: ProviderKind::LocalFolder,
                display_name: "fixtures".into(),
            },
            Arc::new(provider),
        )
        .unwrap();

    assert!(registry.get(&pid(1)).is_some());
    assert_eq!(
        registry.descriptor(&pid(1)).unwrap().kind,
        ProviderKind::LocalFolder
    );
    assert_eq!(registry.list().len(), 1);
    assert!(registry.get(&pid(2)).is_none());

    // Duplicate id and id/provider mismatch are rejected.
    let provider2 = LocalFolderProvider::new(pid(2), tree.path()).unwrap();
    let dup = registry.register(
        ProviderDescriptor {
            id: pid(1),
            kind: ProviderKind::LocalFolder,
            display_name: "dup".into(),
        },
        Arc::new(provider2),
    );
    assert_eq!(dup.unwrap_err().kind(), ProviderErrorKind::InvalidRequest);

    let mismatched = registry.register(
        ProviderDescriptor {
            id: pid(9),
            kind: ProviderKind::LocalFolder,
            display_name: "mismatch".into(),
        },
        Arc::new(LocalFolderProvider::new(pid(3), tree.path()).unwrap()),
    );
    assert_eq!(
        mismatched.unwrap_err().kind(),
        ProviderErrorKind::InvalidRequest
    );

    // The descriptor's declared kind is cross-checked against the
    // provider's self-reported kind — a caller misdescribing the
    // registration is rejected, not trusted.
    let kind_mismatched = registry.register(
        ProviderDescriptor {
            id: pid(4),
            kind: ProviderKind::ProjectAssets,
            display_name: "kind-mismatch".into(),
        },
        Arc::new(LocalFolderProvider::new(pid(4), tree.path()).unwrap()),
    );
    assert_eq!(
        kind_mismatched.unwrap_err().kind(),
        ProviderErrorKind::InvalidRequest
    );
}
