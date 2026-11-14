//! `CacheLookup` — deterministic artifact eligibility evaluation
//! (synvelis#291, RCP-02; contract: `docs/specs/render-checkpoint.md`
//! §Eligibility).
//!
//! The lookup consumes the RCP-01 [`RecipeResolution`] as the *input
//! completeness* authority: media verification alone never promotes an
//! artifact to `EligibleExact`. The per-candidate vocabulary is the
//! RCP-04 [`ArtifactEligibility`]/[`ReproducibilityClass`]/
//! [`AccessState`] types verbatim — this module is the evaluator those
//! types were declared as input for. The lookup *outcome* uses the
//! Issue #291 vocabulary (`EligibleExact`/`EligiblePreview`/`Stale`/
//! `Unverified`/`Corrupt`/`Missing`/`Denied`/`Unknown`/`Unsupported`)
//! without conversion.
//!
//! Selection among multiple artifacts of one recipe is deterministic:
//! best eligibility rank, then reproducibility quality, then lowest
//! artifact ID — never "latest wins", never storage order.

use synvelis_player::rcp::model::{AccessState, ArtifactEligibility, ReproducibilityClass};
use synvelis_project::snapshot::ContentDigest;

use crate::artifact::manifest::{
    AccessClass, ArtifactManifest, ArtifactReproducibility, CodecClass, ManifestError,
    RenderArtifactId, codec_class_consistent, ports_tile_channels, verify_manifest,
};
use crate::artifact::store::{ArtifactStore, StoreError};
use crate::recipe::{FrameInterval, OutputPortContract, RecipeKey, RecipeResolution};

/// What the caller intends to do with the artifact.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IntendedUse {
    /// Exact reuse — all six Exact conditions required.
    Exact,
    /// Preview use — relaxed codec/reproducibility conditions;
    /// non-equivalence stays explicit.
    Preview,
}

/// Trust scope the lookup runs under.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrustScope {
    /// Local store artifacts — no share provenance required.
    Local,
    /// Shared/remote path — the artifact must carry verified-share
    /// trust metadata (`ArtifactTrust.share_verified`) and a
    /// read-permitting ACL that has not expired or been revoked.
    Shared,
}

/// The lookup request — every field is caller-supplied evidence;
/// nothing is inferred from storage location or communication time.
#[derive(Clone, Debug)]
pub struct CacheLookup {
    /// Recipe key to look up in the index.
    pub recipe_key: RecipeKey,
    /// Intended use.
    pub intended_use: IntendedUse,
    /// Half-open frame span the artifact must cover.
    pub requested_span: FrameInterval,
    /// Required output port contract — every entry must be present
    /// with matching channel count and PCM format.
    pub required_ports: Vec<OutputPortContract>,
    /// Trust scope.
    pub trust_scope: TrustScope,
    /// Required renderer compatibility profile digest.
    pub compatibility_profile: ContentDigest,
}

/// Why a media-verified artifact fails the request fitness checks —
/// recorded on the candidate report for evidence, never hidden.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FitRejection {
    /// Artifact interval does not cover `requested_span`.
    InsufficientSpan,
    /// A required port is missing or its format/channels differ.
    PortContract,
    /// Codec class is `Lossy`/`Probe` — Exact-ineligible (Preview may
    /// still apply).
    LossyOrProbeCodec,
    /// Renderer compatibility profile differs.
    IncompatibleProfile,
    /// Reproducibility class is `PreviewOnly`/`Unknown` — Exact gate.
    ReproducibilityClass,
    /// Trust scope/ACL/share provenance insufficient.
    TrustScope,
    /// Access denied (revoked/expired/no-read ACL).
    AccessDenied,
    /// Current recipe resolution is not `Resolved` — currency cannot
    /// be established.
    CurrentUnresolved,
}

/// Per-candidate evaluation report — full RCP-04 vocabulary plus the
/// fitness rejection evidence.
#[derive(Clone, Debug)]
pub struct CandidateReport {
    /// Artifact identity.
    pub artifact: RenderArtifactId,
    /// RCP-04 eligibility state — the state machine result.
    pub eligibility: ArtifactEligibility,
    /// RCP-04 reproducibility class.
    pub reproducibility: ReproducibilityClass,
    /// RCP-04 access state.
    pub access: AccessState,
    /// Fitness rejection when the artifact is media-verified but does
    /// not satisfy the request.
    pub rejection: Option<FitRejection>,
}

/// The lookup outcome in the Issue #291 vocabulary — returned without
/// conversion; `Denied`/`Unknown`/`Unsupported` are first-class
/// results, never collapsed into success.
#[derive(Clone, Debug)]
pub enum LookupOutcome {
    /// A verified artifact satisfies every Exact condition.
    EligibleExact {
        /// Selected artifact (deterministic choice).
        artifact: RenderArtifactId,
    },
    /// A verified artifact satisfies the relaxed Preview conditions.
    EligiblePreview {
        /// Selected artifact (deterministic choice).
        artifact: RenderArtifactId,
    },
    /// Recipe/context/snapshot changed — revalidation needed.
    Stale,
    /// Artifact exists but verification is incomplete.
    Unverified,
    /// Verification failed (hash/manifest/decode contradiction).
    Corrupt,
    /// No artifact exists or is reachable for this request.
    Missing,
    /// Access/ACL/trust denied.
    Denied {
        /// Machine-readable reason.
        reason: String,
    },
    /// Undecidable — never treated as valid.
    Unknown {
        /// Machine-readable reason.
        reason: String,
    },
    /// Capability/format unsupported.
    Unsupported {
        /// Machine-readable reason.
        reason: String,
    },
}

/// Full lookup result — outcome plus per-candidate evidence.
#[derive(Clone, Debug)]
pub struct CacheLookupResult {
    /// The Issue-vocabulary outcome.
    pub outcome: LookupOutcome,
    /// Per-candidate reports in deterministic artifact-ID order.
    pub candidates: Vec<CandidateReport>,
}

/// Rank for deterministic selection — lower is better. Fitness-failed
/// but media-verified artifacts rank below every failure state so a
/// corrupt/stale sibling is always surfaced over "verified but
/// unfit".
fn rank(report: &CandidateReport) -> u8 {
    match report.eligibility {
        ArtifactEligibility::EligibleExact => 0,
        ArtifactEligibility::EligiblePreview => 1,
        ArtifactEligibility::ValidRecipe => 2,
        ArtifactEligibility::Unverified => 3,
        ArtifactEligibility::Stale => 4,
        ArtifactEligibility::Corrupt => 5,
        ArtifactEligibility::Unsupported => 6,
        ArtifactEligibility::VerifiedMedia => 7,
        ArtifactEligibility::Unknown => 8,
        ArtifactEligibility::Missing => 9,
    }
}

/// Reproducibility quality for tie-breaking among equally eligible
/// candidates — higher is better.
fn repro_rank(class: ReproducibilityClass) -> u8 {
    match class {
        ReproducibilityClass::BitExactCertified => 4,
        ReproducibilityClass::VerifiedWithinTolerance => 3,
        ReproducibilityClass::ReplayOfVerifiedArtifact => 2,
        ReproducibilityClass::PreviewOnly => 1,
        ReproducibilityClass::Unknown => 0,
    }
}

fn repro_of(manifest: &ArtifactManifest) -> ReproducibilityClass {
    manifest.reproducibility.to_player()
}

/// Whether the verification receipt is complete enough to claim
/// `VerifiedMedia` — every flag the publish pipeline sets must be
/// present and self-consistent with the manifest fields.
fn receipt_complete(manifest: &ArtifactManifest) -> bool {
    let r = &manifest.verification;
    let d = &r.decode;
    r.whole_digest_verified
        && r.byte_len == manifest.byte_len
        && d.frames == manifest.frame_count
        && d.sample_rate_hz == manifest.temporal.sample_rate_hz
        && d.channels == manifest.temporal.channels
        && d.port_digests_verified
        && d.codec == manifest.codec.codec
        && d.container == manifest.codec.container
}

/// Access evaluation — `Denied` on revoked, expired, no-read ACL, or
/// `TrustScope::Shared` without verified-share provenance.
fn eval_access(
    manifest: &ArtifactManifest,
    scope: TrustScope,
    now: u64,
) -> (AccessState, Option<FitRejection>) {
    let Some(trust) = &manifest.trust else {
        // Local artifact: fine under Local scope; Shared scope has no
        // verified-share provenance → denied.
        return match scope {
            TrustScope::Local => (AccessState::Granted, None),
            TrustScope::Shared => (
                AccessState::Denied {
                    reason: "no share trust metadata".to_owned(),
                },
                Some(FitRejection::TrustScope),
            ),
        };
    };
    if trust.revoked {
        return (
            AccessState::Denied {
                reason: "artifact revoked".to_owned(),
            },
            Some(FitRejection::AccessDenied),
        );
    }
    if trust.expires_at.is_some_and(|at| at <= now) {
        return (
            AccessState::Denied {
                reason: "artifact trust expired".to_owned(),
            },
            Some(FitRejection::AccessDenied),
        );
    }
    if trust.acl == AccessClass::None {
        return (
            AccessState::Denied {
                reason: "artifact ACL denies read".to_owned(),
            },
            Some(FitRejection::AccessDenied),
        );
    }
    if scope == TrustScope::Shared && !trust.share_verified {
        return (
            AccessState::Denied {
                reason: "share channel not verified".to_owned(),
            },
            Some(FitRejection::TrustScope),
        );
    }
    (AccessState::Granted, None)
}

/// Fitness evaluation for a media-verified, non-stale artifact.
/// Returns the tightest satisfied use plus rejection evidence: base
/// gates (span/ports/compat/access) apply to both uses; the Exact
/// gates (lossless codec + certified reproducibility) apply only to
/// `EligibleExact`. `Unknown` reproducibility is never valid — not
/// even for Preview.
fn eval_fitness(
    manifest: &ArtifactManifest,
    query: &CacheLookup,
    access: &AccessState,
    access_rejection: Option<FitRejection>,
) -> (Option<IntendedUse>, Option<FitRejection>) {
    let span_ok = manifest.temporal.interval.start <= query.requested_span.start
        && manifest.temporal.interval.end >= query.requested_span.end;
    if !span_ok {
        return (None, Some(FitRejection::InsufficientSpan));
    }
    for required in &query.required_ports {
        let found = manifest.ports.iter().any(|port| {
            port.port == required.port
                && port.channels == required.channels
                && port.format == required.format
        });
        if !found {
            return (None, Some(FitRejection::PortContract));
        }
    }
    if manifest.renderer.compatibility_profile != query.compatibility_profile {
        return (None, Some(FitRejection::IncompatibleProfile));
    }
    if !matches!(access, AccessState::Granted) {
        return (None, access_rejection.or(Some(FitRejection::AccessDenied)));
    }
    // `Unknown` reproducibility is never valid — not even Preview.
    if manifest.reproducibility == ArtifactReproducibility::Unknown {
        return (None, Some(FitRejection::ReproducibilityClass));
    }
    let codec_ok = manifest.codec.class == CodecClass::Lossless;
    let repro_exact = matches!(
        manifest.reproducibility,
        ArtifactReproducibility::BitExactCertified
            | ArtifactReproducibility::VerifiedWithinTolerance
            | ArtifactReproducibility::ReplayOfVerifiedArtifact
    );
    if query.intended_use == IntendedUse::Exact && codec_ok && repro_exact {
        return (Some(IntendedUse::Exact), None);
    }
    // Preview-relaxed: report which Exact gate failed as evidence.
    let exact_rejection = if !codec_ok {
        Some(FitRejection::LossyOrProbeCodec)
    } else if !repro_exact {
        Some(FitRejection::ReproducibilityClass)
    } else {
        None
    };
    (Some(IntendedUse::Preview), exact_rejection)
}

/// Evaluates one candidate artifact end-to-end. First failure wins —
/// integrity before verification before currency before fitness.
fn eval_candidate(
    store: &dyn ArtifactStore,
    id: RenderArtifactId,
    query: &CacheLookup,
    current_key: &RecipeKey,
    now: u64,
) -> Result<CandidateReport, StoreError> {
    let base = |eligibility, rejection| CandidateReport {
        artifact: id,
        eligibility,
        reproducibility: ReproducibilityClass::Unknown,
        access: AccessState::Granted,
        rejection,
    };
    let Some(bytes) = store.read_manifest(&id)? else {
        return Ok(base(ArtifactEligibility::Missing, None));
    };
    if store.is_quarantined(&id) {
        return Ok(base(ArtifactEligibility::Corrupt, None));
    }
    let manifest = match verify_manifest(&id, &bytes) {
        Ok(m) => m,
        Err(ManifestError::UnsupportedVersion { .. }) => {
            return Ok(base(ArtifactEligibility::Unsupported, None));
        }
        Err(_) => return Ok(base(ArtifactEligibility::Corrupt, None)),
    };
    // Index/manifest recipe agreement — a manifest filed under the
    // wrong recipe key is an integrity contradiction, not staleness.
    if manifest.recipe_key != query.recipe_key {
        return Ok(base(ArtifactEligibility::Corrupt, None));
    }
    // Semantic consistency — codec class must agree with codec
    // identity and ports must tile the declared channel count.
    // Publish and share registration both enforce these; a manifest
    // that reached the store by another path must not promote.
    if !codec_class_consistent(manifest.codec.class, &manifest.codec.codec)
        || !ports_tile_channels(&manifest.ports, manifest.temporal.channels)
    {
        return Ok(base(ArtifactEligibility::Corrupt, None));
    }
    // Media verification: blob presence + digest + receipt.
    let Some(blob) = store.read_blob(&manifest.content_digest)? else {
        return Ok(base(ArtifactEligibility::Missing, None));
    };
    if blob.len() as u64 != manifest.byte_len
        || ContentDigest::sha256(&blob) != manifest.content_digest
    {
        return Ok(base(ArtifactEligibility::Corrupt, None));
    }
    if !receipt_complete(&manifest) {
        return Ok(base(ArtifactEligibility::Unverified, None));
    }
    // Currency: the artifact's recipe must be the currently resolved
    // recipe — media verification alone never promotes to Exact.
    if manifest.recipe_key != *current_key {
        return Ok(base(ArtifactEligibility::Stale, None));
    }
    let (access, access_rejection) = eval_access(&manifest, query.trust_scope, now);
    let repro = repro_of(&manifest);
    let (fit, rejection) = eval_fitness(&manifest, query, &access, access_rejection);
    let eligibility = match fit {
        Some(IntendedUse::Exact) => ArtifactEligibility::EligibleExact,
        Some(IntendedUse::Preview) => ArtifactEligibility::EligiblePreview,
        None => ArtifactEligibility::VerifiedMedia,
    };
    Ok(CandidateReport {
        artifact: id,
        eligibility,
        reproducibility: repro,
        access,
        rejection,
    })
}

/// Runs the lookup: resolves index candidates for `query.recipe_key`,
/// evaluates each, selects deterministically, maps to the
/// Issue-vocabulary outcome.
///
/// `current` is the caller's fresh RCP-01 resolution — the input
/// completeness authority. A non-`Resolved` current resolution yields
/// `Unknown` with no candidates: without recipe validity nothing can
/// be Exact, and pretending otherwise would violate fail-closed.
///
/// # Errors
/// [`StoreError`] — index/read failure; lookup never fabricates a
/// result over a broken store.
pub fn lookup(
    store: &dyn ArtifactStore,
    query: &CacheLookup,
    current: &RecipeResolution,
) -> Result<CacheLookupResult, StoreError> {
    let RecipeResolution::Resolved { key, .. } = current else {
        return Ok(CacheLookupResult {
            outcome: LookupOutcome::Unknown {
                reason: "current recipe resolution is not Resolved".to_owned(),
            },
            candidates: Vec::new(),
        });
    };
    let now = store.now();
    let ids = store.index_entries(&query.recipe_key)?;
    let mut candidates = Vec::with_capacity(ids.len());
    for id in ids {
        candidates.push(eval_candidate(store, id, query, key, now)?);
    }
    // Deterministic order: eligibility rank → reproducibility → id.
    candidates.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| repro_rank(b.reproducibility).cmp(&repro_rank(a.reproducibility)))
            .then_with(|| a.artifact.cmp(&b.artifact))
    });
    let outcome = match candidates.first() {
        None => LookupOutcome::Missing,
        Some(best) => outcome_of(best),
    };
    Ok(CacheLookupResult {
        outcome,
        candidates,
    })
}

/// Maps the best candidate to the Issue-vocabulary outcome — states
/// are never converted: `VerifiedMedia`/`ValidRecipe` (verified but
/// request-unfit) collapse to `Unknown` with evidence, `Denied`
/// access surfaces as `Denied`.
fn outcome_of(best: &CandidateReport) -> LookupOutcome {
    match best.eligibility {
        ArtifactEligibility::EligibleExact => LookupOutcome::EligibleExact {
            artifact: best.artifact,
        },
        ArtifactEligibility::EligiblePreview => LookupOutcome::EligiblePreview {
            artifact: best.artifact,
        },
        ArtifactEligibility::Stale => LookupOutcome::Stale,
        ArtifactEligibility::Unverified => LookupOutcome::Unverified,
        ArtifactEligibility::Corrupt => LookupOutcome::Corrupt,
        ArtifactEligibility::Missing => LookupOutcome::Missing,
        ArtifactEligibility::Unsupported => LookupOutcome::Unsupported {
            reason: "candidate schema/format unsupported".to_owned(),
        },
        ArtifactEligibility::Unknown => LookupOutcome::Unknown {
            reason: "candidate unevaluated".to_owned(),
        },
        ArtifactEligibility::ValidRecipe | ArtifactEligibility::VerifiedMedia => {
            match best.rejection {
                Some(FitRejection::AccessDenied) => LookupOutcome::Denied {
                    reason: "artifact access denied".to_owned(),
                },
                Some(FitRejection::TrustScope | FitRejection::CurrentUnresolved) => {
                    LookupOutcome::Denied {
                        reason: "artifact trust scope insufficient".to_owned(),
                    }
                }
                _ => LookupOutcome::Unknown {
                    reason: "no candidate satisfies the requested use".to_owned(),
                },
            }
        }
    }
}
