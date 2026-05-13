# RFC 0003: Adapter shape matches the engine, not the prior adapter

- **Status:** Accepted
- **Author(s):** Paul Flynn
- **Date:** 2026-05-12

## Summary

Renderer adapters in `vrm-conformance` are not required to share an implementation shape. Each adapter chooses the IPC and lifecycle model that fits its host engine's idioms — long-lived JSON-RPC for engines whose headless mode is idiomatically long-running; batched one-shot for engines whose batch mode is idiomatically "run, do work, exit." The MCP/JSON-RPC operation contract is the cross-adapter abstraction; the adapter shape is not.

## Motivation

The project now has four adapters in flight, two of which exercise this principle in opposite directions:

- `adapters/godot-vrm` uses a persistent IPC shim (`crates/vrm-godot-shim`) that spawns Godot headless and bridges framed stdio ↔ TCP-loopback to a long-lived GDScript dispatcher. The TCP indirection exists because GDScript can't speak byte-safe stdout; the long-lived shape is fine because Godot's `--headless` mode is idiomatic for long-running services.
- `adapters/univrm` (in design, see [`docs/superpowers/specs/2026-05-12-adapter-univrm-design.md`](../docs/superpowers/specs/2026-05-12-adapter-univrm-design.md)) uses batched one-shot — one Unity invocation processes N test_ids and exits. Unity batch mode is idiomatic for "run, do work, exit"; persistent JSON-RPC inside a Unity process swims upstream against engine design. Unity's stdout is polluted by Editor logs, package import chatter, third-party `Debug.Log` calls, and UniVRM's own load-time logging — making in-process JSON-RPC fragile in a way that produces intermittent CI failures hard to root-cause.

Future adapters (Babylon-VRM-Loader in Phase 3, anything beyond) will each face the same question. Forcing all adapters to share one shape would (a) push the wrong shape onto at least one engine, and (b) accumulate hidden cost when "consistent with the prior adapter" overrides "fits this engine." This RFC names the principle so future adapter specs cite it in two lines rather than re-litigating.

## Detailed design

### The principle

> **Adapter shape matches the engine, not the prior adapter.**
>
> The MCP/JSON-RPC operation contract is the cross-adapter abstraction. Below that contract, each adapter is free to choose the IPC model, process lifecycle, and protocol-bridging strategy that fits its host engine's idioms. There is no project-wide requirement that adapters share an implementation shape.

### Worked examples

| Adapter | Engine idiom | Adapter shape | Why |
|---|---|---|---|
| `godot-vrm` | Godot `--headless` is a long-running service | Persistent JSON-RPC over TCP loopback, via Rust shim (`vrm-godot-shim`) | GDScript can't speak byte-safe stdout; TCP indirection is the canonical workaround. Long-lived shape amortizes engine startup across the full corpus. |
| `univrm` | Unity `-batchmode` is "run, do work, exit" | Batched one-shot: one Unity invocation processes the full manifest of test_ids and exits | Unity's stdout pollution defeats in-process framed JSON-RPC. Persistence at the batch level (one Unity per batch invocation) achieves comparable performance without IPC complexity. |
| `vrm-metal-kit` | Swift Foundation gives byte-safe stdio | Direct JSON-RPC over stdio, no shim | Swift can reserve stdout cleanly; the simplest shape is sufficient. |
| `three-vrm` | Node.js orchestrates a Playwright-driven headless Chromium | Direct JSON-RPC over stdio, no shim | Node.js can reserve stdout cleanly; same reasoning as `vrm-metal-kit`. |

### What is shared across adapters

- The **operation contract** in `docs/operation-contract.md` — every adapter implements the same Phase 1 ops (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`), the same error envelope (`-32000 Unimplemented`, `-32001 LoadFailed`, `-32002 RenderFailed`, `-32601 method-not-found`, `-32602 invalid params`), and the same schema for op inputs/outputs.
- **Cross-cutting types** in `crates/vrm-ops`, `crates/vrm-test-plan` — Rust crates the runner and any Rust shims share.
- **Methodology pins** in `docs/methodology.md` — color-space convention, MSAA settings, magenta sentinel background, tone-mapping defaults.

### What is not shared across adapters

- IPC mechanism (stdio vs. TCP vs. filesystem).
- Process lifecycle (one-shot, persistent, batched).
- Output reporting (per-op JSON-RPC response, or batched NDJSON results file).
- Implementation language for the bridge layer (Rust shim, shell launcher, or no bridge at all).

### When to invoke this RFC

A new adapter spec should cite RFC-0003 in two cases:

1. The new adapter's shape differs from at least one existing adapter, and the spec needs to justify the divergence without re-litigating the principle.
2. A reviewer asks "why doesn't this look like the other adapter" and the answer is "engine idioms differ."

## Alternatives considered

### Force a single adapter shape across all engines

Pick the shape that fits most engines (likely persistent JSON-RPC over TCP, since it bridges any engine's stdio limitations), require all adapters to conform.

Rejected: the costs are real. For Unity specifically (see UniVRM design spec), persistent JSON-RPC inside a process that fundamentally wants to log to stdout produces intermittent CI failures that are very hard to root-cause. For Swift/Node, the TCP indirection is unnecessary ceremony. Forcing one shape means at least one adapter pays a recurring cost for cross-adapter consistency that delivers no user-visible value.

### Document the principle only in adapter READMEs

Keep the rationale in each adapter's README rather than in an RFC.

Rejected: future readers won't know to look in multiple READMEs to discover that engine-idiom divergence is a deliberate project-wide stance. An RFC at the governance level — alongside RFC-0001 (monorepo) and RFC-0002 (anti-fraud) — is the canonical place for cross-cutting principles. Adapter READMEs and design specs cite this RFC; the RFC is the index entry.

## Open questions

None — the principle is stated and the worked examples cover the current cases.

## References

- [`docs/operation-contract.md`](../docs/operation-contract.md) — the cross-adapter contract this RFC sits beneath.
- [`docs/superpowers/specs/2026-05-12-adapter-univrm-design.md`](../docs/superpowers/specs/2026-05-12-adapter-univrm-design.md) — UniVRM adapter design, first spec to cite this RFC.
- [`adapters/godot-vrm/README.md`](../adapters/godot-vrm/README.md) — explains the persistent-IPC-via-Rust-shim shape that this RFC formalizes as the godot-vrm engine fit.
- [`adapters/vrm-metal-kit/README.md`](../adapters/vrm-metal-kit/README.md) — explains the direct-stdio shape.
- [RFC-0001](./0001-monorepo-confirmed.md) — adjacent governance precedent.
- [RFC-0002](./0002-anti-fraud-submission-integrity.md) — adjacent governance precedent.
