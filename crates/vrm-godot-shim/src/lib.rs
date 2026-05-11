//! vrm-godot-shim: bridges the runner's stdio JSON-RPC contract to a
//! headless Godot child over TCP loopback. See
//! `docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md`
//! for the architectural rationale (GDScript cannot write byte-exact
//! stdout; this shim owns the wire so Godot only has to dispatch).

pub mod bridge;
pub mod child;
