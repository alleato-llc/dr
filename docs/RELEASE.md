# Release Process

## Overview

`dr` uses [release-please](https://github.com/googleapis/release-please) for automated versioning and releases. When commits following [Conventional Commits](https://www.conventionalcommits.org/) are pushed to `main`, release-please creates a pull request that bumps the version in `Cargo.toml` and updates `CHANGELOG.md`. Merging that PR triggers a GitHub Release with pre-built binaries for all supported platforms.

## Repository Setup

In your GitHub repository settings:

1. Go to **Settings → Actions → General**
2. Under "Workflow permissions", enable **"Allow GitHub Actions to create and approve pull requests"**

This is required for release-please to create and update release PRs.

## How the Pipeline Works

```
Push to main (conventional commits)
        │
        ▼
   CI workflow runs
   (test + build)
        │
        ▼ (on success)
   Release workflow triggers
        │
        ▼
   release-please creates/updates PR
   (bumps Cargo.toml version + CHANGELOG.md)
        │
        ▼ (PR merged)
   release-please creates GitHub Release
        │
        ▼
   upload-assets job builds binaries
   and attaches them to the release
```

## Configuration Files

| File | Purpose |
|------|---------|
| `release-please-config.json` | Release-please behavior: release type, version bump rules |
| `.release-please-manifest.json` | Current version tracker (updated automatically by release-please) |
| `.github/workflows/ci.yml` | CI pipeline: test + multi-platform build |
| `.github/workflows/release.yml` | Release automation: release-please + binary upload |

## Platform Matrix

| Platform | Runner | Binary Name | Architecture |
|----------|--------|-------------|--------------|
| macOS | `macos-latest` | `dr-darwin-arm64` | Apple Silicon (arm64) |
| Linux | `ubuntu-latest` | `dr-linux-amd64` | x86_64 |
| Windows | `windows-latest` | `dr-windows-amd64.exe` | x86_64 |

## Artifact Retention

| Branch Type | Retention |
|-------------|-----------|
| `feature/*` | 1 day |
| Pull request | 7 days |
| `main` | 90 days |

## Rust-Specific Notes

- `release-type: rust` in `release-please-config.json` tells release-please to update the `version` field in `Cargo.toml` when creating release PRs
- `Cargo.lock` is updated automatically when `Cargo.toml` version changes
- Builds use `cargo build --release` for optimized binaries

## Conventional Commits

Release-please determines version bumps from commit messages:

| Commit Type | Version Bump | Example |
|-------------|-------------|---------|
| `feat:` | Minor (0.x.0) | `feat: add CSV export` |
| `fix:` | Patch (0.0.x) | `fix: handle empty directory` |
| `feat!:` or `BREAKING CHANGE:` | Major (x.0.0) | `feat!: change JSON output format` |
| `docs:`, `chore:`, `ci:`, `test:`, `refactor:` | No bump | `docs: update README` |

## Troubleshooting

### release-please PR not created

- Verify conventional commit messages on `main`
- Check that the CI workflow completed successfully (release workflow triggers on CI success)
- Ensure workflow permissions are configured (see Repository Setup)

### Binary upload fails

- Check that the release tag exists (created by release-please)
- Verify `gh` CLI authentication in the workflow (`GITHUB_TOKEN` is provided automatically)
- Use `--clobber` flag on `gh release upload` to overwrite existing assets on retry
