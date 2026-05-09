use crate::cli::{Neo4jArgs, WriteArgs};
use anyhow::Result;

pub async fn run(_args: WriteArgs, _neo4j: Neo4jArgs) -> Result<()> {
    eprintln!("write: not yet implemented");
    Ok(())
}
