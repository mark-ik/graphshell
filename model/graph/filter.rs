/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Faceted filter expression engine.
//!
//! Implements the canonical filter query model from:
//! `design_docs/graphshell_docs/implementation_strategy/canvas/faceted_filter_surface_spec.md §4`
//!
//! ## Authority contract (spec §2)
//!
//! - Filter truth is derived from graph-owned node/edge metadata.
//! - Evaluation runs through reducer-owned intent paths.
//! - Filter output is a **projection** over graph truth — it never mutates node/edge identity.
//!
//! ## Namespaced extension keys (spec §3)
//!
//! Canonical PMEST facet keys (see `FacetKey` constants) are non-namespaced.
//! Extension facets **must** use `"namespace:name"` format. Non-namespaced
//! extension keys are invalid and must emit a `Warn` diagnostic at evaluation
//! time.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Facet value — the runtime type that a facet key resolves to
// ---------------------------------------------------------------------------

/// Runtime value carried by a facet key.
///
/// Used both as the output of [`FacetProjection`] and as the operand type
/// matched by [`FacetOperator`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FacetValue {
    /// Single scalar (string, number, or boolean encoded as string/f64/bool).
    Scalar(FacetScalar),
    /// Multi-valued collection (tags, edge kinds, frame memberships, …).
    Collection(Vec<FacetScalar>),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FacetScalar {
    Text(String),
    Number(f64),
    Bool(bool),
}

impl FacetScalar {
    fn matches_text(&self, text: &str) -> bool {
        match self {
            FacetScalar::Text(t) => t == text,
            FacetScalar::Number(n) => n.to_string() == text,
            FacetScalar::Bool(b) => b.to_string() == text,
        }
    }
}

// ---------------------------------------------------------------------------
// Canonical PMEST facet key constants (spec §3)
// ---------------------------------------------------------------------------

pub mod facet_keys {
    // Personality
    pub const ADDRESS_KIND: &str = "address_kind";
    pub const DOMAIN: &str = "domain";
    pub const TITLE: &str = "title";
    pub const ADDRESS: &str = "address";

    // Matter
    pub const MIME_HINT: &str = "mime_hint";
    pub const VIEWER_BINDING: &str = "viewer_binding";
    pub const CONTENT_LENGTH: &str = "content_length";

    // Energy
    pub const EDGE_KINDS: &str = "edge_kinds";
    pub const TRAVERSAL_COUNT: &str = "traversal_count";
    pub const IN_DEGREE: &str = "in_degree";
    pub const OUT_DEGREE: &str = "out_degree";

    // Space
    pub const FRAME_MEMBERSHIPS: &str = "frame_memberships";
    pub const FRAME_AFFINITY_REGION: &str = "frame_affinity_region";
    pub const UDC_CLASSES: &str = "udc_classes";
    pub const SPATIAL_CLUSTER: &str = "spatial_cluster";

    // Time
    pub const CREATED_AT: &str = "created_at";
    pub const LAST_TRAVERSAL: &str = "last_traversal";
    pub const LIFECYCLE: &str = "lifecycle";

    /// Returns true for canonical (non-namespaced) PMEST keys.
    pub fn is_canonical(key: &str) -> bool {
        matches!(
            key,
            ADDRESS_KIND
                | DOMAIN
                | TITLE
                | ADDRESS
                | MIME_HINT
                | VIEWER_BINDING
                | CONTENT_LENGTH
                | EDGE_KINDS
                | TRAVERSAL_COUNT
                | IN_DEGREE
                | OUT_DEGREE
                | FRAME_MEMBERSHIPS
                | FRAME_AFFINITY_REGION
                | UDC_CLASSES
                | SPATIAL_CLUSTER
                | CREATED_AT
                | LAST_TRAVERSAL
                | LIFECYCLE
        )
    }

    /// Returns true when `key` is a valid namespaced extension key (`"ns:name"`).
    pub fn is_valid_extension(key: &str) -> bool {
        let parts: Vec<&str> = key.splitn(2, ':').collect();
        parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty()
    }

    /// Returns true when a facet key is valid (canonical or correctly namespaced).
    pub fn is_valid(key: &str) -> bool {
        is_canonical(key) || is_valid_extension(key)
    }
}

// ---------------------------------------------------------------------------
// Filter query model (spec §4)
// ---------------------------------------------------------------------------

/// Composable predicate expression over node facets (spec §4.1).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FacetExpr {
    Predicate(FacetPredicate),
    And(Vec<FacetExpr>),
    Or(Vec<FacetExpr>),
    Not(Box<FacetExpr>),
}

/// Single predicate over one facet key (spec §4.2).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FacetPredicate {
    /// PMEST facet key or `"namespace:name"` extension key.
    pub facet_key: String,
    pub operator: FacetOperator,
    pub operand: FacetOperand,
}

/// Filter operators (spec §4.2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FacetOperator {
    Eq,
    NotEq,
    In,
    ContainsAny,
    ContainsAll,
    Range,
    Exists,
    NotExists,
}

/// Operand carried by a predicate.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FacetOperand {
    Scalar(FacetScalar),
    Set(Vec<FacetScalar>),
    Range {
        lo: FacetScalar,
        hi: FacetScalar,
    },
    None,
}

// ---------------------------------------------------------------------------
// Filter result model (spec §6)
// ---------------------------------------------------------------------------

pub type FacetProjection = HashMap<String, FacetValue>;

/// Output of filter evaluation against a graph scope.
#[derive(Debug, Clone)]
pub struct FilterResult {
    /// Node keys satisfying the `FacetExpr`.
    pub matched_nodes: Vec<crate::model::graph::NodeKey>,
    /// Node keys excluded by current filters.
    pub filtered_out_nodes: Vec<crate::model::graph::NodeKey>,
    /// Per-facet bucket counts for visible scope.
    pub facet_counts: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Evaluation (spec §5.1 — runs through reducer-owned intent paths)
// ---------------------------------------------------------------------------

/// Error produced when a predicate cannot be evaluated.
#[derive(Debug, Clone)]
pub enum FilterEvalError {
    /// Operator applied to an incompatible facet value type.
    TypeMismatch {
        facet_key: String,
        operator: FacetOperator,
        actual_value_type: &'static str,
    },
    /// Extension key used without the `"namespace:name"` format.
    InvalidExtensionKey { key: String },
    /// Facet key not found in projection (used with `Exists`/`NotExists`).
    KeyAbsent { key: String },
}

impl FacetExpr {
    /// Evaluate this expression against a node's facet projection.
    ///
    /// Per spec §4.2 operator invariants:
    /// - Operator/type mismatch resolves to **no match** (returns `false`) and
    ///   the caller should emit a `Warn` diagnostic via the diagnostics channel
    ///   `ux:facet_filter_type_mismatch`.
    /// - `Range` on non-ordered types is invalid (resolves to no match).
    /// - `ContainsAny`/`ContainsAll` require collection-valued facets.
    ///
    /// Returns `Ok(true/false)` for normal match/no-match, or
    /// `Err(FilterEvalError)` for structural problems (invalid key, type mismatch)
    /// that the caller must log as a diagnostic.
    pub fn evaluate(
        &self,
        projection: &FacetProjection,
    ) -> Result<bool, FilterEvalError> {
        match self {
            FacetExpr::Predicate(pred) => pred.evaluate(projection),
            FacetExpr::And(exprs) => {
                for e in exprs {
                    if !e.evaluate(projection)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            FacetExpr::Or(exprs) => {
                for e in exprs {
                    if e.evaluate(projection)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            FacetExpr::Not(inner) => Ok(!inner.evaluate(projection)?),
        }
    }
}

impl FacetPredicate {
    pub fn evaluate(
        &self,
        projection: &FacetProjection,
    ) -> Result<bool, FilterEvalError> {
        // Validate key before evaluation
        if !facet_keys::is_valid(&self.facet_key) {
            return Err(FilterEvalError::InvalidExtensionKey {
                key: self.facet_key.clone(),
            });
        }

        match self.operator {
            FacetOperator::Exists => return Ok(projection.contains_key(&self.facet_key)),
            FacetOperator::NotExists => return Ok(!projection.contains_key(&self.facet_key)),
            _ => {}
        }

        let Some(value) = projection.get(&self.facet_key) else {
            // Key absent = no match for all other operators
            return Ok(false);
        };

        match (&self.operator, value, &self.operand) {
            (FacetOperator::Eq, FacetValue::Scalar(scalar), FacetOperand::Scalar(operand)) => {
                Ok(scalar == operand)
            }
            (FacetOperator::NotEq, FacetValue::Scalar(scalar), FacetOperand::Scalar(operand)) => {
                Ok(scalar != operand)
            }
            (FacetOperator::In, FacetValue::Scalar(scalar), FacetOperand::Set(set)) => {
                Ok(set.contains(scalar))
            }
            (FacetOperator::ContainsAny, FacetValue::Collection(coll), FacetOperand::Set(set)) => {
                Ok(set.iter().any(|s| coll.contains(s)))
            }
            (
                FacetOperator::ContainsAll,
                FacetValue::Collection(coll),
                FacetOperand::Set(set),
            ) => Ok(set.iter().all(|s| coll.contains(s))),
            (
                FacetOperator::Range,
                FacetValue::Scalar(FacetScalar::Number(n)),
                FacetOperand::Range {
                    lo: FacetScalar::Number(lo),
                    hi: FacetScalar::Number(hi),
                },
            ) => Ok(n >= lo && n <= hi),
            // Type mismatches: return Err so caller can emit Warn diagnostic
            (op, _, _) => Err(FilterEvalError::TypeMismatch {
                facet_key: self.facet_key.clone(),
                operator: op.clone(),
                actual_value_type: value_type_name(value),
            }),
        }
    }
}

fn value_type_name(value: &FacetValue) -> &'static str {
    match value {
        FacetValue::Scalar(FacetScalar::Text(_)) => "text",
        FacetValue::Scalar(FacetScalar::Number(_)) => "number",
        FacetValue::Scalar(FacetScalar::Bool(_)) => "bool",
        FacetValue::Collection(_) => "collection",
    }
}

// ---------------------------------------------------------------------------
// Omnibar `facet:` token parser (spec §7.1)
// ---------------------------------------------------------------------------

/// Parse a minimal `facet:key=value` or `facet:key` (Exists check) token
/// entered via the omnibar.
///
/// Returns `None` with no side effects on invalid input — the caller is
/// responsible for emitting `ux:facet_filter_invalid_query` (Warn).
///
/// Supported syntax:
/// - `facet:key=value`  → `Eq` predicate
/// - `facet:!key=value` → `NotEq` predicate
/// - `facet:key`        → `Exists` predicate
pub fn parse_omnibar_facet_token(token: &str) -> Option<FacetPredicate> {
    let body = token.strip_prefix("facet:")?;
    if body.is_empty() {
        return None;
    }

    // `facet:!key=value` → NotEq
    if let Some(rest) = body.strip_prefix('!') {
        if let Some((key, value)) = rest.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if !facet_keys::is_valid(&key) || key.is_empty() || value.is_empty() {
                return None;
            }
            return Some(FacetPredicate {
                facet_key: key,
                operator: FacetOperator::NotEq,
                operand: FacetOperand::Scalar(FacetScalar::Text(value)),
            });
        }
        return None;
    }

    // `facet:key=value` → Eq
    if let Some((key, value)) = body.split_once('=') {
        let key = key.trim().to_string();
        let value = value.trim().to_string();
        if !facet_keys::is_valid(&key) || key.is_empty() || value.is_empty() {
            return None;
        }
        return Some(FacetPredicate {
            facet_key: key,
            operator: FacetOperator::Eq,
            operand: FacetOperand::Scalar(FacetScalar::Text(value)),
        });
    }

    // `facet:key` → Exists
    let key = body.trim().to_string();
    if !facet_keys::is_valid(&key) || key.is_empty() {
        return None;
    }
    Some(FacetPredicate {
        facet_key: key,
        operator: FacetOperator::Exists,
        operand: FacetOperand::None,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> FacetScalar {
        FacetScalar::Text(s.to_string())
    }

    fn num(n: f64) -> FacetScalar {
        FacetScalar::Number(n)
    }

    fn proj(pairs: &[(&str, FacetValue)]) -> FacetProjection {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    // G03 — canonical facet keys are valid, non-namespaced extensions are not
    #[test]
    fn canonical_keys_are_valid_non_namespaced_extensions_are_not() {
        assert!(facet_keys::is_valid(facet_keys::TITLE));
        assert!(facet_keys::is_valid(facet_keys::LIFECYCLE));
        assert!(facet_keys::is_valid("myns:custom_facet")); // valid extension
        assert!(!facet_keys::is_valid("custom_facet")); // missing namespace
        assert!(!facet_keys::is_valid("ns:")); // empty name
        assert!(!facet_keys::is_valid(":name")); // empty namespace
    }

    // Spec §9: namespaced extension keys enforced
    #[test]
    fn invalid_extension_key_returns_error() {
        let pred = FacetPredicate {
            facet_key: "notnamespaced".to_string(),
            operator: FacetOperator::Eq,
            operand: FacetOperand::Scalar(text("foo")),
        };
        let p = proj(&[]);
        let result = pred.evaluate(&p);
        assert!(
            matches!(result, Err(FilterEvalError::InvalidExtensionKey { .. })),
            "non-namespaced extension key must return InvalidExtensionKey error"
        );
    }

    // Spec §9: operator semantics are type-safe
    #[test]
    fn type_mismatch_returns_error_not_panic() {
        // Range on a text scalar is a type mismatch
        let pred = FacetPredicate {
            facet_key: facet_keys::TITLE.to_string(),
            operator: FacetOperator::Range,
            operand: FacetOperand::Range {
                lo: num(0.0),
                hi: num(10.0),
            },
        };
        let p = proj(&[(facet_keys::TITLE, FacetValue::Scalar(text("hello")))]);
        let result = pred.evaluate(&p);
        assert!(
            matches!(result, Err(FilterEvalError::TypeMismatch { .. })),
            "Range on text facet must return TypeMismatch, not panic"
        );
    }

    // Spec §9: PMEST canonical facets are queryable
    #[test]
    fn eq_predicate_matches_scalar_facet() {
        let pred = FacetPredicate {
            facet_key: facet_keys::LIFECYCLE.to_string(),
            operator: FacetOperator::Eq,
            operand: FacetOperand::Scalar(text("Active")),
        };
        let p_match = proj(&[(facet_keys::LIFECYCLE, FacetValue::Scalar(text("Active")))]);
        let p_miss = proj(&[(facet_keys::LIFECYCLE, FacetValue::Scalar(text("Cold")))]);

        assert_eq!(pred.evaluate(&p_match).unwrap(), true);
        assert_eq!(pred.evaluate(&p_miss).unwrap(), false);
    }

    #[test]
    fn contains_any_predicate_matches_collection_facet() {
        let pred = FacetPredicate {
            facet_key: facet_keys::EDGE_KINDS.to_string(),
            operator: FacetOperator::ContainsAny,
            operand: FacetOperand::Set(vec![text("Hyperlink"), text("UserGrouped")]),
        };
        let p = proj(&[(
            facet_keys::EDGE_KINDS,
            FacetValue::Collection(vec![text("Hyperlink"), text("TraversalDerived")]),
        )]);
        assert_eq!(pred.evaluate(&p).unwrap(), true);
    }

    #[test]
    fn range_predicate_matches_numeric_facet() {
        let pred = FacetPredicate {
            facet_key: facet_keys::IN_DEGREE.to_string(),
            operator: FacetOperator::Range,
            operand: FacetOperand::Range {
                lo: num(1.0),
                hi: num(5.0),
            },
        };
        let p_in = proj(&[(
            facet_keys::IN_DEGREE,
            FacetValue::Scalar(num(3.0)),
        )]);
        let p_out = proj(&[(
            facet_keys::IN_DEGREE,
            FacetValue::Scalar(num(0.0)),
        )]);
        assert_eq!(pred.evaluate(&p_in).unwrap(), true);
        assert_eq!(pred.evaluate(&p_out).unwrap(), false);
    }

    #[test]
    fn exists_predicate_checks_key_presence() {
        let pred = FacetPredicate {
            facet_key: facet_keys::MIME_HINT.to_string(),
            operator: FacetOperator::Exists,
            operand: FacetOperand::None,
        };
        let p_present = proj(&[(facet_keys::MIME_HINT, FacetValue::Scalar(text("text/html")))]);
        let p_absent: FacetProjection = HashMap::new();

        assert_eq!(pred.evaluate(&p_present).unwrap(), true);
        assert_eq!(pred.evaluate(&p_absent).unwrap(), false);
    }

    #[test]
    fn and_expr_requires_all_predicates() {
        let expr = FacetExpr::And(vec![
            FacetExpr::Predicate(FacetPredicate {
                facet_key: facet_keys::LIFECYCLE.to_string(),
                operator: FacetOperator::Eq,
                operand: FacetOperand::Scalar(text("Active")),
            }),
            FacetExpr::Predicate(FacetPredicate {
                facet_key: facet_keys::DOMAIN.to_string(),
                operator: FacetOperator::Eq,
                operand: FacetOperand::Scalar(text("example.com")),
            }),
        ]);
        let p_both = proj(&[
            (facet_keys::LIFECYCLE, FacetValue::Scalar(text("Active"))),
            (facet_keys::DOMAIN, FacetValue::Scalar(text("example.com"))),
        ]);
        let p_one = proj(&[
            (facet_keys::LIFECYCLE, FacetValue::Scalar(text("Active"))),
            (facet_keys::DOMAIN, FacetValue::Scalar(text("other.com"))),
        ]);
        assert_eq!(expr.evaluate(&p_both).unwrap(), true);
        assert_eq!(expr.evaluate(&p_one).unwrap(), false);
    }

    #[test]
    fn not_expr_inverts_predicate() {
        let expr = FacetExpr::Not(Box::new(FacetExpr::Predicate(FacetPredicate {
            facet_key: facet_keys::LIFECYCLE.to_string(),
            operator: FacetOperator::Eq,
            operand: FacetOperand::Scalar(text("Cold")),
        })));
        let p_cold = proj(&[(facet_keys::LIFECYCLE, FacetValue::Scalar(text("Cold")))]);
        let p_active = proj(&[(facet_keys::LIFECYCLE, FacetValue::Scalar(text("Active")))]);
        assert_eq!(expr.evaluate(&p_cold).unwrap(), false);
        assert_eq!(expr.evaluate(&p_active).unwrap(), true);
    }

    // Omnibar token parser tests
    #[test]
    fn omnibar_facet_token_parses_eq() {
        let pred = parse_omnibar_facet_token("facet:lifecycle=Active").unwrap();
        assert_eq!(pred.facet_key, "lifecycle");
        assert_eq!(pred.operator, FacetOperator::Eq);
        assert_eq!(pred.operand, FacetOperand::Scalar(text("Active")));
    }

    #[test]
    fn omnibar_facet_token_parses_not_eq() {
        let pred = parse_omnibar_facet_token("facet:!lifecycle=Cold").unwrap();
        assert_eq!(pred.operator, FacetOperator::NotEq);
        assert_eq!(pred.operand, FacetOperand::Scalar(text("Cold")));
    }

    #[test]
    fn omnibar_facet_token_parses_exists() {
        let pred = parse_omnibar_facet_token("facet:mime_hint").unwrap();
        assert_eq!(pred.facet_key, "mime_hint");
        assert_eq!(pred.operator, FacetOperator::Exists);
    }

    #[test]
    fn omnibar_facet_token_rejects_invalid_key() {
        // Non-namespaced extension key must be rejected
        assert!(parse_omnibar_facet_token("facet:custom_key=foo").is_none());
    }

    #[test]
    fn omnibar_facet_token_accepts_namespaced_extension() {
        let pred = parse_omnibar_facet_token("facet:myns:custom=foo").unwrap();
        assert_eq!(pred.facet_key, "myns:custom");
        assert_eq!(pred.operator, FacetOperator::Eq);
    }

    #[test]
    fn omnibar_token_without_prefix_returns_none() {
        assert!(parse_omnibar_facet_token("lifecycle=Active").is_none());
        assert!(parse_omnibar_facet_token("facet:").is_none());
    }
}
