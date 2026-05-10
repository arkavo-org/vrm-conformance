# RFC 0001: Monorepo confirmed

- **Status:** Accepted
- **Author(s):** Paul Flynn
- **Date:** 2026-05-10

## Summary

`arkavo-org/vrm-conformance` ships as a single polyglot monorepo containing the asset generator, runner, diff engine, MCP/CLI adapters (across multiple languages), test plans, comparison site, and governance documents. The original handover specced a polyrepo split across four GitHub repositories; this RFC supersedes that decision.

## Motivation

The polyrepo design optimized for per-language CI isolation and per-renderer maintainer ownership. In practice, at the team's current size, the coordination cost of cross-repo changes (asset-generator schema → runner consumer → adapter contract → site) outweighs the isolation benefit. A monorepo gives:

- One CI surface, one PR per cross-cutting change.
- Simpler atomic refactors of the operation catalog (`vrm-ops`).
- One issue tracker, one release cadence.

## Detailed design

Top-level layout per [README.md](../README.md). Polyglot is handled by:

- A Cargo workspace at the repo root for all Rust crates.
- A self-contained Swift package per adapter under `adapters/`. Adapters are not part of the Cargo workspace; they build via `swift build` and run as subprocesses driven via stdio JSON-RPC or per-op CLI invocations.
- A self-contained Vite project under `site/`.
- Per-language CI workflows in `.github/workflows/`, scoped to changed paths.

Goldens (binary `.png` and `.mov` artifacts) live on **S3**, not git LFS. `goldens/manifest.json` records every artifact's S3 URL, BLAKE3 content hash, and submission metadata. This RFC commits to S3 over LFS to keep clone times bounded and decouple binary churn from code review.

## Alternatives considered

- **Polyrepo (original spec).** Rejected on coordination cost, see motivation.
- **Monorepo with git LFS for goldens.** Rejected. LFS works but adds a credential dependency for every clone and inflates clone time. S3 with a content-addressed manifest is cleaner.
- **Submodules pointing to per-language repos.** Rejected as worst-of-both-worlds: monorepo coordination cost without monorepo atomicity.

## Open questions

None.

## References

- Original handover document, §3 Repository Layout, §11 Open Question 1.
- KhronosGroup/glTF-Render-Fidelity uses git LFS; we deliberately diverge on storage.
