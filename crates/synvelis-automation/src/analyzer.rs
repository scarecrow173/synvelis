//! Script analyzer registry and the Luau port-level adapter
//! (synvelis#274 / SV-SCRIPT-04).
//!
//! [`AnalyzerRegistry`] is the single resolution authority for
//! [`synvelis_script::ScriptAnalyzer`] implementations: analyzers are
//! registered once and resolved by `ScriptLanguageId` +
//! [`WantedCapabilities`]. Resolution is fail-closed — an unregistered
//! language is [`AnalyzerError::Unavailable`] (F-18), an unadvertised
//! capability request is [`AnalyzerError::Unsupported`] (F-13), and a
//! contract-version mismatch is [`AnalyzerError::VersionMismatch`].
//!
//! [`LuauPortAnalyzer`] is the deterministic port-level adapter this
//! slice ships: it performs a small, honest subset of Luau static
//! analysis (unterminated string/comment syntax diagnostics,
//! unknown-API and arity type diagnostics against the injected
//! `declare function` signatures, and prefix completion) entirely
//! in-process — no external `luau-lsp`, no FFI, no VM. Its public
//! contract is exactly the [`ScriptAnalyzer`] port so the Ruviform
//! Luau.Analysis native bridge (ruviform#560) can replace it without
//! consumer changes; real bridge integration is a separate lane gate.
//!
//! Analyzer success is a static-check signal only — it never mints
//! execution permission, grants, or project truth.

use std::collections::BTreeMap;
use std::fmt;

use synvelis_script::{
    AnalyzeRequest, AnalyzeResult, AnalyzerCapabilities, AnalyzerDescriptor, AnalyzerError,
    Completion, CompletionKind, Cursor, Diagnostic, DiagnosticKind, DiagnosticLevel,
    DiagnosticSeverity, ScriptAnalyzer, ScriptBackendVersion, ScriptLanguageId, ScriptSource,
    SourcePosition, SourceRange, SourceRevision, TypeDefinition, WantedCapabilities,
    verify_result_revision,
};

/// The analyzer-port contract version this build resolves against.
/// An analyzer advertising any other `contract_version` fails closed
/// with [`AnalyzerError::VersionMismatch`] at resolve time.
pub const ANALYZER_CONTRACT_VERSION: u32 = 1;

/// Errors produced while registering an analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnalyzerRegistryError {
    /// Two analyzers claim the same supported language.
    DuplicateLanguage {
        /// The contested language id.
        language: String,
    },
    /// The analyzer advertises no supported languages — it can never
    /// be resolved, so registration is rejected rather than silently
    /// dead.
    EmptySupportedLanguages {
        /// The analyzer id.
        analyzer_id: String,
    },
}

impl fmt::Display for AnalyzerRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateLanguage { language } => {
                write!(f, "duplicate analyzer for language {language:?}")
            }
            Self::EmptySupportedLanguages { analyzer_id } => {
                write!(
                    f,
                    "analyzer {analyzer_id:?} advertises no supported languages"
                )
            }
        }
    }
}

impl std::error::Error for AnalyzerRegistryError {}

/// Registry of [`ScriptAnalyzer`] implementations indexed by every
/// language in `capabilities.supported_languages`.
///
/// Resolution (`resolve`) is the only capability/availability check —
/// callers must not re-implement support tests elsewhere.
#[derive(Default)]
pub struct AnalyzerRegistry {
    analyzers: Vec<Box<dyn ScriptAnalyzer>>,
    /// language id → index into `analyzers`.
    by_language: BTreeMap<String, usize>,
}

impl AnalyzerRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `analyzer` under each language it advertises.
    ///
    /// # Errors
    /// [`AnalyzerRegistryError::DuplicateLanguage`] — another analyzer
    /// already claims the language;
    /// [`AnalyzerRegistryError::EmptySupportedLanguages`] — the
    /// descriptor advertises no languages.
    pub fn register(
        &mut self,
        analyzer: Box<dyn ScriptAnalyzer>,
    ) -> Result<(), AnalyzerRegistryError> {
        let descriptor = analyzer.descriptor();
        if descriptor.capabilities.supported_languages.is_empty() {
            return Err(AnalyzerRegistryError::EmptySupportedLanguages {
                analyzer_id: descriptor.analyzer_id.clone(),
            });
        }
        for language in &descriptor.capabilities.supported_languages {
            if self.by_language.contains_key(&language.0) {
                return Err(AnalyzerRegistryError::DuplicateLanguage {
                    language: language.0.clone(),
                });
            }
        }
        let index = self.analyzers.len();
        for language in &descriptor.capabilities.supported_languages {
            self.by_language.insert(language.0.clone(), index);
        }
        self.analyzers.push(analyzer);
        Ok(())
    }

    /// Resolves an analyzer for `language` that satisfies `wanted`.
    ///
    /// Fail-closed order: availability → contract version →
    /// capabilities.
    ///
    /// # Errors
    /// [`AnalyzerError::Unavailable`] — no analyzer for the language;
    /// [`AnalyzerError::VersionMismatch`] — contract version differs
    /// from [`ANALYZER_CONTRACT_VERSION`];
    /// [`AnalyzerError::Unsupported`] — a wanted capability exceeds
    /// the advertised set.
    pub fn resolve(
        &self,
        language: &ScriptLanguageId,
        wanted: &WantedCapabilities,
    ) -> Result<&dyn ScriptAnalyzer, AnalyzerError> {
        let analyzer = self
            .by_language
            .get(&language.0)
            .and_then(|i| self.analyzers.get(*i))
            .map(Box::as_ref)
            .ok_or(AnalyzerError::Unavailable)?;
        let descriptor = analyzer.descriptor();
        let actual = descriptor.version.contract_version.to_string();
        if descriptor.version.contract_version != ANALYZER_CONTRACT_VERSION {
            return Err(AnalyzerError::VersionMismatch {
                field: "contract_version".to_owned(),
                expected: ANALYZER_CONTRACT_VERSION.to_string(),
                actual,
            });
        }
        if let Some(level) = wanted.diagnostics
            && diagnostic_level_rank(level)
                > diagnostic_level_rank(descriptor.capabilities.diagnostics)
        {
            return Err(AnalyzerError::Unsupported {
                capability: format!("diagnostics:{}", diagnostic_level_label(level)),
            });
        }
        if wanted.completion && !descriptor.capabilities.completion {
            return Err(AnalyzerError::Unsupported {
                capability: "completion".to_owned(),
            });
        }
        Ok(analyzer)
    }

    /// The descriptor of the analyzer registered for `language`,
    /// regardless of wanted capabilities — for status display.
    #[must_use]
    pub fn descriptor_for(&self, language: &ScriptLanguageId) -> Option<&AnalyzerDescriptor> {
        self.by_language
            .get(&language.0)
            .and_then(|i| self.analyzers.get(*i))
            .map(|a| a.descriptor())
    }

    /// All registered descriptors (sorted by analyzer id then
    /// language), for status surfaces.
    #[must_use]
    pub fn descriptors(&self) -> Vec<AnalyzerDescriptor> {
        let mut out: Vec<AnalyzerDescriptor> = self
            .analyzers
            .iter()
            .map(|a| a.descriptor().clone())
            .collect();
        out.sort_by(|a, b| (&a.analyzer_id, &a.language.0).cmp(&(&b.analyzer_id, &b.language.0)));
        out
    }

    /// Whether no analyzer is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.analyzers.is_empty()
    }

    /// Number of registered analyzers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.analyzers.len()
    }
}

impl fmt::Debug for AnalyzerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnalyzerRegistry")
            .field("languages", &self.by_language.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

/// Diagnostics depth ordering for capability checks.
fn diagnostic_level_rank(level: DiagnosticLevel) -> u8 {
    match level {
        DiagnosticLevel::Syntax => 1,
        DiagnosticLevel::SyntaxAndType => 2,
        _ => 0,
    }
}

fn diagnostic_level_label(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Syntax => "syntax",
        DiagnosticLevel::SyntaxAndType => "syntax-and-type",
        _ => "unsupported",
    }
}

/// Headless analysis entry: resolve `request.language`/`wanted`
/// through the registry, then run the analyzer. GUI-free — this is
/// the public API a headless CLI/script caller uses.
///
/// # Errors
/// Whatever [`AnalyzerRegistry::resolve`] or
/// [`ScriptAnalyzer::analyze`] returns.
pub fn analyze_source(
    registry: &AnalyzerRegistry,
    request: &AnalyzeRequest,
) -> Result<AnalyzeResult, AnalyzerError> {
    registry
        .resolve(&request.language, &request.wanted)?
        .analyze(request)
}

/// [`analyze_source`] with type definitions supplied as a
/// [`synvelis_script::DefinitionInjection`] (the `ScriptApiModel` →
/// `LuauStub` projection). The injection's digests are *not* part of
/// `AnalyzeResult`; consumers pin them separately so digest drift can
/// be detected as stale.
///
/// # Errors
/// [`AnalyzerError::InvalidRequest`] — `source.language` is empty;
/// otherwise as [`analyze_source`].
pub fn analyze_with_injection(
    registry: &AnalyzerRegistry,
    source: ScriptSource,
    injection: &synvelis_script::DefinitionInjection,
    wanted: WantedCapabilities,
    cursor: Option<Cursor>,
) -> Result<AnalyzeResult, AnalyzerError> {
    if source.language.0.is_empty() {
        return Err(AnalyzerError::InvalidRequest {
            reason: "source language is empty".to_owned(),
        });
    }
    let language = source.language.clone();
    let request = AnalyzeRequest {
        source,
        language,
        type_definitions: vec![injection.type_definition()],
        wanted,
        cursor,
    };
    analyze_source(registry, &request)
}

/// Deterministic port-level Luau analyzer (see module docs).
pub struct LuauPortAnalyzer {
    descriptor: AnalyzerDescriptor,
}

impl LuauPortAnalyzer {
    /// An analyzer advertising `SyntaxAndType` diagnostics and
    /// completion for `luau` at [`ANALYZER_CONTRACT_VERSION`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: AnalyzerDescriptor {
                analyzer_id: "synvelis-luau-port".to_owned(),
                language: ScriptLanguageId::luau(),
                version: ScriptBackendVersion {
                    impl_version: env!("CARGO_PKG_VERSION").to_owned(),
                    contract_version: ANALYZER_CONTRACT_VERSION,
                },
                capabilities: AnalyzerCapabilities {
                    diagnostics: DiagnosticLevel::SyntaxAndType,
                    completion: true,
                    max_source_bytes: 0,
                    supported_languages: vec![ScriptLanguageId::luau()],
                },
            },
        }
    }

    /// Extracts `(name, param_count, signature)` from
    /// `declare function <name>(<params>): <ret>` lines in the
    /// injected definition sources — the only API knowledge this
    /// adapter trusts.
    fn declared_functions(defs: &[TypeDefinition]) -> Vec<DeclaredFunction> {
        let mut out = Vec::new();
        for def in defs {
            for line in def.source.lines() {
                let line = line.trim();
                let Some(rest) = line.strip_prefix("declare function ") else {
                    continue;
                };
                let Some(open) = rest.find('(') else {
                    continue;
                };
                let name = rest[..open].trim().to_owned();
                if name.is_empty() {
                    continue;
                }
                let close = rest.rfind(')').unwrap_or(rest.len());
                let params = &rest[open + 1..close.min(rest.len())];
                let count = if params.trim().is_empty() {
                    0
                } else {
                    params.split(',').count()
                };
                out.push(DeclaredFunction {
                    name,
                    arity: count,
                    signature: rest.to_owned(),
                });
            }
        }
        out
    }

    /// Byte offset → 0-based (line, byte-column) position.
    fn position_at(text: &str, byte_offset: usize) -> SourcePosition {
        let offset = byte_offset.min(text.len());
        let before = &text[..offset];
        let line =
            u32::try_from(before.bytes().filter(|b| *b == b'\n').count()).unwrap_or(u32::MAX);
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        SourcePosition {
            line,
            column: u32::try_from(offset - line_start).unwrap_or(u32::MAX),
        }
    }

    /// Byte span → [`SourceRange`] positions.
    fn range_at(text: &str, start: usize, end: usize) -> SourceRange {
        SourceRange {
            start: Self::position_at(text, start),
            end: Self::position_at(text, end),
        }
    }

    fn diagnostic(
        severity: DiagnosticSeverity,
        kind: DiagnosticKind,
        code: &str,
        message: String,
        range: SourceRange,
        revision: SourceRevision,
    ) -> Diagnostic {
        Diagnostic {
            severity,
            kind,
            code: code.to_owned(),
            message,
            range,
            source_revision: revision,
        }
    }

    /// Deterministic syntax scan: unterminated `'…'`/`"…"` strings,
    /// `[[…]]`/`[=*[ … ]=*]` long strings and `--[[…]]` long comments.
    /// Reports the opener position; `--` line comments always
    /// terminate at EOL and are never diagnosed.
    fn syntax_diagnostics(text: &str, revision: SourceRevision) -> Vec<Diagnostic> {
        let bytes = text.as_bytes();
        let mut state = LexState::Code;
        let mut i = 0usize;
        let mut diagnostics = Vec::new();
        while i < bytes.len() {
            let b = bytes[i];
            match state {
                LexState::Code => {
                    if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
                        if let Some((level, _)) = long_bracket_level(bytes, i + 2) {
                            state = LexState::Long {
                                level,
                                opener: i,
                                comment: true,
                            };
                            i += 2;
                            continue;
                        }
                        state = LexState::LineComment;
                    } else if b == b'\'' || b == b'"' {
                        state = LexState::ShortString {
                            quote: b,
                            opener: i,
                        };
                    } else if b == b'['
                        && let Some((level, _)) = long_bracket_level(bytes, i)
                    {
                        state = LexState::Long {
                            level,
                            opener: i,
                            comment: false,
                        };
                    }
                }
                LexState::LineComment => {
                    if b == b'\n' {
                        state = LexState::Code;
                    }
                }
                LexState::Long { level, .. } => {
                    if b == b']' && long_bracket_close(bytes, i, level) {
                        i += 1 + level + 1;
                        state = LexState::Code;
                        continue;
                    }
                }
                LexState::ShortString { quote, opener } => {
                    if b == b'\\' {
                        i += 1; // skip the escaped byte
                    } else if b == quote {
                        state = LexState::Code;
                    } else if b == b'\n' {
                        // Short strings cannot span lines — report and
                        // resume scanning on the next line.
                        diagnostics.push(Self::diagnostic(
                            DiagnosticSeverity::Error,
                            DiagnosticKind::Syntax,
                            "syntax-unterminated-string",
                            format!("unterminated string literal opened at byte {opener}"),
                            Self::range_at(text, opener, i),
                            revision,
                        ));
                        state = LexState::Code;
                    }
                }
            }
            i += 1;
        }
        if let Some(d) = Self::unterminated_diagnostic(state, text, revision) {
            diagnostics.push(d);
        }
        diagnostics
    }

    /// Diagnostic for a scan that ended inside an open construct.
    fn unterminated_diagnostic(
        state: LexState,
        text: &str,
        revision: SourceRevision,
    ) -> Option<Diagnostic> {
        match state {
            LexState::ShortString { opener, .. } => Some(Self::diagnostic(
                DiagnosticSeverity::Error,
                DiagnosticKind::Syntax,
                "syntax-unterminated-string",
                format!("unterminated string literal opened at byte {opener}"),
                Self::range_at(text, opener, text.len()),
                revision,
            )),
            LexState::Long {
                opener, comment, ..
            } => {
                let (code, what) = if comment {
                    ("syntax-unterminated-comment", "long comment")
                } else {
                    ("syntax-unterminated-long-string", "long string")
                };
                Some(Self::diagnostic(
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Syntax,
                    code,
                    format!("unterminated {what} opened at byte {opener}"),
                    Self::range_at(text, opener, text.len()),
                    revision,
                ))
            }
            LexState::Code | LexState::LineComment => None,
        }
    }

    /// `synvelis<Name>(args)` call sites vs injected signatures —
    /// unknown-api / wrong-arity type diagnostics.
    fn type_diagnostics(
        text: &str,
        functions: &[DeclaredFunction],
        revision: SourceRevision,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut search_from = 0usize;
        while let Some(idx) = text[search_from..].find("synvelis") {
            let abs = search_from + idx;
            let rest = &text[abs..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let called = &rest[..end];
            search_from = abs + end.max(1);
            if called.len() <= "synvelis".len() {
                continue;
            }
            let Some(decl) = functions.iter().find(|f| f.name == called) else {
                diagnostics.push(Self::diagnostic(
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Type,
                    "unknown-api",
                    format!("unknown script api {called:?}"),
                    Self::range_at(text, abs, abs + end),
                    revision,
                ));
                continue;
            };
            let after_name = &rest[end..];
            if let Some(arg_text) = call_args(after_name) {
                let actual = if arg_text.trim().is_empty() {
                    0
                } else {
                    arg_text.split(',').count()
                };
                if actual != decl.arity {
                    diagnostics.push(Self::diagnostic(
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Type,
                        "wrong-arity",
                        format!("{called} expects {} arguments, got {actual}", decl.arity),
                        Self::range_at(text, abs, abs + end),
                        revision,
                    ));
                }
            }
        }
        diagnostics
    }
}

impl Default for LuauPortAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

struct DeclaredFunction {
    name: String,
    arity: usize,
    signature: String,
}

/// Lexical scan state for the deterministic syntax pass.
#[derive(Clone, Copy)]
enum LexState {
    Code,
    LineComment,
    Long {
        level: usize,
        opener: usize,
        comment: bool,
    },
    ShortString {
        quote: u8,
        opener: usize,
    },
}

/// Long-bracket opener at `bytes[i] == '['`: `[` followed by `level`
/// `=` bytes then `[`. Returns `(level, index just past the opener)`.
fn long_bracket_level(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    if bytes.get(i) != Some(&b'[') {
        return None;
    }
    let mut j = i + 1;
    let mut level = 0usize;
    while bytes.get(j) == Some(&b'=') {
        level += 1;
        j += 1;
    }
    if bytes.get(j) == Some(&b'[') {
        Some((level, j + 1))
    } else {
        None
    }
}

/// Whether `bytes[i] == ']'` starts a long-bracket close of `level`
/// (`]` + `level` `=` + `]`).
fn long_bracket_close(bytes: &[u8], i: usize, level: usize) -> bool {
    if bytes.get(i) != Some(&b']') {
        return false;
    }
    let mut j = i + 1;
    for _ in 0..level {
        if bytes.get(j) != Some(&b'=') {
            return false;
        }
        j += 1;
    }
    bytes.get(j) == Some(&b']')
}

/// Argument text of a `name(args)` call — between the first `(` and
/// its matching `)` (nesting-aware). `None` when no call parens.
fn call_args(after_name: &str) -> Option<&str> {
    let open = after_name.find('(')?;
    let inner = &after_name[open + 1..];
    let mut depth = 1usize;
    for (i, c) in inner.char_indices() {
        match c {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&inner[..i]);
                }
            }
            _ => {}
        }
    }
    None
}

impl ScriptAnalyzer for LuauPortAnalyzer {
    fn descriptor(&self) -> &AnalyzerDescriptor {
        &self.descriptor
    }

    fn analyze(&self, request: &AnalyzeRequest) -> Result<AnalyzeResult, AnalyzerError> {
        if request.language != request.source.language {
            return Err(AnalyzerError::InvalidRequest {
                reason: format!(
                    "request language {:?} does not match source language {:?}",
                    request.language.0, request.source.language.0
                ),
            });
        }
        let max = self.descriptor.capabilities.max_source_bytes;
        if max > 0 && request.source.text.len() as u64 > max {
            return Err(AnalyzerError::InvalidRequest {
                reason: format!(
                    "source is {} bytes, limit is {max}",
                    request.source.text.len()
                ),
            });
        }
        if let Some(cursor) = request.cursor
            && (cursor.byte_offset > request.source.text.len()
                || !request.source.text.is_char_boundary(cursor.byte_offset))
        {
            return Err(AnalyzerError::InvalidRequest {
                reason: format!(
                    "cursor byte offset {} is outside the source",
                    cursor.byte_offset
                ),
            });
        }

        let revision = request.source.revision;
        let text = &request.source.text;
        let functions = Self::declared_functions(&request.type_definitions);

        let mut diagnostics = Vec::new();
        if let Some(DiagnosticLevel::Syntax | DiagnosticLevel::SyntaxAndType) =
            request.wanted.diagnostics
        {
            diagnostics.extend(Self::syntax_diagnostics(text, revision));
        }
        if request.wanted.diagnostics == Some(DiagnosticLevel::SyntaxAndType) {
            diagnostics.extend(Self::type_diagnostics(text, &functions, revision));
        }

        let completions = if request.wanted.completion {
            request.cursor.map(|cursor| {
                // Prefix = identifier chars immediately before cursor.
                let before = &text[..cursor.byte_offset];
                let prefix_start = before
                    .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .map_or(0, |i| i + 1);
                let prefix = &before[prefix_start..];
                functions
                    .iter()
                    .filter(|f| f.name.starts_with(prefix))
                    .map(|f| Completion {
                        label: f.name.clone(),
                        kind: CompletionKind::GeneratedFunction,
                        detail: Some(f.signature.clone()),
                        insert_text: None,
                        replace_range: SourceRange {
                            start: LuauPortAnalyzer::position_at(text, prefix_start),
                            end: LuauPortAnalyzer::position_at(text, cursor.byte_offset),
                        },
                        source_revision: revision,
                    })
                    .collect()
            })
        } else {
            None
        };

        Ok(AnalyzeResult {
            source_revision: revision,
            diagnostics,
            completions,
        })
    }
}

/// Verifies a result against the revision it must have been computed
/// for — re-exported consumer-side check (F-14) so callers of this
/// crate get the same single authority.
///
/// # Errors
/// [`AnalyzerError::StaleResult`] when `result.source_revision`
/// differs from `expected`.
pub fn verify_revision(
    result: &AnalyzeResult,
    expected: SourceRevision,
) -> Result<(), AnalyzerError> {
    verify_result_revision(result, expected)
}
