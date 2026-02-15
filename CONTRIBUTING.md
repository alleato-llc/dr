# Contributing to dr

## Prerequisites

- [Rust](https://rustup.rs/) stable toolchain

## Build & Test

```bash
# Build
cargo build

# Run
cargo run -- ~/Music/Album/

# Test
cargo test

# Test with output
cargo test -- --nocapture

# Build release binary
cargo build --release
```

## Conventional Commits

All commits must follow [Conventional Commits](https://www.conventionalcommits.org/). This is required for automated versioning and changelog generation.

### Format

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Types

| Type | Purpose |
|------|---------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `test` | Adding or updating tests |
| `chore` | Maintenance (dependencies, tooling) |
| `ci` | CI/CD changes |
| `perf` | Performance improvement |

### Breaking Changes

Append `!` after the type or include `BREAKING CHANGE:` in the footer:

```
feat!: change JSON output format

BREAKING CHANGE: The "dr" field is now "dynamic_range" in JSON output.
```

### Examples

```bash
git commit -m "feat: add STDIN piping support"
git commit -m "fix: handle files shorter than 3 seconds"
git commit -m "docs: add algorithm description to ARCHITECTURE.md"
git commit -m "refactor: extract block stats into separate function"
git commit -m "test: add integration test for album DR averaging"
git commit -m "chore: update symphonia to 0.6"
git commit -m "perf: reduce memory allocation in sample buffer"
```

## Pull Request Process

1. Branch from `main`
2. Use conventional commit messages
3. Ensure `cargo test` passes
4. Open a PR against `main`
