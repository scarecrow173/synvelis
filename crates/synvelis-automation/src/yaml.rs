//! YAML safe-subset parser for the workflow DSL.
//!
//! This is deliberately *not* a general YAML parser. The accepted subset
//! is fail-closed:
//!
//! - Block mappings and block sequences only — flow collections
//!   (`[...]`, `{...}`) are rejected.
//! - No anchors (`&`), aliases (`*`), tags (`!`), merge keys (`<<`),
//!   or block scalars (`|`, `>`).
//! - Implicit typing is restricted to a fixed set: `null`/`~`,
//!   `true`/`false` (YAML 1.2 core spellings), canonical decimal
//!   integers, and canonical decimal floats. Everything else stays a
//!   string — `yes`, `on`, dates, hex, octal, and underscores never
//!   silently become another type. Typed interpretation happens later,
//!   per field, in the definition decoder.
//! - Keys must be single-line scalars; duplicate keys are rejected.
//! - Single-line scalars only: plain, single-quoted, or double-quoted
//!   (JSON escape subset). No multi-line scalars.
//! - Budgets: input bytes, nesting depth, and node count are capped.
//!
//! The output [`YamlValue`] converts losslessly into
//! `serde_json::Value`, which the typed definition decoder consumes.

use std::collections::BTreeMap;
use std::fmt;

/// Maximum accepted source size in bytes.
pub const MAX_YAML_BYTES: usize = 1_048_576;
/// Maximum nesting depth of mappings/sequences.
pub const MAX_YAML_DEPTH: usize = 64;
/// Maximum total number of scalar/collection nodes.
pub const MAX_YAML_NODES: usize = 200_000;
/// Maximum scalar text length in bytes.
pub const MAX_SCALAR_BYTES: usize = 65_536;

/// A parsed YAML-safe-subset value.
#[derive(Clone, Debug, PartialEq)]
pub enum YamlValue {
    /// `null`, `~`, or an empty value.
    Null,
    /// `true`/`false` (core-schema spellings only).
    Bool(bool),
    /// Canonical decimal integer.
    Int(i64),
    /// Canonical decimal float (always finite).
    Float(f64),
    /// String (quoted or plain).
    Str(String),
    /// Block sequence.
    Seq(Vec<YamlValue>),
    /// Block mapping with string keys.
    Map(BTreeMap<String, YamlValue>),
}

/// A parse failure with a 1-based line number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YamlError {
    /// 1-based line number (0 when not line-specific).
    pub line: usize,
    /// Reason.
    pub reason: String,
}

impl fmt::Display for YamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

impl std::error::Error for YamlError {}

#[derive(Clone)]
struct Line {
    /// 1-based line number in the source.
    number: usize,
    /// Count of leading spaces.
    indent: usize,
    /// Full line content, right-trimmed.
    text: String,
}

struct Parser {
    lines: Vec<Line>,
    pos: usize,
    nodes: usize,
}

/// Parses `source` as the YAML safe subset into a [`YamlValue`].
///
/// # Errors
/// [`YamlError`] on any syntax outside the subset or budget violation.
pub fn parse(source: &str) -> Result<YamlValue, YamlError> {
    if source.len() > MAX_YAML_BYTES {
        return Err(YamlError {
            line: 0,
            reason: format!("input exceeds {MAX_YAML_BYTES} bytes"),
        });
    }
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let normalized = normalized.strip_prefix('\u{feff}').unwrap_or(&normalized);

    let mut lines = Vec::new();
    let mut doc_started = false;
    for (idx, raw) in normalized.split('\n').enumerate() {
        let number = idx + 1;
        // Inline comments are stripped once, quote-aware, so keys,
        // sequence items, and values all see the same comment-free
        // text. `strip_comment` preserves `#` inside quotes.
        let text = strip_comment(raw.trim_end()).trim_end();
        // Indentation counts spaces only; a tab anywhere in the
        // leading whitespace is malformed indentation.
        let ws_len = text.len() - text.trim_start_matches([' ', '\t']).len();
        let leading = &text[..ws_len];
        if leading.contains('\t') {
            return Err(YamlError {
                line: number,
                reason: "tab character in indentation".to_owned(),
            });
        }
        let indent = ws_len;
        let content = &text[indent..];
        if content.is_empty() || content.starts_with('#') {
            continue;
        }
        if content == "---" {
            // Exactly one document marker is allowed and only before
            // any content — a later marker starts a second document.
            if !doc_started && lines.is_empty() {
                doc_started = true;
                continue;
            }
            return Err(YamlError {
                line: number,
                reason: "document marker '---' is only allowed once before the root node"
                    .to_owned(),
            });
        }
        if content == "..." {
            // End-of-document marker: only blank/comment lines may
            // follow — a second document is rejected as multi-doc.
            let rest = normalized.split('\n').skip(idx + 1).any(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with('#')
            });
            if rest {
                return Err(YamlError {
                    line: number,
                    reason: "multiple YAML documents are not supported".to_owned(),
                });
            }
            break;
        }
        if content.starts_with('%') {
            return Err(YamlError {
                line: number,
                reason: "YAML directives are not supported".to_owned(),
            });
        }
        lines.push(Line {
            number,
            indent,
            text: text.to_owned(),
        });
    }
    if lines.is_empty() {
        return Ok(YamlValue::Null);
    }

    let mut parser = Parser {
        lines,
        pos: 0,
        nodes: 0,
    };
    let root_indent = parser.lines[0].indent;
    let value = parser.parse_block(root_indent, 0)?;
    if parser.pos < parser.lines.len() {
        let line = &parser.lines[parser.pos];
        return Err(YamlError {
            line: line.number,
            reason: format!(
                "unexpected content at indent {} after root node",
                line.indent
            ),
        });
    }
    Ok(value)
}

impl Parser {
    fn err(&self, reason: impl Into<String>) -> YamlError {
        let line = self.lines.get(self.pos).map_or(0, |l| l.number).max(1);
        YamlError {
            line,
            reason: reason.into(),
        }
    }

    fn bump(&mut self) -> Result<(), YamlError> {
        self.nodes += 1;
        if self.nodes > MAX_YAML_NODES {
            return Err(self.err(format!("node budget exceeded ({MAX_YAML_NODES})")));
        }
        Ok(())
    }

    fn content(&self, idx: usize, indent: usize) -> &str {
        &self.lines[idx].text[indent..]
    }

    /// Parses a block node whose first line sits at `indent`.
    fn parse_block(&mut self, indent: usize, depth: usize) -> Result<YamlValue, YamlError> {
        if depth > MAX_YAML_DEPTH {
            return Err(self.err(format!("depth budget exceeded ({MAX_YAML_DEPTH})")));
        }
        let idx = self.pos;
        let content = self.content(idx, indent).to_owned();
        if is_seq_marker(&content) {
            self.parse_seq(indent, depth)
        } else if find_map_colon(&content).is_some() {
            self.parse_map(indent, depth)
        } else {
            self.bump()?;
            self.pos += 1;
            parse_scalar_text(&content, self.lines[idx].number)
        }
    }

    /// Parses a block sequence at `indent`; each item line starts with `-`.
    fn parse_seq(&mut self, indent: usize, depth: usize) -> Result<YamlValue, YamlError> {
        let mut items = Vec::new();
        loop {
            if self.pos >= self.lines.len() {
                break;
            }
            let line = &self.lines[self.pos];
            if line.indent < indent {
                break;
            }
            if line.indent > indent {
                return Err(YamlError {
                    line: line.number,
                    reason: format!(
                        "unexpected indentation {} inside sequence at {}",
                        line.indent, indent
                    ),
                });
            }
            let content = self.content(self.pos, indent).to_owned();
            if !is_seq_marker(&content) {
                return Err(YamlError {
                    line: line.number,
                    reason: "expected a sequence entry ('- ')".to_owned(),
                });
            }
            // Split "-" + separation spaces + item content.
            let after_dash = &content[1..];
            let spaces = after_dash.len() - after_dash.trim_start_matches(' ').len();
            let item_text = after_dash[spaces..].to_owned();
            let item_col = indent + 1 + spaces;
            let parent_line = line.number;
            self.pos += 1;
            let item = self.parse_item(&item_text, item_col, indent, parent_line, depth)?;
            items.push(item);
        }
        self.bump()?;
        Ok(YamlValue::Seq(items))
    }

    /// Parses one sequence item whose raw content `text` begins at
    /// column `col` (0-based, absolute). `parent_indent` is the
    /// sequence's own indent, used to bound nested blocks.
    fn parse_item(
        &mut self,
        text: &str,
        col: usize,
        parent_indent: usize,
        line_no: usize,
        depth: usize,
    ) -> Result<YamlValue, YamlError> {
        if depth > MAX_YAML_DEPTH {
            return Err(YamlError {
                line: line_no,
                reason: format!("depth budget exceeded ({MAX_YAML_DEPTH})"),
            });
        }
        if text.is_empty() {
            // `-` alone: nested block on following deeper lines, else null.
            if self.pos < self.lines.len() && self.lines[self.pos].indent > parent_indent {
                let child_indent = self.lines[self.pos].indent;
                return self.parse_block(child_indent, depth + 1);
            }
            self.bump()?;
            return Ok(YamlValue::Null);
        }
        if is_seq_marker(text) {
            // Inline nested sequence: `- - a`. The inner sequence lives
            // at column `col`; its first item is `text` itself.
            return self.parse_inline_seq(text, col, line_no, depth);
        }
        if find_map_colon(text).is_some() {
            // Inline mapping start: `- key: value` with continuation
            // keys aligned at `col`.
            return self.parse_inline_map(text, col, line_no, depth);
        }
        // Scalar item — no children may follow at deeper indent.
        self.bump()?;
        let value = parse_scalar_text(text, line_no)?;
        if self.pos < self.lines.len() && self.lines[self.pos].indent > parent_indent {
            let line = &self.lines[self.pos];
            return Err(YamlError {
                line: line.number,
                reason: "unexpected indented content after scalar sequence item".to_owned(),
            });
        }
        Ok(value)
    }

    /// Parses `- - a` style inline nested sequences.
    fn parse_inline_seq(
        &mut self,
        text: &str,
        col: usize,
        line_no: usize,
        depth: usize,
    ) -> Result<YamlValue, YamlError> {
        // `text` starts with a seq marker at column `col`.
        let after_dash = &text[1..];
        let spaces = after_dash.len() - after_dash.trim_start_matches(' ').len();
        let first_text = after_dash[spaces..].to_owned();
        let first_col = col + 1 + spaces;
        let mut items = Vec::new();
        items.push(self.parse_item(&first_text, first_col, col, line_no, depth + 1)?);
        // Continuation items are lines at indent == col starting with `-`.
        loop {
            if self.pos >= self.lines.len() {
                break;
            }
            let line = &self.lines[self.pos];
            if line.indent < col {
                break;
            }
            if line.indent > col {
                return Err(YamlError {
                    line: line.number,
                    reason: format!(
                        "unexpected indentation {} inside inline sequence at {}",
                        line.indent, col
                    ),
                });
            }
            let content = self.content(self.pos, col).to_owned();
            if !is_seq_marker(&content) {
                return Err(YamlError {
                    line: line.number,
                    reason: "expected a sequence entry ('- ')".to_owned(),
                });
            }
            let ad = &content[1..];
            let sp = ad.len() - ad.trim_start_matches(' ').len();
            let item_text = ad[sp..].to_owned();
            let item_col = col + 1 + sp;
            let num = line.number;
            self.pos += 1;
            items.push(self.parse_item(&item_text, item_col, col, num, depth + 1)?);
        }
        self.bump()?;
        Ok(YamlValue::Seq(items))
    }

    /// Parses `- key: value` inline mappings; continuation pairs are
    /// lines at indent == `col`.
    fn parse_inline_map(
        &mut self,
        text: &str,
        col: usize,
        line_no: usize,
        depth: usize,
    ) -> Result<YamlValue, YamlError> {
        let mut map = BTreeMap::new();
        let (key, value) = self.parse_pair(text, col, line_no, depth, &mut map)?;
        let _ = (key, value);
        // `parse_pair` inserts into `map`; loop over continuation lines.
        loop {
            if self.pos >= self.lines.len() {
                break;
            }
            let line = &self.lines[self.pos];
            if line.indent < col {
                break;
            }
            if line.indent > col {
                return Err(YamlError {
                    line: line.number,
                    reason: format!(
                        "unexpected indentation {} inside inline mapping at {}",
                        line.indent, col
                    ),
                });
            }
            let content = self.content(self.pos, col).to_owned();
            if find_map_colon(&content).is_none() {
                return Err(YamlError {
                    line: line.number,
                    reason: "expected a mapping entry".to_owned(),
                });
            }
            let num = line.number;
            self.pos += 1;
            self.parse_pair(&content, col, num, depth, &mut map)?;
        }
        self.bump()?;
        Ok(YamlValue::Map(map))
    }

    /// Parses a block mapping at `indent`.
    fn parse_map(&mut self, indent: usize, depth: usize) -> Result<YamlValue, YamlError> {
        let mut map = BTreeMap::new();
        loop {
            if self.pos >= self.lines.len() {
                break;
            }
            let line = &self.lines[self.pos];
            if line.indent < indent {
                break;
            }
            if line.indent > indent {
                return Err(YamlError {
                    line: line.number,
                    reason: format!(
                        "unexpected indentation {} inside mapping at {}",
                        line.indent, indent
                    ),
                });
            }
            let content = self.content(self.pos, indent).to_owned();
            if find_map_colon(&content).is_none() {
                return Err(YamlError {
                    line: line.number,
                    reason: "expected a mapping entry ('key: value')".to_owned(),
                });
            }
            let num = line.number;
            self.pos += 1;
            self.parse_pair(&content, indent, num, depth, &mut map)?;
        }
        self.bump()?;
        Ok(YamlValue::Map(map))
    }

    /// Parses one `key: value` pair. `text` is the pair text starting at
    /// column `col`; the value may be an inline scalar or a nested block
    /// on following deeper-indented lines. Inserts into `map`.
    fn parse_pair(
        &mut self,
        text: &str,
        col: usize,
        line_no: usize,
        depth: usize,
        map: &mut BTreeMap<String, YamlValue>,
    ) -> Result<(String, YamlValue), YamlError> {
        if depth > MAX_YAML_DEPTH {
            return Err(YamlError {
                line: line_no,
                reason: format!("depth budget exceeded ({MAX_YAML_DEPTH})"),
            });
        }
        let colon = find_map_colon(text).ok_or_else(|| YamlError {
            line: line_no,
            reason: "expected a mapping entry ('key: value')".to_owned(),
        })?;
        let key_text = text[..colon].trim();
        if key_text.is_empty() {
            return Err(YamlError {
                line: line_no,
                reason: "empty mapping key".to_owned(),
            });
        }
        if key_text == "<<" {
            return Err(YamlError {
                line: line_no,
                reason: "merge keys ('<<') are not supported".to_owned(),
            });
        }
        let key = parse_key(key_text, line_no)?;
        // Value text: everything after ':' — separation space(s) then
        // scalar, or nothing → nested block / null. The caller has
        // already consumed this pair's line, so `self.pos` points at
        // the first line that may hold a nested block.
        let after_colon = &text[colon + 1..];
        // Comments were already stripped quote-aware at line scan.
        let value_text = after_colon.trim().to_owned();
        let value = if value_text.is_empty() {
            if self.pos < self.lines.len() && self.lines[self.pos].indent > col {
                let child_indent = self.lines[self.pos].indent;
                self.parse_block(child_indent, depth + 1)?
            } else {
                self.bump()?;
                YamlValue::Null
            }
        } else {
            self.bump()?;
            parse_scalar_text(&value_text, line_no)?
        };
        if map.insert(key.clone(), value.clone()).is_some() {
            return Err(YamlError {
                line: line_no,
                reason: format!("duplicate mapping key: {key}"),
            });
        }
        Ok((key, value))
    }
}

/// `-` followed by a separation space or end of line.
fn is_seq_marker(content: &str) -> bool {
    content == "-" || content.starts_with("- ")
}

/// Finds the first `:` followed by a space or end-of-line, outside
/// single/double quotes. Returns the byte index of the colon.
fn find_map_colon(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                if bytes.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                in_single = false;
            }
        } else if in_double {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_double = false;
            }
        } else {
            match b {
                b'\'' => in_single = true,
                b'"' => in_double = true,
                b':' => {
                    let next = bytes.get(i + 1);
                    if next.is_none() || next == Some(&b' ') {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Removes a trailing ` #...` comment outside quotes.
fn strip_comment(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                if bytes.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                in_single = false;
            }
        } else if in_double {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_double = false;
            }
        } else {
            match b {
                b'\'' => in_single = true,
                b'"' => in_double = true,
                b'#' if i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t' => {
                    return &text[..i];
                }
                _ => {}
            }
        }
        i += 1;
    }
    text
}

/// Parses a mapping key scalar into its string form.
fn parse_key(text: &str, line: usize) -> Result<String, YamlError> {
    let value = parse_scalar_text(text, line)?;
    Ok(match value {
        YamlValue::Str(s) => s,
        YamlValue::Int(i) => i.to_string(),
        YamlValue::Float(f) => f.to_string(),
        YamlValue::Bool(b) => b.to_string(),
        YamlValue::Null => "null".to_owned(),
        YamlValue::Seq(_) | YamlValue::Map(_) => {
            return Err(YamlError {
                line,
                reason: "complex mapping keys are not supported".to_owned(),
            });
        }
    })
}

/// Parses scalar text (already comment-stripped and trimmed) with the
/// restricted implicit-typing rules.
fn parse_scalar_text(text: &str, line: usize) -> Result<YamlValue, YamlError> {
    if text.len() > MAX_SCALAR_BYTES {
        return Err(YamlError {
            line,
            reason: format!("scalar exceeds {MAX_SCALAR_BYTES} bytes"),
        });
    }
    let err = |reason: &str| YamlError {
        line,
        reason: reason.to_owned(),
    };
    let bytes = text.as_bytes();
    match bytes.first() {
        Some(b'\'') => parse_single_quoted(text, line),
        Some(b'"') => parse_double_quoted(text, line),
        Some(b'!') => Err(err("YAML tags ('!') are not supported")),
        Some(b'&') => Err(err("anchors ('&') are not supported")),
        Some(b'*') => Err(err("aliases ('*') are not supported")),
        Some(b'|' | b'>') => Err(err("block scalars ('|' / '>') are not supported")),
        Some(b'[' | b'{') => Err(err("flow collections ('[' / '{') are not supported")),
        Some(b'?') if text.starts_with("? ") => {
            Err(err("explicit key indicators ('? ') are not supported"))
        }
        Some(b'-') if is_seq_marker(text) => Err(err(
            "nested sequence indicators are not valid scalar values",
        )),
        _ => resolve_plain(text, line),
    }
}

/// Single-quoted scalar: `''` escapes a literal quote; no other escapes.
fn parse_single_quoted(text: &str, line: usize) -> Result<YamlValue, YamlError> {
    let err = |reason: &str| YamlError {
        line,
        reason: reason.to_owned(),
    };
    let bytes = text.as_bytes();
    debug_assert_eq!(bytes[0], b'\'');
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if bytes.get(i + 1) == Some(&b'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            // Closing quote — nothing but whitespace may follow.
            if text[i + 1..].trim().is_empty() {
                return Ok(YamlValue::Str(out));
            }
            return Err(err("trailing characters after quoted scalar"));
        }
        // Copy one UTF-8 scalar — bytes ≥ 0x80 are never `'` or `\`,
        // so scanning bytes is safe as long as we advance by scalar.
        let ch_len = utf8_len(bytes[i]);
        let end = i + ch_len;
        let s = text.get(i..end).ok_or_else(|| err("invalid utf-8"))?;
        out.push_str(s);
        i = end;
    }
    Err(err("unterminated single-quoted scalar"))
}

/// Double-quoted scalar with the JSON escape subset.
fn parse_double_quoted(text: &str, line: usize) -> Result<YamlValue, YamlError> {
    let err = |reason: &str| YamlError {
        line,
        reason: reason.to_owned(),
    };
    let bytes = text.as_bytes();
    debug_assert_eq!(bytes[0], b'"');
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'"' => {
                if text[i + 1..].trim().is_empty() {
                    return Ok(YamlValue::Str(out));
                }
                return Err(err("trailing characters after quoted scalar"));
            }
            b'\\' => {
                i += 1;
                let esc = *bytes.get(i).ok_or_else(|| err("unterminated escape"))?;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000C}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex = text
                            .get(i + 1..i + 5)
                            .ok_or_else(|| err("incomplete unicode escape"))?;
                        let code = u32::from_str_radix(hex, 16)
                            .map_err(|_| err("invalid unicode escape"))?;
                        let ch = char::from_u32(code)
                            .ok_or_else(|| err("invalid unicode code point"))?;
                        out.push(ch);
                        i += 4;
                    }
                    _ => return Err(err("unsupported escape sequence")),
                }
                i += 1;
            }
            0x00..=0x1F => return Err(err("raw control character in quoted scalar")),
            _ => {
                // Copy one UTF-8 scalar.
                let ch_len = utf8_len(b);
                let end = i + ch_len;
                let s = text.get(i..end).ok_or_else(|| err("invalid utf-8"))?;
                out.push_str(s);
                i = end;
            }
        }
    }
    Err(err("unterminated double-quoted scalar"))
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first < 0xE0 {
        2
    } else if first < 0xF0 {
        3
    } else {
        4
    }
}

/// Restricted implicit typing for plain scalars.
fn resolve_plain(text: &str, line: usize) -> Result<YamlValue, YamlError> {
    let err = |reason: &str| YamlError {
        line,
        reason: reason.to_owned(),
    };
    if text.is_empty() {
        return Ok(YamlValue::Null);
    }
    // Plain scalars cannot contain ': ' or end with ':' — that would be
    // an ambiguous mapping.
    if text.contains(": ") || text.ends_with(':') {
        return Err(err("plain scalar contains an ambiguous ':'"));
    }
    match text {
        "null" | "Null" | "NULL" | "~" => return Ok(YamlValue::Null),
        "true" | "True" | "TRUE" => return Ok(YamlValue::Bool(true)),
        "false" | "False" | "FALSE" => return Ok(YamlValue::Bool(false)),
        _ => {}
    }
    if let Some(int) = parse_int_literal(text)? {
        return Ok(YamlValue::Int(int));
    }
    if let Some(float) = parse_float_literal(text)? {
        return Ok(YamlValue::Float(float));
    }
    Ok(YamlValue::Str(text.to_owned()))
}

/// `[-+]?(0|[1-9][0-9]*)` → i64, or an error on overflow.
fn parse_int_literal(text: &str) -> Result<Option<i64>, YamlError> {
    let bytes = text.as_bytes();
    let mut i = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i = 1;
    }
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return Ok(None);
    }
    if bytes[i] == b'0' {
        // Canonical int: a lone `0` (optionally signed) only.
        if i + 1 != bytes.len() {
            return Ok(None); // leading zero → not a canonical int
        }
    } else {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i != bytes.len() {
            return Ok(None);
        }
    }
    text.parse::<i64>().map(Some).map_err(|_| YamlError {
        line: 0,
        reason: format!("integer literal out of range: {text}"),
    })
}

/// Canonical decimal float: `[-+]?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][-+]?[0-9]+)?`
/// with at least one `.` or exponent. Always finite.
fn parse_float_literal(text: &str) -> Result<Option<f64>, YamlError> {
    let bytes = text.as_bytes();
    let mut i = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i = 1;
    }
    // Integer part: 0 or [1-9][0-9]*
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return Ok(None);
    }
    if bytes[i] == b'0' {
        i += 1;
        if i < bytes.len() && bytes[i].is_ascii_digit() {
            return Ok(None); // leading zero
        }
    } else {
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    let mut is_float = false;
    // Optional fraction.
    if i < bytes.len() && bytes[i] == b'.' {
        is_float = true;
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return Ok(None); // "1." → not a float literal
        }
    }
    // Optional exponent.
    if i < bytes.len() && matches!(bytes[i], b'e' | b'E') {
        is_float = true;
        i += 1;
        if i < bytes.len() && matches!(bytes[i], b'+' | b'-') {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return Ok(None);
        }
    }
    if i != bytes.len() || !is_float {
        return Ok(None);
    }
    let value: f64 = text.parse().map_err(|_| YamlError {
        line: 0,
        reason: format!("float literal out of range: {text}"),
    })?;
    if !value.is_finite() {
        return Err(YamlError {
            line: 0,
            reason: format!("non-finite float literal: {text}"),
        });
    }
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_block() {
        let src = "a:\n  b: 1\n  c:\n    - x\n    - y\n";
        let v = parse(src).unwrap();
        let YamlValue::Map(m) = v else {
            panic!("expected map")
        };
        assert_eq!(
            m["a"],
            YamlValue::Map(BTreeMap::from([
                ("b".to_owned(), YamlValue::Int(1)),
                (
                    "c".to_owned(),
                    YamlValue::Seq(vec![YamlValue::Str("x".into()), YamlValue::Str("y".into())])
                ),
            ]))
        );
    }
}
