//! Typed expression and condition ASTs.
//!
//! [`Expr`] is the only value-producing node in the DSL: literals,
//! workflow-input references (`$input.path`), prior-step output
//! references (`$steps.<id>.<field.path>`), and for-each item
//! references (`$item.path`). There is deliberately no env access, no
//! function calls, and no script evaluation — every expression lowers
//! to data the compiler can type statically.
//!
//! [`Condition`] is the restricted boolean AST allowed in `if` steps:
//! comparisons over [`Expr`] operands plus `and`/`or`/`not`. Arbitrary
//! code, eval, and environment/network access do not exist here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A reference to a prior step's output: `step_id` plus an optional
/// field path into its output schema (`field` empty = whole output).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepOutputRef {
    /// The producing step's id.
    pub step: String,
    /// Path segments into the output value (empty = whole output).
    #[serde(default)]
    pub field: Vec<String>,
}

/// A typed value expression.
///
/// YAML encoding uses `$`-prefixed plain scalars for references
/// (`$input.name`, `$steps.s1.field`, `$item`) and ordinary scalars or
/// block collections for literals; programmatic builders construct the
/// same variants directly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "expr", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Expr {
    /// A literal JSON value (`null`, bool, number, string, array,
    /// object — literals only, no nested references).
    Literal {
        /// The literal value.
        value: serde_json::Value,
    },
    /// `$input.<path...>` — a declared workflow input.
    Input {
        /// Path segments into the input value (first segment is the
        /// input name; empty path is rejected).
        path: Vec<String>,
    },
    /// `$steps.<step>.<field...>` — a prior step's output.
    StepOutput {
        /// The step reference.
        step_output: StepOutputRef,
    },
    /// `$item.<path...>` — the current `for-each-bounded` item.
    Item {
        /// Path segments into the item (empty = whole item).
        #[serde(default)]
        path: Vec<String>,
    },
    /// `$param.<name>` — an atomic-macro parameter reference
    /// (AUTX-04). Meaningful only inside [`crate::AtomicMacroDefinition`]
    /// step args/targets; the workflow [`crate::PlanCompiler`] rejects
    /// it fail-closed.
    Param {
        /// Path segments into the parameter value (first segment is
        /// the parameter name; v1 accepts the bare name only).
        path: Vec<String>,
    },
    /// `$output.<name>` — a named atomic-macro output binding
    /// reference (AUTX-04). Resolves to the stable id the bound step
    /// creates; meaningful only inside macro definitions.
    Output {
        /// The output binding name.
        name: String,
    },
    /// An array of expressions (evaluated element-wise).
    Array {
        /// Element expressions.
        items: Vec<Expr>,
    },
    /// An object of named expressions.
    Object {
        /// Field name → expression.
        fields: BTreeMap<String, Expr>,
    },
    /// A field/index path projected onto an arbitrary expression.
    ///
    /// Not expressible in the YAML surface — produced internally by
    /// workflow import inlining (synvelis#273) when a referenced
    /// workflow's typed inputs/outputs are substituted into call-site
    /// expressions. Type-checked and evaluated like any other expr.
    Field {
        /// Base expression.
        base: Box<Expr>,
        /// Path segments projected into the base value.
        path: Vec<String>,
    },
}

impl Default for Expr {
    /// The empty object expression — the default for omitted `args`.
    fn default() -> Self {
        Self::Object {
            fields: BTreeMap::new(),
        }
    }
}

impl Expr {
    /// The canonical `$`-string form for reference expressions, used by
    /// the YAML frontend. Returns `None` for literals and compound
    /// expressions.
    #[must_use]
    pub fn reference_text(&self) -> Option<String> {
        match self {
            Self::Input { path } => Some(format!("$input.{}", path.join("."))),
            Self::StepOutput { step_output } => {
                let mut s = format!("$steps.{}", step_output.step);
                for seg in &step_output.field {
                    s.push('.');
                    s.push_str(seg);
                }
                Some(s)
            }
            Self::Item { path } if path.is_empty() => Some("$item".to_owned()),
            Self::Item { path } => Some(format!("$item.{}", path.join("."))),
            Self::Param { path } => Some(format!("$param.{}", path.join("."))),
            Self::Output { name } => Some(format!("$output.{name}")),
            _ => None,
        }
    }
}

/// A restricted boolean condition for `if` steps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Condition {
    /// `left == right`.
    Eq {
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// `left != right`.
    Ne {
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// `left < right` (numeric operands only).
    Lt {
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// `left <= right` (numeric operands only).
    Le {
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// `left > right` (numeric operands only).
    Gt {
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// `left >= right` (numeric operands only).
    Ge {
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// The expression evaluates to a non-null, non-false value.
    Exists {
        /// The operand.
        expr: Box<Expr>,
    },
    /// Logical conjunction — at least one condition required.
    And {
        /// Conjuncts.
        all: Vec<Condition>,
    },
    /// Logical disjunction — at least one condition required.
    Or {
        /// Disjuncts.
        any: Vec<Condition>,
    },
    /// Logical negation.
    Not {
        /// The negated condition.
        cond: Box<Condition>,
    },
}
