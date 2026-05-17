use std::collections::HashMap;
use std::fmt;

use gram_codec::parse_gram as codec_parse;
use pattern_core::{Pattern, Subject};

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct GramGraph {
    pub nodes: Vec<GramNode>,
    pub edges: Vec<GramEdge>,
    pub paths: Vec<GramPath>,
    pub nested: Vec<NestingRelation>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GramNode {
    pub id: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
    pub is_nested: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GramEdge {
    pub id: Option<String>,
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub properties: HashMap<String, serde_json::Value>,
    pub directed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GramPath {
    pub id: Option<String>,
    pub members: Vec<PathMember>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", content = "id")]
pub enum PathMember {
    Node(String),
    Edge(String),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NestingRelation {
    pub parent: String,
    pub children: Vec<String>,
}

#[derive(Debug)]
pub enum RenderError {
    ParseError(String),
    EmptyGraph,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::ParseError(msg) => write!(f, "parse error: {msg}"),
            RenderError::EmptyGraph => write!(f, "gram file contains no nodes"),
        }
    }
}

impl std::error::Error for RenderError {}

// ── Public API ─────────────────────────────────────────────────────────────────

pub fn parse_gram(source: &str) -> Result<GramGraph, RenderError> {
    let patterns = codec_parse(source).map_err(|e| RenderError::ParseError(e.to_string()))?;

    let mut builder = GraphBuilder::default();
    for pattern in &patterns {
        builder.walk(pattern, None);
    }

    let graph = builder.finish();
    if graph.nodes.is_empty() {
        return Err(RenderError::EmptyGraph);
    }
    Ok(graph)
}

// ── Internal graph builder ─────────────────────────────────────────────────────

#[derive(Default)]
struct GraphBuilder {
    nodes: HashMap<String, GramNode>,
    edges: Vec<GramEdge>,
    paths: Vec<GramPath>,
    nested: Vec<NestingRelation>,
    anon_counter: usize,
    edge_counter: usize,
}

impl GraphBuilder {
    fn finish(self) -> GramGraph {
        let mut nodes: Vec<GramNode> = self.nodes.into_values().collect();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        GramGraph {
            nodes,
            edges: self.edges,
            paths: self.paths,
            nested: self.nested,
        }
    }

    fn anon_node_id(&mut self) -> String {
        let id = format!("_n{}", self.anon_counter);
        self.anon_counter += 1;
        id
    }

    fn anon_edge_id(&mut self) -> String {
        let id = format!("_e{}", self.edge_counter);
        self.edge_counter += 1;
        id
    }

    fn ensure_node(&mut self, pattern: &Pattern<Subject>) -> String {
        let subject = pattern.value();
        let raw_id = subject.identity.0.clone();
        let id = if raw_id.is_empty() {
            self.anon_node_id()
        } else {
            raw_id
        };

        self.nodes.entry(id.clone()).or_insert_with(|| GramNode {
            id: id.clone(),
            labels: subject.labels.iter().cloned().collect(),
            properties: convert_properties(&subject.properties),
            is_nested: false,
        });

        id
    }

    /// Walk a pattern, adding nodes/edges/paths to the builder.
    /// `path_ctx` is Some(path_id) when we're inside a named subject_pattern.
    fn walk(&mut self, pattern: &Pattern<Subject>, path_ctx: Option<&str>) {
        let elements = pattern.elements();
        let subject = pattern.value();
        let identity = &subject.identity.0;

        if elements.is_empty() {
            // Atomic: standalone node
            if !identity.is_empty() {
                self.ensure_node(pattern);
                if let Some(pid) = path_ctx {
                    self.add_node_to_path(pid, identity.clone());
                }
            }
            return;
        }

        // Non-atomic: relationship or named group
        if elements.len() == 2 {
            self.walk_relationship(pattern, path_ctx);
            return;
        }

        // Named group / subject_pattern with multiple elements
        if !identity.is_empty() {
            let path_id = identity.clone();
            self.paths.push(GramPath {
                id: Some(path_id.clone()),
                members: vec![],
            });
            for elem in elements {
                self.walk(elem, Some(&path_id));
            }
        } else {
            for elem in elements {
                self.walk(elem, path_ctx);
            }
        }
    }

    fn walk_relationship(&mut self, pattern: &Pattern<Subject>, path_ctx: Option<&str>) {
        let elements = pattern.elements();
        let edge_subject = pattern.value();
        let edge_raw_id = edge_subject.identity.0.clone();

        // gram-codec left-associates chains: outer edge is outermost pattern.
        // elements[0] = left side — either a leaf node or a nested (inner) relationship.
        // elements[1] = right side — always a leaf node.

        // Ensure target node (always a leaf on the right)
        let target_id = self.ensure_node(&elements[1]);

        // Determine source: if elements[0] is atomic it's a node; otherwise walk the
        // inner chain first and use its right leaf as the source of this edge.
        let source_id = if elements[0].elements().is_empty() {
            self.ensure_node(&elements[0])
        } else {
            // Walk the inner chain to add its nodes/edges
            self.walk_relationship(&elements[0], path_ctx);
            // The source of this edge is the rightmost leaf of the inner chain
            let leaf = Self::rightmost_leaf_of(&elements[0]);
            self.ensure_node(leaf)
        };

        let edge_id = if edge_raw_id.is_empty() {
            Some(self.anon_edge_id())
        } else {
            Some(edge_raw_id)
        };

        let label = edge_subject.labels.iter().next().cloned();

        self.edges.push(GramEdge {
            id: edge_id.clone(),
            source: source_id.clone(),
            target: target_id.clone(),
            label,
            properties: convert_properties(&edge_subject.properties),
            directed: true, // gram-codec normalizes direction but doesn't preserve it
        });

        if let Some(pid) = path_ctx {
            self.add_node_to_path(pid, source_id);
            self.add_node_to_path(pid, target_id);
            if let Some(eid) = edge_id {
                self.add_edge_to_path(pid, eid);
            }
        }
    }

    /// Returns the rightmost leaf (atomic) node in a left-associative chain.
    fn rightmost_leaf_of(pattern: &Pattern<Subject>) -> &Pattern<Subject> {
        if pattern.elements().is_empty() {
            pattern
        } else {
            Self::rightmost_leaf_of(&pattern.elements()[pattern.elements().len() - 1])
        }
    }

    fn find_path_mut(&mut self, path_id: &str) -> Option<&mut GramPath> {
        self.paths
            .iter_mut()
            .rev()
            .find(|p| p.id.as_deref() == Some(path_id))
    }

    fn add_node_to_path(&mut self, path_id: &str, node_id: String) {
        if let Some(path) = self.find_path_mut(path_id) {
            let already = path
                .members
                .iter()
                .any(|m| matches!(m, PathMember::Node(id) if id == &node_id));
            if !already {
                path.members.push(PathMember::Node(node_id));
            }
        }
    }

    fn add_edge_to_path(&mut self, path_id: &str, edge_id: String) {
        if let Some(path) = self.find_path_mut(path_id) {
            path.members.push(PathMember::Edge(edge_id));
        }
    }
}

// ── Value conversion ───────────────────────────────────────────────────────────

fn convert_properties(
    props: &HashMap<String, pattern_core::Value>,
) -> HashMap<String, serde_json::Value> {
    props
        .iter()
        .map(|(k, v)| (k.clone(), convert_value(v)))
        .collect()
}

fn convert_value(v: &pattern_core::Value) -> serde_json::Value {
    use pattern_core::Value;
    use serde_json::json;
    match v {
        Value::VString(s) | Value::VSymbol(s) => serde_json::Value::String(s.clone()),
        Value::VInteger(n) => json!(n),
        Value::VDecimal(d) => json!(d),
        Value::VBoolean(b) => json!(b),
        Value::VArray(arr) => serde_json::Value::Array(arr.iter().map(convert_value).collect()),
        Value::VMap(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), convert_value(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        Value::VRange(r) => {
            json!({
                "lower": r.lower,
                "upper": r.upper,
            })
        }
        Value::VTaggedString { tag, content } => json!({ "tag": tag, "value": content }),
        Value::VMeasurement { unit, value } => json!({ "unit": unit, "value": value }),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_node() {
        let g = parse_gram("(alice:Person {name: \"Alice\"})").unwrap();
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.nodes[0].id, "alice");
        assert!(g.nodes[0].labels.contains(&"Person".to_string()));
    }

    #[test]
    fn parses_directed_edge() {
        let g = parse_gram("(alice)-[:KNOWS]->(bob)").unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].source, "alice");
        assert_eq!(g.edges[0].target, "bob");
        assert_eq!(g.edges[0].label.as_deref(), Some("KNOWS"));
    }

    #[test]
    fn parses_chained_path() {
        let g = parse_gram("(a)-[:R1]->(b)-[:R2]->(c)").unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    fn empty_gram_returns_error() {
        let result = parse_gram("// just a comment");
        assert!(matches!(result, Err(RenderError::EmptyGraph)));
    }
}
