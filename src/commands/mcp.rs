use crate::cli::McpArgs;
use crate::gram_render::{parse_gram, render_html, render_svg};
use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RenderGramParams {
    /// The gram source text to render.
    pub gram_source: String,
    /// Output format: "svg" (default) or "html".
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "svg".to_string()
}

#[derive(Debug, Clone)]
struct RelateServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl RelateServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Render a gram graph source as SVG or HTML.
    #[tool(
        name = "render_gram",
        description = "Parse gram graph notation and render it as SVG or HTML. \
                       Returns the rendered output as a string."
    )]
    async fn render_gram(
        &self,
        Parameters(params): Parameters<RenderGramParams>,
    ) -> CallToolResult {
        let graph = match parse_gram(&params.gram_source) {
            Ok(g) => g,
            Err(e) => {
                return CallToolResult::error(vec![Content::text(e.to_string())]);
            }
        };

        let output = match params.format.to_lowercase().as_str() {
            "html" => render_html(&graph),
            _ => render_svg(&graph),
        };

        CallToolResult::success(vec![Content::text(output)])
    }
}

#[tool_handler]
impl ServerHandler for RelateServer {}

pub async fn run(_args: McpArgs) -> Result<()> {
    let server = RelateServer::new();
    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}
