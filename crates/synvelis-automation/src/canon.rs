//! Canonical JSON encoding and content digests.
//!
//! The canonical form is a deterministic JSON serialization: object keys
//! are sorted lexicographically at every level, no insignificant
//! whitespace is emitted, and scalars use the shortest round-trip
//! representation (`serde_json` number formatting). Two semantically
//! identical documents always produce identical bytes, which is what
//! makes [`definition_digest`] stable across runs and platforms.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::AutomationError;

/// Serializes `value` into canonical JSON bytes.
///
/// # Errors
/// [`AutomationError::Json`] on serialization failure (unreachable for
/// values produced by the YAML parser/serde decoding, kept total).
pub fn canonical_json(value: &Value) -> Result<Vec<u8>, AutomationError> {
    let mut out = Vec::new();
    write_value(value, &mut out)?;
    Ok(out)
}

fn write_value(value: &Value, out: &mut Vec<u8>) -> Result<(), AutomationError> {
    let json_err = |e: serde_json::Error| AutomationError::Json {
        reason: e.to_string(),
    };
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Value::Number(n) => out.extend_from_slice(n.to_string().as_bytes()),
        Value::String(s) => {
            // serde_json's string escaping is deterministic (control
            // characters, quote and backslash escaped; UTF-8 passthrough).
            out.extend_from_slice(serde_json::to_string(s).map_err(json_err)?.as_bytes());
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(item, out)?;
            }
            out.push(b']');
        }
        Value::Object(map) => {
            out.push(b'{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(serde_json::to_string(key).map_err(json_err)?.as_bytes());
                out.push(b':');
                write_value(&map[*key], out)?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}

/// Lowercase hex SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// The deterministic digest of a canonical-JSON-serializable value.
///
/// # Errors
/// [`AutomationError::Json`] on serialization failure.
pub fn digest_of(value: &Value) -> Result<String, AutomationError> {
    Ok(sha256_hex(&canonical_json(value)?))
}

/// Computes the [`crate::WorkflowDefinition`] digest: canonical JSON of
/// the definition → SHA-256 hex. Stable for a given definition
/// regardless of YAML source formatting or JSON key order.
///
/// # Errors
/// Returns [`AutomationError::Json`] when the definition cannot be
/// represented as a JSON value.
pub fn definition_digest(
    definition: &crate::WorkflowDefinition,
) -> Result<String, AutomationError> {
    let value = serde_json::to_value(definition).map_err(|e| AutomationError::Json {
        reason: e.to_string(),
    })?;
    digest_of(&value)
}
