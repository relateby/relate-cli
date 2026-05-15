// Golden-file test directory
#[cfg(test)]
const FIXTURE_DIR: &str = "tests/render_svg/fixtures";

use std::collections::HashMap;

use geo::algorithm::ConvexHull;
use geo::{MultiPoint, Point};
use kurbo::{BezPath, PathEl, Point as KPoint};
use svg::node::element::{Circle, Definitions, Group, Marker, Path as SvgPath, Text};
use svg::node::Node;
use svg::Document;

use super::graph::{GramEdge, GramGraph, GramNode, GramPath, PathMember};
use super::layout::{self, Vec2};

const NODE_RADIUS: f64 = 22.0;
const ENVELOPE_PAD: f64 = 32.0;
const CANVAS: f64 = 600.0;

// Per-path HSL hues (distinct, semi-transparent fills)
const PATH_HUES: [u16; 6] = [200, 120, 40, 300, 0, 160];

pub fn render_svg(graph: &GramGraph) -> String {
    let layout = layout::compute(graph);
    let positions = &layout.positions;

    let mut doc = Document::new()
        .set("xmlns", "http://www.w3.org/2000/svg")
        .set("viewBox", format!("0 0 {CANVAS} {CANVAS}"))
        .set("font-family", "monospace")
        .set("font-size", "10");

    // ── Arrowhead marker ────────────────────────────────────────────────────────
    let arrow_marker = Marker::new()
        .set("id", "arrow")
        .set("markerWidth", 8)
        .set("markerHeight", 8)
        .set("refX", 6)
        .set("refY", 3)
        .set("orient", "auto")
        .add(
            SvgPath::new()
                .set("d", "M0,0 L0,6 L8,3 z")
                .set("fill", "#aaa"),
        );

    let defs = Definitions::new().add(arrow_marker);
    doc = doc.add(defs);

    // ── Layer groups ────────────────────────────────────────────────────────────
    let mut envelopes = Group::new().set("id", "path-envelopes");
    let mut edges_g = Group::new().set("id", "edges");
    let mut nodes_g = Group::new().set("id", "nodes");
    let mut labels_g = Group::new().set("id", "labels");

    // ── Path envelopes ──────────────────────────────────────────────────────────
    for (idx, path) in graph.paths.iter().enumerate() {
        if let Some(elem) = build_envelope(path, positions, idx) {
            envelopes = envelopes.add(elem);
        }
    }

    // ── Edges ───────────────────────────────────────────────────────────────────
    for edge in &graph.edges {
        let sp = match positions.get(&edge.source) {
            Some(p) => *p,
            None => continue,
        };
        let tp = match positions.get(&edge.target) {
            Some(p) => *p,
            None => continue,
        };
        let edge_elems = build_edge(edge, sp, tp);
        for e in edge_elems {
            edges_g = edges_g.add(e);
        }
    }

    // ── Nodes ───────────────────────────────────────────────────────────────────
    for node in &graph.nodes {
        let pos = match positions.get(&node.id) {
            Some(p) => *p,
            None => continue,
        };
        let (circle, label) = build_node(node, pos);
        nodes_g = nodes_g.add(circle);
        labels_g = labels_g.add(label);
    }

    doc = doc
        .add(envelopes)
        .add(edges_g)
        .add(nodes_g)
        .add(labels_g);

    doc.to_string()
}

// ── Envelope builder ───────────────────────────────────────────────────────────

fn build_envelope(
    path: &GramPath,
    positions: &HashMap<String, Vec2>,
    idx: usize,
) -> Option<Group> {
    let pts: Vec<Vec2> = path
        .members
        .iter()
        .filter_map(|m| match m {
            PathMember::Node(id) => positions.get(id).copied(),
            PathMember::Edge(_) => None,
        })
        .collect();

    if pts.len() < 2 {
        return None;
    }

    // Build geo MultiPoint for convex hull
    let geo_pts: MultiPoint<f64> = MultiPoint::from(
        pts.iter()
            .map(|p| Point::new(p.x, p.y))
            .collect::<Vec<_>>(),
    );
    let hull = geo_pts.convex_hull();
    let hull_pts: Vec<Vec2> = hull
        .exterior()
        .coords()
        .map(|c| Vec2 { x: c.x, y: c.y })
        .collect();

    if hull_pts.len() < 2 {
        return None;
    }

    // Expand hull outward from centroid
    let cx = hull_pts.iter().map(|p| p.x).sum::<f64>() / hull_pts.len() as f64;
    let cy = hull_pts.iter().map(|p| p.y).sum::<f64>() / hull_pts.len() as f64;
    let expanded: Vec<KPoint> = hull_pts
        .iter()
        .map(|p| {
            let dx = p.x - cx;
            let dy = p.y - cy;
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            KPoint::new(cx + (len + ENVELOPE_PAD) * dx / len, cy + (len + ENVELOPE_PAD) * dy / len)
        })
        .collect();

    // Build Bézier-rounded closed path
    let path_data = rounded_poly(&expanded);

    let hue = PATH_HUES[idx % PATH_HUES.len()];
    let fill = format!("hsla({hue},65%,60%,0.18)");
    let stroke = format!("hsla({hue},65%,45%,0.5)");

    let mut group = Group::new();
    group = group.add(
        SvgPath::new()
            .set("d", path_data)
            .set("fill", fill)
            .set("stroke", stroke)
            .set("stroke-width", 1.5),
    );

    // Path label
    if let Some(ref pid) = path.id {
        group = group.add(
            Text::new(pid.clone())
                .set("x", cx)
                .set("y", cy - ENVELOPE_PAD - 6.0)
                .set("text-anchor", "middle")
                .set("font-size", 11)
                .set("fill", "#666"),
        );
    }

    Some(group)
}

/// Build a closed Bézier path through `pts` with rounded corners.
fn rounded_poly(pts: &[KPoint]) -> String {
    if pts.is_empty() {
        return String::new();
    }
    let n = pts.len();
    if n == 1 {
        return format!("M {},{} Z", pts[0].x, pts[0].y);
    }
    if n == 2 {
        return format!("M {},{} L {},{} Z", pts[0].x, pts[0].y, pts[1].x, pts[1].y);
    }

    let mut path = BezPath::new();
    let mid = |a: KPoint, b: KPoint| KPoint::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);

    let start = mid(pts[n - 1], pts[0]);
    path.push(PathEl::MoveTo(start));

    for i in 0..n {
        let p = pts[i];
        let next = pts[(i + 1) % n];
        let m = mid(p, next);
        path.push(PathEl::QuadTo(p, m));
    }
    path.push(PathEl::ClosePath);

    path_to_svg_d(&path)
}

fn path_to_svg_d(path: &BezPath) -> String {
    let mut d = String::new();
    for el in path.iter() {
        match el {
            PathEl::MoveTo(p) => d.push_str(&format!("M {:.2},{:.2} ", p.x, p.y)),
            PathEl::LineTo(p) => d.push_str(&format!("L {:.2},{:.2} ", p.x, p.y)),
            PathEl::QuadTo(c, p) => {
                d.push_str(&format!("Q {:.2},{:.2} {:.2},{:.2} ", c.x, c.y, p.x, p.y))
            }
            PathEl::CurveTo(c1, c2, p) => d.push_str(&format!(
                "C {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} ",
                c1.x, c1.y, c2.x, c2.y, p.x, p.y
            )),
            PathEl::ClosePath => d.push('Z'),
        }
    }
    d.trim_end().to_string()
}

// ── Edge builder ───────────────────────────────────────────────────────────────

fn build_edge(edge: &GramEdge, sp: Vec2, tp: Vec2) -> Vec<Box<dyn Node>> {
    let mut elems: Vec<Box<dyn Node>> = Vec::new();

    let dx = tp.x - sp.x;
    let dy = tp.y - sp.y;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let ux = dx / len;
    let uy = dy / len;

    // Shorten to node boundary
    let x1 = sp.x + ux * NODE_RADIUS;
    let y1 = sp.y + uy * NODE_RADIUS;
    let x2 = tp.x - ux * NODE_RADIUS;
    let y2 = tp.y - uy * NODE_RADIUS;

    let mut line = SvgPath::new()
        .set("d", format!("M {x1:.2},{y1:.2} L {x2:.2},{y2:.2}"))
        .set("stroke", "#aaa")
        .set("stroke-width", 1.5)
        .set("fill", "none");

    if edge.directed {
        line = line.set("marker-end", "url(#arrow)");
    }
    elems.push(Box::new(line));

    // Edge label at midpoint
    if let Some(ref lbl) = edge.label {
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;
        let perp_x = -uy * 8.0;
        let perp_y = ux * 8.0;
        elems.push(Box::new(
            Text::new(lbl.clone())
                .set("x", mx + perp_x)
                .set("y", my + perp_y)
                .set("text-anchor", "middle")
                .set("font-size", 10)
                .set("fill", "#888"),
        ));
    }

    elems
}

// ── Node builder ───────────────────────────────────────────────────────────────

fn build_node(node: &GramNode, pos: Vec2) -> (Circle, Text) {
    let fill = if node.is_nested { "#e8e8f8" } else { "#e8f4fb" };
    let stroke = if node.is_nested { "#9999cc" } else { "#3399cc" };

    let circle = Circle::new()
        .set("cx", pos.x)
        .set("cy", pos.y)
        .set("r", NODE_RADIUS)
        .set("fill", fill)
        .set("stroke", stroke)
        .set("stroke-width", 1.5);

    let label_text = if let Some(lbl) = node.labels.first() {
        format!("{}:{}", node.id, lbl)
    } else {
        node.id.clone()
    };

    let label = Text::new(label_text)
        .set("x", pos.x)
        .set("y", pos.y + 4.0)
        .set("text-anchor", "middle")
        .set("font-size", 10)
        .set("fill", "#333");

    (circle, label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gram_render::graph::parse_gram;
    use std::path::Path;

    fn render_fixture(name: &str) -> String {
        let gram_path = format!("{FIXTURE_DIR}/{name}.gram");
        let source = std::fs::read_to_string(&gram_path)
            .unwrap_or_else(|_| panic!("cannot read {gram_path}"));
        let graph = parse_gram(&source)
            .unwrap_or_else(|e| panic!("parse failed for {gram_path}: {e}"));
        render_svg(&graph)
    }

    fn assert_golden(name: &str) {
        let svg = render_fixture(name);
        let golden = format!("{FIXTURE_DIR}/{name}.svg");

        if !Path::new(&golden).exists() || std::env::var("UPDATE_GOLDEN").is_ok() {
            std::fs::write(&golden, &svg)
                .unwrap_or_else(|e| panic!("cannot write golden {golden}: {e}"));
            return;
        }

        let expected = std::fs::read_to_string(&golden)
            .unwrap_or_else(|e| panic!("cannot read golden {golden}: {e}"));
        assert_eq!(svg, expected, "SVG for {name} differs from golden file");
    }

    #[test]
    fn golden_simple_graph() {
        assert_golden("simple");
    }

    #[test]
    fn golden_graph_with_named_path() {
        assert_golden("with_path");
    }

    #[test]
    fn deterministic_render() {
        let svg1 = render_fixture("simple");
        let svg2 = render_fixture("simple");
        assert_eq!(svg1, svg2, "render_svg must be deterministic");
    }
}
