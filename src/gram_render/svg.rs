// Golden-file test directory
#[cfg(test)]
const FIXTURE_DIR: &str = "tests/render_svg/fixtures";

use std::collections::HashMap;
use std::f64::consts::PI;

use geo::algorithm::ConvexHull;
use geo::{MultiPoint, Point};
use kurbo::{BezPath, PathEl, Point as KPoint};
use svg::node::element::{Circle, Group, Path as SvgPath, Text};
use svg::node::Node;
use svg::Document;

use super::graph::{GramEdge, GramGraph, GramNode, GramPath, PathMember};
use super::layout::{self, Vec2};

const NODE_RADIUS: f64 = 22.0;
const DEFLECTION_STEP: f64 = 30.0; // degrees per parallel-edge step
const MAX_DEFLECTION: f64 = 150.0; // degrees total spread cap
const SAGITTA_PER_DEG: f64 = 1.5; // pixels of arc height per degree of deflection
const SHAFT_R: f64 = 1.0;         // half of shaft width (2px total)
const HEAD_R: f64 = 4.0;          // half of head width (8px total)
const HEAD_HEIGHT: f64 = 8.0;
const PADDING: f64 = 44.0;        // NODE_RADIUS * 2

// Per-path HSL hues (distinct, semi-transparent fills)
const PATH_HUES: [u16; 6] = [200, 120, 40, 300, 0, 160];

pub fn render_svg(graph: &GramGraph) -> String {
    let layout = layout::compute(graph);
    let positions = &layout.positions;

    // Dynamic viewBox: bounding box of node centres + generous padding for arcs
    let view_box = compute_view_box(positions);

    let mut doc = Document::new()
        .set("xmlns", "http://www.w3.org/2000/svg")
        .set("viewBox", view_box)
        .set("font-family", "monospace")
        .set("font-size", "10");

    // ── Layer groups ────────────────────────────────────────────────────────────
    let mut envelopes = Group::new().set("id", "path-envelopes");
    let mut edges_g   = Group::new().set("id", "edges");
    let mut nodes_g   = Group::new().set("id", "nodes");
    let mut labels_g  = Group::new().set("id", "labels");

    // ── Path envelopes ──────────────────────────────────────────────────────────
    for (idx, path) in graph.paths.iter().enumerate() {
        if let Some(elem) = build_envelope(path, positions, idx) {
            envelopes = envelopes.add(elem);
        }
    }

    // ── Parallel-edge grouping ──────────────────────────────────────────────────
    let edge_info = group_parallel_edges(&graph.edges);

    // ── Edges ───────────────────────────────────────────────────────────────────
    for (i, edge) in graph.edges.iter().enumerate() {
        let sp = match positions.get(&edge.source) { Some(p) => *p, None => continue };
        let tp = match positions.get(&edge.target) { Some(p) => *p, None => continue };
        let (count, pos_in_group) = edge_info[i];
        let deflection = compute_deflection(count, pos_in_group);
        for elem in build_edge(edge, sp, tp, deflection) {
            edges_g = edges_g.add(elem);
        }
    }

    // ── Nodes ───────────────────────────────────────────────────────────────────
    for node in &graph.nodes {
        let pos = match positions.get(&node.id) { Some(p) => *p, None => continue };
        let (circle, label) = build_node(node, pos);
        nodes_g = nodes_g.add(circle);
        labels_g = labels_g.add(label);
    }

    doc = doc.add(envelopes).add(edges_g).add(nodes_g).add(labels_g);
    doc.to_string()
}

// ── ViewBox ─────────────────────────────────────────────────────────────────────

fn compute_view_box(positions: &HashMap<String, Vec2>) -> String {
    if positions.is_empty() {
        return "0 0 600 600".to_string();
    }
    let xs: Vec<f64> = positions.values().map(|p| p.x).collect();
    let ys: Vec<f64> = positions.values().map(|p| p.y).collect();
    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    // Include node radius + room for arc peaks (max sagitta = MAX_DEFLECTION * SAGITTA_PER_DEG)
    let arc_pad = MAX_DEFLECTION * SAGITTA_PER_DEG;
    let pad = NODE_RADIUS + arc_pad.max(PADDING);
    let vx = min_x - pad;
    let vy = min_y - pad;
    let vw = (max_x - min_x) + 2.0 * pad;
    let vh = (max_y - min_y) + 2.0 * pad;
    format!("{vx:.2} {vy:.2} {vw:.2} {vh:.2}")
}

// ── Parallel-edge grouping ───────────────────────────────────────────────────────

fn group_parallel_edges(edges: &[GramEdge]) -> Vec<(usize, usize)> {
    let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, edge) in edges.iter().enumerate() {
        let key = if edge.source <= edge.target {
            (edge.source.clone(), edge.target.clone())
        } else {
            (edge.target.clone(), edge.source.clone())
        };
        groups.entry(key).or_default().push(i);
    }
    let mut info = vec![(1usize, 0usize); edges.len()];
    for indices in groups.values() {
        let count = indices.len();
        for (pos, &i) in indices.iter().enumerate() {
            info[i] = (count, pos);
        }
    }
    info
}

fn compute_deflection(count: usize, position: usize) -> f64 {
    if count <= 1 {
        return 0.0;
    }
    let total = DEFLECTION_STEP * (count - 1) as f64;
    let step = if total > MAX_DEFLECTION {
        MAX_DEFLECTION / (count - 1) as f64
    } else {
        DEFLECTION_STEP
    };
    -total / 2.0 + position as f64 * step
}

// ── Arc geometry ─────────────────────────────────────────────────────────────────

struct ArcParams {
    arc_cx: f64, arc_cy: f64,
    r: f64,
    t1_angle: f64, t2_angle: f64,
    sweep: f64,
    sweep_flag: u8, large_arc: u8,
    t2x: f64, t2y: f64,
    tx: f64, ty: f64, // tangent at T2 (for arrowhead orientation)
    px: f64, py: f64, // outward radius at T2
}

fn compute_arc_params(
    sp: Vec2, tp: Vec2,
    natural_angle: f64, centre_distance: f64,
    deflection_deg: f64,
) -> Option<ArcParams> {
    let sagitta = deflection_deg * SAGITTA_PER_DEG;
    let abs_s = sagitta.abs();
    if abs_s < 0.5 {
        return None;
    }

    let perp_x = -natural_angle.sin();
    let perp_y = natural_angle.cos();
    let half_chord = centre_distance / 2.0;
    let r = (half_chord * half_chord + abs_s * abs_s) / (2.0 * abs_s);
    let sign = if sagitta > 0.0 { 1.0_f64 } else { -1.0_f64 };

    let mid_x = (sp.x + tp.x) / 2.0;
    let mid_y = (sp.y + tp.y) / 2.0;
    let arc_cx = mid_x - sign * perp_x * (r - abs_s);
    let arc_cy = mid_y - sign * perp_y * (r - abs_s);

    // Angles from arc centre to node centres (arc passes through both)
    let alpha_s = (sp.y - arc_cy).atan2(sp.x - arc_cx);
    let alpha_t = (tp.y - arc_cy).atan2(tp.x - arc_cx);

    // Angular step from each node centre to its attachment point on the arc
    let cos_arg = (1.0 - (NODE_RADIUS * NODE_RADIUS) / (2.0 * r * r)).clamp(-1.0, 1.0);
    let delta = cos_arg.acos();

    // Determine sweep direction
    let mut raw_sweep = alpha_t - alpha_s;
    while raw_sweep > PI  { raw_sweep -= 2.0 * PI; }
    while raw_sweep < -PI { raw_sweep += 2.0 * PI; }
    if sign > 0.0 && raw_sweep > 0.0 { raw_sweep -= 2.0 * PI; }
    if sign < 0.0 && raw_sweep < 0.0 { raw_sweep += 2.0 * PI; }
    let sweep_dir = if raw_sweep > 0.0 { 1.0_f64 } else { -1.0_f64 };

    let t1_angle = alpha_s + sweep_dir * delta;
    let t2_angle = alpha_t - sweep_dir * delta;
    let t2x = arc_cx + r * t2_angle.cos();
    let t2y = arc_cy + r * t2_angle.sin();

    let mut sweep = t2_angle - t1_angle;
    if sweep_dir > 0.0 { while sweep < 0.0 { sweep += 2.0 * PI; } }
    else               { while sweep > 0.0 { sweep -= 2.0 * PI; } }

    let sweep_flag = if sweep_dir > 0.0 { 1u8 } else { 0u8 };
    let large_arc  = if sweep.abs() > PI { 1u8 } else { 0u8 };

    let cos_e = t2_angle.cos();
    let sin_e = t2_angle.sin();
    let tx = if sweep_flag == 1 {  sin_e } else { -sin_e };
    let ty = if sweep_flag == 1 { -cos_e } else {  cos_e };

    Some(ArcParams {
        arc_cx, arc_cy, r,
        t1_angle, t2_angle, sweep,
        sweep_flag, large_arc,
        t2x, t2y,
        tx, ty, px: cos_e, py: sin_e,
    })
}

// ── Edge builder ─────────────────────────────────────────────────────────────────

fn build_edge(edge: &GramEdge, sp: Vec2, tp: Vec2, deflection_deg: f64) -> Vec<Box<dyn Node>> {
    let dx = tp.x - sp.x;
    let dy = tp.y - sp.y;
    let centre_distance = (dx * dx + dy * dy).sqrt().max(1.0);
    let natural_angle = dy.atan2(dx);

    match compute_arc_params(sp, tp, natural_angle, centre_distance, deflection_deg) {
        Some(arc) => build_arc_edge(edge, &arc, natural_angle),
        None      => build_straight_edge(edge, sp, tp, natural_angle),
    }
}

fn build_straight_edge(
    edge: &GramEdge, sp: Vec2, tp: Vec2, natural_angle: f64,
) -> Vec<Box<dyn Node>> {
    let mut elems: Vec<Box<dyn Node>> = Vec::new();
    let cos_a = natural_angle.cos();
    let sin_a = natural_angle.sin();
    let perp_x = -sin_a;
    let perp_y =  cos_a;

    // Attachment points on node surfaces
    let x1 = sp.x + cos_a * NODE_RADIUS;
    let y1 = sp.y + sin_a * NODE_RADIUS;
    let x2 = tp.x - cos_a * NODE_RADIUS;
    let y2 = tp.y - sin_a * NODE_RADIUS;

    let path_d = if edge.directed {
        // Shaft end stepped back from arrowhead tip
        let sx = x2 - cos_a * HEAD_HEIGHT;
        let sy = y2 - sin_a * HEAD_HEIGHT;
        format!(
            "M {:.2},{:.2} L {:.2},{:.2} L {:.2},{:.2} L {:.2},{:.2} L {:.2},{:.2} L {:.2},{:.2} L {:.2},{:.2} Z",
            x1 - perp_x * SHAFT_R, y1 - perp_y * SHAFT_R,
            sx - perp_x * SHAFT_R, sy - perp_y * SHAFT_R,
            sx - perp_x * HEAD_R,  sy - perp_y * HEAD_R,
            x2, y2,
            sx + perp_x * HEAD_R,  sy + perp_y * HEAD_R,
            sx + perp_x * SHAFT_R, sy + perp_y * SHAFT_R,
            x1 + perp_x * SHAFT_R, y1 + perp_y * SHAFT_R,
        )
    } else {
        format!(
            "M {:.2},{:.2} L {:.2},{:.2} L {:.2},{:.2} L {:.2},{:.2} Z",
            x1 - perp_x * SHAFT_R, y1 - perp_y * SHAFT_R,
            x2 - perp_x * SHAFT_R, y2 - perp_y * SHAFT_R,
            x2 + perp_x * SHAFT_R, y2 + perp_y * SHAFT_R,
            x1 + perp_x * SHAFT_R, y1 + perp_y * SHAFT_R,
        )
    };
    elems.push(Box::new(
        SvgPath::new().set("d", path_d).set("fill", "#aaa").set("stroke", "none"),
    ));

    if let Some(ref lbl) = edge.label {
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;
        let deg = natural_angle.to_degrees();
        let deg = if deg > 90.0 || deg < -90.0 { deg + 180.0 } else { deg };
        elems.push(Box::new(
            Text::new(lbl.clone())
                .set("x", mx)
                .set("y", my)
                .set("text-anchor", "middle")
                .set("fill", "#888")
                .set("transform", format!("rotate({deg:.1},{mx:.2},{my:.2})")),
        ));
    }
    elems
}

fn build_arc_edge(edge: &GramEdge, arc: &ArcParams, natural_angle: f64) -> Vec<Box<dyn Node>> {
    let mut elems: Vec<Box<dyn Node>> = Vec::new();
    let r = arc.r;
    let r_outer = r + SHAFT_R;
    let r_inner = (r - SHAFT_R).max(0.0);
    let step_angle = HEAD_HEIGHT / r;

    let shaft_end_angle = if arc.sweep_flag == 1 {
        arc.t2_angle - step_angle
    } else {
        arc.t2_angle + step_angle
    };

    let osx = arc.arc_cx + r_outer * arc.t1_angle.cos();
    let osy = arc.arc_cy + r_outer * arc.t1_angle.sin();
    let isx = arc.arc_cx + r_inner * arc.t1_angle.cos();
    let isy = arc.arc_cy + r_inner * arc.t1_angle.sin();
    let oex = arc.arc_cx + r_outer * shaft_end_angle.cos();
    let oey = arc.arc_cy + r_outer * shaft_end_angle.sin();
    let iex = arc.arc_cx + r_inner * shaft_end_angle.cos();
    let iey = arc.arc_cy + r_inner * shaft_end_angle.sin();

    let sf  = arc.sweep_flag;
    let isf = 1u8 - sf;
    let la  = arc.large_arc;
    let px = arc.px; let py = arc.py;
    let tx = arc.tx; let ty = arc.ty;
    let hr  = HEAD_R - SHAFT_R; // extra spread for arrowhead flanks

    let path_d = if edge.directed {
        format!(
            "M {:.2},{:.2} A {:.2},{:.2} 0 {la} {sf} {:.2},{:.2} \
             L {:.2},{:.2} L {:.2},{:.2} L {:.2},{:.2} \
             L {:.2},{:.2} A {:.2},{:.2} 0 {la} {isf} {:.2},{:.2} Z",
            osx, osy,
            r_outer, r_outer, oex, oey,
            oex + (px - tx) * hr, oey + (py - ty) * hr,
            arc.t2x, arc.t2y,
            iex - (px + tx) * hr, iey - (py + ty) * hr,
            iex, iey,
            r_inner, r_inner, isx, isy,
        )
    } else {
        format!(
            "M {:.2},{:.2} A {:.2},{:.2} 0 {la} {sf} {:.2},{:.2} \
             L {:.2},{:.2} A {:.2},{:.2} 0 {la} {isf} {:.2},{:.2} Z",
            osx, osy,
            r_outer, r_outer, oex, oey,
            iex, iey,
            r_inner, r_inner, isx, isy,
        )
    };
    elems.push(Box::new(
        SvgPath::new().set("d", path_d).set("fill", "#aaa").set("stroke", "none"),
    ));

    if let Some(ref lbl) = edge.label {
        let mid_angle = arc.t1_angle + arc.sweep * 0.5;
        let mid_x = arc.arc_cx + r * mid_angle.cos();
        let mid_y = arc.arc_cy + r * mid_angle.sin();
        let deg = natural_angle.to_degrees();
        let deg = if deg > 90.0 || deg < -90.0 { deg + 180.0 } else { deg };
        elems.push(Box::new(
            Text::new(lbl.clone())
                .set("x", mid_x)
                .set("y", mid_y)
                .set("text-anchor", "middle")
                .set("fill", "#888")
                .set("transform", format!("rotate({deg:.1},{mid_x:.2},{mid_y:.2})")),
        ));
    }
    elems
}

// ── Node builder ─────────────────────────────────────────────────────────────────

fn build_node(node: &GramNode, pos: Vec2) -> (Circle, Text) {
    let fill   = if node.is_nested { "#e8e8f8" } else { "#e8f4fb" };
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
        .set("fill", "#333");

    (circle, label)
}

// ── Envelope builder ─────────────────────────────────────────────────────────────

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
            PathMember::Edge(_)  => None,
        })
        .collect();

    if pts.len() < 2 {
        return None;
    }

    let geo_pts: MultiPoint<f64> =
        MultiPoint::from(pts.iter().map(|p| Point::new(p.x, p.y)).collect::<Vec<_>>());
    let hull = geo_pts.convex_hull();
    let hull_pts: Vec<Vec2> = hull.exterior().coords().map(|c| Vec2 { x: c.x, y: c.y }).collect();

    if hull_pts.len() < 2 {
        return None;
    }

    let cx = hull_pts.iter().map(|p| p.x).sum::<f64>() / hull_pts.len() as f64;
    let cy = hull_pts.iter().map(|p| p.y).sum::<f64>() / hull_pts.len() as f64;
    const ENVELOPE_PAD: f64 = 32.0;
    let expanded: Vec<KPoint> = hull_pts
        .iter()
        .map(|p| {
            let dx = p.x - cx;
            let dy = p.y - cy;
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            KPoint::new(cx + (len + ENVELOPE_PAD) * dx / len, cy + (len + ENVELOPE_PAD) * dy / len)
        })
        .collect();

    let path_data = rounded_poly(&expanded);
    let hue    = PATH_HUES[idx % PATH_HUES.len()];
    let fill   = format!("hsla({hue},65%,60%,0.18)");
    let stroke = format!("hsla({hue},65%,45%,0.5)");

    let mut group = Group::new();
    group = group.add(
        SvgPath::new()
            .set("d", path_data)
            .set("fill", fill)
            .set("stroke", stroke)
            .set("stroke-width", 1.5),
    );
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

fn rounded_poly(pts: &[KPoint]) -> String {
    if pts.is_empty() { return String::new(); }
    let n = pts.len();
    if n == 1 { return format!("M {},{} Z", pts[0].x, pts[0].y); }
    if n == 2 { return format!("M {},{} L {},{} Z", pts[0].x, pts[0].y, pts[1].x, pts[1].y); }

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
            PathEl::MoveTo(p)       => d.push_str(&format!("M {:.2},{:.2} ", p.x, p.y)),
            PathEl::LineTo(p)       => d.push_str(&format!("L {:.2},{:.2} ", p.x, p.y)),
            PathEl::QuadTo(c, p)    => d.push_str(&format!("Q {:.2},{:.2} {:.2},{:.2} ", c.x, c.y, p.x, p.y)),
            PathEl::CurveTo(c1, c2, p) => d.push_str(&format!(
                "C {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} ", c1.x, c1.y, c2.x, c2.y, p.x, p.y
            )),
            PathEl::ClosePath       => d.push('Z'),
        }
    }
    d.trim_end().to_string()
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
