//! Runtime evaluation of the typed [`Expr`]/[`Condition`] ASTs.
//!
//! The compiler proves these expressions *type-check* statically; this
//! module evaluates them against concrete run state — declared inputs,
//! settled step outputs, and the current `for-each` item. Evaluation is
//! pure data access: no I/O, no authority, no script engine. Every
//! failure is a typed [`EvalError`] the coordinator journals as a step
//! failure — a missing reference is never guessed into a default.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::Value;

use crate::expr::{Condition, Expr};

/// Concrete evaluation environment for one step occurrence.
pub struct EvalEnv<'a> {
    /// Provided workflow inputs by declared name.
    pub inputs: &'a serde_json::Map<String, Value>,
    /// Latest settled output per step id (`$steps.*`).
    pub steps: &'a BTreeMap<String, Value>,
    /// Current `for-each-bounded` item (`$item.*`), when inside a body.
    pub item: Option<&'a Value>,
}

/// A typed evaluation failure.
#[derive(Clone, Debug, PartialEq)]
pub enum EvalError {
    /// `$input.<name>` named an undeclared/unprovided input.
    MissingInput {
        /// The input name.
        name: String,
    },
    /// `$steps.<id>` named a step with no settled output.
    StepNotSettled {
        /// The step id.
        step: String,
    },
    /// A field/index path did not resolve inside a value.
    MissingPath {
        /// Dotted path attempted.
        path: String,
    },
    /// `$item` was referenced outside a `for-each` body.
    ItemOutOfScope,
    /// `$param`/`$output` reached workflow evaluation — macro-only
    /// references are rejected fail-closed.
    MacroOnlyReference {
        /// The reference text.
        reference: String,
    },
    /// An operand had the wrong JSON shape for the operation.
    WrongType {
        /// Dotted path of the offending value.
        path: String,
        /// What was expected.
        expected: &'static str,
    },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInput { name } => write!(f, "missing workflow input: {name}"),
            Self::StepNotSettled { step } => {
                write!(f, "step output not available: {step}")
            }
            Self::MissingPath { path } => write!(f, "path does not resolve: {path}"),
            Self::ItemOutOfScope => write!(f, "$item referenced outside for-each body"),
            Self::MacroOnlyReference { reference } => {
                write!(f, "macro-only reference in workflow: {reference}")
            }
            Self::WrongType { path, expected } => {
                write!(f, "type mismatch at {path}: expected {expected}")
            }
        }
    }
}

impl std::error::Error for EvalError {}

/// Evaluates `expr` against `env`.
///
/// # Errors
/// [`EvalError`] on the first unresolvable reference or type violation.
pub fn eval_expr(expr: &Expr, env: &EvalEnv<'_>) -> Result<Value, EvalError> {
    match expr {
        Expr::Literal { value } => Ok(value.clone()),
        Expr::Input { path } => {
            let Some((name, rest)) = path.split_first() else {
                return Err(EvalError::MissingInput {
                    name: String::new(),
                });
            };
            let base = env
                .inputs
                .get(name)
                .ok_or_else(|| EvalError::MissingInput { name: name.clone() })?;
            resolve_path(base, rest, &format!("$input.{}", path.join("."))).cloned()
        }
        Expr::StepOutput { step_output } => {
            let base =
                env.steps
                    .get(&step_output.step)
                    .ok_or_else(|| EvalError::StepNotSettled {
                        step: step_output.step.clone(),
                    })?;
            let at = format!("$steps.{}", step_output.step);
            resolve_path(base, &step_output.field, &at).cloned()
        }
        Expr::Item { path } => {
            let Some(item) = env.item else {
                return Err(EvalError::ItemOutOfScope);
            };
            resolve_path(item, path, "$item").cloned()
        }
        Expr::Param { path } => Err(EvalError::MacroOnlyReference {
            reference: format!("$param.{}", path.join(".")),
        }),
        Expr::Output { name } => Err(EvalError::MacroOnlyReference {
            reference: format!("$output.{name}"),
        }),
        Expr::Array { items } => items
            .iter()
            .map(|e| eval_expr(e, env))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Expr::Object { fields } => {
            let mut out = serde_json::Map::new();
            for (k, e) in fields {
                out.insert(k.clone(), eval_expr(e, env)?);
            }
            Ok(Value::Object(out))
        }
        Expr::Field { base, path } => {
            let value = eval_expr(base, env)?;
            resolve_path(&value, path, "<field>").cloned()
        }
    }
}

/// Evaluates a boolean [`Condition`].
///
/// # Errors
/// [`EvalError`] when an operand cannot be evaluated or is not a number
/// for an ordering comparison.
pub fn eval_condition(cond: &Condition, env: &EvalEnv<'_>) -> Result<bool, EvalError> {
    match cond {
        Condition::Eq { left, right } => Ok(eval_expr(left, env)? == eval_expr(right, env)?),
        Condition::Ne { left, right } => Ok(eval_expr(left, env)? != eval_expr(right, env)?),
        Condition::Lt { left, right } => compare(left, right, env, std::cmp::Ordering::is_lt),
        Condition::Le { left, right } => compare(left, right, env, std::cmp::Ordering::is_le),
        Condition::Gt { left, right } => compare(left, right, env, std::cmp::Ordering::is_gt),
        Condition::Ge { left, right } => compare(left, right, env, std::cmp::Ordering::is_ge),
        Condition::Exists { expr } => {
            let v = eval_expr(expr, env)?;
            Ok(!v.is_null() && v != Value::Bool(false))
        }
        Condition::And { all } => {
            for c in all {
                if !eval_condition(c, env)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Or { any } => {
            for c in any {
                if eval_condition(c, env)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::Not { cond } => Ok(!eval_condition(cond, env)?),
    }
}

/// Numeric comparison for the ordering operators — integers compare
/// exactly, anything else falls back to `f64` (documented precision
/// bound for extreme magnitudes).
fn compare(
    left: &Expr,
    right: &Expr,
    env: &EvalEnv<'_>,
    pred: impl FnOnce(std::cmp::Ordering) -> bool,
) -> Result<bool, EvalError> {
    let l = eval_expr(left, env)?;
    let r = eval_expr(right, env)?;
    let at = expr_path(left);
    let ordering = if let (Some(a), Some(b)) = (l.as_i64(), r.as_i64()) {
        a.cmp(&b)
    } else {
        let (Some(a), Some(b)) = (l.as_f64(), r.as_f64()) else {
            return Err(EvalError::WrongType {
                path: at,
                expected: "number",
            });
        };
        a.partial_cmp(&b).ok_or(EvalError::WrongType {
            path: at,
            expected: "number",
        })?
    };
    Ok(pred(ordering))
}

fn expr_path(expr: &Expr) -> String {
    match expr {
        Expr::Input { path } => format!("$input.{}", path.join(".")),
        Expr::StepOutput { step_output } => {
            format!("$steps.{}", step_output.step)
        }
        Expr::Item { .. } => "$item".to_owned(),
        _ => "<expr>".to_owned(),
    }
}

/// Walks `path` segments into `base`: object keys by name, array
/// indices by canonical decimal. An empty path returns `base`.
fn resolve_path<'a>(base: &'a Value, path: &[String], at: &str) -> Result<&'a Value, EvalError> {
    let mut cur = base;
    let mut walked = String::new();
    for seg in path {
        walked.push('.');
        walked.push_str(seg);
        cur = match cur {
            Value::Object(map) => map.get(seg).ok_or_else(|| EvalError::MissingPath {
                path: format!("{at}{walked}"),
            })?,
            Value::Array(arr) => {
                let idx: usize = seg.parse().map_err(|_| EvalError::MissingPath {
                    path: format!("{at}{walked}"),
                })?;
                arr.get(idx).ok_or_else(|| EvalError::MissingPath {
                    path: format!("{at}{walked}"),
                })?
            }
            _ => {
                return Err(EvalError::MissingPath {
                    path: format!("{at}{walked}"),
                });
            }
        };
    }
    Ok(cur)
}
