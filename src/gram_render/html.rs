use super::graph::GramGraph;
use super::layout;

const PAPER_JS: &str = include_str!("../../assets/vendor/paper-core.min.js");
const D3_FORCE: &str = include_str!("../../assets/vendor/d3-force.min.js");
const RENDER_JS: &str = include_str!("../../assets/templates/render.js");

pub fn render_html(graph: &GramGraph) -> String {
    let layout = layout::compute(graph);

    // Build the combined JSON payload: graph + pre-computed layout
    #[derive(serde::Serialize)]
    struct Payload<'a> {
        nodes: &'a [super::graph::GramNode],
        edges: &'a [super::graph::GramEdge],
        paths: &'a [super::graph::GramPath],
        nested: &'a [super::graph::NestingRelation],
        layout: &'a layout::LayoutResult,
    }

    let payload = Payload {
        nodes: &graph.nodes,
        edges: &graph.edges,
        paths: &graph.paths,
        nested: &graph.nested,
        layout: &layout,
    };

    let data_json =
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>gram</title>
  <style>
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{ display: flex; height: 100vh; font-family: monospace; background: #fafafa; }}
    #gram-canvas {{ flex: 1; display: block; }}
    #sidebar {{
      width: 260px; min-width: 200px; padding: 12px; overflow-y: auto;
      border-left: 1px solid #ddd; background: #fff;
      font-size: 12px; white-space: pre-wrap; color: #333;
    }}
    #sidebar:empty::before {{ content: "Click a node to inspect"; color: #aaa; }}
  </style>
</head>
<body>
  <canvas id="gram-canvas"></canvas>
  <pre id="sidebar"></pre>
  <script>{PAPER_JS}</script>
  <script>{D3_FORCE}</script>
  <script type="application/json" id="gram-data">{data_json}</script>
  <script>{RENDER_JS}</script>
</body>
</html>"#,
        PAPER_JS = PAPER_JS,
        D3_FORCE = D3_FORCE,
        data_json = data_json,
        RENDER_JS = RENDER_JS,
    )
}
