//! Gateway-scoped typed identifiers and opaque references.
//!
//! Two families are defined here and deliberately kept distinct:
//!
//! - **Gateway-assigned `u128` ids** ([`OperationId`], [`AdapterId`]) —
//!   serialized as `"<kind>-<32 lowercase hex>"`, same canonical form as
//!   `synvelis-project` stable ids.
//! - **Caller/foreign-domain references** ([`RequestId`], [`TaskRef`],
//!   [`GrantRef`], [`ProposalRef`], [`ApprovalRef`], [`SnapshotRef`],
//!   [`IdempotencyKey`]) — bounded non-empty strings. Holding one of these
//!   is *not* an authorization proof; they are typed references whose
//!   evaluation belongs to GW-02+ (GTC-002).

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::outcome::GatewayError;

macro_rules! define_u128_id {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u128);

        impl $name {
            /// The serialization kind tag, e.g. `"operation"`.
            pub const KIND: &'static str = $kind;

            /// Wraps a raw `u128` value.
            #[must_use]
            pub const fn new(raw: u128) -> Self {
                Self(raw)
            }

            /// The raw `u128` value.
            #[must_use]
            pub const fn raw(self) -> u128 {
                self.0
            }

            /// The canonical `"<kind>-<32hex>"` form.
            #[must_use]
            pub fn to_tagged(self) -> String {
                format!("{}-{:032x}", Self::KIND, self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_tagged())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.to_tagged())
            }
        }
    };
}

macro_rules! define_string_ref {
    ($(#[$meta:meta])* $name:ident, $max:literal, $doc:literal) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        pub struct $name(String);

        impl $name {
            /// Wraps a raw string, rejecting empty or over-long values.
            ///
            /// # Errors
            /// [`GatewayError::MalformedRequest`] when `text` is empty or
            /// exceeds the type's length bound.
            pub fn new(text: impl Into<String>) -> Result<Self, GatewayError> {
                let text = text.into();
                if text.is_empty() || text.chars().count() > $max {
                    return Err(GatewayError::MalformedRequest {
                        reason: concat!($doc, " must be 1..=", $max, " chars").to_owned(),
                    });
                }
                Ok(Self(text))
            }

            /// The raw string.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = String::deserialize(d)?;
                Self::new(text).map_err(D::Error::custom)
            }
        }
    };
}

define_u128_id!(
    /// Identifies one accepted operation instance inside the gateway.
    ///
    /// Assigned by [`crate::Gateway::bind`]; never carried on the wire —
    /// the wire only sees the caller-assigned [`RequestId`].
    OperationId, "operation"
);

define_u128_id!(
    /// Identifies a trusted adapter endpoint that attested a
    /// [`crate::CallerIdentity`]. Minted by the crate's adapter
    /// constructions; not a wire value.
    AdapterId, "adapter"
);

define_string_ref!(
    /// Caller-assigned request correlation id.
    ///
    /// Correlates a wire request to its response. It is *not* the identity
    /// of the accepted operation — that is [`OperationId`].
    RequestId, 128, "request id"
);

define_string_ref!(
    /// Opaque reference to an authorized task (GW-02 domain).
    ///
    /// Presence in a request grants nothing; evaluation is the policy
    /// evaluator's concern.
    TaskRef, 256, "task ref"
);

define_string_ref!(
    /// Opaque reference to an execution grant (GW-02 domain).
    ///
    /// Presence in a request grants nothing; evaluation is the policy
    /// evaluator's concern.
    GrantRef, 256, "grant ref"
);

define_string_ref!(
    /// Opaque reference to a proactive-activation permit (GW-02 domain).
    ///
    /// Identifies the explicit activation authority under which a task
    /// was proactively started — separate from any execution grant
    /// (AHG-007: delegated apply is not proactive-activation authority).
    ProactivePermitRef, 256, "proactive permit ref"
);

define_string_ref!(
    /// Opaque reference to a prepared proposal (GW-05 domain).
    ProposalRef, 256, "proposal ref"
);

define_string_ref!(
    /// Opaque reference to an approval request or receipt (GW-05 domain).
    ApprovalRef, 256, "approval ref"
);

define_string_ref!(
    /// Opaque reference to a fixed immutable snapshot (GW-03 domain).
    SnapshotRef, 256, "snapshot ref"
);

define_string_ref!(
    /// Client-supplied idempotency key.
    ///
    /// Scoped to (principal, project): the same key with the same payload
    /// fingerprint is a duplicate; with a different fingerprint it is a
    /// conflict. Durable record/replay semantics belong to GW-06.
    IdempotencyKey, 256, "idempotency key"
);
