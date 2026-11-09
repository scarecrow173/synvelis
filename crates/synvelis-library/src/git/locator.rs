//! Git asset locator grammar: `<rev>:<path>`.
//!
//! A locator names an entry inside one repository at one revision
//! expression. `rev` is a branch/tag/commit-ish resolved by the
//! provider at call time; `path` is a `/`-separated tree path
//! (`""` = the commit's root tree). Locators never carry
//! credentials and never address the provider's working tree —
//! only immutable objects reachable from `rev`.
//!
//! Path components reject `.`/`..`/empty/`\`/control chars so a
//! locator can never escape the tree grammar. `:` *is* allowed
//! inside path components: git's own `<rev>:<path>` syntax splits
//! at the first colon and permits `:` in paths, and our parser does
//! the same — the grammar stays unambiguous because revs can never
//! contain `:`.

use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::OpaqueLocator;

/// Maximum locator text length (mirrors `MAX_OPAQUE_LEN` headroom).
const MAX_LOCATOR_LEN: usize = 2048;
/// Maximum single path component length (git's own limit is 4096;
/// we stay far below for locator sanity).
const MAX_COMPONENT_LEN: usize = 255;
/// Maximum revision expression length.
const MAX_REV_LEN: usize = 256;

/// A parsed `<rev>:<path>` locator. `path` is `""` for the root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitLocator {
    /// Revision expression (branch/tag/sha/ref) — resolved per call.
    pub rev: String,
    /// `/`-separated tree path; `""` = root tree.
    pub path: String,
}

impl GitLocator {
    /// Parses an [`OpaqueLocator`].
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] on malformed rev/path.
    pub fn parse(locator: &OpaqueLocator) -> Result<Self, ProviderError> {
        let text = locator.as_str();
        if text.len() > MAX_LOCATOR_LEN {
            return Err(invalid("locator exceeds length bound"));
        }
        let (rev, path) = match text.find(':') {
            Some(at) => (&text[..at], &text[at + 1..]),
            None => (text, ""),
        };
        validate_rev(rev)?;
        validate_path(path)?;
        Ok(Self {
            rev: rev.to_owned(),
            path: path.to_owned(),
        })
    }

    /// The locator for `name` directly under this locator's path.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when `name` is not a
    /// single valid path component.
    pub fn child(&self, name: &str) -> Result<OpaqueLocator, ProviderError> {
        validate_component(name)?;
        let text = if self.path.is_empty() {
            format!("{}:{name}", self.rev)
        } else {
            format!("{}:{}/{name}", self.rev, self.path)
        };
        OpaqueLocator::new(text)
    }
}

fn invalid(detail: &str) -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidRequest, detail.to_owned())
}

/// Revision grammar — a safe subset of git ref names plus raw
/// object ids. Rejects option-looking (`-`), traversal (`..`),
/// and reflog (`@{`) forms.
fn validate_rev(rev: &str) -> Result<(), ProviderError> {
    if rev.is_empty() || rev.len() > MAX_REV_LEN {
        return Err(invalid("locator revision is empty or over-long"));
    }
    if !rev
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b'-'))
    {
        return Err(invalid("locator revision has unsupported characters"));
    }
    if rev.starts_with('-')
        || rev.starts_with('/')
        || rev.ends_with('/')
        || rev.ends_with('.')
        || rev.to_ascii_lowercase().ends_with(".lock")
        || rev.contains("..")
        || rev.contains("//")
        || rev == "@"
    {
        return Err(invalid("locator revision is not a valid ref"));
    }
    Ok(())
}

/// Tree path grammar: `/`-separated components, no empty/`.`/`..`.
fn validate_path(path: &str) -> Result<(), ProviderError> {
    if path.is_empty() {
        return Ok(());
    }
    if path.starts_with('/') || path.ends_with('/') {
        return Err(invalid("locator path must not start or end with '/'"));
    }
    for comp in path.split('/') {
        validate_component(comp)?;
    }
    Ok(())
}

fn validate_component(comp: &str) -> Result<(), ProviderError> {
    if comp.is_empty() {
        return Err(invalid("locator path has an empty component"));
    }
    if comp == "." || comp == ".." {
        return Err(invalid("locator path contains a traversal component"));
    }
    if comp.len() > MAX_COMPONENT_LEN {
        return Err(invalid("locator path component exceeds length bound"));
    }
    if comp
        .bytes()
        .any(|b| b == b'\\' || b == 0 || b.is_ascii_control())
    {
        return Err(invalid(
            "locator path component contains a forbidden character",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::OpaqueLocator;

    fn parse(text: &str) -> Result<GitLocator, ProviderError> {
        GitLocator::parse(&OpaqueLocator::new(text).unwrap())
    }

    #[test]
    fn parses_rev_and_path() {
        let l = parse("main:assets/drums").unwrap();
        assert_eq!(l.rev, "main");
        assert_eq!(l.path, "assets/drums");
        let l = parse("19c10ccecaa1815078cbfa4dc1113eb06038c813:").unwrap();
        assert_eq!(l.path, "");
        let l = parse("HEAD").unwrap();
        assert_eq!(l.rev, "HEAD");
        assert_eq!(l.path, "");
    }

    #[test]
    fn rejects_traversal_and_bad_rev() {
        for bad in [
            "main:../etc",
            "main:a/../b",
            "main:a//b",
            "main:/abs",
            "main:a/",
            "main:a\\b",
            ":path",
            "-x:y",
            "main@{1}:y",
            // NB: "rev:with space" is *valid* — spaces are legal in
            // git path components (rev="rev", path="with space").
        ] {
            assert!(parse(bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn colon_is_legal_inside_path() {
        // git `<rev>:<path>` splits at the first colon; paths may
        // contain ':' (e.g. a file literally named "a:b.txt").
        let l = parse("main:a:b.txt").unwrap();
        assert_eq!(l.rev, "main");
        assert_eq!(l.path, "a:b.txt");
    }

    #[test]
    fn child_builds_nested_locator() {
        let l = parse("main:dir").unwrap();
        let c = l.child("sub dir").unwrap();
        assert_eq!(c.as_str(), "main:dir/sub dir");
        let root = parse("main:").unwrap();
        assert_eq!(root.child("f.txt").unwrap().as_str(), "main:f.txt");
    }
}
