use std::path::PathBuf;

use anyhow::Result;

use crate::cli::{OutputFormat, RenderArgs};
use crate::gram_render;

pub fn run(args: RenderArgs) -> Result<()> {
    let source = std::fs::read_to_string(&args.file)
        .map_err(|e| anyhow::anyhow!("cannot read {:?}: {e}", args.file))?;

    let graph = gram_render::parse_gram(&source).map_err(|e| anyhow::anyhow!("{e}"))?;

    let output_path = resolve_output(&args)?;

    let content = match args.format {
        OutputFormat::Html => gram_render::render_html(&graph),
        OutputFormat::Svg => gram_render::render_svg(&graph),
    };

    std::fs::write(&output_path, &content)
        .map_err(|e| anyhow::anyhow!("cannot write {:?}: {e}", output_path))?;

    if args.json {
        println!(
            "{{\"output\":{},\"format\":\"{}\"}}",
            serde_json::to_string(output_path.to_str().unwrap_or("")).unwrap(),
            args.format,
        );
    } else {
        eprintln!("rendered → {}", output_path.display());
    }

    if args.open {
        if let Err(e) = open::that(&output_path) {
            eprintln!("warning: could not open file: {e}");
        }
    }

    Ok(())
}

fn resolve_output(args: &RenderArgs) -> Result<PathBuf> {
    if let Some(ref path) = args.output {
        return Ok(path.clone());
    }
    let stem = args
        .file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = match args.format {
        OutputFormat::Html => "html",
        OutputFormat::Svg => "svg",
    };
    let mut out = args.file.clone();
    out.set_file_name(format!("{stem}.{ext}"));
    Ok(out)
}
