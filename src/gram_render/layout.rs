use std::collections::HashMap;

use super::graph::GramGraph;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct LayoutResult {
    pub positions: HashMap<String, Vec2>,
    pub bounds: Bounds,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

// ── Spring embedder ────────────────────────────────────────────────────────────

const CANVAS: f64 = 600.0;
const PADDING: f64 = 40.0;
const ITERATIONS: usize = 500;
const K_REPULSION: f64 = 8000.0;
const K_ATTRACTION: f64 = 0.06;
const REST_LENGTH: f64 = 100.0;
const CENTER_FORCE: f64 = 0.015;
const COOLING_START: f64 = 1.0;
const COOLING_END: f64 = 0.05;

pub fn compute(graph: &GramGraph) -> LayoutResult {
    let node_ids: Vec<String> = graph.nodes.iter().map(|n| n.id.clone()).collect();
    let n = node_ids.len();

    if n == 0 {
        return LayoutResult {
            positions: HashMap::new(),
            bounds: Bounds {
                x: 0.0,
                y: 0.0,
                w: CANVAS,
                h: CANVAS,
            },
        };
    }

    // Initial circular placement — deterministic, index-based
    let mut pos: Vec<Vec2> = (0..n)
        .map(|i| {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            let r = CANVAS * 0.35;
            Vec2 {
                x: CANVAS / 2.0 + r * angle.cos(),
                y: CANVAS / 2.0 + r * angle.sin(),
            }
        })
        .collect();

    // Build adjacency from edge list
    let index: HashMap<&str, usize> = node_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    let adj: Vec<(usize, usize)> = graph
        .edges
        .iter()
        .filter_map(|e| {
            let s = index.get(e.source.as_str())?;
            let t = index.get(e.target.as_str())?;
            Some((*s, *t))
        })
        .collect();

    // Force iterations with linear cooling
    let mut vel: Vec<Vec2> = vec![Vec2 { x: 0.0, y: 0.0 }; n];

    for step in 0..ITERATIONS {
        let t = step as f64 / ITERATIONS as f64;
        let cooling = COOLING_START + t * (COOLING_END - COOLING_START);

        let mut force: Vec<Vec2> = vec![Vec2 { x: 0.0, y: 0.0 }; n];

        // Coulomb repulsion between all pairs
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[j].x - pos[i].x;
                let dy = pos[j].y - pos[i].y;
                let dist2 = (dx * dx + dy * dy).max(1.0);
                let dist = dist2.sqrt();
                let mag = K_REPULSION / dist2;
                let fx = mag * dx / dist;
                let fy = mag * dy / dist;
                force[i].x -= fx;
                force[i].y -= fy;
                force[j].x += fx;
                force[j].y += fy;
            }
        }

        // Hooke attraction along edges
        for &(s, t) in &adj {
            let dx = pos[t].x - pos[s].x;
            let dy = pos[t].y - pos[s].y;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let stretch = dist - REST_LENGTH;
            let mag = K_ATTRACTION * stretch;
            let fx = mag * dx / dist;
            let fy = mag * dy / dist;
            force[s].x += fx;
            force[s].y += fy;
            force[t].x -= fx;
            force[t].y -= fy;
        }

        // Centering force toward canvas center
        for i in 0..n {
            force[i].x += CENTER_FORCE * (CANVAS / 2.0 - pos[i].x);
            force[i].y += CENTER_FORCE * (CANVAS / 2.0 - pos[i].y);
        }

        // Integrate with cooling
        for i in 0..n {
            vel[i].x = (vel[i].x + force[i].x) * cooling;
            vel[i].y = (vel[i].y + force[i].y) * cooling;
            pos[i].x += vel[i].x;
            pos[i].y += vel[i].y;
        }
    }

    // Normalize to fit [PADDING, CANVAS-PADDING]²
    let usable = CANVAS - 2.0 * PADDING;
    let min_x = pos.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_x = pos.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = pos.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = pos.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);

    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);
    let scale = (usable / span_x).min(usable / span_y);

    let positions: HashMap<String, Vec2> = node_ids
        .iter()
        .zip(pos.iter())
        .map(|(id, p)| {
            (
                id.clone(),
                Vec2 {
                    x: PADDING + (p.x - min_x) * scale,
                    y: PADDING + (p.y - min_y) * scale,
                },
            )
        })
        .collect();

    let bounds = Bounds {
        x: 0.0,
        y: 0.0,
        w: CANVAS,
        h: CANVAS,
    };

    LayoutResult { positions, bounds }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gram_render::graph::parse_gram;

    #[test]
    fn layout_single_node() {
        let g = parse_gram("(a)").unwrap();
        let lr = compute(&g);
        assert!(lr.positions.contains_key("a"));
    }

    #[test]
    fn layout_no_two_nodes_overlap() {
        let g = parse_gram("(a)-[:R]->(b)-[:R]->(c)-[:R]->(d)-[:R]->(e)").unwrap();
        let lr = compute(&g);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let p1 = lr.positions[ids[i]];
                let p2 = lr.positions[ids[j]];
                let dist = ((p1.x - p2.x).powi(2) + (p1.y - p2.y).powi(2)).sqrt();
                assert!(dist > 10.0, "nodes {i} and {j} are too close ({dist:.1}px)");
            }
        }
    }
}
