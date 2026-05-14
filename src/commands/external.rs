use crate::cli::Neo4jArgs;
use std::process::Command;

pub fn exec_extension(name: &str, ext_args: &[String], neo4j: &Neo4jArgs) -> ! {
    let binary = format!("relate-{name}");
    let mut cmd = Command::new(&binary);
    cmd.args(ext_args);
    cmd.env("RELATE_URI", &neo4j.uri);
    cmd.env("RELATE_USER", &neo4j.user);
    if let Some(pw) = &neo4j.password {
        cmd.env("RELATE_PASSWORD", pw);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        handle_exec_error(&binary, err);
    }

    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(err) => handle_exec_error(&binary, err),
        }
    }
}

fn handle_exec_error(binary: &str, err: std::io::Error) -> ! {
    let name = binary.strip_prefix("relate-").unwrap_or(binary);
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            eprintln!(
                "error: external subcommand `{binary}` not found on PATH \
                 — install it to use `relate {name}`"
            );
            std::process::exit(127);
        }
        std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "error: external subcommand `{binary}` exists but is not executable \
                 — check file permissions"
            );
            std::process::exit(126);
        }
        _ => {
            eprintln!("error: failed to execute `{binary}`: {err}");
            std::process::exit(1);
        }
    }
}
