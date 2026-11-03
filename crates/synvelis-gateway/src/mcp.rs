//! GW-08 MCP server adapter — protocol surface types (GIA-004).
//!
//! The MCP edge is a *protocol boundary*, not a source of truth: this
//! module owns the JSON-RPC / MCP method surface, the bounded-transfer
//! limits, and the result/error vocabulary the adapter speaks.
//! Translation to the internal typed contract lives in
//! [`crate::mcp_server`] and [`crate::mcp_tools`]; connection permits
//! live in [`crate::mcp_permit`].
//!
//! Transport (stdio framing, sockets, HTTP) is deliberately **not**
//! here — the crate boundary doc lists MCP transport as outside the
//! gateway's ownership. The adapter core is transport-agnostic: a
//! transport hands one JSON value in and carries zero-or-one JSON
//! value out.
//!
//! Implemented protocol revision: MCP `2025-06-18` (initialize,
//! ping, tools/list, tools/call, resources/list,
//! resources/templates/list, resources/read, prompts/list,
//! notifications/*). Batches and the Tasks extension are not
//! advertised — async completion is reported through the
//! `operation.get` status tool instead of a second job authority.

use serde::Serialize;
use serde_json::{Value, json};

use crate::outcome::ErrorEnvelope;

/// The MCP protocol revision this adapter negotiates.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// `serverInfo.name` reported at initialize.
pub const SERVER_NAME: &str = "synvelis-mcp";
/// `serverInfo.version` reported at initialize.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The adapter-side tool contract revision, reported under
/// `_synvelis.toolContract` at initialize — the tool names and
/// argument shapes are the adapter's own contract, not a 1:1 mirror
/// of internal capability ids (GIA-004).
pub const TOOL_CONTRACT: &str = "1.0.0";

/// The adapter endpoint identity every MCP-bound caller attests
/// through (both channels share it — the *channel* separates trust,
/// not the adapter id).
pub const MCP_ADAPTER_RAW: u128 = 0x4D43_5008;

/// `synvelis.connect` — the first-request handshake that presents a
/// connection permit and binds the session's trusted identity.
pub const CONNECT_METHOD: &str = "synvelis.connect";

// ── JSON-RPC error codes ─────────────────────────────────────
/// Parse error (invalid JSON / not an object).
pub const PARSE_ERROR: i64 = -32700;
/// Invalid request (missing jsonrpc/method, batch).
pub const INVALID_REQUEST: i64 = -32600;
/// Method not found.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// Invalid params.
pub const INVALID_PARAMS: i64 = -32602;
/// Internal error.
pub const INTERNAL_ERROR: i64 = -32603;
/// Server-defined: a connection permit is required first.
pub const PERMIT_REQUIRED: i64 = -32002;
/// Server-defined: the presented permit is invalid (unknown,
/// expired, revoked, or bound to another channel — one blind
/// verdict, nothing about existence disclosed).
pub const PERMIT_INVALID: i64 = -32001;
/// Server-defined: the bound session may not use this method.
pub const FORBIDDEN: i64 = -32003;
/// Server-defined: the resource URI is unknown to this session
/// (existence-blind).
pub const UNKNOWN_RESOURCE: i64 = -32004;
/// Server-defined: the adapter is administratively disabled
/// (`AI/MCP OFF` — the DAW must not depend on it).
pub const ADAPTER_DISABLED: i64 = -32005;

/// Which transport channel a session arrived on.
///
/// The channel is part of the trust decision: a `Client` permit can
/// never bind on `Control`, and vice versa — the MCP client channel
/// carries agent principals only, while the control channel is the
/// `PoC` stand-in for the Synvelis-side trusted surface (GW-09's UI
/// replaces it). A permit is issued *for* a channel.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpChannel {
    /// The external MCP client channel (stdio helper).
    Client,
    /// The authenticated local control channel — operator/host acts
    /// (approval issuance, policy admin) enter here, never through
    /// the client channel. It is the `PoC` stand-in for the
    /// Synvelis-side trusted surface.
    Control,
}

/// Bounded-transfer limits enforced at the protocol edge (DATA-08,
/// GIA-004 "Bounded Range/Chunk").
///
/// These bounds are the adapter's own — they exist so an external
/// request can never demand an unbounded transfer *before* the typed
/// contract is even consulted. The engine-side [`crate::ContextBudget`]
/// still applies underneath.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpBounds {
    /// Maximum inbound message bytes a transport may present.
    /// The stdio transport enforces this per line; the session
    /// enforces it again on the serialized params.
    pub max_message_bytes: usize,
    /// Maximum serialized `arguments` size for one `tools/call`.
    pub max_tool_args_bytes: usize,
    /// Maximum page `limit` accepted on a context query tool.
    pub max_page_limit: u32,
    /// Maximum `range.len` (domain-native units) accepted on a
    /// context query tool — a note query over an effectively
    /// unbounded window is rejected at the edge.
    pub max_range_len: i64,
    /// Maximum retained operation records per host.
    pub max_ops: usize,
    /// Maximum selector id list accepted on a query tool.
    pub max_selector_ids: usize,
}

impl Default for McpBounds {
    fn default() -> Self {
        Self {
            max_message_bytes: 262_144,
            max_tool_args_bytes: 65_536,
            max_page_limit: 256,
            max_range_len: 1_048_576,
            max_ops: 1024,
            max_selector_ids: 64,
        }
    }
}

/// Builds a JSON-RPC success response.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // response construction consumes the value into the envelope
pub fn rpc_result(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id.clone(), "result": result})
}

/// Builds a JSON-RPC error response (`id` is `null` on parse-level
/// failures, per the spec).
#[must_use]
pub fn rpc_error(id: &Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({"code": code, "message": message});
    if let Some(d) = data {
        error["data"] = d;
    }
    json!({"jsonrpc": "2.0", "id": id.clone(), "error": error})
}

/// MCP `CallToolResult` — the tool-level success/error contract.
///
/// Per the MCP shape every result carries a text content block plus
/// `structuredContent`; semantic failures (the typed gateway error
/// taxonomy) are `isError: true` results — JSON-RPC errors are
/// reserved for protocol violations.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// The mandatory content blocks.
    pub content: Vec<Value>,
    /// The structured payload (tool output or the error envelope).
    pub structured_content: Value,
    /// Whether the tool call is a semantic failure.
    pub is_error: bool,
}

impl CallToolResult {
    /// A successful tool call carrying `structured` as both the
    /// machine-readable payload and the text block.
    #[must_use]
    pub fn ok(structured: Value) -> Self {
        Self {
            content: vec![json!({
                "type": "text",
                "text": structured.to_string(),
            })],
            structured_content: structured,
            is_error: false,
        }
    }

    /// A failed tool call — `error` is the structured payload
    /// (a [`crate::ExternalError`] projection or an adapter-local
    /// typed fault).
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // the error becomes the owned payload
    pub fn err(error: Value) -> Self {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("tool call failed")
            .to_owned();
        Self {
            content: vec![json!({
                "type": "text",
                "text": message,
            })],
            structured_content: json!({"error": error}),
            is_error: true,
        }
    }

    /// Serializes to the wire result shape.
    ///
    /// # Panics
    /// Impossible in practice — the result shape is plain JSON.
    #[must_use]
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("tool result serializes")
    }

    /// The safe-to-disclose message this result carries.
    #[must_use]
    pub fn message(&self) -> String {
        self.structured_content
            .pointer("/error/message")
            .and_then(Value::as_str)
            .map_or_else(|| "tool call failed".to_owned(), str::to_owned)
    }

    /// The structured error payload, when this is an error result.
    #[must_use]
    pub fn error_value(&self) -> Option<Value> {
        self.structured_content.get("error").cloned()
    }
}

/// An adapter-local fault — protocol-edge failures that are *not*
/// gateway errors: tool-argument validation, bound violations, and
/// unavailable adapter functions. Serializes in the same flat
/// `code`/`message` shape as [`crate::ExternalError`] so the client
/// sees one error vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum AdapterFault {
    /// Tool arguments failed adapter-side validation.
    InvalidArguments {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// A bounded-transfer limit was exceeded at the edge.
    LimitExceeded {
        /// Which bound was exceeded.
        limit: String,
        /// The configured maximum.
        max: u64,
    },
    /// The requested function is not available on this session
    /// (wrong channel, disabled adapter, or no commit boundary wired).
    Unavailable {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// The named operation is unknown to this session — unknown and
    /// foreign operations share one blind verdict.
    UnknownOperation,
    /// The operation exists but cannot be cancelled (already settled).
    NotCancellable {
        /// The settled status name.
        status: String,
    },
}

impl AdapterFault {
    /// The safe-to-disclose message.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::InvalidArguments { reason } => format!("invalid tool arguments: {reason}"),
            Self::LimitExceeded { limit, max } => {
                format!("request exceeds {limit} (max {max})")
            }
            Self::Unavailable { reason } => format!("function unavailable: {reason}"),
            Self::UnknownOperation => "unknown operation".to_owned(),
            Self::NotCancellable { status } => {
                format!("operation already settled ({status})")
            }
        }
    }

    /// The wire projection `{code, message}` — same vocabulary the
    /// gateway's `ExternalError` uses (`code` comes from the serde
    /// tag).
    ///
    /// # Panics
    /// Impossible in practice — the fault shape is plain JSON.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut v = serde_json::to_value(self).expect("adapter fault serializes");
        v["message"] = json!(self.message());
        v
    }
}

impl From<AdapterFault> for CallToolResult {
    fn from(f: AdapterFault) -> Self {
        Self::err(f.to_value())
    }
}

impl From<&ErrorEnvelope> for CallToolResult {
    /// The gateway's typed envelope is projected through
    /// [`ErrorEnvelope::to_external`] — internal diagnostics never
    /// cross the MCP boundary (DATA disclosure hygiene).
    fn from(env: &ErrorEnvelope) -> Self {
        let external = env.to_external();
        Self::err(serde_json::to_value(external).expect("external error serializes"))
    }
}
