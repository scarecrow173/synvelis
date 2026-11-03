//! GW-07 generation provenance — bounded, evidence-bearing records
//! binding a job to its inputs and outputs (GIA-003).
//!
//! Every field carries an [`EvidenceLevel`]:
//!
//! - `Confirmed` — the adapter independently verified the fact (a
//!   provider-attested field, or an adapter-observed transformation).
//! - `Declared` — the provider stated it; not independently verified.
//! - `Unknown` — the provider said nothing. Unknown model/rights are
//!   represented as `Unknown` — never guessed, never left blank to
//!   look authoritative.
//!
//! The record is sanitized *by construction*: no prompt text, no
//! tokens, no grant credentials can ever reach it — the type has no
//! such fields. `disclosed` is the only serialization the job-result
//! boundary emits; it is the form that may later be embedded in
//! project Asset/Source metadata without leaking secrets.

use serde_json::{Value, json};
use synvelis_project::ProjectRevision;

use crate::id::{JobRef, MediaRef, OperationId};
use crate::request::ResourceRef;

/// How strongly a provenance fact is evidenced (GIA-003).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EvidenceLevel {
    /// Independently verified by the adapter (attested provider claim
    /// or adapter-observed fact).
    Confirmed,
    /// Stated by the provider, unverified.
    Declared,
    /// No claim exists.
    Unknown,
}

impl EvidenceLevel {
    /// The wire name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Declared => "declared",
            Self::Unknown => "unknown",
        }
    }
}

/// One bounded provenance field — `value` is `None` exactly when the
/// evidence is `Unknown`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvenanceField {
    /// The bounded claim value (field names and lengths are fixed by
    /// the mapping — never free-form provider text).
    pub value: Option<String>,
    /// The evidence level.
    pub evidence: EvidenceLevel,
}

impl ProvenanceField {
    fn unknown() -> Self {
        Self {
            value: None,
            evidence: EvidenceLevel::Unknown,
        }
    }

    fn map(value: Option<&str>, attested: bool) -> Self {
        match value {
            None => Self::unknown(),
            Some(v) => Self {
                value: Some(v.chars().take(256).collect()),
                evidence: if attested {
                    EvidenceLevel::Confirmed
                } else {
                    EvidenceLevel::Declared
                },
            },
        }
    }

    fn disclosed(&self) -> Value {
        json!({
            "value": self.value,
            "evidence": self.evidence.name(),
        })
    }
}

/// The provider's raw provenance claim — what [`crate::job::JobPort::claim`]
/// returns. `attested` names the fields the provider signed; the
/// adapter maps attested → `Confirmed`, present → `Declared`,
/// absent → `Unknown`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderClaim {
    /// Claimed producer/provider identity.
    pub producer: Option<String>,
    /// Claimed model identity.
    pub model: Option<String>,
    /// Claimed model version.
    pub model_version: Option<String>,
    /// Claimed rights/license tag.
    pub rights: Option<String>,
    /// Field names the provider attests (`"producer"`, `"model"`,
    /// `"rights"`).
    pub attested: std::collections::BTreeSet<String>,
}

/// The bounded generation-provenance record — one per materialized
/// job result (GIA-003 Producer/Model/Rights/Transformation).
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationProvenance {
    /// The job the record binds.
    pub job: JobRef,
    /// The gateway operation the job was admitted under.
    pub operation: OperationId,
    /// Who produced the output.
    pub producer: ProvenanceField,
    /// The generating model (`"name@version"` when both are claimed).
    pub model: ProvenanceField,
    /// The declared rights/license tag.
    pub rights: ProvenanceField,
    /// The transformation applied — adapter-observed, always
    /// `Confirmed` for a recorded result.
    pub transformation: ProvenanceField,
    /// The bound input resources (the input snapshot).
    pub inputs: Vec<ResourceRef>,
    /// The committed revision the inputs pinned.
    pub input_revision: Option<ProjectRevision>,
    /// The staged media outputs the record covers.
    pub media: Vec<MediaRef>,
}

impl GenerationProvenance {
    /// Maps a [`ProviderClaim`] into the bounded record — no prompt
    /// text, tokens, or grant credentials can ever enter through this
    /// path; they are not representable.
    #[must_use]
    pub fn build(
        job: &JobRef,
        operation: OperationId,
        claim: &ProviderClaim,
        inputs: Vec<ResourceRef>,
        input_revision: Option<ProjectRevision>,
        media: Vec<MediaRef>,
        kind: crate::job::JobKind,
    ) -> Self {
        let model_value = match (&claim.model, &claim.model_version) {
            (Some(name), Some(version)) => Some(format!("{name}@{version}")),
            (Some(name), None) => Some(name.clone()),
            (None, _) => None,
        };
        Self {
            job: job.clone(),
            operation,
            producer: ProvenanceField::map(
                claim.producer.as_deref(),
                claim.attested.contains("producer"),
            ),
            model: ProvenanceField::map(model_value.as_deref(), claim.attested.contains("model")),
            rights: ProvenanceField::map(
                claim.rights.as_deref(),
                claim.attested.contains("rights"),
            ),
            // The adapter observed the pipeline — the job's own kind
            // names the transformation, never provider-claimed.
            transformation: ProvenanceField {
                value: Some(kind.name().to_owned()),
                evidence: EvidenceLevel::Confirmed,
            },
            inputs,
            input_revision,
            media,
        }
    }

    /// The project-embeddable disclosure — the only JSON form the
    /// result boundary emits. Bounded: refs, digests, evidence levels,
    /// no secrets (there are none to emit).
    #[must_use]
    pub fn disclosed(&self) -> Value {
        json!({
            "job": self.job.as_str(),
            "operation": self.operation.to_tagged(),
            "producer": self.producer.disclosed(),
            "model": self.model.disclosed(),
            "rights": self.rights.disclosed(),
            "transformation": self.transformation.disclosed(),
            "inputs": self.inputs.iter().map(crate::job::tagged).collect::<Vec<_>>(),
            "input_revision": self.input_revision.map(|r| r.0),
            "media": self.media.iter().map(MediaRef::as_str).collect::<Vec<_>>(),
        })
    }
}
