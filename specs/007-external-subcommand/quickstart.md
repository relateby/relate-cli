# Quickstart: External Subcommand Support

**Branch**: `007-external-subcommand`

## What This Enables

After this feature, any binary named `relate-<something>` on your `PATH` becomes callable as `relate <something>`. This creates a lightweight plugin ecosystem for the `relate` CLI.

## Try It in 60 Seconds

```bash
# 1. Create a stub extension
cat > /usr/local/bin/relate-hello <<'EOF'
#!/bin/sh
echo "Hello from relate-hello!"
echo "Args: $@"
echo "URI: $RELATE_URI"
EOF
chmod +x /usr/local/bin/relate-hello

# 2. Invoke it via relate
relate hello world
# → Hello from relate-hello!
# → Args: world
# → URI: bolt://localhost:7687

# 3. Pass global flags
relate --uri bolt://myserver:7687 hello
# → URI: bolt://myserver:7687

# 4. What happens when not found
relate nonexistent
# error: external subcommand `relate-nonexistent` not found on PATH — install it to use `relate nonexistent`
# exits with code 127
```

## How It Works

1. You type `relate csp solve foo.gram`
2. `relate` checks: is `csp` a built-in? (lint, parse, query, write, read, mcp) → No
3. `relate` looks for `relate-csp` on PATH
4. If found: replaces itself with `relate-csp solve foo.gram` (process replacement on Unix)
5. `RELATE_URI`, `RELATE_USER`, and (if set) `RELATE_PASSWORD` are in the environment

## Extension Author Notes

Your extension binary:
- Receives all arguments after the subcommand name in `argv`
- Inherits the full calling environment plus `RELATE_*` variables
- Should follow the [relate constitution](../../.specify/memory/constitution.md) for consistent UX
- Should ship its own `SKILL.md` for agent discoverability

## Building an Extension

Any executable language works. Minimal Rust example:

```rust
// relate-hello/src/main.rs
fn main() {
    let uri = std::env::var("RELATE_URI").unwrap_or_default();
    let args: Vec<_> = std::env::args().skip(1).collect();
    println!("Connecting to {uri}");
    println!("Args: {args:?}");
}
```

Name the binary `relate-hello`, put it on PATH, and run `relate hello`.
