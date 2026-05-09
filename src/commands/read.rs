use crate::cli::{Neo4jArgs, ReadArgs};
use anyhow::Result;

pub async fn run(_args: ReadArgs, _neo4j: Neo4jArgs) -> Result<()> {
    eprintln!("read: not yet implemented");
    Ok(())
}
