# godot-vrm renderer adapter

Scaffold for the V-Sekai/godot-vrm renderer adapter. Architecture: the
runner spawns `vrm-godot-shim` (Rust); the shim spawns this directory
as a headless Godot project; the two talk newline-delimited JSON over
TCP loopback. See [`docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md`](../../docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md).

Full README lands in Task 10 of the v2 scaffold plan.
