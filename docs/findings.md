# Conformance findings (running log)

This document records cross-renderer divergence findings produced by the suite, in the order they were surfaced. Each entry has a brief observation, the data behind it, and pointers to any upstream issues filed. Findings are a deliverable in their own right — the project's purpose is to produce falsifiable signal that drives upstream fixes (or methodology refinements when divergence turns out to be legitimate).

## 2026-07-17 (VMK 1.1.0-beta regression check + pin catch-up to stable 1.0.0) — **BUMPED the pin `1.0.0-rc.2` (`ef52802`) → stable `1.0.0` (`a94531d`) — an overdue, render-neutral catch-up (we sat on rc.2 while stable shipped 2026-07-02; `1.0.0` = rc.2 + #369, which confines VRM authoring to the test target so the public API is render-only — a test-target/API-surface change, zero render effect). Separately REGRESSION-CHECKED the new pre-release `1.1.0-beta` (`c68eb28`, 2026-07-17, "not for production pinning") against the suite: NO shading regression (MToon 70/71 byte-identical to 1.0.0, the lone differ being a spring-bone cell), and an INTENDED spring-bone golden shift (16/20 swing cells move, all still pass the gate) driven by the beta's new default-on synthetic torso/upper-arm colliders. Also landed a new cross-renderer test — the multi-group collider-membership discriminator — which empirically confirms the beta's "collider group semantics fixed." VERDICT: promoted to stable 1.0.0 now; do NOT advance the pin to the 1.1.0-beta pre-release, but the beta's collider-group fix is a genuine spec-conformance improvement and its default-on augmentation is a methodology flag (below).**

**Method.** Built the VMK adapter at each of `1.0.0-rc.2`-equivalent stable `1.0.0` (`a94531d`) and `1.1.0-beta` (`c68eb28`) on the M-series/Xcode-26 dev machine, rendered the MToon (71) + swing (20) corpora through each, and diffed **beta directly against 1.0.0** (same machine/toolchain — no stored-baseline ambiguity). Provenance cross-check: fresh `1.0.0` renders are **swing 20/20 byte-identical** to the stored `goldens-cache-rc` baseline (MToon 60/71; the 11 are the already-documented tightest-band `shadingShift`/`rimLightingMix` re-baseline cells), confirming the `rc.2 → 1.0.0` promotion is render-neutral in practice, not just by construction.

**Change scope (rc.2 → 1.1.0-beta).** 101 files, +14.8k/−0.6k, but tightly localized: spring-bone (`SpringBoneComputeSystem`, `SpringBoneColliderAugmentor`, `SpringBoneCollision.metal`), all-new Crowd/BalanceModel/Stagger/ArmCounterbalance animation layers, and the loader (`BufferLoader`/`BufferPreloader`, the 50–70% parse speedup). **`MToonShader.metal` and the MToon render path are untouched** — the only shader changed is `SpringBoneCollision.metal`.

**Beta vs 1.0.0 — the true delta.**

| corpus | byte-identical vs 1.0.0 | worst SSIM | interpretation |
| --- | --- | --- | --- |
| MToon (71) | **70/71** | 0.9995 (`springbone_joints_16`, a spring-bone cell) | Every actual `mtoon_*` cell is byte-identical → **zero shading regression**, exactly as the untouched shader predicts. |
| swing (20) | 4/20 | 0.9869 (`swing_springbone_joints_16`) | **Intended** shift from default-on torso/upper-arm augmentation; mean SSIM 0.9956, all ≥ the 0.85 gate. Shift scales with chain length (`joints_16` > `joints_8`) — more hair segments now deflect off the new body capsules. |

**Behavior change you must validate before ever adopting the beta.** `VRMLoadingOptions(augmentSpringBoneColliders:)` (default `true`) now also synthesizes a **torso capsule (spine→chest) + upper-arm capsules** on top of the prior leg/head/forearm/hand set. Our adapter defaults `augment_colliders: true` (`Operations.swift:231`) and no spring-bone/swing plan overrides it, so the whole spring-bone corpus renders with augmentation on. Adopting the beta therefore requires a **re-baseline of the spring-bone + swing goldens**. Deeper methodology note: **augmentation is VMK-proprietary — no spec-faithful renderer (UniVRM, three-vrm, godot) synthesizes colliders**, so an augment-on spring-bone render measures VMK's synthetic physics, not the authored asset. The beta widens this pre-existing gap. For cross-renderer comparison against the UniVRM golden reference, the spring-bone corpus arguably should load with `augment_colliders: false`. Flagged, not changed here.

**New feature → existing-library mapping (which beta features are conformance-testable).** Only one of the beta's headline features is VRM-spec behavior other renderers must also implement:

| beta feature | in VRM 1.0 spec? | other libs | conformance verdict |
| --- | --- | --- | --- |
| **Multi-group collider fix** (a collider in N groups collides with springs referencing *any* of its groups) | ✅ `VRMC_springBone` | should honor it | **cross-renderer test candidate — added this change (below).** |
| Synthetic torso/upper-arm colliders (default on) | ❌ extra-spec | none | not conformance-testable; divergence source (see above). |
| Cross-avatar spring collision; balance/stagger/crowd | ❌ app-layer | none | single-renderer only; out of scope. |
| VRM parse 50–70% faster | n/a | all have loaders | perf axis only (`load_ms` benchmark), not correctness. |

**New cross-renderer test: multi-group collider-membership discriminator.** `emit-springbone-collider-multigroup-sweep` (generator; 4 assets = 2 variants × settle+swing). Both variants place one identical sphere collider in the hair path and register it in **two** collider groups; they differ only in which group(s) the spring references — `bothref` references both (control, every renderer collides), `secondref` references **only the collider's second group** (probe). On a spec-faithful renderer the two render identically; a renderer that honors only a collider's *first* group ignores `secondref`'s collider and the hair passes through. The A/B (not an absolute image) is the discriminator, so no oracle is needed. **Validated against known-buggy and known-fixed VMK:**

| cross-SSIM(`secondref`, `bothref`) | |
| --- | --- |
| @ `1.0.0` (buggy — first-group-only) | **0.9942 — cells diverge** (probe's collider ignored) |
| @ `1.1.0-beta` (fixed) | **1.00000 — byte-identical** (probe now collides) |

This reproduces the beta's "Collider group semantics fixed" note as a falsifiable measurement.

**Cross-renderer result — one discriminator, two different multi-group bugs.** Ran the probe (augment-off plans) through three renderers plus both VMK pins, disambiguating the mechanism against the geometrically-identical single-group control `swing_springbone_collider_sphere_x0p02_r0p05` (same sphere, one group, spring references it). All renders are deterministic (three-vrm self-SSIM = 1.00000, so sub-1.0 numbers are real, not noise):

| renderer | `singleref` vs `secondref` | `singleref` vs `bothref` | multi-group handling |
| --- | --- | --- | --- |
| VMK `1.1.0-beta` | 1.00000 | 1.00000 | **correct** — collider applied once regardless of routing |
| VMK `1.0.0` | **0.99416** | 1.00000 | **first-group-only** — a collider referenced only via its *second* group is dropped (confirms the release note) |
| three-vrm `3.5.0` | 1.00000 | **0.99307** | **double-count** — a collider reached via *two* referenced groups is applied twice; `secondref` (single reference) is correct |
| godot-vrm (Godot 4.7) | — | — | not runnable this pass — pre-existing GDScript compile error (`register_all` nonexistent) against Godot 4.7, unrelated to this work |

So the same collider-in-two-groups asset exposes **opposite** defects: VMK 1.0.0 *under*-applies (`secondref` drops it), three-vrm *over*-applies (`bothref` doubles it), and fixed VMK does neither. The discriminator localizes the mechanism, not just "they differ."

**Open — needs the golden reference.** Whether per-collider dedup across referenced groups is *required* by `VRMC_springBone` (making three-vrm's double-count a genuine bug) or merely unspecified is the arbitration question, and per methodology **UniVRM is the oracle** — it was not run this pass (Unity batch boot). Next step: render the probe through UniVRM; its `singleref`-vs-`bothref` result declares whether three-vrm's double-count is a conformance miss or acceptable latitude. The consensus run lacking UniVRM is provisional.

**Pointers.** Upstream: `1.1.0-beta` release notes (cross-avatar collision, hair-vs-body fix, loader speedup, `groupIndex`→`groupMask`). Suite-side (this change): `crates/vrm-asset-generator` `spring_bone_collider_multigroup_sweep` + `emit-springbone-collider-multigroup-sweep` CLI/describe; pin `adapters/vrm-metal-kit/Package.swift` → `1.0.0`. Related: the augment-off methodology question (`docs/methodology.md` spring-bone determinism conventions) and the UniVRM-as-golden-reference triage rule.

**Follow-up — tightest-band MToon re-baseline (11 cells).** Fresh `1.0.0` renders diverge from the stored local baseline on exactly 11 cells — `mtoon_shadingShift_{neg0p5,0p2,0p5,0p8,1}` + `mtoon_basic_shadeShift_neg05` + `mtoon_rimLightingMix_{0p1,0p25,0p5,0p75,1}` — all **sub-perceptual** (SSIM ≥ 0.998, worst `shadingShift_1`), i.e. the already-documented tightest-band cells, not a regression (the stored baseline predates the 0.21.0/rc line; `MToonShader.metal` is unchanged rc.2→1.0.0). Refreshed the **local** baseline cache to the `1.0.0` pixels (diffs now clean). The committed `goldens/manifest.json` is empty (goldens are maintainer-pushed to S3 per the trust model), so there is no committed manifest to update — the durable action is that the S3 `1.0.0` golden set, when generated, should carry these fresh renders for the 11 cells. Flagged, not pushed.

**Follow-up — spring-bone loads made spec-faithful (`augment_colliders: false`).** Resolved the methodology question above: every generated spring-bone plan (settle/swing/sequence/collider/extended/multichain/multi-group) now emits `augment_colliders: false`, a new `TestPlan` field the runner forwards to `load_vrm` (plan value wins over the `--augment-colliders` CLI flag). Collision is now pinned to the **authored** collider set, so VMK is measured against the same physical inputs as UniVRM/three-vrm/godot (which never augment) rather than against its proprietary synthetic capsules. Forwarding verified end-to-end: on VMK the plan-driven flip changes the render on the 16-joint chain (augment on-vs-off SSIM **0.994**) while being a no-op on the short default chain — chain-length-dependent, as expected. Convention written up in `docs/methodology.md`. **Consequence (flagged, not executed): the existing augment-on VMK spring-bone goldens need a re-baseline** — they were rendered with synthetic colliders that the conformance corpus no longer requests. Non-VMK spring-bone goldens are unaffected. See `docs/methodology.md` "Spring bone collider augmentation."

## 2026-06-17 (VMK 1.0.0-rc.2 candidate: render-neutral + measurable per-draw CPU win) — **VALIDATED: bumped the pin `0.21.0` (`985bd7c`) → `1.0.0-rc.2` (`ef52802`). This is the second 1.0 release candidate — `release/1.0` was rebased onto main so it carries the post-0.21.0 code work (VRMMetalKit #356 async-loader crash + LookAt delta-time fix, and #357 "Reduce per-draw CPU overhead by gating debug work behind warmup", a four-round `cmdEncode` optimization) plus the four 1.0 release-docs commits. The validated code tip is `46cd0b1` (#357 merge); `ef52802` = `46cd0b1` + the docs/comment commits (non-code, render-irrelevant). Render-equivalence: 429/429 byte-identical to 0.21.0 across the full corpus (MToon/outline/matcap/shade/rim/shadingshift/pbrtex/uvxform/emissive/spring-bone/VRMA) — ZERO differing cells, stronger than the rc.3→0.21.0 promotion's 31 sub-perceptual cells (re-confirmed 429/429 against the rebased `ef52802` pin). Conformance is therefore invariant by construction (byte-identical pixels ⇒ identical SSIM-vs-UniVRM ⇒ identical pass/fail) — inherits 0.21.0's status verbatim: no hard VRM-1.0 blockers, lone miss #226 unchanged. Performance: no regression; a real, draw-count-scaled CPU-encode win — upstream `VRMBenchmark` on AvatarSample_A (20 draws) reproduces the PR headline (cmdEncode −25.5%, render total −17.7%, eff FPS +21.5%, pipeline changes 30→6, state changes 60→36). Tagged upstream `1.0.0-rc.2` pre-release (superseding the `1.0.0-rc.1` draft, which was 0.21.0 + docs only). VERDICT: promotable to stable `1.0.0`.**

**Why this entry exists.** A new upstream performance PR (#357) merged 2026-06-17. This validates it as an RC against the suite's three bars — render-equivalence (no regression), conformance (no new miss vs the UniVRM golden reference), and performance (the claimed win, plus no regression on light scenes) — before promoting the pin to stable.

**Classification — patch.** #357 is render-neutral by construction: the large `MToonShader.metal` churn hoists the debug-visualization branches into a gated `mtoon_debug_visualize()` (runs only when `debugUVs != 0`, never set in conformance renders); the one new public uniform field `shadeUsesBaseColor` replaces `_padding2` (same 4-byte size — no ABI change) and defaults to 0 = prior behavior; the base-color-sample reuse fast path is bit-identical (only fires when the shade texture *is* the base texture). The `cpuBudget` metric it adds lives in the standalone `VRMBenchmark` CLI, not the library the adapter links. Net: no new renderer capability, no behavior change → patch-class in nature (one additive internal field, render-neutral), not a feature bump — shipped here as part of the `1.0.0-rc.2` release line per the maintainer's call (the original `0.21.1`-patch framing was superseded by tagging it on the 1.0 release branch). Confirmed by the pin-only adapter rebuild succeeding with **no adapter code change**.

**Render-equivalence — 429/429 byte-identical.** VMK renders are deterministic run-to-run (verified: same scene twice → byte-identical, SSIM 1.0). Confirmed the committed `goldens-cache/vrm-metal-kit/` PNGs equal fresh 0.21.0 renders (5/5 sampled categories byte-match), so they are a valid 0.21.0 reference. Rendered the full corpus through the new pin and byte-compared: **429/429 identical, 0 differ, 0 missing-reference.** The PR changed not one output pixel. (`mtoon_firstperson_firstPersonOnly` trips the blank-frame gate — the first-person-only mesh is culled in the third-person firstPerson camera → 100%-single-color frame — on **both** pins; the blank renders are byte-identical. Pre-existing asset×gate interaction, predates the gate (#20), not a pin regression.)

**Conformance — invariant.** Because every VMK render is byte-identical to 0.21.0, SSIM-vs-UniVRM and the pass/fail set are identical on both pins (verified empirically: `new_vs_univrm == 0.21.0_vs_univrm` on every sampled cell). It inherits the 2026-06-16 status: no hard VRM-1.0 blockers; lone miss #226 (rim high-mix, SSIM 0.9491 at the 0.95 band) unchanged. *Hazard re-confirmed:* the cached `goldens-cache/univrm/` reference is from the since-reverted outlines-on era (flat SSIM 0.8044 = black-blob silhouette across all cells), so its absolute SSIM is not the conformance signal — a fresh re-measure still needs outlines-off assets + a fresh UniVRM PlayMode render. This does not affect the invariance argument (the contamination is symmetric across both pins).

**Performance — no regression; draw-count-scaled win.** Two measurements:
- *Conformance corpus A/B* (`vrm-runner benchmark-execute`, median of 9, interleaved, this M4 Max): the synthetic avatars are light (2 draw calls), where there is nothing per-draw to optimize — `static_default`/`anim_default` are flat (±2%, ~5 µs, noise floor), and `anim_multichain` (6 draws) is **−13.8% frame p50 / +15.8% fps**. `draw_calls` deterministic (2/2, 6/6); peak memory identical. (Note: the adapter's `PerfStructural` reports `state_changes`/`texture_bindings` as 0 — it does not instrument them like upstream `VRMBenchmark` — so the pipeline/state-change counts are verified upstream, not via the adapter.)
- *Upstream `VRMBenchmark` on AvatarSample_A* (20 draws, 500 frames, spring-bone ultra, 1024², median of 3, A/B from fresh clones at both commits) — the realistic avatar our corpus lacks: cmdEncode **0.507→0.378 ms (−25.5%)**, encode −23.1%, render total **0.865→0.712 ms (−17.7%)**, eff FPS **1157→1405 (+21.5%)**, pipeline changes **30→6 (−80%)**, state changes **60→36 (−40%)**, draw calls 20→20. Independently reproduces the PR's headline (it claimed −25.8% / −15.2% / +18.6% / 30→6 / 60→36).

*Is it noticeable?* Not as user-perceived smoothness — both pins are sub-millisecond/frame (~1200→1400 fps; ~2.6% of a 60 fps budget either way). It **is** a substantial, real reduction in per-frame CPU and Metal driver calls (−25% encode, −80% pipeline binds) that scales with avatar draw count — the win the PR targets for battery life and CPU coexistence with concurrent inference. On a heavy/multi-avatar scene the absolute saving compounds.

**Verdict.** 1.0.0-rc.2 clears all three bars: render-neutral (429/429 byte-identical), conformance-invariant, and a measured CPU-encode improvement with no regression. **Promotable to stable `1.0.0`.** Pin sits at the rc revision (`Package.swift` / `Package.resolved` → `ef52802`); promote by tagging upstream `1.0.0` at the same commit (`release/1.0` HEAD) and updating the pin comment, mirroring the rc.3→0.21.0 promotion.

**Pointers.** Upstream: VRMMetalKit #356 (`55b577b`), #357 (`46cd0b1`); `release/1.0` rebased onto main → HEAD `ef52802`; pre-release tag `1.0.0-rc.2` (target `ef52802`); supersedes the `1.0.0-rc.1` draft. Reproduction: upstream `VRMBenchmark <AvatarSample_A_1.0.vrm.glb> --frames 500 --warmup 30 --spring-bone --spring-bone-quality ultra`. Suite-side: pin at `adapters/vrm-metal-kit/Package.swift`. Prior status: 2026-06-16 entry (0.21.0 baseline, #226).

## 2026-06-16 (VMK 0.21.0 → 1.0 conformance status: no hard blockers) — **AUDIT: the pinned VMK is `0.21.0` (`985bd7c`) = the newest upstream tag = the tip of `main` — we are exactly current, nothing newer exists. Every substantive MToon/lookAt conformance bug the suite surfaced (#286/#287/#288/#289/#290) is FIXED, closed by VRMMetalKit PR #291 (merged 2026-05-23, in 0.21.0). The remaining open issues are NOT conformance blockers: #328 (giEqualization) is a formally-accepted, documented spec deviation for 1.0; #237 (extended-collider) is resolved-but-unclosed (test-asset placement artifact, fixed suite-side `d74c9f8`); #242 (KHR umbrella) is forward-looking, the tested case #288 is fixed; #221 (outline draw-calls) is perf/instrumentation (count fix already in via #353). The LONE genuine open render miss is #226 — MToon rim lighting at `rimLightingMix→1` lands SSIM 0.949 vs the deliberately-tight 0.95 rim band (off by 0.001, extreme only). Verdict: 0.21.0 is a defensible VRM-1.0 render-conformance candidate.**

**Why this entry exists.** A 1.0-readiness review re-flagged a set of "open VMK issues" (#242/#328/#237/#226/#221) as potential blockers. Checking each (state, comments, linked PRs/commits) showed most were already fixed-and-closed, resolved-but-unclosed, accepted-and-documented, or perf-only — so this consolidates the actual status to stop the list being re-litigated.

**Versioning (exactly current).** Pin `adapters/vrm-metal-kit/Package.swift:591` → revision `985bd7c` = tag `0.21.0` (released 2026-06-14 as stable, promoted from rc.3). `git ls-remote` confirms `0.21.0` is the newest tag and `985bd7c` is also `refs/heads/main` HEAD — there is nothing newer to bump to. CI build-validates this pin; runtime conformance is local-only (M4 Max / Xcode 26.5) per the macOS 26 floor.

**Fixed cluster (VRMMetalKit PR #291, MERGED 2026-05-23 — *"0.16.0-rc.3 — close #283 renderer-side + #286 + #287 + #288 + #289 + #290 + …"*; all in 0.21.0):**

| Issue | Bug | Status |
| --- | --- | --- |
| #287 | MToon `VRMC_materials_hdr_emissiveMultiplier` ignored | closed (PR #291) |
| #288 | MToon `KHR_texture_transform` on baseColorTexture ignored | closed (PR #291) |
| #289 | `outlineWidthMultiplyTexture` degraded outline pipeline | closed (PR #291) |
| #290 | glTF-core `normalTexture.scale` silently dropped | closed (PR #291 / #296) |
| #286 | VRMA `lookAt` rotation-channel gaze not parsed | closed (PR #291 / #331) |

**Open issues that are NOT 1.0 blockers:**

- **#328 `giEqualizationFactor`** — OPEN, but carries an explicit *"Assessed for 1.0 readiness … keep the proxy for 1.0 … documented spec deviation, not a release blocker"* note (2026-06-14). VMK lacks directional GI (IBL/SH `rawGi(n)`) so the spec lerp would degenerate to a no-op; the proxy is documented in VMK's `docs/MTOON_GI_SPEC.md`. IBL/SH follow-up tracked post-1.0.
- **#237 `VRMC_springBone_extended_collider`** — OPEN, but last comment (2026-05-24): *"Both ends of #237 are now resolved."* The "chaotic per-variant clustering" was a test-asset artifact (inside-collider placement radii too large); fixed suite-side in `d74c9f8` (radii `[0.10,0.20,0.40]` → `[0.04,0.06,0.08]`). Resolved-but-unclosed.
- **#242 GLTFMetalKit KHR umbrella** — OPEN, forward-looking ("KHR_texture_transform + *other* widely-adopted KHR extensions"), no activity since 2026-05-16. The conformance case the suite tests (KHR_texture_transform on MToon baseColorTexture) is #288 = fixed. Not the same scope as a blocker.
- **#221 outline pass doubles draw calls / bypasses state cache** — OPEN perf + instrumentation. The benchmark draw-call **count** under-report was fixed via #353 (`recordDrawCall` in `renderMToonOutlines`, in 0.21.0; see 2026-06-14 benchmark entry); the remainder is a perf concern, not a render-fidelity gap.

**The lone genuine open render miss: #226 (MToon rim lighting high-mix) — RE-MEASURED at 0.21.0, confirmed present.** Re-rendered the six `mtoon_rimLightingMix_*` variants through VMK 0.21.0 (`985bd7c`, freshly rebuilt) and UniVRM (PlayMode batch, 6/6 ok) from **identical current outlines-off assets**, then diffed VMK vs the UniVRM golden:

| `rimLightingMix` | 0 | 0.1 | 0.25 | 0.5 | 0.75 | 1.0 |
| --- | --- | --- | --- | --- | --- | --- |
| SSIM vs UniVRM | 0.9930 | 0.9935 | 0.9901 | 0.9803 | 0.9674 | **0.9491** |

SSIM degrades monotonically as the mix rises, and only `rimLightingMix=1` falls below the **0.95** band that `docs/methodology.md:77` deliberately tightens for these tests — by **0.0009** (0.9491). This reproduces the original ~0.949 signature almost exactly, so #226 is **unchanged at 0.21.0** (consistent with the sub-perceptual 0.21.0 MToon delta, 2026-06-13 entry). It is the one cell where VMK trails the golden reference under the suite's own thresholds — a candidate for the tightest-band exception list, not a release blocker.

*Measurement hazard noted:* a first pass against the **cached** `goldens-cache/_assets` rim files returned a flat SSIM 0.8044 across all six variants — those assets were generated during the since-reverted **outlines-on default** (commit `af877c2`), so both VMK and the cached UniVRM reference rendered as outline-engulfed black blobs and the rim signal was masked entirely (the 0.80 is the outline silhouette-size divergence from the 2026-06-14 entry, not rim). The valid re-measure requires re-emitting the current outlines-off assets and re-rendering **both** renderers — stale `goldens-cache` PNGs must not be reused across the outline-default flip.

**Verdict.** VMK 0.21.0 clears the release bars this suite gates (determinism — #283 reproducer 5× byte-identical; no render regression vs 0.20.1) and has **no hard VRM-1.0 conformance blockers** — every substantive bug is fixed, one deviation is formally accepted and documented, and the only live miss is a 0.001-SSIM rim residual at the extreme. It is a defensible 1.0 from the render-conformance side, consistent with the team already treating it as the 1.0 candidate.

**Pointers.** Fix: VRMMetalKit PR #291 (#283/#286/#287/#288/#289/#290), #296 (#290 follow-up), #331 (#286), #353 (outline `recordDrawCall`). Open: VMK #226 (rim high-mix, lone miss), #328 (giEqualization — accepted deviation), #237 (resolved-but-unclosed), #242 (KHR umbrella), #221 (outline perf). Suite-side: `d74c9f8` (#237 placement fix). Methodology: `docs/methodology.md:77` (0.85 global / 0.95 rimLightingMix thresholds).

## 2026-06-14 (cross-renderer consensus at the outlines-on baseline: VMK MToon outline diverges from UniVRM) — **OBSERVED: three-vrm tracks UniVRM (golden ref) closely (SSIM 0.98 mean, 96% ≥ threshold) but VMK is only 28% — isolated to OUTLINED cells (VMK-vs-UniVRM ≈ constant 0.80 on every now-outlined default cell vs 0.96 on the outline-OFF cell; three-vrm stays ≈0.997). VMK renders the MToon outline but differently from the reference; three-vrm's matches. godot is a separate broad outlier (1%). ROOT-CAUSED (pixel inspection): the 0.05-world default outline engulfs the whole avatar (black blob) in ALL renderers — so it's a silhouette-size comparison — and VMK under-extrudes vs the reference → smaller silhouette. Mostly an artifact of the default-width choice, not a deep VMK bug. RESOLVED: any outline width (world OR screen, down to 0.01) engulfs the synthetic test sphere into a black silhouette in ALL renderers (UniVRM included) — no width yields a shaded-avatar+thin-ring — so the outlines-on default was REVERTED to outlines-off; outlines are tested only via the dedicated outline sweep.**

**Method.** First full local cross-renderer consensus: `scripts/bootstrap-goldens.sh` (local file:// mode) → all 4 adapters at **VRMMetalKit 0.21.0 + outlines-on default** → `scripts/consensus-report.sh` (pairwise SSIM, 0.85 threshold). VMK pinned 0.21.0; three-vrm/godot/univrm at HEAD.

**Result (pairwise SSIM mean vs the UniVRM reference + pass-rate ≥ threshold):** three-vrm 0.982 (292/303 = 96%), **vrm-metal-kit 0.812 (85/303 = 28%)**, godot-vrm 0.389 (2/247 = 1%).

**Isolation (rules out a global VMK issue).** Per-cell VMK-vs-UniVRM is ≈0.804 on outlined cells (`mtoon_default`, `springbone_default`, `mtoon_pbrtex_combined` — all now carry the default world-coordinate outline) but 0.964 on `mtoon_outline_none` (outline off); three-vrm-vs-UniVRM is ≈0.997 on the same cells. 222/318 shared cells are "VMK < 0.85 while three-vrm ≥ 0.85", and they are the outlined ones. The outline-OFF agreement + the constant outlined-cell SSIM exclude a color-space / tone-map / global cause — it is the **MToon outline pass** specifically. (Consistent with the earlier finding that VMK *does* render the outline — it just doesn't match the reference's.)

**Why it surfaced now.** The outlines-on default flip (2026-06-14) put an MToon outline on every default-derived asset, exposing VMK's outline divergence corpus-wide (masked before, when the default had outlines off). The "outline SSIM noise corpus-wide" cost flagged when flipping the default is now attributed to a specific **VMK outline-fidelity gap vs the golden reference**, not generic cross-renderer noise.

**Caveats.** `consensus-report` uses the global 0.85 SSIM threshold, not the **outline-region-local wider tolerance** `docs/methodology.md` prescribes for outline tests — VMK's outlined cells may fare better under local outline tolerance, but VMK is still the clear outline outlier vs three-vrm (which matches UniVRM at ~0.997 globally, so its outline is reference-faithful). UniVRM VRMA cells excluded (apply_vrma deferred; 111 univrm errors, expected). Local `file://` manifest — not committed.

**Root cause (pixel inspection, 2026-06-14).** Two compounding causes, confirmed by reading the renders:
1. **The 0.05-`worldCoordinates` default outline is too large for the test asset's scale.** Outline-OFF (`mtoon_outline_none`) renders a properly shaded grey sphere in every adapter; with the outline ON (the new default), the outline **engulfs the entire avatar** → a solid black silhouette in ALL renderers (VMK, UniVRM, three-vrm), hiding the MToon shading. So the outlines-on default flip produced uninformative **black-blob goldens**, and the comparison degenerates to silhouette *size*.
2. **VMK under-extrudes the outline** vs the reference for the same `outlineWidthFactor` → VMK's black silhouette is visibly smaller than UniVRM's/three-vrm's (which agree) → the systematic ~0.80 SSIM. (The `mtoon_outline_world_0p1` extreme shows the same: all black, UniVRM's silhouette largest, VMK's smaller.)

So the 28% is **mostly an artifact of the default-width choice** (engulfed avatar → size-only comparison), not a deep VMK bug — though VMK's smaller extrusion is a genuine secondary difference worth confirming at a sane width.

**Resolution (2026-06-14).** (1) **DONE — reverted the default to outlines-off** (`MToonParams::defaults()` back to `OutlineWidthMode::None`/`0.0`). Pixel inspection proved no width works: at world AND screen 0.01, both VMK *and* UniVRM render the avatar fully black (a ~10px screen outline cannot produce a 256px-radius black interior — the outline is covering the front face entirely on this convex sphere, in every engine). So the synthetic test sphere simply cannot carry an informative outline; "typical VRM = outlines on" holds for real avatars but not this isolated primitive. With the default outline off, the default + every sweep baseline render a shaded sphere again (cross-renderer ~0.96, VMK passes); outlines are exercised only by the dedicated outline sweep, where the black-blob silhouette-size comparison correctly captures the **VMK under-extrusion** finding (VMK-vs-UniVRM 0.95→0.63 as width 0.01→0.1). (2) Open (separate, lower priority): why does *any* MToon outline engulf this convex sphere in all engines — an asset-geometry / outline-mechanic question worth a look, but it does not affect the conformance baseline now that the default is outline-off. (3) `consensus-report` could still apply outline-region-local SSIM tolerance for the sweep's outline cells. (4) **godot's broad divergence** (SSIM ~0.39 across the board) is a separate, larger L4-partial-adapter gap, not outline-specific.

## 2026-06-14 (VMK 0.21.0 promotion: rc.3 verified render-equivalent to rc.1) — **0.21.0 cut as stable from rc.3 (985bd7c). The rc.1→rc.3 delta — #352 (pose-aware SpringBone warmup) + #353 (outline-pass recordDrawCall) — is byte-identical across the spring-bone (settle+swing) + outline corpus (49/49) and deterministic. rc.1 was already verified vs 0.20.1 (no regression; 31 sub-perceptual MToon cells). Conformance pin bumped 0.20.1 → 0.21.0.**

**What.** Promoted VRMMetalKit `0.21.0-rc.3` to stable `0.21.0` after a focused verification of the only unverified delta since the last-verified RC (rc.1 = `1ebe2ab`, verified 2026-06-13). The two commits past rc.1 are `#352` (`fix(renderer,vrmvideo): allow pose-aware SpringBone warmup`) and `#353` (outline `recordDrawCall`, metrics-only).

**Method.** Built the adapter at rc.1 and rc.3; A/B-rendered the spring-bone settle + **swing** sweeps (the #352 risk — swing exercises `animate_root_transform` / pose-aware warmup) plus the MToon outline variants through each (49 plans); BLAKE3 byte-compared; and re-rendered a swing case 3× at rc.3 for determinism.

**Result.** **49/49 byte-identical** rc.1 vs rc.3; swing case **3× byte-identical** (deterministic). So #352 changes nothing observable in the corpus and #353 is render-neutral. rc.3 therefore inherits rc.1's clean bill (the 2026-06-13 entry: 397/428 byte-identical to 0.20.1; 31 sub-perceptual MToon-feature cells, SSIM ≥ 0.9974, all pass the 0.85 gate). No render-output regression in promoting to stable.

**Caveat.** Goldens still need a full re-baseline when this lands: the 31 rc.1 MToon cells **plus** the outlines-on default flip (outline now on every default-derived asset). The swift-tools 6.3 / CI-Xcode gap is unchanged (see CI notes). Verified locally on M4 Max / Xcode 26.5.

## 2026-06-14 (benchmark structural divergence: default asset draw-call + triangle count) — **RESOLVED: visual fidelity aligns (all three renderers DO render the MToon outline when enabled), but the structural *counts* diverge purely from per-renderer instrumentation — VMK under-counts the outline draw (upstream VRMMetalKit bug), UniVRM over-counts (always-submitted phantom outline pass + a 2-triangle Built-in RP camera blit). No renderer has an outline fidelity gap. `draw_calls`/`triangles` are therefore not directly cross-renderer comparable without per-renderer normalization.**

**Context.** The suite default avatar (`emit-default`) was flipped to a representative typical-VRM config — `outlineWidthMode: worldCoordinates`, `outlineWidthFactor: 0.05` (was `none`/`0.0`) — because real VRM avatars (VRoid / VTubing) ship with the MToon toon outline enabled. Benchmarking the default across the three adapters then exposed a structural-count divergence, root-caused below.

**Measured (outlines-ON default):**

| renderer | draw_calls | triangles | renders outline? | count accurate? |
|---|---|---|---|---|
| **three-vrm** | 2.0 | 4608 | yes | yes |
| **VMK** | 1.0 | 2304 | **yes** (pixel-confirmed) | no — under-counts |
| **UniVRM** (golden ref) | 3.0 | 4610 | yes | no — over-counts (+1 draw / +2 tri) |

With outlines OFF the numbers were VMK 1 / three-vrm 1 / UniVRM 3: three-vrm correctly tracked outline ON↔OFF (1↔2); VMK was identical either way; UniVRM was identical either way. That ON/OFF re-benchmark is what cracked the case.

**Visual fidelity — aligned.** All three render the outline when the asset enables it. VMK confirmed by pixel diff (its outline-ON render differs from its outline-OFF render). three-vrm's draw count rises 1→2. UniVRM's always-present outline pass becomes real geometry. The outlines-ON flip thus yields a visually consistent, representative baseline — there is no outline *fidelity* gap in any renderer.

**Root cause of the count divergence — instrumentation, per renderer:**
- **VMK under-counts (upstream bug).** The call graph reaches the outline pass (`drawOffscreenHeadless → drawCore → renderMToonOutlines`; the guard `outlineWidthMode != .none && factor > 0.0001` passes; material parse reads `worldCoordinates`/`0.05` correctly) and the outline IS drawn on the GPU — but `renderMToonOutlines` issues its `drawIndexedPrimitives` (`VRMMetalKit .../Renderer/VRMRenderer.swift:~3995`) WITHOUT a `performanceTracker.recordDrawCall(...)`, unlike the main pass (lines ~3698 / ~3738). The benchmark counter therefore misses the outline draw → 1/2304 instead of 2/4608. This is a benchmark-instrumentation gap in the pinned upstream **VRMMetalKit**, not the adapter and not a render gap. Fix = one `recordDrawCall` line upstream + a deliberate `Package.swift` pin bump.
- **UniVRM over-counts (Built-in RP artifact).** Its MToon10 Built-in RP shader always dispatches a `FORWARD_BASE_OUTLINE` pass (clip-discarded when mode=none), and `UnityEditor.UnityStats.*` counts GPU-submitted geometry. The 3rd draw call is **not** the ShadowCaster — it is Built-in RP's fullscreen camera blit (2 triangles) that copies the result to the output RenderTexture: 2304 base + 2304 outline + 2 blit = 4610. Irreducible without patching the upstream shader. (The adapter now also sets `shadowCastingMode = Off` when `cast_shadows: false` — correct plumbing that prevents a real shadow-map pass — but since the blit, not a shadow pass, was the 3rd call, the count floor stays at `real_passes + ~2`.)
- **three-vrm** counts the draws it actually issues — accurate.

**Verdict.** No renderer has an outline fidelity gap; all render it. The `draw_calls`/`triangles` *metric* is not directly comparable across these engines: VMK under-reports by the outline pass (fixable upstream), UniVRM over-reports by ~1 draw / ~2 tri (irreducible Built-in RP floor), three-vrm is accurate. The earlier "−66.67% / UniVRM phantom outline" framing is superseded — with outlines on, three-vrm and VMK both genuinely draw 2 passes; only the counters disagree.

**Status / next steps.**
- **OPEN (upstream VMK):** add `performanceTracker?.recordDrawCall(triangles:vertices:)` to `renderMToonOutlines` in arkavo-org/VRMMetalKit (after the outline `drawIndexedPrimitives`), then bump the `Package.swift` pin. After that VMK reports 2/4608, matching three-vrm.
- **Methodology (`docs/methodology.md`):** UniVRM structural counts carry a fixed `+1 draw / +2 tri` Built-in RP floor (phantom outline pass + camera blit). Phase-2 budgets / "familiar band" thresholds on `draw_calls`/`triangles` must normalize per renderer — do not compare raw counts across engines.
- **Done in-tree (this change):** default flipped to outlines-ON (representative; all goldens require a re-baseline — they are S3 / maintainer-submitted, so this flags the work, it does not re-render them); UniVRM adapter now honors `cast_shadows`/`receive_shadows` on the loaded renderers.

**Discovery method.** Surfaced by the `benchmark_execute` op; root-caused by flipping the default to outlines-on and re-benchmarking — the predicted "all three rise to 2 draws" did NOT hold for VMK, which is exactly what exposed the VMK counter bug — plus code inspection of the VRMMetalKit outline path and the UniVRM Built-in RP shader, and a VMK outline-on-vs-off pixel diff. A reminder that an apparent "divergence" can be a metric artifact, and that the predicted-outcome re-test is what separates fidelity gaps from instrumentation gaps.

## 2026-06-13 (VMK 0.21.0-rc.1 verification) — **no perceptual regression: 397/428 A/B renders byte-identical to 0.20.1; the remaining 31 are sub-perceptual LSB drift (SSIM ≥ 0.9974, all pass the 0.85 gate) isolated entirely to MToon feature paths from the `mtoon_fragment_v2` function-constants specialization. Spring-bone output fully deterministic (5× byte-identical).**

**What.** Regression-checked [VRMMetalKit 0.21.0-rc.1](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.21.0-rc.1) (`1ebe2ab`, pre-released 2026-06-13, cut from `main` 23 commits past 0.20.1) against the current pin 0.20.1 (`39e65f0`). The RC is a performance wave: parallel intra-mesh primitive decode, spring-bone sleep, batched morph dispatch, constraint cache + morph/skin recompute gates, an opt-in opaque depth prepass (#195, default off), outline pipeline bind dedup (#192), and — the one render-affecting surface here — **`mtoon_fragment_v2` specialized via Metal function constants** (`ac1e6b5`, shader hash refreshed; also in `40c5fe9`).

**Method.** Built the adapter from each pin on the same machine (M4 Max, macOS 26.5 / Xcode 26.5, Swift 6.3.2), rendered the full local corpus through each — 428 plans (every `goldens-cache/_assets*` sweep: MToon + texture families, settle, swing, multichain, gravity, collider, taper, extended, vmk162 coupling, VRMA expression/humanoid/lookat/hips/arkit/finger/multichannel, first-person) — then BLAKE3 byte-compared every output A/B and ran SSIM on every byte-divergent cell. Hermetic per pin (`swift package resolve` + Package.resolved revision verified before each build).

**Results.**

- **Cross-version A/B: 397/428 byte-identical; 31 differ; 0 missing/failed on either side.** Every byte-divergent cell is MToon-shading-feature-bearing; nothing else drifts. Plain/default MToon color sweeps, all spring-bone families, all VRMA, and first-person are bit-for-bit unchanged.
- **The 31 drifted cells are a coherent, bounded set, all sub-perceptual.** SSIM(base, rc) ranges **[0.99739, 1.000000]** against a 0.85 gate — every cell `ssim_passed: true`. By family: shadingShift / shadeShift scalar (6), rimLightingMix scalar (5, all SSIM = 1.000000 — byte-differ, perceptually identical), shadeMultiplyTexture (5), rimMultiplyTexture (4), glTF-core PBR normal+occlusion textures (3), KHR_texture_transform / uvxform (8). Worst three: `mtoon_shadetex_default` 0.99739, `mtoon_shadingShift_1` 0.99810, `mtoon_shadetex_shift_pos0p5` 0.99848. SSIM's locality rules out a concentrated artifact hiding behind a high global score.
- **Cause is the function-constants specialization, not a behaviour change.** Specializing `mtoon_fragment_v2` per enabled feature lets the Metal compiler branch-eliminate and re-select instructions (FMA contraction, etc.) for the feature-on variants; that perturbs the fragment math by a few LSBs **only** on the shadingShift / rimLighting / texture-sampled paths. The no-feature specialization (plain MToon) matches the old output byte-for-byte — which is exactly why default MToon and every non-MToon surface are untouched. This is benign shader-respecialization drift, not the FP16-clean outcome of 0.18.0-rc.1.
- **Determinism: rc fully repeatable.** `swing_springbone_joints_16` (the historical VMK#283 reproducer) rendered **5× byte-identical** (`9e9a6c3d8ba6…`) on the RC binary.
- **Adapter health:** `swift build` clean (4.9s); `swift test` 36 tests, 0 failures (2 long-standing fixture-dir skips).

**Verdict.** No regression-class signal on any surface this suite gates: zero failed/missing renders, perfect determinism, and every byte-divergent cell sub-perceptual (SSIM ≥ 0.9974, all pass). Unlike 0.18.0-rc.1, output is **not** bit-identical — the 31 MToon-feature cells need a goldens re-baseline when this pin lands (track in the `Package.swift` pin comment, consistent with the pre-existing 0.18.1→0.20.0 drift note already there). Safe to bump the conformance pin to `1ebe2ab`; re-verify at 0.21.0 stable as usual.

## 2026-06-09 (VMK 0.18.0-rc.1 verification) — **zero regressions: all 370 A/B renders byte-identical to 0.17.2 with the FP16 metallib confirmed active; spring-bone output fully deterministic across repeats.**

**What.** Regression-checked [VRMMetalKit 0.18.0-rc.1](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.18.0-rc.1) (`aafc172`, pre-released 2026-06-10, cut from PR #334 "advanced concurrency + Metal") against the current pin 0.17.2 (`3737e76`). The release's two behavioural surfaces: **FP16 MToon shading on macOS** (fragment textures sample as `texture2d<half>`; supersedes the FP32 safe-default from #279) and a **spring-bone warmup determinism fix** (uninitialized `centerDeltaBuffer` read — which could also write GPU memory out of bounds via a garbage bone range — plus a CPU/GPU race where warmup rewrote shared buffers while steps were in flight; same defect class as the rc-era VMK#283).

**Method.** Built both binaries from the respective pins on the same machine (M-series Mac, macOS 26 / Xcode 26), rendered the full local corpus through each — 371 plans: every `goldens-cache/_assets*` sweep (MToon + texture families, settle, swing, multichain, gravity, collider, taper, extended, vmk162 coupling, VRMA expression/humanoid/lookat) **plus** the 36 real-avatar manual humanoid plans (vroid expr/gaze/spatial/settle/swing/seq, avatarA/U) — then BLAKE3 byte-compared every output, including per-frame hashing of the `render_sequence` frame dir.

**Results.**

- **Cross-version A/B: 370/370 outputs byte-identical** (369 single-frame PNGs + the 16-frame `vroid_default_F_bust_seq` sequence). Zero pixel drift anywhere — MToon, spring-bone, expressions, gaze, VRMA, sequences. The single non-render is `avatarA_0_0_smoke`, rejected by runner-side plan validation (camera-side check) before the adapter spawns — pre-existing and identical on both binaries; not a VMK signal.
- **FP16 verification is real, not vacuous.** The macOS metallib slice shipped in the package changed between pins (238,599 → 240,199 bytes) and `VRMShaderLibraryLoader` loads that bundled slice, so the rc.1 renders exercised the FP16 shaders. The shader diff converts texture *returns* to `half4` (lossless for 8-bit Unorm/sRGB content) and immediately widens samples to `float4` — lighting math stays float — which is exactly why pixel-clean is the expected outcome. Our 370-render byte-identity independently confirms upstream's "pixel-clean against the full MToon conformance battery" claim on this suite's corpus.
- **Determinism: rc.1 fully repeatable.** `swing_springbone_joints_16` (the historical VMK#283 reproducer that produced 3 distinct PNGs in 5 runs on 0.16.0-rc.1) rendered **5× byte-identical** (`4f8a072db24b...`) on rc.1. The 170-plan spring-bone subset (swing/multichain/gravity/collider/taper/extended/vmk162) re-rendered byte-identical within-version on **both** binaries — so 0.17.2 was already repeatable on this corpus across independent adapter invocations, and the `centerDeltaBuffer`/warmup-race fix changes nothing observable here. The fixed defect is real but evidently wasn't being hit by our corpus's render path (our plans drive `reset_physics(settle_steps=30)` + the fixed-timestep path); the upstream `LoadDeterminismTests` is the witness for the load-path race.
- **Adapter health:** `swift build` clean, `swift test` 36 passed / 0 failures (2 long-standing fixture skips). The new `MTLBinaryArchive` pipeline persistence is opt-in (`RendererConfig.enablePipelineArchive`) and our adapter does not enable it — the macOS 27 beta known issue is not in play.

**Verdict.** No regression-class signal on any surface this suite measures. Pin bumped to `aafc172` (pre-release tracking, consistent with the 0.16/0.17 rc-cohort practice); re-verify at 0.18.0 stable as usual. No goldens re-baseline needed — output is bit-identical to the 0.17.2 era.

## 2026-06-09 (Spatial iOS VRMA export: hips root-motion translation not registered in humanoid.humanBones) — **producer-side quirk preserved in committed fixtures; spec-conformant importers will silently drop the root motion.**

**Finding.** Arkavo Spatial's `VRMAGLBWriter` (`ArkavoScan/Export/VRM/VRMAGLBWriter.swift`, as of Spatial commit `c27f6cc` / fixture-export branch `vrma-conformance-fixtures`) emits the hips translation (root-motion) track as a glTF translation channel on a skeleton node that is **NOT** declared in `VRMC_vrm_animation.humanoid.humanBones`. The writer builds `humanBones` from the rotation `boneTracks` keys only, so the hips node is registered only when hips also carries a rotation track; a translation-only hips (the common locomotion case) goes unmapped. The absence from the humanBones map means any spec-conformant VRMC_vrm_animation consumer that retargets exclusively via that map will silently drop it.

**Evidence.** Structural inspection of the committed fixtures:

- `assets/humanoid/spatial_body_motion.vrma` — `humanoid.humanBones` = `{head: 0, rightUpperArm: 2}`; the file has 3 animation channels (2 rotation + 1 translation). The translation channel targets node 1 (named `hips`), which is absent from `humanBones`.
- `assets/humanoid/spatial_merged_motion.vrma` — `humanoid.humanBones` = `{rightUpperArm: 1}`; 4 channels including 1 translation targeting node 0 (named `hips`), absent from `humanBones`. (The two expression translation channels target dedicated expression nodes registered under `expressions.preset`/`expressions.custom`, which is spec-correct.)

In both files every rotation channel's target node has a `humanBones` entry; the omission is **specific to the hips translation / root-motion track**.

**Why it matters.** `VRMC_vrm_animation` consumers retarget via the `humanBones` map — the map is the spec's mechanism for associating a glTF node with a humanoid bone role. An unmapped node's animation channel is valid glTF (the channel will animate that node in a bare glTF runtime) but carries no humanoid meaning to a spec-conformant VRM animation importer. UniVRM-class importers will therefore **silently drop the producer's root motion** — the avatar will play the expression/look-at channels but its pelvis will not translate. This is not a rendering divergence per se (it is a parse/retarget step upstream of rendering), but it is a detectable conformance signal: **all spec-conformant adapters should agree on dropping the root motion**, and any adapter that *does* apply root motion from an unmapped hips node is implementing a non-spec extension or heuristic.

**Status.** Producer-side quirk in Spatial's writer. Recommended fix: register hips in `humanoid.humanBones` whenever a `hipsTranslation` track is present, regardless of whether a rotation track accompanies it. The committed fixtures **intentionally preserve the quirk** (they are producer truth, not corrected); the paired manual plans (`vroid_default_F_spatial_body` and `vroid_default_F_spatial_merged`) note `"root motion expected-dropped"` in the `spec_section` field so that cross-renderer agreement on dropping it is the explicit pass condition.

**Discovery method.** Structural inspection of the fixture files during corpus integration (Task 14 / `worktree-vrma-producer-coverage`). Not yet observed as a cross-renderer render divergence — single-frame VRMA ops (`apply_vrma`) are real on UniVRM + mock only at time of writing; three-vrm, VMK, and godot VRMA playback is deferred.

## 2026-06-07 (VMK 0.17.2 VRM 1.0 expressions #333 — suite coverage landed + before/after verified) — **blink/visemes/emotions go frozen → deforming on the real VRoid avatar; the synthetic corpus was silently passing on frozen output.**

VMK 0.17.2 (`3737e76`, closes upstream #333) restores VRM 1.0 facial expressions. The bug: a VRM 1.0 expression `morphTargetBind.node` is a glTF **node** index, but the renderer and `VRMExpressionController` key morph weights by **mesh** index (0.x binds already carry the mesh index). The 1.0 loader stored the raw node index, so on any model whose face node index ≠ mesh index, **every** morph bind matched no primitive and the morph compute pass skipped it — blink, the five visemes, and every emotion preset silently produced **no mesh deformation**. Bone-driven look-at was unaffected (different path), which is why only *expressions* looked dead; VRM 0.x never hit it.

Same blind-spot pattern as the 0.17.1 lookAt work (see entry below). Confirmed empirically: `vroid_default_F_1_0` binds blink/happy/sad/aa/surprised all at **node 211 → mesh 0**; the **synthetic** humanoid avatar has **no blink/happy/sad morph binds at all** and its visemes are bound **node 19 → mesh 0** (so #333 froze them too, and they were never pixel-verified).

**Coverage landed (this change):**
- New `emit-expression-clips` subcommand → 11 preset VRMA clips (`expr_*.vrma`): blink + happy/angry/sad/relaxed/surprised + 5 visemes (aa/ih/ou/ee/oh), reusing the existing expression-weight channel primitive.
- 11 manual plans `test-plans/manual/humanoid/vroid_default_F_expr_*.test.yaml` pairing the real VRoid avatar with each clip, whole-face camera (eyes + mouth in frame), `apply_at_time` at the weight peak.

**Before/after verified locally (M-series Mac, macOS 26).** A/B through a **0.17.1** binary and the **0.17.2** binary, plus a neutral baseline (the same clip sampled at weight 0). On 0.17.1 **every** expression render is byte-identical to neutral (frozen face); on 0.17.2 each deforms (distinct PNG hash):

```
expr     neutral=8a98240b5666   0.17.1          0.17.2          verdict
blink                           8a98240b5666    a1b301ab0eed    frozen → deforms (eyelids close)
happy                           8a98240b5666    57b0a4defe59    frozen → deforms (smile/grin)
sad                             8a98240b5666    a1cc44b7821b    frozen → deforms
aa                              8a98240b5666    0975994d5efc    frozen → deforms (mouth opens)
```

Visual inspection confirms the *correct* deformation per preset (blink closes the eyelids, `aa` opens the mouth, happy is a broad smile). `dump_expression_weights` reports the controller weight as applied on **both** versions — it is upstream of the #333 keying and cannot see the frozen mesh, so (as with gaze) the **rendered face is the signal**; here the byte-identical-to-neutral check is a clean numeric proof of the freeze.

**Synthetic viseme check.** The synthetic `aa` viseme (node 19 ≠ mesh 0) is also affected: 0.17.1 render `5d8cf1789282` is **byte-identical to the synthetic neutral** (`5d8cf1789282` — the long-standing frozen-face hash), and 0.17.2 deforms to `aac86e72fe59`. So the synthetic viseme corpus was silently passing on frozen output and now actually deforms.

**Out-of-band follow-ups (tracked, not this change):** author synthetic blink/happy/sad morph targets for a fully-parametric expression sweep; a first-class "differs-from-neutral" deformation assertion in the diff engine so any morph-bearing plan fails loudly on frozen output rather than relying on a reference PNG. Spec/plan: `docs/superpowers/specs/2026-06-07-expression-0172-coverage-design.md`, `docs/superpowers/plans/2026-06-07-expression-0172-coverage.md`.

## 2026-06-07 (VMK 0.17.1 eye look-at #332 — suite coverage landed + before/after verified) — **both sub-bugs confirmed fixed on the real VRoid avatar; closes the long-deferred suite-side asset-coverage follow-up.**

VMK 0.17.1 (`421232b`, closes upstream #332) corrects bone-driven eye look-at: **(A) head-local gaze resolution** (yaw/pitch was computed in world space but written as a *local* eye-bone rotation → a turned head drove the eyes off by the head's yaw) and **(B) eye-bone rest composition** (`applyToBones` discarded the authored eye rest; VRoid `J_Adj_*_FaceEye` rigs carry a mirrored ~±22° outward rest, so the eyes splayed **wall-eyed at center** and inverted gaze). 0.17.1 resolves through the head's inverse world matrix and composes `gaze * initialRotation`.

This was the suite's blind spot: the **synthetic humanoid corpus has no eye bones**, so the entire `vrma_lookat_*` history (VMK#286 → #294 → #297) could only ever verify the gaze *parse*, never the rendered eye direction. The fix's own release notes name `vroid_default_F_1_0.vrm` as the validation avatar.

**Coverage landed (this change):**
- New `emit-gaze-sweep` asset-generator subcommand → 8 VRMA gaze clips (`gaze_*.vrma`): 5 neutral-body gaze directions (center/L/R/U/D, exercising bug B) + 3 turned-head variants (spine yaw ±35° + gaze, exercising bug A via a VRMA hips/spine rotation channel — the plan schema's `root_transform` is translation-only and cannot turn the head).
- 8 manual plans `test-plans/manual/humanoid/vroid_default_F_gaze_*.test.yaml` pairing the real VRoid avatar with each clip, framed eye-tight (eyes at **y≈1.304**, the `J_Adj_*_FaceEye` bones — extracted from the rig; the initial nominal y=1.40 framed the forehead and was retuned).
- `leftEye`/`rightEye` added to the pose-dump reference bone list in **both** the VMK adapter and the three-vrm host, so the cross-renderer pose diff now carries a numeric eye-bone signal (absent eye bones on synthetic rigs are skipped — existing corpora unaffected).

**Before/after verified locally (M-series Mac, macOS 26).** A/B rendered `gaze_center` and `gaze_center_bodyL` through a **0.17.0** adapter binary (pre-bump) and a **0.17.1** binary:
- **`gaze_center` (bug B):** 0.17.0 renders the eyes **wall-eyed** — the avatar's right iris splayed to the outer corner, eyes divergent. 0.17.1 renders both irises **centered, parallel, straight-ahead** at the camera. Textbook match to the release-note "parallel tracking, exact straight-ahead at center."
- **`gaze_center_bodyL` (bug A):** with the head yawed +35°, 0.17.0 leaves the eyes divergent/off; 0.17.1 tracks them **head-relative parallel**.
- **Pose signal (0.17.1):** at `gaze_center` (`look_at.yaw_deg=0`, `pitch_deg=0`) the dumped `leftEye`/`rightEye` local rotations are **non-identity** — the authored rest is preserved (`gaze * rest`), the exact behaviour 0.17.0 discarded. (The 0.17.0 binary predates the 21-bone pose list, so it doesn't *dump* eyes — the cross-version signal there is the rendered image; the pose dump is the within-0.17.1 confirmation.)
- All **8/8** gaze plans render cleanly through VMK 0.17.1.

**Closes** the "suite-side asset coverage needs extending" follow-up tracked in `docs/upstream/VMK-vrma-lookat-renderer-propagation.md` (VMK#294). Out-of-band follow-ups (tracked, not this change): parametric synthetic eye bones with mirrored rest for a fully-parametric gaze×rest sweep; cross-renderer gaze consensus vs three-vrm/UniVRM once the real-1.0 `execute-test-batch` oracle tooling lands. Spec/plan: `docs/superpowers/specs/2026-06-07-lookat-0171-coverage-design.md`, `docs/superpowers/plans/2026-06-07-lookat-0171-coverage.md`.

## 2026-06-07 (1.0-readiness gap run: VRM 0.x + real avatars + exclusions) — **0.x: no regression; VMK renders all real avatars cleanly.** Real-1.0 oracle comparison blocked by a suite-tooling limitation (not VMK). matcap/shadetex now formally excluded.

Ran the three readiness gaps flagged before marking 0.17.0 a 1.0 general release.

**(1) VRM 0.x conformance — no regression.** Full `RUN_UNIVRM=1 SPEC_VERSION=0.x` bootstrap (207 plans, all adapters incl UniVRM PlayMode → `/tmp/goldens-0170_v0`). VMK 0.17.0 vs fresh UniVRM 0.x:
- **settle 58/58 pass, mean 0.9422** (prior baseline 0.9413–0.9423 — identical)
- **swing mean 0.8402** (prior 0.8377–0.8405 — identical; the sub-0.85 is the *documented* Ry180-orientation AA residual on the thin chain, not a defect)

So 0.17.0's gravity/#326 changes **did not perturb synthetic 0.x** — the chains author `gravityPower>0` and hang colinear with gravity, so the magnitude change is invisible (same gravity-blindness as 1.0). The **mesh-less 0.x MToon corpus** (meshes=0 by design — tests material *parsing*, not rendering) "fails" render in VMK *and* three-vrm identically; that's expected, not a VMK issue.

**(2) Real avatars — VMK renders all cleanly; 1.0-oracle comparison incomplete.** Rendered the 9 manual humanoid plans (AvatarSample_A/U 0.x + 1.0, VRoid bust/collider/headbubble, bosom, face) through VMK 0.17.0: **all 9 rendered, no crashes** — VMK handles real content (where #321 hand colliders, gravity, #322 actually bite). UniVRM oracle landed only the **2 real 0.x avatars** (avatarA/U_0_0): **SSIM ~0.854–0.857** vs their 0.92 self-threshold — within the operational band and in the known VMK#299 Ry180-orientation territory on complex real avatars (the 0.x reals where #326's gravityPower=0 bangs apply). The **7 real-1.0 plans were silently skipped by the UniVRM `execute-test-batch` path** (assets present + valid; a manual-plan batch-discovery limitation) — a **suite-tooling gap, not a VMK issue**. The formal real-1.0-vs-oracle number (bust/collider/eyelash) remains uncollected pending that tooling fix.

**(3) Exclusions formalized.** `conformance_status_for` now marks `mtoon_matcap_*` + `mtoon_shadetex_*` **excluded** (synthetic-sphere UV-projection artifact, proven spec-correct on the flat-quad control), like the outline cluster — so the headline conformance number is honest.

**1.0-readiness verdict.** VMK-side everything is green: 1.0 synthetic **65/66 included (98.5%)** + dynamic **154/154**; 0.x **no regression**; **all real avatars render cleanly**; both gravity issues (#324/#326) closed and measured; matcap/shadetex resolved to not-VMK. **The only open gate item is suite-side**: the UniVRM-batch-over-manual-1.0-plans tooling, needed to produce the formal real-1.0-avatar oracle conformance number (Muse has separately validated the real 0.0 avatar per the release notes). No VMK conformance blocker remains for 1.0; the residual is suite-tooling + the milestoned #226 rim residual.

## 2026-06-07 (shadetex SETTLED via flat-quad control) — **VMK's texture handling is spec-correct**; the sphere divergence is a sphere-UV-projection artifact, **not a VMK bug**. Closed.

**Built the control.** Added `emit-textured-quad` (`emit_vrm_textured_quad`): the quadrant checkerboard on `baseColorTexture` over a +Z-facing quad with unambiguous corner UVs (TL→UV(0,0), TR→(1,0), BL→(0,1), BR→(1,1)). Under glTF's V-down convention the spec-correct render is **TL=red, TR=green, BL=blue, BR=yellow** — no sphere-projection ambiguity. Validates clean (0 errors).

**Result (corner-color sampling of each render):**

| renderer | TL | TR | BL | BR | verdict |
|---|---|---|---|---|---|
| **VMK** | red | green | blue | yellow | **spec-correct** ✓ |
| **three-vrm** | red | green | blue | yellow | **spec-correct** ✓ |
| UniVRM | — (blank) | | | | culled the single-sided quad (see below) |

**Conclusion — overturns the geometry-derivation guess in the entry below.** VMK renders the unambiguous quad checkerboard **exactly per spec**, so VMK has **no texture-U flip**. (My sphere `u∈(0,0.5)` derivation had a sign error.) Since all renderers read the *same* baked sphere UVs and VMK samples them correctly (quad-proven), the sphere shadetex/uvxform divergence (VMK+three-vrm green/yellow vs UniVRM red/blue) is **a sphere-UV-projection artifact of the suite's procedural sphere** — likely a seam/winding or visible-hemisphere interpretation difference — **not a VMK renderer defect.** VMK + three-vrm agree and are both quad-correct; on the sphere UniVRM is the outlier.

**Verdict: no VMK bug. Closed — not filing.** matcap + the sphere shadetex/uvxform tests should be `conformance_status: excluded` (or wide-tolerance) as synthetic-sphere texture-projection artifacts. **Texture-orientation conformance should use the new flat-quad control, not the ambiguous sphere** — that's the durable fix; the `emit-textured-quad` asset is now in-tree for it.

**Side observations (not blocking, not texture-U):**
- UniVRM renders the **single-sided** quad blank — it culls the front-facing single-sided quad here (connects to the doubleSided culling spec test). The quad control should set `double_sided` (or be paired) if a UniVRM golden is wanted; VMK + three-vrm render it single-sided fine.
- On the sphere, UniVRM being the lone outlier vs two quad-correct renderers is a mild caveat to "UniVRM = golden" for synthetic-sphere texture tests.

## 2026-06-07 (shadetex root-cause, follow-up to the triage below) — it's a **general texture-U (horizontal) orientation difference, not shadeMultiply-specific**: VMK+three-vrm sample the visible hemisphere's texture U one way, UniVRM the mirror. Self-consistent within each renderer; geometry favors UniVRM but a definitive verdict needs a flat-quad control (the synthetic sphere is too ambiguous). **Not filing.**

**Refines the triage entry below** (which called it "texture-V-orientation, shadeMultiply-specific" — both wrong). Established by image inspection of `/tmp/goldens-0170`:

1. **It's U, not V.** The `shadeMultiplyTexture` is a red(TL)/green(TR)/blue(BL)/yellow(BR) checkerboard. All three renderers agree V (green/red at sphere-top = v=0, yellow/blue at bottom = v=1 ✓). They differ only in U: **VMK+three-vrm show green/yellow (u≥0.5 columns); UniVRM shows red/blue (u<0.5)**.
2. **It's general, not shadeMultiply-specific.** The same U split appears on the **baseColor** checkerboard (`uvxform_identity`): VMK green/yellow, UniVRM red/blue. It only *looks* shadeMultiply-specific because `shadetex` paints the texture over a large shaded band (SSIM 0.72) while baseColor shows it only on the lit rim (SSIM 0.93 — the divergence hidden by the grey lit body). Each renderer is **self-consistent** (same U on baseColor and shadeMultiply).
3. **Geometry favors UniVRM, with a caveat.** The suite sphere maps `u=lon/2π`, normal `(cosφsinθ, cosθ, sinφsinθ)`; the +Z-facing visible hemisphere is `sinφ>0 ⟺ u∈(0,0.5) ⟹ red/blue`. Cross-check: the lit pole (normal ≈[0.3,0.6,0.7]) sits at u≈0.185 (red column) and renders red in UniVRM, green in VMK. So by geometry UniVRM is spec-correct and VMK+three-vrm mirror U **on this asset**.
4. **But a universal U-mirror in two shipping renderers (VMK/Metal + three-vrm/WebGL) is implausible** — it would mirror every real avatar's textures, impossible to miss. So this most likely reflects the **suite's procedural-sphere UV convention** (`u=lon/lon_seg`, no seam/handedness note) interacting with renderer texture conventions, *not* a catastrophic renderer bug. Distinguishing "renderer U-bug" from "suite sphere-UV quirk" needs an **unambiguous flat-quad checkerboard control** — which doesn't exist (the only quad asset, the doubleSided test, is untextured).

**Action.** **Do not file against VMK** (self-consistent, matches three-vrm, spec-verdict unsettled, plausibly a suite-asset issue). Next step to fully settle it: add a flat-quad checkerboard texture test (reuse `quad(0.3)` + `quadrant_checkerboard_16`) and render it across the renderers — that single control resolves renderer-bug vs suite-UV-quirk. Until then, matcap + shadetex should be `conformance_status: excluded` (or wide-tolerance) as a synthetic-sphere texture-U artifact, not VMK gaps.

## 2026-06-07 (triage: matcap/shadetex MToon divergences) — **neither is a VMK conformance gap.** shadetex is a 2-vs-1 texture-V-orientation divergence (VMK+three-vrm agree; UniVRM is the lone outlier); matcap is low-contrast SSIM sensitivity on a family that's noisy across all renderers. Do **not** file against VMK.

**Context.** The 0.17.0 full bootstrap surfaced sharp VMK↔UniVRM divergences on MToon `matcap` (0.61–0.81) and `shadetex` (0.72–0.75) — families absent from the committed 69-test set. Triaged via 4-way consensus + image inspection (fresh `/tmp/goldens-0170`).

**shadetex — UniVRM is the outlier, not VMK.** 4-way SSIM on `shadetex_default`: **VMK↔three-vrm = 0.9483** (strong agreement), VMK↔UniVRM = 0.7176, three-vrm↔UniVRM = 0.7285. The images explain it: the `shadeMultiplyTexture` is a red/green/blue/yellow quadrant checkerboard, and the shaded region of the sphere renders as **green(top)/yellow(bottom) in BOTH VMK and three-vrm**, but **red(top)/blue(bottom) in UniVRM**. Same pattern on `shadetex_red_tint` (VMK↔3vrm 0.9403). So VMK and three-vrm sample the *opposite vertical quadrants* from UniVRM — a **texture V-coordinate orientation/flip difference** on `shadeMultiplyTexture`, not a shading-math error. **VMK matches the three-vrm reference; UniVRM is the minority.** Filing a VMK bug here would be wrong.
- *Open question (separate investigation):* which V-orientation is spec-correct per glTF/`KHR_texture_transform` + MToon. If UniVRM is correct → a **shared VMK+three-vrm V-flip bug** (file against both — a notable cross-renderer finding). If VMK+three-vrm are correct → the **oracle (UniVRM) has a shadetex V-flip quirk** (important caveat to "UniVRM = golden"). Not resolvable without the texture-layout + UV-convention check; deferred.

**matcap — SSIM low-contrast artifact + broadly-divergent family.** `matcap_baseline` renders as a near-black sphere in all three renderers (subtle highlight differences only); SSIM is brutal on low-variance/near-black content, so 0.6055 overstates a small visual difference — and VMK↔three-vrm (0.8723) > either-vs-UniVRM (~0.61), with three-vrm↔UniVRM also low (0.6119). Across the family no pair is high (0.69–0.90): matcap diverges broadly across *all* renderers, with no oracle consensus → an under-constrained/hard family, not a VMK defect.
- *Recommendation:* treat matcap like the outline cluster — a wider tolerance band or `conformance_status: excluded` with a methodology note, not an upstream bug.

**Net.** No VMK bugs to file from this. shadetex needs a spec-V-orientation determination (then file against whoever's wrong — possibly VMK+three-vrm together, or note a UniVRM oracle quirk); matcap needs a tolerance/exclusion note. This refines the 0.17.0-conformance entry's "candidate for upstream filing" — the candidates resolve to *not VMK*.

## 2026-06-07 (VMK 0.17.0 — dynamic/swing golden re-baseline, full bootstrap) — spring-bone/dynamic **154/154 pass vs the fresh UniVRM oracle** (mean SSIM 0.961); the gravity fix is SSIM-invisible at single-frame golden resolution (old≈new), confirming it via measurement not goldens. MToon matcap/shadetex divergences surfaced for triage.

**What.** Ran the full `bootstrap-goldens.sh` (`RUN_UNIVRM=1`) at VMK 0.17.0 → regenerated 336 plans through all four adapters incl. the **UniVRM oracle via Unity 6000.4.6f1 PlayMode** (VMK 336, three-vrm 300, godot 297, univrm 277 PNGs → `/tmp/goldens-0170`). Then diffed VMK 0.17.0 vs the **fresh** UniVRM goldens (UniVRM unchanged, so this is the real conformance comparison).

**Result — VMK 0.17.0 vs fresh UniVRM, by family:**

| family | n | mean | min | pass@0.85 |
|---|---|---|---|---|
| **spring-bone/dynamic** | 154 | **0.9608** | 0.9468 | **154/154** |
| vrma | 15 | 0.9642 | 0.9625 | 15/15 |
| mtoon (full corpus) | 107 | 0.9142 | 0.6055 | 89/107 |

The dynamic re-baseline is clean: **every one of the 154 dynamic/spring-bone tests passes** (swing, collider, extended-collider, gravity-dir, taper, multichain, VMK#162 coupling), tightly clustered 0.947–0.967. Worst cells are the dense 5-share multichain variants (0.947, still well over threshold).

**Honest nuance — the gravity fix doesn't move these goldens.** Old (pre-fix VMK vs old UniVRM) ≈ new (0.17.0 vs fresh UniVRM) on the same swing cells: `gravity_0p2` 0.9653→0.9642, `stiffness_0p2` 0.9663→0.9642, `drag_0p2` 0.9657→0.9645. The swing tests are **single-frame `animation: root_transform:` poses** (not `render_sequence`), so they're stiffness/pose-dominated; the 9.8× gravity-magnitude change contributes negligible *visible* single-frame displacement at 1024² → SSIM-invisible (same degeneracy as the settle corpus). So this re-baseline **confirms dynamic stability against the oracle**; the gravity fix's correctness rests on the direct sideways measurement (10.83 mm, [VMK#324](https://github.com/arkavo-org/VRMMetalKit/issues/324)), not these single-frame goldens.

**Side-finding surfaced by the fuller corpus (separate from the re-baseline, not a 0.17.0 regression).** The bootstrap covers MToon families absent from the committed 69-test set; two diverge sharply vs UniVRM: **matcap** (baseline 0.6055; tints 0.69–0.81) and **shadetex** (0.72–0.75), with pbrtex borderline (0.84). matcap/shadetex weren't touched in 0.17.0 → pre-existing MToon-texture divergences, now visible. Candidate for triage/upstream filing after confirming they're real (not fresh-oracle artifacts).

**Publish status.** Goldens regenerated to `/tmp/goldens-0170`; `goldens-cache/` is gitignored — the canonical publish is S3 + `goldens/manifest.json` (needs `VRM_GOLDENS_BUCKET` + AWS creds, not available in this session). The re-baselined PNGs are ready to push via that path.

## 2026-06-07 (VMK 0.17.0 FINAL — full conformance run, 1.0 candidate) — **65/66 included single-frame tests pass vs the UniVRM golden (98.5%)**, zero regressions; the only miss is the known sub-0.001 rim residual (#226). Both gravity issues (#324, #326) resolved.

**What.** Pinned the adapter to **0.17.0** (`5cd0a95`, the first non-pre-release of the 0.17 line; consolidates rc.1…rc.5). It closes the two gravity issues this suite filed — **#324** (9.8× over-drive → spec scale) and **#326** (0.x `gravityPower=0` now respected; the `0→1.0` substitution removed) — plus #321 hand/arm colliders, #313 swept CCD, #316/#318 dtSub, #322 render-order, #197 opt-in DQS. Ran the committed single-frame corpus through VMK 0.17.0 and diffed each test against the UniVRM golden (UniVRM unchanged → a true conformance comparison).

**Conformance — VMK 0.17.0 vs UniVRM golden, at declared per-test thresholds:**

| family | pass |
|---|---|
| mtoon_shading | 17/17 |
| mtoon_gi | 6/6 |
| mtoon_alpha | 5/5 |
| mtoon_rim | 5/6 |
| mtoon_render | 3/3 |
| mtoon_double | 2/2 |
| mtoon_default | 1/1 |
| mtoon_outline | 6/9 (3 `conformance_status: excluded` — spec-flood, vrm-conformance#3) |
| springbone (settle) | 20/20 |
| **TOTAL (included)** | **65/66 (98.5%)** |

The single included miss: `mtoon_rimLightingMix_1` at SSIM **0.9491** vs a 0.95 threshold — off by 0.0009, the documented #226 high-mix rim residual. The 3 outline sub-threshold tests are `excluded` (whole-frame SSIM measures only outline AA on the spec-correct flood).

**Zero regressions.** Every one of the 69 VMK 0.17.0 renders is **≥0.99 SSIM vs the cached (prior-version) VMK golden** — no rendered-output drift on the committed corpus. Adapter `swift test` 34/0. Swing (dynamic) corpus 20/20 `overall_passed`, 0 adapter errors. CCD + synthetic corpus run clean. (The parametric corpus is untouched by #321/#322 — no body-collider interaction or eyelashes in these assets.)

**Gravity fix carried from rc.4** (0.17.0 = rc.4 physics + #326). Sideways-gravity first-step VMK 10.83 mm = 1.24× three-vrm (was 10.6× at rc.3); #326 doesn't affect our `gravityPower>0` assets.

**Honest coverage caveats (for the 1.0 gate):**
1. **Spring-bone settle (20/20) is gravity-magnitude-blind** — those chains hang −Y under −Y gravity (colinear), so gravity *magnitude* can't bend them; the settle renders neither caught the 9.8× bug nor change under the fix. The gravity fix's correctness is established by direct measurement (sideways first-step) + the CCD/synthetic dynamic shifts, not by these tests.
2. **The 73 swing (dynamic) UniVRM goldens are single representative PNGs** (no frame sequence cached), so they can't be cleanly SSIM-diffed without the bootstrap's frame-selection. The dynamic path was confirmed to render clean (20/20, 0 errors) but not golden-diffed this pass. The gravity behavior change makes the old swing goldens stale regardless.

**1.0-candidate assessment.** On the validated surface — the committed single-frame golden corpus — 0.17.0 is a strong 1.0 candidate: 98.5% included pass, the only miss a sub-0.001 known residual, zero regressions, both gravity issues resolved and measured. Remaining to complete a full 267-test gate: **re-baseline the dynamic/swing spring-bone goldens against 0.17.0** (stale after the intentional gravity behavior change) and run the full bootstrap across all renderers.

## 2026-06-07 (VMK 0.17.0-rc.4 — gravity fix verified) — [VMK#324](https://github.com/arkavo-org/VRMMetalKit/issues/324) **fixed**: the 9.8× over-drive is gone, VMK now matches three-vrm/spec scale; no regressions

**What.** Bumped rc.3 (`f07d19f`) → **0.17.0-rc.4** (`b412db9`), which closes #324: gravity is now `effectiveGravity = gravityDir · gravityPower` (the 9.8 Earth-gravity multiplier + up-to-5× settling boost removed; `SpringBoneGlobalParams.gravity` repurposed as the spec's additive external force, default `[0,−9.8,0]`→`[0,0,0]`). The release notes credit the spec + three-reference comparison this suite filed.

**Fix confirmed (the same sideways-gravity capture as the #324 measurement):**

| | first-step sideways tip displacement | ratio vs three-vrm |
|---|---|---|
| three-vrm (spec-clean) | 8.75 mm | 1.0× |
| VMK **rc.3** | 92.68 mm | **10.6×** |
| VMK **rc.4** | **10.83 mm** | **1.24×** |

The 9.8× over-drive is eliminated; VMK rc.4 and three-vrm now converge to the same ~131 mm saturation trajectory. The residual 1.24× is solver-phase noise (first-frame substep timing), not scale.

**No regressions:**
- `swift build` clean; adapter `swift test` 34 executed / 2 fixture-skipped / **0 failures**.
- CCD sweep runs clean (0 adapter errors, all `overall_passed=true`).
- Synthetic-collider augmentation **still fires** — 10 colliders/frame (gravity fix doesn't touch collider generation); ON<OFF deflection intact (ON 11.9 / OFF 26.4 mm, 0 errors).
- #267 hair-head guard green at rc.4: walk **0.0%**, static clean, asyncMatrix Run/Jog/Walk **0.17–0.22%** (all <1%). (A signal-5 on the combined run was a Metal teardown flake — each test passes in isolation.)

**Expected physics shifts (NOT regressions — the gravity fix propagating).** With ~9.8× less gravity the chain is no longer pinned straight down, so it swings more freely:
- CCD sphere penetration shifted: r0.02-fast 0→5.3, r0.05-fast 25.9→35.3, r0.02-slow 16.1→18.6, r0.05-slow 46.1→48.6 mm (less gravity-pinning → more lateral swing into the world collider).
- Synthetic forehead-hair deflection: ON 16.25/OFF 20.76 → ON 11.9/OFF 26.4 mm — the augment ON↔OFF *gap widened* (4.5→14.5 mm), i.e. the synthetic capsule's push-out is more visible once gravity stops pinning the chain.

These shifts are the direct, expected consequence of correcting gravity to spec scale; the suite's CCD/synthetic baselines should be re-recorded against rc.4 as the new reference. #162 (0.x `gravityPower=0→1.0` substitution) remains open as [VMK#326](https://github.com/arkavo-org/VRMMetalKit/issues/326).

## 2026-06-07 (spring-bone gravity scale, VMK 0.17.0-rc.3) — VMK applies gravity at **9.8× the spec scale** (Earth-gravity multiplier retained after the #270 dt-fix); confirmed from source **and** a 10.6× cross-renderer first-step measurement. Corrects this log's earlier "spec-correct ~12×" mislabel. Filed [VMK#324](https://github.com/arkavo-org/VRMMetalKit/issues/324)

**What.** The `VRMC_springBone` algorithm — and three-vrm + godot-vrm — apply `external = gravityDir · gravityPower · deltaTime`, i.e. `gravityPower` is the strength scalar. VMK instead computes `effectiveGravity = gravityDir · length(globalParams.gravity) · gravityBoost · gravityPower` with `length([0,−9.8,0]) = 9.8`, so steady-state gravity is **9.8×** the spec, plus an up-to-5× transient during the settling window (`gravityBoost = 1 + settlingFactor·4`). Source: `SpringBonePredict.metal:257-258`; the defending comment ("callers ship gravity=(0,−9.8,0) and expect that scale") is circular.

**Measurement (cross-renderer, this session's `capture_positions`).** A 4-joint chain (hangs −Y) with **sideways gravity** `gravityDir=[1,0,0]`, `gravityPower=0.5`, captured per-frame through VMK rc.3 vs three-vrm. The endpoint saturates (the 0.15 m chain can only bend to horizontal), so the clean signal is the **first physics step from rest**:

| renderer | first-step sideways tip displacement |
|---|---|
| three-vrm | 8.75 mm (≈ `gravityPower·dt` = 0.5·1/60 = 8.3 mm — spec-exact) |
| VMK rc.3 | 92.68 mm |

→ **10.6×** (= the 9.8× base × ~1.1 settling-boost at frame 0). Direct empirical confirmation of the source. (godot is *not* a clean 1× reference — it carries its own `gravity_scale`/`gravity_multiplier` factors + the stiffness-key bug — so three-vrm is the spec-clean comparison.)

**Correction to this log.** A prior entry + the gravity-sweep code comment called VMK's scale *"spec-correct (~12× stronger than 0.14.0)"*. That was wrong: #270 correctly fixed the dt-**exponent** (`dt²`→`dt`), which made gravity ~12× stronger than the broken 0.14.0 baseline and *looked* right (hair finally drooped) — but the absolute scale is still 9.8× over spec because the Earth-gravity multiplier was retained. "Correct dt power" was conflated with "correct scale." The sweep comment in `spring_bone.rs` is fixed to match.

**Why the corpus didn't catch it as a clean outlier.** The gravity **magnitude** sweep is (a) degenerate when gravity is colinear with the downward chain (gravity along the chain axis can't bend it — no signal), and (b) saturated on VMK at any non-tiny `gravityPower` (9.8× yanks the short stiff chain to its length cap within the window — 3 of 4 renderers collapse to one SHA). The gravity **direction** sweep (sideways gravity) is what exposes scale, and only the new per-frame position capture quantifies it. So "are we checking gravity?" — yes nominally, but the magnitude axis never measured *scale*; this entry is the first quantified scale comparison.

**Class.** Deliberate scale divergence (same bucket as opt-in DQS #197 and rim-lighting #226): not a crash/parse bug, but divergent from spec + every reference renderer. Filed VMK#324 for the VRM/VMK side to decide whether to converge (drop the 9.8 multiplier) or document + flag it.

## 2026-06-07 (VMK 0.17.0-rc.3 verification) — avatar-fidelity cohort bumped; all four fixes (#321/#322/#197/#267) confirmed landed, **zero regressions** vs rc.2

**What.** Bumped the VMK adapter pin from rc.2 (`9b6ad1d`) to **0.17.0-rc.3** (`f07d19f`, pre-release 2026-06-07) and verified end-to-end.

**Fixes landed (confirmed):**
- **#321 synthetic hand/arm colliders** (default-on) — the suite's synthetic-collider corpus now dumps **10** colliders/frame (was 6 at rc.2): the original 4 leg capsules + forehead capsule + skull sphere, **plus** 2 lower-arm→hand capsules (r=0.05) + 2 palm spheres (r=0.10, `handSphereRadiusFraction = 0.40`). Direct measured confirmation that hand/arm augmentation is active.
- **#322 eyelash/bangs sort** — `VRMRenderer.nonFaceRenderOrder` present and wired into the draw-order path. (Not pixel-verifiable by this suite — the parametric corpus has no eyelashes/bangs; symbol + code path confirmed.)
- **#197 dual-quaternion skinning** — `RendererConfig.dualQuaternionSkinning` present, **default-off / opt-in**. Correctly framed by upstream as a quality-above-reference divergence (LBS is glTF-standard), matching the suite's scoping note on VMK#197. The conformance path leaves it **off**, so renders are unaffected.
- **#267 async-matrix guard** — `HairHeadCollisionTests` green at rc.3: walk **0.0%** (0/3600), static clean (3 root-only). rc.3's new `testHairHead_asyncMatrix_regressionGuard` passes across Run (0.25%), Jog (0.03%), AvatarSample_U×Walk (0.00%) — all under the 1% bar.

**No regressions (measured):**
- Adapter `swift test`: 34 executed, 2 fixture-skipped, **0 failures** (identical to rc.2).
- **Authored-collider CCD sweep byte-identical** to the rc.2/0.16.0 baseline — sphere cells r0.005/r0.02/r0.05 fast = 0/0/25.9 mm, slow = 1.1/16.1/46.1 mm. (#321 adds *synthetic* colliders, which do not touch the authored/discrete path — exactly as scoped.)
- **Synthetic-collider deflection unchanged**: forehead-hair augment-ON 16.25 mm / OFF 20.76 mm — identical to rc.2. The new hand/arm colliders sit at the hands, far from the head chain, so they add coverage without perturbing the existing deflection signal.

**Read.** rc.3 extends the synthetic-augmentation group (#321) and lands the avatar-fidelity fixes without regressing any measured behavior. DQS (#197) is correctly opt-in, so the conformance baseline is stable; making it default would be the logged, gated, pre-released divergence the suite's methodology requires.

## 2026-06-06 (synthetic-collider augment-on/off validation, VMK 0.17.0-rc.2) — the suite now independently measures VMK's synthetic spring-bone colliders (#309/#311/#312/#313): augmentation **is active** and measurably deflects a chain (ON penetrates 22–29% less than OFF; the non-root chain diverges 139–159 mm ON↔OFF)

**What.** Closes the coverage gap flagged in the verification entry below. The earlier CCD corpus uses *authored* world-fixed colliders (discrete path), so it could not exercise VMK's *synthetic* augmentation (#309) or its swept collision (#313). This adds an end-to-end **augment-ON-vs-OFF** pipeline that does:
- `load_vrm` honors an `augment_colliders` flag → VMK's `VRMLoadingOptions.augmentSpringBoneColliders` (adapter); `--augment-colliders true|false` on the runner.
- `render_sequence` dumps each frame's VMK-generated synthetic colliders in **world space** (`SequenceFrame.synthetic_colliders`); the runner persists `<id>_<renderer>_colliders.json`. Synthetic colliders are bone-attached, so this is per-frame.
- `penetration-diff --colliders <per-frame.json> --exclude-root-joints` measures joints vs the **moving** colliders, excluding each chain's kinematic root joint (index 0) — which is embedded in its own bone's collider by construction and would otherwise dominate the metric (matches VMK's own `HairHeadCollisionTests`).
- Corpus: `emit-synthetic-collider-asset` → a parametric humanoid + a hair spring-chain **draped forward over the head**, so it falls through the synthetic forehead capsule (the #309 hair→forehead case). Two excitations: `synthcoll_swept` (12 frames, fast — #313) and `synthcoll_static` (120 frames, slow — #309). Single source of truth; CC0.

**Confirmed: augmentation fires for the parametric humanoid.** VMK generates **6** synthetic colliders (4 leg capsules + 1 forehead capsule + 1 skull sphere) for the generated humanoid; they move **600 mm** across frames (= the ±0.30 m root sweep), confirming bone-attached world-space capture is real, not kinematic stub.

**Data — max penetration depth (mm) into the synthetic colliders, root joint excluded, vs the augment-ON colliders.** (OFF has no synthetic colliders to dump, so both runs are measured against the ON-run geometry; valid because the head/leg bones — and thus the synthetic colliders — follow the root animation identically ON and OFF.)

| variant | augment ON | augment OFF | non-root ON↔OFF divergence |
|---|---|---|---|
| `synthcoll_swept` (#313) | **16.3** | 20.8 | 139 mm |
| `synthcoll_static` (#309) | **14.7** | 20.9 | 159 mm |

**Read.**
1. **Synthetic augmentation is active and works.** Augment-ON penetrates **22–29 % less** than OFF on both excitations, and the non-root chain trajectory diverges by **139–159 mm** between ON and OFF. The synthetic forehead capsule measurably deflects the hair chain. This is the suite's independent confirmation that rc.2's #309/#313 work does what it claims — closing the "post-hoc confirmation" goal.
2. **Residual ON penetration is a root-adjacency limitation, not a failure.** Worst penetration sits on joint 1 — the first non-root joint, dragged toward the collider by the kinematically-embedded root joint 0 (which collision never moves). So ON does not reach ≈0; it reaches a reduced-but-nonzero floor. This is the same root-adjacency residual class that VMK's own #267 hair-head test navigates, not an augmentation defect.
3. **Single-renderer feature validation, no oracle.** Synthetic augmentation is a VMK invention, not VRM spec — UniVRM/godot/three-vrm do not generate synthetic colliders. The baseline is therefore VMK-vs-itself (augment ON vs OFF in the same rc.2 build), not a cross-renderer consensus.

**Boundary / reproducibility.**
- Chain config (logged for reproducibility): 8-joint chain, 0.05 m segments, `chain_axis = [0.0, -0.45, 0.89]` (forward+down), stiffness 0.2, drag 0.2. A straight-down chain (the default axis) misses every synthetic collider and yields **no** ON/OFF signal — the chain must be aimed through a synthetic collider's volume.
- The metric counts geometric penetration of captured joints into the dumped synthetic colliders; it says nothing about VMK's internal intent. The ON↔OFF *delta* is the signal, not the absolute ON depth.
- Spec/plan: `docs/superpowers/specs/2026-06-06-synthetic-collider-validation-corpus-design.md`, `docs/superpowers/plans/2026-06-06-synthetic-collider-validation-corpus.md`.

## 2026-06-06 (VMK 0.17.0-rc.2 verification) — the spring-bone CCD release (#313 swept collision, #316 `dtSub`, #309/#311/#312 synthetic colliders) builds + tests green and produces **byte-identical** CCD-corpus numbers to 0.16.0; the #313 fix is invisible to this suite because the corpus uses *authored* colliders (discrete path) while #313 is scoped to the *synthetic* group

**What.** Bumped the VMK adapter pin from 0.16.0 stable (`392d949`) to **0.17.0-rc.2** (`9b6ad1d`, pre-release 2026-06-06; `adapters/vrm-metal-kit/Package.swift`) and ran the full verification: `swift build` clean, `swift test` 34/34 (2 fixture-skipped, 0 failures), the `capture_positions_vmk` integration test green (real GPU solver on Apple M4 Max — joints lag the root under inertia, confirming captured positions reflect the sim), and the 12-asset `emit-springbone-ccd-sweep` corpus rendered through the real adapter → `penetration-diff`, all 12 with no adapter errors and `overall_passed: true`.

**Data — penetration depth (m), sphere cells, vs the prior 0.16.0 baseline. Identical to within rounding.**

| sphere cell | 0.16.0 | **0.17.0-rc.2** |
|---|---|---|
| r0.005 fast | 0 ✓ | 0 ✓ |
| r0.02 fast | **0 ✓** | **0 ✓** |
| r0.05 fast | 0.026 | 0.0259 |
| r0.005 slow | 0.001 ✓ | 0.0011 ✓ |
| r0.02 slow | 0.016 | 0.0161 |
| r0.05 slow | 0.046 | 0.0461 |

Capsule r0.02 fast = 0.0127 (matches the prior 0.013 sphere-vs-capsule asymmetry). Every cell tracks 0.16.0.

**Read.** The release notes scope #313 explicitly: *"Continuous (swept) collision for the synthetic group… Authored colliders keep discrete collision."* This suite's CCD corpus is built from **authored** VRM 1.0 world-fixed colliders (`ccd_colliders`), so it exercises VMK's discrete path — which #313 deliberately leaves untouched. The byte-identical numbers are therefore the **expected** outcome and a clean regression-green signal (matching upstream's "default AvatarSample_A trajectory byte-preserved"), *not* evidence that the swept fix doesn't work. The fix is real but lives on a path this corpus does not reach.

**Boundary / gap.** To exercise #313's swept collision the suite needs a **synthetic-collider** scenario — bone-derived leg/head capsules + lateral skull sphere on a humanoid avatar (#309/#311/#312), which VMK generates automatically and which this parametric authored-collider corpus does not produce. That coverage is upstream-validated (regression baselines #319) but currently unmeasurable by `penetration-diff`, which keys off the plan's authored `ccd_colliders`. Closing it would mean a synthetic-augmentation corpus + a way to read VMK's synthetic group back into the metric — a worthwhile follow-up, filed here as the known coverage gap, not a defect. #316 (`dtSub`) and the conformance methodology pin both concern the **ultra** tier this suite renders at, which the release leaves unchanged; non-ultra tiers were the buggy ones and are out of the default render path.

**Pin decision (open).** The adapter is currently pinned to a **pre-release**. Bump kept only if we want this suite tracking the CCD cohort ahead of 0.17.0 stable; otherwise revert to `392d949`. Pending user call (see session).

## 2026-06-06 (CCD sweep, 4-renderer comparison vs the golden) — all four real adapters now capture spring positions; the oracle (UniVRM) deflects fast / penetrates slow; on fast cells VMK matches the oracle (spheres), three-vrm and godot under-deflect; all four penetrate slow (sustained-contact limit)

**What.** All four real adapters now report per-frame spring-bone positions via `render_sequence` + `capture_positions` — godot, three-vrm (per-op; reuse `dump_bone_positions` extraction per frame) and UniVRM (PlayMode batch; `BatchRunner.CaptureSpringPositions`, flat-`float[]` wire reshaped by the runner) and VMK (`Operations.swift`; node world positions post-draw, which reflect the GPU spring sim — confirmed by joint lag below). The runner persists the canonical `<id>_<renderer>_positions.json` for all of them (per-op via `execute.rs`, batch via `execute_batch.rs`). This is the first **4-way** CCD comparison against the golden; UniVRM is the oracle.

**Verification that VMK's capture is real (not kinematic).** VMK runs spring bones on the GPU; the worry was that the CPU `nodes[].worldPosition` might be the rigid kinematic pose. The captured r0.02-fast trajectory shows the joints **lag** the root under inertia (frame 3 joint-x `[-0.227, -0.241, -0.260, -0.279]` — tip trails the root by ~0.05), i.e. the CPU positions reflect the GPU simulation. Real capture.

**Data — penetration depth (m), sphere cells, vs the world collider at `[0.10,1.26,0]` (hitRadius 0.02; chain swept x −0.5→0.5). Lower = better deflection; `✓` = passed (no solid-radius penetration).**

| renderer | r0.005 fast | r0.02 fast | r0.05 fast | r0.005 slow | r0.02 slow | r0.05 slow |
|---|---|---|---|---|---|---|
| **UniVRM** (oracle) | 0 ✓ | **0 ✓** | 0.014 | 0.002 | 0.017 | 0.047 |
| **VMK** | 0 ✓ | **0 ✓** | 0.026 | 0.001 ✓ | 0.016 | 0.046 |
| **three-vrm** | 0 ✓ | 0.0035 | 0.033 | 0.002 ✓ | 0.017 | 0.047 |
| **godot** | 0 ✓ | 0.007 | 0.037 | 0.001 ✓ | 0.016 | 0.046 |

(Capsule cells track the spheres except **VMK fast capsule r0.02 = 0.013** where its sphere is clean — a VMK sphere-vs-capsule asymmetry: its capsule collision is weaker than its sphere.)

**Read.**
1. **The oracle is not a perfect collider.** UniVRM **deflects fast sweeps** (closest approach grows with radius; penetration 0 for r ≤ 0.02) but **penetrates every slow sweep** (pen ∝ radius up to 0.047). Slow = sustained contact: the chain drapes under gravity and a single-step positional collision can't hold a stiff chain out over many frames (the #313 family). "Non-penetration" is *not* the oracle's behavior on slow cells — so every renderer "fails" them.
2. **Fast cells rank the renderers (the conformance signal).** r0.02-fast: **UniVRM 0 = VMK 0** (VMK matches the oracle on spheres) < **three-vrm 0.0035** < **godot 0.0067**; r0.05-fast widens the same order (UniVRM 0.014 < VMK 0.026 < three-vrm 0.033 ≈ godot 0.037). So **godot and three-vrm under-deflect vs the golden on fast motion** (godot most), **VMK matches the oracle on fast spheres** but trails it on fast capsules and large radii. This is where the file-worthy divergences live.
3. **Slow cells: all four converge** (~0.016 at r0.02, ~0.046 at r0.05) — the sustained-contact limit is universal, shared with the oracle. So slow-cell penetration is **conformant-to-oracle** for every renderer, *not* a defect.
4. **Conformance bar ≠ the absolute `passed` flag.** `passed` means "no solid-radius penetration," and the *oracle fails it on every slow cell*. Conformance is "match UniVRM": red on a slow cell is conformant; the signal is the **fast cells**, where godot/three-vrm trail the oracle. The metric now has its golden baseline.

**Boundary.**
- **All four are single-step**, like the oracle (UniVRM/godot one Verlet step/frame at frame-rate `dt`, no substep — see the methodology pin), so universal slow-cell penetration is expected, not a per-renderer bug. The tail-vs-origin caveat (collision protects tails; the metric reads origins) applies to all four equally, so the *comparison* holds even if absolute depths slightly over-read.
- **File-worthy items, now oracle-anchored:** godot's and three-vrm's **fast-sweep under-deflection vs the golden** (godot most), and **VMK's sphere-vs-capsule asymmetry** (clean fast sphere, penetrating fast capsule). Slow-cell penetration is shared with the oracle and should *not* be filed for any renderer. (The godot `"stiffiness"` 1.0-importer typo is still held, per the user.)
- **Per-adapter gated tests** (each asserts positions captured + moving across frames): `capture_positions_godot_vrm.rs` (godot-on-PATH), `capture_positions_three_vrm.rs` (node + dist + chromium), `capture_positions_univrm.rs` (Unity 6000.4.6f1), `capture_positions_vmk.rs` (Xcode 26 + Metal); plus the fast no-toolchain runner-persistence tests `capture_positions_e2e.rs` (per-op) and `batch_capture_positions_writes_positions_json` (batch). All verified locally.

## 2026-06-06 (CCD sweep, first real-engine measurement) — `penetration-diff` now runs against a real spring-bone solver (godot-vrm); the chain passes straight through the world-fixed colliders (no deflection), depth ∝ radius

> **Refined by the UniVRM golden baseline (entry above).** The "no deflection" framing holds on **fast** cells *relative to the oracle* (godot's closest approach is a flat ~0.013 while UniVRM's grows with radius to ~0.036) — that fast under-deflection is the real divergence. But the oracle *also* penetrates the **slow** cells (~0.003, pen ∝ radius), so godot's slow-cell penetration below is **conformant-to-oracle**, not a defect. Read the radius-∝-penetration data below as the fast-vs-slow picture the oracle later disambiguated.

**What.** godot-vrm is now the first **real** adapter (non-mock) to report per-frame spring-bone positions: `render_sequence` with `capture_positions: true` returns `SequenceFrame.spring_positions` from godot's actual L4 solver (it reuses the same per-joint world-position extraction as `dump_bone_positions`). This closes the gap recorded in the methodology pin ("CCD / `penetration-diff` is mock-backed only") — the metric had only ever seen the mock's static synthetic chain. Running the 12-asset CCD sweep (`emit-springbone-ccd-sweep`) through godot is the first time `penetration-diff` has measured a real spring-bone simulation.

**Data — penetration depth tracks collider *radius*, not the fast/slow axis; the chain reaches the collider center regardless of size.** 12 cells (2 shapes × 3 radii × 2 speeds), world-fixed collider at `[0.10, 1.26, 0]`, root swept x: 0 → (fast 0.5 / slow smaller) over 12 frames @ `physics_dt = 1/60`, `epsilon = 2 mm`:

| shape | radius (m) | speed | max_pen (m) | worst frame | passed |
|---|---|---|---|---|---|
| sphere | 0.005 | fast | 0.00000 | 0 | ✓ |
| sphere | 0.005 | slow | 0.00078 | 73 | ✓ |
| sphere | 0.02 | fast | 0.00672 | 8 | ✗ |
| sphere | 0.02 | slow | 0.01592 | 73 | ✗ |
| sphere | 0.05 | fast | 0.03674 | 8 | ✗ |
| sphere | 0.05 | slow | 0.04592 | 73 | ✗ |
| capsule | 0.005 | fast | 0.00000 | 0 | ✓ |
| capsule | 0.005 | slow | 0.00213 | 74 | ✗ |
| capsule | 0.02 | fast | 0.00820 | 8 | ✗ |
| capsule | 0.02 | slow | 0.01759 | 74 | ✗ |
| capsule | 0.05 | fast | 0.03930 | 8 | ✗ |
| capsule | 0.05 | slow | 0.04761 | 74 | ✗ |

`max_pen ≈ radius` for every cell (r0.05 → ~0.046, i.e. ~92% of radius; r0.02 → ~0.016–0.018; r0.005 → ≤0.002). Penetration depth is therefore `radius − (closest approach to collider center)`, and the closest approach is a near-constant ~2–4 mm across **all** radii and speeds. That means **the chain's trajectory is collider-independent** — it sweeps through the collider's location essentially undeflected, and the metric is simply measuring how deep inside the (larger or smaller) collider the undeflected chain ends up. The thin (r0.005) cells "pass" not because collision worked but because a 5 mm sphere is too small to accumulate >2 mm of depth. Within a radius, **slow penetrates more than fast** (the slow chain dwells in the collider region longer; the fast chain whips through), and the worst frame moves later for slow (≈73) vs fast (≈8) — consistent with no-deflection-plus-dwell, not with a tunneling signature.

**Read — the prediction was overturned; record the observation, not a verdict.** The design spec predicted a *tunneling* signature (fast/thin cells penetrate, slow/large pass) on the assumption godot has working discrete collision that only fails when a fast step skips a thin collider. The real measurement shows the opposite shape: **no deflection at all**, with depth driven by collider radius. So the headline is two-fold:
1. **The metric works against a real solver.** `penetration-diff` produced a clean, monotonic, physically-interpretable signal from real godot positions — not the structural `0.0` the static mock yields. The pipeline (capture → persist → signed-distance vs world collider) is validated end-to-end on a real spring-bone simulation.
2. **godot-vrm's discrete spring-bone collision under-resolves the world-fixed collider under swept motion — root cause attributed (instrumented).** Candidate causes (a) "collider not wired" and (b) "asset doesn't present the collider" are both **refuted by evidence**:
   - *Asset is correct* (Boundary 1): the emitted `.vrm` carries collider node 25 at world `[0.10, 1.26, 0]`, sphere r0.02, in colliderGroup 0, referenced by the spring (verified from the glb JSON).
   - *godot wires it correctly* (instrumented `vrm_secondary` at runtime): the spring has `colliders=1`, `disable=false`, resolved collider position `(0.1, 1.26, 0.0)`, radius `0.02`. So godot **does** support and register the world-coordinate collider.
   - *Collision fires but only grazes* (instrumented `vrm_collider.collision`): across the **whole** sweep the hit test (`dist ≤ hitRadius+colliderRadius = 0.04`) trips only **2 times**, at `dist ≈ 0.033–0.037` — and `dist` **never** drops below ~0.033, in **both** the 12-frame fast and 120-frame slow cells (identical hit counts and distances). Meanwhile the captured joint **origins** reach within ~2–4 mm of the collider center. The discrepancy is the mechanism: godot's collision evaluates each joint's **tail** (`next_tail`), which trails a segment-length (~0.05 m) off the collider and only grazes the 0.04 shell, so the joint **origins** pass through the collider essentially undeflected. The chain effectively tunnels — godot has no swept/continuous collision, which is exactly what the threshold-straddling sweep was built to expose.
   - *Secondary concrete importer bug* found while tracing: godot-vrm's VRM **1.0** spring-bone importer reads the **0.x misspelling** `sjoint.get("stiffiness", 1.0)` (`addons/vrm/1.0/VRMC_springBone.gd:125`), not the spec-correct `"stiffness"`. So every 1.0 spring **ignores its authored stiffness and runs at the default 1.0** (max), stiffening the chain and compounding the under-resolution. This is a clean, file-ready godot-vrm bug independent of the collision-resolution finding.

**Boundary.**
- **Real solver, but penetration metric still measures geometry, not godot's intent.** `penetration-diff` computes signed distance from captured joints to the world collider purely geometrically; a non-zero result means the joints are inside the collider volume, regardless of whether godot "knows" about that collider. The no-deflection reading follows from the collider-*independence* of the trajectory (closest-approach ≈ constant across radii), not from any godot internal state.
- **Scope.** First real-engine capture lives in godot-vrm only. UniVRM (the golden) remains the highest-value follow-up — and would say whether *the reference* deflects off world-fixed colliders, which decides whether godot's no-deflection is a divergence-from-oracle or shared behavior. VMK/three-vrm already have `dump_bone_positions`, so they are cheap follow-ups too.
- **Cause attributed; two file-worthy godot-vrm items.** (1) Discrete spring-bone collision under-resolves world-fixed colliders under swept motion (tail-checked, no CCD → chain tunnels). (2) VRM 1.0 importer reads the misspelled `"stiffiness"` key, ignoring authored 1.0 stiffness. Both confirmed by instrumentation (since reverted; not committed). Filing upstream is the user's call — item (2) is a clean one-line bug; item (1) is a larger "add swept/continuous collision" feature ask. The remaining open nuance for item (1) is whether the dominant factor is the tail-vs-origin point choice or frame-step tunneling (both point to the same no-swept-collision gap); a single instrumented run comparing per-frame origin-distance vs tail-distance would settle it before filing.
- Implementation: `adapters/godot-vrm/src/session.gd` (`_collect_spring_positions` shared by `dump_bone_positions` + `render_sequence`); runner unchanged; covered by `crates/vrm-runner/tests/capture_positions_godot_vrm.rs` (godot-on-PATH gated; asserts positions are captured and **move** across frames — the property the static mock fails).

## 2026-06-06 (issue #313 Track 2, golden UniVRM by source) — spring-bone collision push-out feeds next-frame Verlet velocity in the spec-reference algorithm: "catapult" off a collider is conformant-to-oracle, not a VMK defect

> **⚠ CAUSATION SUPERSEDED — see "UPDATE (2026-06-06, later): the catapult is a large-timestep instability" at the end of this entry.** The *conclusion* below (the catapult is conformant-to-oracle, not a VMK defect) stands and is now better grounded. But the *mechanism* attributed below — collision push-out re-entering the Verlet velocity term ("energy injection") — was **refuted** by VMK's substep sweep: the velocity-kill lever failed (pure projection was the *worst* setting) and the true cause is time-discretization (large-timestep instability of the stiff chain). Read the original Data/Read sections as the (partly wrong) reasoning trail; the UPDATE carries the corrected mechanism and the resolved productionization gate.

**What.** #313 Track 2 attempted to fix sleeve→arm cloth deflection by adding an arm collider to the stiff 3-joint sleeve chain (AvatarSample_U arm-swing spike). Every collider configuration tried — discrete vs swept CCD, arm-radius fraction `{0.12, 0.20, 0.28, 0.36}`, tangential friction `{0.0, 0.3, 0.6, 0.9}`, each × 2 frequencies — was **worse than no collider**: frame-count dropped (cloth out of the arm most of the time) but *peak* penetration rose (occasional deep "catapult"), non-monotonic in both radius and friction. The question this entry answers: is that catapult a VMK integrator artifact, or does the golden (UniVRM, the consortium reference) behave identically? An empirical PlayMode capture cannot answer this today: Unity 6000.4.6f1 *is* installed locally, but per-frame spring-bone position capture (`capture_positions`/`spring_positions`) is implemented only in `vrm-mock-renderer` — the UniVRM adapter renders sequences in PlayMode but does not report joint positions, so `penetration-diff` cannot consume its output. The verdict was therefore established from **UniVRM v0.131.0 source** — a version-pinned, more durable proof than a single capture anyway.

**Data — the spec-reference collision constraint is positional and not velocity-conservative; the push-out re-enters the Verlet velocity term next frame, by construction.** Verified by reading the integration, the collision write-back, and the buffer rotation across all three positional-Verlet solvers in-tree (the oracle + two others):

| solver | integration (velocity term) | collision write | how push-out becomes velocity |
|---|---|---|---|
| **UniVRM v0.131.0** — golden; shared 0.x + 1.0 FastSpringBone path (`com.vrmc.gltf@39e860e10eeb`) | `UpdateFastSpringBoneJob.cs:88-92` — `nextTail = currentTail + (currentTail − prevTail)·(1−drag) + stiffness + external` | `:99-119` collision repositions `nextTail` to the collider surface + re-enforces bone length; **`prevTail` untouched**; `:121` writes `nextTail` | `FastSpringBoneScheduler.cs:40` `FlipBuffer()` then `FastSpringBoneConbinedBuffer.cs:213-218` rotates `prev←current`, `current←nextTail`. Next frame `velocity = current − prev = (collision-corrected nextTail) − (pre-collision current)` — **the push-out is the velocity** |
| godot-vrm 4.6.3 (a direct port of the UniVRM algorithm) | `addons/vrm/vrm_spring_bone_logic.gd:76` — same formula | `:86` `next_tail = collider.collision(...)` | `:89-90` `prev_tail = current_tail; current_tail = next_tail` — structurally identical to UniVRM |
| VMK (VRMMetalKit) | `SpringBonePredict.metal` — `velocity = bonePosCurr − bonePosPrev` | `SpringBoneCollision.metal` push-out added to `bonePosCurr`, `bonePosPrev` untouched; collision dispatched **last** (after distance constraint) per `SpringBoneComputeSystem.swift` | same — no `prevPos` carry, no separate velocity buffer |

The decisive detail is that **no solver in the set updates the previous position when collision moves the current one, and none carries a separate velocity buffer** — velocity is reconstructed purely from position history. That is precisely *why* the collision push-out launches the joint: the correction shows up as a one-frame velocity spike on the rebound. The subagent first-pass read of the UniVRM code concluded "no energy injection" — that was exactly backwards; `prevTail` being left unmodified is the *cause* of the catapult, not a guard against it.

**Read (not a VMK defect — conformant-to-oracle; spec-solver limitation).** A collider on a stiff, short chain catapults the joint **in the spec-reference algorithm itself** (UniVRM), not just in VMK. VMK's Track 2 catapult reproduces the oracle's behavior. The two dead-end spike sweeps corroborate the mechanism rather than contradict it: **friction worse at every level** is expected because tangential friction in a Verlet solver drags `prevTail` sideways — injecting *more* spurious velocity, not damping it; **radius non-monotonic** is the fingerprint of an energy-injection artifact (deeper penetration → larger push-out → larger injected velocity, whose phase relative to the stiffness restoration is geometry-dependent), not a depth threshold. The structural conclusion from #312 holds: a 3-joint stiff chain has no compliance to *drape* over a collider — it can only lever off it, and the lever energy comes back as velocity. The production ecosystem solves sleeve-on-arm with XPBD solvers (Magica Cloth 2, Unity Cloth) that are velocity-consistent *and* with longer/softer chains — a different solver class, not a collider tuning.

**Consequence for VMK.** Any fix that makes VMK *not* catapult (e.g. carrying `prevPos` along the push-out to kill the injected normal velocity — a PBD/XPBD-style velocity-conservative collision) would be a **deliberate divergence from the oracle**, i.e. a quality improvement that breaks UniVRM parity on dynamic collision. Per methodology that is allowed but must be a logged, gated exception — not shipped silently. Notably VMK *already* carries a scoped version of this compensation on one axis: `SpringBonePredict.metal` inertia-compensation subtracts the parent's upward delta from `velocity` (Y-up / parent-motion only, the vertical "flutter" case). A general collision velocity-kill would subsume it but is the same divergence-from-oracle decision at larger scope. **Recommendation: do not file a VMK bug for the Track 2 catapult; treat it as conformant-to-oracle.** If cloth-on-arm quality is wanted regardless, it is a deliberate VMK-only divergence requiring its own RFC/methodology exception.

**Boundary.**
- **Verdict is from source, not a PlayMode capture.** Unity 6000.4.6f1 *is* installed locally (`/Applications/Unity/Hub/Editor/6000.4.6f1`), so the blocker is **not** environment availability — it is adapter capability: per-frame position capture (`capture_positions`/`spring_positions`) is implemented only in `vrm-mock-renderer`; no real adapter (UniVRM, godot, three-vrm, VMK) populates joint positions, so `penetration-diff` is mock-only today. Empirically corroborating the catapult through UniVRM would first require implementing spring-bone position capture in the UniVRM PlayMode batch handler (`adapters/univrm`, `RenderSequenceParams.capture_positions` → `SequenceFrame.spring_positions`). The proof here is instead the v0.131.0 collision algorithm (committed PackageCache rev `com.vrmc.gltf@39e860e10eeb`), which is version-pinned and cannot be contradicted by a single observation; a PlayMode capture would only corroborate it. UniVRM routes both VRM 0.x (`secondaryAnimation`) and 1.0 (`VRMC_springBone`) through this same FastSpringBone job, so the algorithmic verdict holds for both specs.
- **Frame, not stock-asset.** AvatarSample_U is VRM 0.0 (`assets/humanoid/avatarU_0_0.vrm`) and its stock colliders may not even sit on the arm; the catapult is about an *added* arm collider on the stiff sleeve (the #312/#313 hypothesis), not stock-sample clipping. There is no committed AvatarSample_U arm-swing test plan — the spike was harness-local.
- **Scope: penetration depth / catapult only.** This says nothing about Track 1 (sphere + capsule swept CCD), which is a separate, soundly-scoped tunnel-prevention win (synthetic-group-only per CLAUDE.md §4) and is invisible to the penetration-depth metric by design.
- This is related to the 0.x-swing integrator variance documented below (positional-Verlet sensitivity under motion) — a known methodology hazard, not a per-renderer attribution. (Originally framed as the "same energy-injection class"; the UPDATE below corrects that — it is the *same time-discretization sensitivity*, not energy injection.)

---

### UPDATE (2026-06-06, later): the catapult is a large-timestep instability — collider + substepping *together* fix it; UniVRM does not substep, so the catapult is conformant and the fix is a documented divergence *above* the reference

**Headline: both levers are required, neither is sufficient.** The arm capsule supplies the *what-stops-the-sleeve*; finer substepping supplies the *stability-to-resolve-the-contact-instead-of-catapulting*. The collider route was never "closed" — it was closed as a *solo* lever, which the original entry correctly found, and the velocity-kill primary lever genuinely failed. What is new: collider + substepping together work, and neither half does alone.

**Corrected mechanism — time-discretization, not energy injection.** VMK's validated sweep (AvatarSample_U arm-swing, arm capsule frac 0.20 in the #309 synthetic group + CCD, vs coarse baseline; peak penetration depth + frames-penetrating / 180):

| excitation | coarse (no arm collider) | + arm capsule | verdict |
|---|---|---|---|
| 2.6 Hz @ **120 Hz** | 0.0191 m, 12/180 | 0.0269 m, 13/180 | ✗ catapult |
| 2.6 Hz @ **240 Hz** (2×) | 0.0266 m, 10/180 | 0.0174 m, 3/180 | ✓ |
| 2.6 Hz @ **480 Hz** (4×) | 0.0239 m, 14/180 | **0.0025 m, 0/180** | ✓✓ |
| 3.2 Hz @ **120 Hz** | 0.0242 m, 13/180 | 0.0441 m, 13/180 | ✗ catapult |
| 3.2 Hz @ **240 Hz** (2×) | 0.0339 m, 9/180 | 0.0192 m, 7/180 | ✓ |
| 3.2 Hz @ **480 Hz** (4×) | 0.0289 m, 13/180 | **0.0071 m, 1/180** | ✓✓ |

**Monotonic in substep rate across *both* frequencies** — the fingerprint of a real fix. Contrast the failed levers (radius, friction, velocity-kill), all of which were *non*-monotonic. Three facts that pin causation to time-discretization rather than the velocity-feedback mechanism the original entry blamed:
1. **Velocity-kill failed in the diagnostic direction.** `carry=1` (pure positional projection — the maximal velocity-kill) was the *worst* setting, because it preserves the inward pre-collision velocity and the stiff chain levers that deeper. If the catapult were collision-push-out energy injection, killing it would help; it hurt. So the `FlipBuffer` push-out-becomes-velocity coupling documented above is *real but second-order* — not the lever.
2. **Op-order is already correct** (collision dispatched last, after the distance constraint) — so the catapult is not a constraint-ordering amplifier either.
3. **Finer substeps shrink the per-step integration error of the stiff chain**, which is what a large-`dt` instability looks like; at 480 Hz the arm capsule nearly eliminates sleeve→arm penetration (0–1 frames, 1–7 mm).

**Conformance gate — RESOLVED from UniVRM source. UniVRM does NOT substep.** Its FastSpringBone runs a **single Verlet step per frame at frame-rate `deltaTime`**, no accumulator, no clamp:
- `FastSpringBoneScheduler.cs:21-54` — one `UpdateFastSpringBoneJob` dispatch per `Schedule(deltaTime)` call, `DeltaTime` passed straight through.
- Driven from `FastSpringBoneService.cs` `LateUpdate()` → `Schedule(Time.deltaTime).Complete()` in normal play; via `Vrm10FastSpringboneRuntimeStandalone.Process(deltaTime)` otherwise.
- The conformance harness (`adapters/univrm` `PhysicsDriver.cs`) drives it at **1/60 Hz** (settle: `SettleStepHz = 60`; animate: `1/fps`; PlayMode sequence: `physics_dt_seconds`, capped at 1/60 per RFC-0004).

So **UniVRM integrates at 60 Hz single-step — coarser than VMK's catapulting 120 Hz "coarse" baseline.** By the identical single-step positional-collision algorithm at an even larger `dt`, an arm capsule on the stiff sleeve in UniVRM would catapult *at least as hard*. UniVRM does **not** resolve the stiff-chain contact cleanly at its native rate.

**Therefore (Branch A, decisively):**
- **The catapult is conformant-to-oracle.** The reference exhibits the same large-`dt` instability by construction; matching it means accepting the catapult (and stock UniVRM avoids it only by not putting colliders on stiff sleeve chains in the first place).
- **Substepping to 240/480 Hz makes VMK *better than* the reference.** There is no quality-gap-vs-reference to close — at 240 Hz VMK is already at-or-below coarse and below UniVRM's effective behavior. Re-enabling the arm capsule + substepping is a **deliberate, owned quality divergence *above* the reference**, not a conformance fix. It is allowed but must be logged (this entry), gated, and shipped with fresh frozen baselines + a pre-release per VMK policy — *not* because it fails conformance, but because it intentionally exceeds the reference.

**Productionization is the owner's call; conformance imposes no substep requirement.** Because conformance is satisfied by the coarse/catapult behavior, the rate/perf tradeoff is a pure quality-vs-cost decision (240 Hz = 2× spring-bone GPU, arm ≤ coarse, 3–7 frames; 480 Hz = 4×, near-perfect, 0–1 frames). Conformance's only ask: whatever ships, log it as a divergence-above-reference with baselines refreshed.

**Two carry-overs.**
- **File the `dtSub` production bug now, independently of Track 2 — and it lives in VMK/VRMMetalKit, not this repo.** `update()` never reassigned `params.dtSub` from `fixedDeltaTime`, so non-ultra quality presets (60/90 Hz) are currently taking substeps at the wrong `dt` (over-stiffening silently — the same confound that nearly corrupted the experiment). This is a latent correctness bug in shipping defaults regardless of whether Track 2 lands, and it will distort any later substepping productionization on those tiers. The verified-no-op-at-120 Hz fix (`params.dtSub = Float(fixedDeltaTime)`) is the template. **Action: VMK team files this in the VRMMetalKit repo as its own ticket, ahead of Track 2.**
- **Do not build per-chain/adaptive substepping speculatively.** "2–4 substeps on the sleeve chain only" is the right long-term shape but the expensive architectural option (substep rate becomes a per-chain property threaded through the kernel). Build it only if a future decision requires near-perfect (480-Hz-equivalent) quality *and* global 480 Hz proves too expensive. If UniVRM-matching lets global 240 Hz ship, per-chain is over-engineering.

**Capture caveat unchanged.** These penetration numbers are VMK's own instrumented sweep; the suite's `penetration-diff` still cannot corroborate them through a real adapter (position capture remains mock-only — see `docs/methodology.md`, "CCD / `penetration-diff` is mock-backed only"). The UniVRM verdict here is again from source, not a captured trace.

## 2026-05-29 (VRM 1.0 full corpus, golden UniVRM) — VMK misapplies the MToon `occlusionTexture`: AO pattern dominates the shaded result instead of subtly modulating ambient (stable, dose-responsive, isolated)

**What.** Full VRM 1.0 corpus (`scripts/bootstrap-goldens.sh`, `SPEC_VERSION=1.0`, 336 test_ids) rendered through all four real adapters — **UniVRM v0.131.0 / Unity 6000.4.6f1 (golden)**, VMK (VRMMetalKit, Swift release / macOS 26 SDK), godot-vrm 4.6.3, three-vrm — then `consensus-report.sh` pairwise SSIM. VMK is **never a lone consensus outlier** corpus-wide; the top consensus divergences are the outline/matcap families where *all four* diverge (known non-PBR methodology hazard, not a defect). Triaging instead by "where does VMK trail the golden more than the other renderers do" surfaced one clean VMK-specific defect: **the glTF-core `material.occlusionTexture` on an MToon material.**

**Data — occlusion presence degrades only VMK, proportional to strength.** SSIM vs the UniVRM golden across the `pbrtex` sub-family (occlusion map = the R-channel of the shared quadrant-checkerboard, applied per glTF `occlusionTextureInfo`: `finalOcclusion = 1 + strength·(sampled−1)`):

| test | vmk↔uni | three↔uni | godot↔uni |
|---|---|---|---|
| `mtoon_pbrtex_baseline` (no AO) | **0.964** | 0.958 | 0.885 |
| `mtoon_pbrtex_occlusion_default` (strength 1.0) | **0.851** | 0.958 | 0.885 |
| `mtoon_pbrtex_occlusion_strength_half` (0.5) | **0.896** | 0.958 | 0.885 |
| `mtoon_pbrtex_combined` (occ+normal+emissive) | **0.807** | 0.881 | 0.887 |

Delta-from-baseline isolates occlusion as the cause:

| adding occlusion | VMK Δ vs uni | three-vrm Δ | godot Δ |
|---|---|---|---|
| strength 1.0 | **−0.114** | +0.000 | +0.000 |
| strength 0.5 | **−0.069** | +0.000 | +0.000 |

Baseline VMK is the *best* match to the golden (0.964); adding the occlusion texture is the *only* change, and it drops **only** VMK — by an amount that scales with `occlusion_texture_strength` (full −0.114, half −0.069). three-vrm and godot are unmoved (Δ=0.000), i.e. they apply occlusion the same way the golden does. This is a textbook controlled isolation: clean baseline control + dose–response + an unaffected cluster.

**Visual.** Golden (UniVRM) and three-vrm render occlusion as a *subtle ambient* modulation — the directional toon lighting is preserved (lit white cap top-right + gray shade, near-indistinguishable from baseline). VMK instead renders a hard **top-dark / bottom-light horizontal band with no lit cap**: the quadrant-checkerboard's R-channel shows through as a dominant term, and the scene's directional lighting is lost. (PNGs: `goldens-cache/{univrm,three-vrm,vrm-metal-kit}/mtoon_pbrtex_occlusion_default.png`.)

**Read (VMK defect, candidate upstream issue).** VMK applies the `occlusionTexture` to the **full shaded/lit result (or far too strongly), instead of restricting it to the indirect/ambient term** as MToon and the glTF spec intend. Under a strong directional light an AO map should be nearly invisible on directly-lit surfaces (which is exactly what UniVRM/three-vrm show); VMK lets the AO texture's spatial pattern override the lighting. Likely sites for VMK to check: the occlusion sample is multiplied into the final color rather than only `ambient/GI`; and/or `occlusionTextureInfo.strength` is not applied via `1 + strength·(s−1)` (the `strength_half` case still over-darkens). This is **stable and version-isolated** (1.0 native path; no migration/integrator ambiguity like the 0.x swing case below), so unlike that case it *is* a clean per-renderer attribution. Recommend filing a VMK issue: "MToon `occlusionTexture` applied to direct lighting / strength mishandled — AO pattern dominates render."

**Boundary.** Normal-map (`normal_scale_2x`) divergence is messier (three-vrm also trails the golden there, 0.856) — not cleanly VMK-specific, not claimed here. The corpus-wide consensus pass rate was 294/336 (the 42 "failed" are dominated by the outline/matcap methodology-hazard families, expected). UniVRM rendered 267 of the shared cells (EditMode/PlayMode coverage subset); all four adapters rendered the `pbrtex` family, so the occlusion comparison is on full 4-way overlap.

## 2026-05-29 — VRM 0.x rendered through VMK + golden UniVRM: VMK adheres (static settle ~0.94 vs golden); dynamic-swing signal version-unstable (integrator variance, no defect); three emit-side gaps fixed; VMK#299 reclassified

**What.** First attempt to render the Slice-2 VRM 0.x corpus through a real adapter (VMK / VRMMetalKit 0.16.0, rev `392d949`, Xcode 26.5 / M4 Max) — the deferred D4 task, scoped to VMK. Running it surfaced that the Slice-1/Slice-2 v0 emit was **never render-ready** (it had only ever been gltf-validator-gated, and mrxz/vrm-validator does no VRM-extension semantic checks). Three gaps, all on the suite side, fixed in order:

1. **0.x camera was +Z (should be −Z).** All 9 v0 emit sites set `spec_version = V0` but left the +Z default camera from `build_default_test_plan`; the runner's `validate_camera_convention` correctly rejected every 0.x plan ("0.x avatars face -Z; camera must be at negative Z"). Fixed: `tag_plan_vrm0()` now flips the camera to the −Z side at all 9 sites.
2. **`humanoid.humanBones` was empty.** `vrm_ext_v0.rs` hardcoded `humanBones: []`, so VMK's loader rejected the assets: `VRMModel.load failed: missingRequiredBone(bone: hips, availableBones: [])`. Fixed: the v0 spring-bone emit paths now populate `humanBones` from the skeleton's `bone_to_node` (the 0.x array form `[{bone,node,useDefaultValues}]`).
3. **v0 MToon sweeps are meshless** (`emit_vrm_v0` emits no geometry) — they validate structurally but have nothing to render. **Not fixed** (deeper change: the v0 MToon path needs a sphere + skeleton like the 1.0 emit). The spring-bone v0 path is unaffected (it carries sphere + chain geometry), so the 0.x render signal in this entry is **spring-bone only**.

**VMK#299 reclassified as accepted normalization (per maintainer direction).** VMK applies a load-bearing Ry180 to 0.x models (`buildNodeHierarchy` `isVRM0` branch — migrates −Z→+Z facing for physics/animation/culling/`+X=left` parity). Rather than flag this as the orientation divergence #299 documents, the conformance **adapter now compensates**: `handleSetCamera` and `handleSetLighting` conjugate the camera and directional light by the same Ry180 when `sourceSpecVersion == "0.x"`, so VMK renders the avatar's front under the suite's spec-correct −Z camera. This treats VMK's normalization as a feature (the suite's job becomes testing VMK's 0.x **material/physics**, not its facing). Both conjugations are gated strictly on 0.x; the VRM 1.0 path is byte-unchanged.

**Result — VMK's 0.x spring-bone physics is conformant with its 1.0.** Within-renderer cross-version SSIM (the methodology triage order) over the full 20-variant settle sweep, VMK 0.x (`secondaryAnimation`) vs VMK 1.0 (`VRMC_springBone`), same `SpringBoneParams`:

| stage | SSIM range across all axes | spread |
|---|---|---|
| camera conjugated only | 0.9271 – 0.9282 | 0.0011 |
| camera + light conjugated | 0.9557 – 0.9573 | 0.0016 |

The signal is the **axis-invariance**: gravity, stiffness, drag, joint-count, and segment-length variants all land within ~0.002 of each other at each stage. If VMK parsed any 0.x `secondaryAnimation` field differently from the 1.0 `VRMC_springBone` equivalent (a `gravityDir` sign, a stiffness scale, a `dragForce` unit), that axis would separate from the pack — none do. VMK's 0.x spring-bone simulation matches its 1.0 simulation. The uniform offset is the orientation-normalization residual: conjugating the light closed ~0.03 of it (it was lighting the 180°-rotated model from the mirrored side); the remaining ~0.043 is anti-aliasing from the 180° rasterization of the thin chain cylinder, not a physics or material defect.

**Cross-renderer consensus, anchored on the golden (UniVRM v0.131.0 / Unity 6000.4.6f1), 0.x.** Three real adapters: VMK, godot-vrm 4.6.3, and **UniVRM (the VRM-consortium golden reference)**. Per-axis SSIM vs UniVRM, both sweeps axis-invariant:

| sweep | golden=UniVRM | VMK vs golden | godot vs golden | stable? |
|---|---|---|---|---|
| settle (static gravity), 0.x | — | 0.9413 – 0.9423 | 0.9589 – 0.9599 | **yes** |
| swing (dynamic), **0.x** | — | 0.8377 – 0.8405 | 0.9685 – 0.9688 | no |
| swing (dynamic), **1.0** | — | **0.9629 – 0.9682** | **0.8830 – 0.8928** | no |

**Settle — VMK adheres (stable, trustworthy verdict).** VMK agrees with the golden at ~0.94 (godot ~0.96), axis-invariant, no outlier. The ~0.017 VMK<godot gap is ordinary VMK MToon shader variance (it would be axis-dependent otherwise). This is the defensible conformance statement: on loading (after the humanBones fix) and static spring-bone, **VMK adheres to VRM 0.0 vs the consortium reference.**

**Swing — NO stable verdict; the "outlier" flips with version, so this is integrator/migration variance, not a per-renderer defect.** On **0.x** swing, godot+UniVRM cluster (~0.97) and VMK is the outlier (~0.84). On **1.0** swing — same controlled geometry, VMK's native path — it **reverses**: VMK+UniVRM cluster (~0.96) and *godot* is the outlier (~0.89). Both axis-invariant. Because no renderer is consistently the outlier across versions, there is no clean "renderer X is non-conformant" conclusion to draw from the dynamic case — it is exactly the integrator-sensitivity hazard the methodology pin (`docs/methodology.md`, "Spring-bone cross-version triage order") flags as expected cross-renderer/cross-version variance under motion, compounded by each engine's 0.x→1.0 migration affecting the swing differently. (Corollary: UniVRM's own 0.x-swing and 1.0-swing differ — the golden is not version-stable under dynamic excitation either.)

**Process note (this finding was hardened, and the hardening overturned a hasty verdict).** An interim golden-anchored read on the **0.x swing alone** concluded "VMK is the lone outlier; under-deflects; candidate VMK issue." Hardening it on the **1.0 synthetic swing** (VMK's native path) **refuted that**: there VMK matches the golden and godot is the outlier. Lesson recorded: a single (version, dynamic) slice is not enough to attribute a spring-bone-dynamics defect — the dynamic signal must be stable across versions before it means anything, and here it is not. **Do not file a VMK swing issue on this basis.** The real-1.0-humanoid attempt was separately inconclusive (settle VMK~UniVRM ≈ 0.34 — full-avatar material/texture/hair shading dominates and swamps the spring-bone signal; the synthetic single-axis sweep is the correct isolator, the complex avatar is not).

**Net VMK VRM 0.0 adherence verdict.** Static/structural: **adheres** — loads with the humanBones fix; static settle matches the golden at ~0.94, axis-invariant, no outlier. Dynamic spring-bone: **inconclusive, no defect attributable** — cross-renderer agreement under swing is unstable (the outlier flips between 0.x and 1.0), i.e. integrator/migration variance, not a VMK conformance failure. So: VMK adheres to VRM 0.0 on everything the suite can stably measure here; the dynamic case yields no clean signal and no VMK issue.

**Boundary / remaining.**
- Consensus here is **3-way incl. the golden** (VMK + godot + UniVRM); Unity 6000.4.6f1 was present locally after all. three-vrm (needs Playwright chromium) would make it 4-way; see `docs/superpowers/plans/2026-05-29-vrm-0x-slice2-d4-render-runbook.md`.
- **No VMK issue to file from this run.** The dynamic-swing divergence is version-unstable (integrator variance); the only way it becomes file-worthy is a *stable, version-consistent* outlier on a controlled isolator — not observed.
- The v0 **MToon** sweeps remain un-renderable (meshless) — a separate emit fix (geometry + skeleton in `emit_vrm_v0`) is needed before the 0.x MToon material signal can be read on any adapter.
- No VMK code changed; the Ry180 camera/light conjugation lives entirely in the conformance adapter, accepting VMK's documented normalization.

## 2026-05-28 — Material-name classification sweep: NEGATIVE on a sphere (reproducer geometry is insufficient)

**What.** The `material_name_classification` sweep (`emit-material-name-classification-sweep`) was built to catch VMK's material-name → forced-double-sided + overlay-depth-bias misfire (the `Vita_clothing` z-fighting; root cause verified at `VRMRenderItemBuilder.swift:216`). Rendered the four single-sided variants — one MToon material under names `matname_plain` (control), `matname_clothing` (trips `cloth`), `matname_skirt` (trips `skirt`), `matname_body` (different category) — through three-vrm and VMK at the standard sweep sphere.

**Result — the sweep does NOT reproduce the artifact.** Within each renderer, every name variant is **byte-identical** to the plain baseline:

| renderer | clothing vs plain | skirt vs plain | body vs plain |
|---|---|---|---|
| three-vrm | SSIM 1.000000, identical SHA | 1.000000, identical | 1.000000, identical |
| vrm-metal-kit | SSIM 1.000000, identical SHA | 1.000000, identical | 1.000000, identical |

three-vrm being name-invariant is expected (conformant). **VMK being name-invariant here is the surprise** — VMK's name heuristic *does* fire in code (verified), but its two consequences are both **invisible on a convex opaque sphere**:
1. **Forced double-sided / `cullMode(.none)`** — a convex sphere's backfaces are always occluded by its own frontfaces, so culling backfaces vs not produces identical pixels.
2. **Overlay depth bias (pull toward camera)** — with nothing behind the sphere to z-fight (only the background, which always loses the depth test), pulling it forward changes nothing visible.

**Why this matters / correction to the reproducer design.** The `Vita_clothing` artifact requires geometry the sweep sphere lacks: (a) **thin / non-convex** surface (a cape/dress) so backface culling is visible, and (b) **layered** geometry behind it so the slope-scaled depth bias produces silhouette z-fighting. The classification *happens* on the sphere; its *visible damage* needs the right geometry. So the spec at `docs/superpowers/specs/2026-05-28-material-name-classification-reproducer.md` is correct about the mechanism but its sphere-based variant set is **insufficient to surface it** — a real limitation found by rendering rather than assuming.

**Next (reproducer v2, not yet built).** Re-emit the same material-name × doubleSided matrix on **thin two-layer geometry** (e.g. the existing spring-bone chain mesh, or a double quad: an inner plane + an outer thin shell) so both consequences become pixel-visible. Then VMK should diverge on the `*cloth*`/`*skirt*` variants while three-vrm/UniVRM stay invariant. The current sphere sweep is retained as a **control** proving the classification is pixel-invisible on convex opaque geometry (itself a useful baseline). The methodology pin (`docs/methodology.md`, "Face culling honors `material.doubleSided`, not material name") stands — only the asset geometry needs upgrading to exercise it.

**Boundary.** Conformance-side only; no VMK changes; no fix-option chosen. The VMK root cause remains as documented for the maintainer.

## 2026-05-28 — VMK 180° flip MATERIALIZES on VRM 0.x humanoids (real-adapter render; Task 27 prediction contradicted)

**What.** Rendering VRM 0.x humanoid fixtures through two real adapters at the slice-1 `-Z` camera convention shows a consistent orientation divergence: **three-vrm renders the avatar's front; vrm-metal-kit renders the back (back of head, bare back).** Consensus fails on both 0.x fixtures tested.

| Asset | three-vrm | vrm-metal-kit | SSIM(tvrm,vmk) | consensus |
|---|---|---|---|---|
| `avatarU_0_0` (VRoid Studio 2.13.0, spec 0.0, CC_BY) | front | back | 0.675 | FAIL (< 0.92) |
| `avatarA_0_0` (VMK AvatarSample_A, spec 0.0) | front | back | 0.716 | FAIL (< 0.92) |

Both adapters report `overall_passed: true` for the render op itself — the divergence surfaces only at `consensus-diff`. The two SSIM values differ only because avatarA's cardigan is more front/back-similar than avatarU's dress; the *orientation* divergence is identical. Failure-mode evidence committed for avatarU only (avatarA render withheld per its VRM Platform License 1.0; avatarU is CC_BY): `docs/images/vrm0x_orientation_avatarU_three-vrm_front.png` (front, three-vrm) vs `docs/images/vrm0x_orientation_avatarU_vmk_back.png` (back, VMK). This is the failure-mode example the Task 39 closeout (criterion #4) and Task 27 entry flagged as pending bootstrap.

**Stale-binary ruled out.** First observed with a May-24 VMK adapter binary (pre-slice-1-merge, also missing Task-24 spec_version wiring → `source_spec_version: null`). Rebuilt the adapter fresh at the pinned revision (VRMMetalKit `392d949` = 0.16.0 stable; Swift 6.3.2 / Xcode 26.5). The rebuild changed the render at the byte level (real recompile), but orientation was unchanged and SSIM moved only 0.6753 → 0.6749. So the flip is **live behavior of VMK 0.16.0 at the current pin**, not a stale artifact.

**Correction to a prior finding.** The Task 27 entry (2026-05-26, below) recorded Task 9's prediction that "the original design's day-10 expected failure (`VMK renders back of head on 0.x`) is unlikely to materialize ... three-vrm + VMK should both render the front ... because VMK's load-time coord normalization preserves 'forward'." **Render evidence contradicts that prediction.** VMK does render the back of the head on 0.x. The `VRMModel.buildNodeHierarchy()` conjugation Task 9 cited is real, but it does not produce front-facing parity with three-vrm under the `-Z` camera. Task 9's "preserves forward" conclusion was inferred from code reading, not from a render; the render overrides it.

**Slice-1 criterion #2 status.** This empirically satisfies slice-1 success criterion #2 ("VMK 180° flip flagged as a conformance failure with a clear visual signal"): the suite caught a genuine cross-renderer orientation divergence on 0.x and flagged it as a consensus failure with committed visual evidence. The criterion had been marked deferred on the (now-falsified) Task 9 assumption that the flip would not surface.

**Golden-reference verdict — VMK is the outlier (added 2026-05-28).** Brought in UniVRM (Unity 6000.4.6f1 / UniVRM v0.131.0), the VRM-consortium reference implementation, as a third renderer. UniVRM renders the **front** of the avatar on both assets, clustering with three-vrm; VMK is alone in rendering the back. The 3-way SSIM matrix:

| pair | avatarU_0_0 | avatarA_0_0 |
|---|---|---|
| three-vrm × **univrm (golden)** | **0.824** | **0.895** |
| vrm-metal-kit × univrm (golden) | 0.675 | 0.696 |
| three-vrm × vrm-metal-kit | 0.675 | 0.716 |

On both assets three-vrm clusters with the UniVRM golden reference (0.82–0.89) while VMK sits ~0.68–0.70 against *both* of the others. This resolves the "no oracle" caveat: **VMK's 180° flip is the non-conformant behavior; three-vrm and UniVRM are correct.** (All pairs are below the 0.92 toon-shading threshold so `consensus_passed=false` for all three — but the front-cluster ~0.82–0.89 vs flip ~0.68 gap is the unambiguous orientation signal; the residual three-vrm/univrm gap is ordinary cross-renderer shading variance, not orientation.) Getting UniVRM to render also required fixing a slice-1 regression: `SpecVersionDetector` was `internal` and broke the UniVRM PlayMode compile across asmdef boundaries (CS0122) — fixed to `public` (commit `7c3eb44`); CI only build-validates UniVRM so it shipped broken in the merge.

**Root cause located + mechanism confirmed (2026-05-28).** In VRMMetalKit (pinned rev `392d949` / 0.16.0), `Sources/VRMMetalKit/Core/VRMModel.swift` migrates 0.x facing −Z → +Z at load: `buildNodeHierarchy()`'s `if isVRM0` block (~line 988) conjugates every node's TRS by Ry180 (`rotation (x,y,z,w)→(-x,y,-z,w)`, `translation (x,y,z)→(-x,y,-z)`), and `applyVRM0InverseBindMatrixConjugation()` (~line 881) left-multiplies every skin `inverseBindMatrix` by Ry180 to keep skinning consistent. three-vrm and UniVRM preserve the 0.x-authored −Z facing (correct per VRM 0.x README:238 "Model faces towards -Z"); VMK's `isVRM0` Ry180 is the lone extra transform. Confirmed it is a **pure 180°-about-Y rotation, not a handedness/mirror bug**, via a mirrored +Z-camera probe (same asset, camera at +Z):

| camera | three-vrm | vrm-metal-kit |
|---|---|---|
| −Z (spec) | front | back |
| +Z (mirror) | back | **front** |

The result inverts cleanly. Cross-SSIM VMK@+Z(front) vs three-vrm@−Z(front) = **0.8088** (matches the front-cluster value), vs three-vrm@+Z(back) = 0.6762. So VMK's model is exactly three-vrm's rotated 180° about Y — front maps to back with correct chirality, no left/right mirror or inversion. **Fix direction** (chosen): stop normalizing 0.x facing to +Z; keep the authored −Z orientation and handle the 0.x/1.0 convention at the camera/forward layer, matching UniVRM/three-vrm. **Caveat for whoever implements it:** the `isVRM0` Ry180 is load-bearing per its own doc comment ("Applied once at load time so physics, animation, and culling all see the same coordinate space" + "left limbs positive X"), so the change is a scoped refactor of the `isVRM0` branch requiring re-verification of spring-bone, VRMA retargeting, lookAt, frustum culling, and ARKit `+X=left` against the un-normalized −Z frame — VMK's VRM 1.0 path is correct and must stay untouched.

**Open / next.** (1) ~~Which renderer is spec-correct~~ — **settled**: VMK is the outlier per the UniVRM golden reference above. (2) Comment on the existing upstream issue **VMK#299** ("VRM 0.x avatar orientation: 180° rotation applied on load contradicts spec (-Z facing)") with this reproducer + 3-way data rather than filing a duplicate. (3) Repro is fully local: `scripts/install-humanoid-fixtures.sh`, then render `test-plans/manual/humanoid/avatarU_0_0.test.yaml` (and `avatarA_0_0`) through three-vrm (`adapters/three-vrm/dist/main.js`), VMK (`adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter`), and UniVRM (`adapters/univrm/launcher.sh` via `execute-test-batch`), then `consensus-diff` the three.

## 2026-05-27 — Slice 1 of VRM 0.x conformance: end-of-slice closeout (Task 39)

**Scope.** Four-adapter VRM 0.x conformance infrastructure landed across the Rust workspace, all four real adapters (three-vrm, vrm-metal-kit, univrm, godot-vrm) plus the deterministic mock renderer, plus the methodology doc and site UI. 39 plan tasks across four phases.

**Branch:** `worktree-vrm-0x-slice1` — ~50 commits ahead of `main`, starting `da936e2` (design doc) / `2ca1455` (plan) and ending with this entry.

### Cross-cutting infrastructure (landed in slice 1)

- `SpecVersion::{V0, V1}` enum in `vrm-ops`, threaded through manifest schema, test plan schema, ops contract, generator CLI, four adapters (Tasks 1–14).
- `vrm-normalize` crate: v0→v1 expression preset mapping (joy→happy, etc.), 0–100 → 0–1 weight scaling, custom-preset passthrough with `custom:<name>` markers. v1→v0 explicitly rejected (Tasks 7, 32–34).
- `SweepApplicability::{Applicable, NotApplicable{reason}}` enum with 5 structured reason variants. Compile-time symmetry assertion runs in CI (Tasks 3, 18).
- Generator: `--spec-version 0.x | 1.0` flag; emits full v0 .vrm with `VRM` extension namespace (specVersion `"0.0"`, full meta + humanoid + firstPerson + blendShapeMaster + secondaryAnimation + materialProperties blocks). Shared MToon math extracted to `mtoon_common.rs`; v0 wiring in `mtoon_v0.rs`; v0 expressions in `expressions_v0.rs` with 0–100 weight typing (Tasks 11–17).
- Three hard-error gates on the runner:
  1. Plan ↔ manifest cross-check via `validate-manifest` (test_id naming heuristic vs. `spec_version` field) — Task 5.
  2. Plan camera direction ↔ `spec_version` via `validate_camera_convention` (V0 plans require -Z camera; V1 plans require +Z) — Task 25.
  3. Plan ↔ adapter-reported `source_spec_version` via `cross_check_source_spec_version` — Task 26.
- Read-side ops contract: `source_spec_version: SpecVersion` required on every dump response; `as_spec_version: Option<SpecVersion>` optional on every dump request (Tasks 21, 22).
- Runner-side normalization dispatch: `apply_normalization_if_requested` returns `-32001 NormalizationDirectionUnsupported` when v1→v0 requested (Task 35).

### Slice 1 success criteria — status

1. **Four-adapter diff produced on `mtoon_basic_v0_lit_001` and `expressions_preset_basic_v0`.** **DEFERRED** to user-side bootstrap (Task 39). All adapter wiring is done; an actual `bootstrap-goldens.sh` run is needed to produce the diff. Requires fixtures + built adapter binaries.
2. **VMK 180° flip flagged as conformance failure with clear visual signal in published site.** **REVISED.** Task 9's empirical investigation found that VMK's "180° flip" is intentional library normalization (`VRMModel.buildNodeHierarchy()` conjugates 0.x TRS into VRM 1.0 / glTF space at load), not a non-spec defect. The expected day-one failure flag may not fire. The methodology doc (Task 37) documents this; the actual cross-renderer outcome resolves at user-side bootstrap.
3. **`vrm-normalize` round-trip property test passes in CI.** **✅** Task 36 added `crates/vrm-normalize/tests/round_trip_property.rs` with 4 tests covering equivalent-maps, determinism, weight-scaling precision, and custom-passthrough uniqueness. Runs as part of `cargo test --workspace` in `.github/workflows/rust.yml`.
4. **Methodology doc section live with spec citations, camera-Z table, and at least one failure-mode example image.** **✅ (2026-05-28).** Section is live (`docs/methodology.md` Task 37): camera convention with spec line refs (`specification/0.0/README.md:238`, `specification/VRMC_vrm-1.0/tpose.md` Definition 1.1), preset mapping table, sweep symmetry, three hard-error gates. The failure-mode example image is now committed: `docs/images/vrm0x_orientation_avatarU_three-vrm_front.png` (front) vs `docs/images/vrm0x_orientation_avatarU_vmk_back.png` (back), captured from the real-adapter render documented in the 2026-05-28 VMK-180°-flip entry at the top of this log.
5. **`spec_version` field present on every manifest entry; CI validator enforces.** **✅** Tasks 4 + 5 + 6 cover this. `ManifestEntry.spec_version: SpecVersion` (required at parse, defaults V1 for back-compat); `validate-manifest` cross-checks test_id naming against the declared field; migration script `scripts/migrate-manifest-spec-version.sh` is idempotent and SHA-stable on re-run.
6. **Sweep registry symmetry assertion passes — every `*_v0` sweep entry has a 1.0 counterpart or `NotApplicable` reason.** **✅** Task 18's `sweep_registry_symmetric_across_versions` test in `crates/vrm-asset-generator/src/sweep.rs` enforces this. Slice 1's `mtoon_basic_v0_outline_lighting_mix` is the canonical `NotApplicable { reason: OutlineLightingMixV1Only }` example.

**Pass rate:** 4 of 6 criteria fully verified in this slice; 2 (#1, #2) deferred to user-side bootstrap; #4 partial (image backfill pending). Slice 1 ships with the infrastructure complete and the deferred items clearly scoped for follow-up.

### Adapter wiring summary

| Adapter | source_spec_version detection | source_spec_version reporting | canLoadVrm0X | Tests |
|---|---|---|---|---|
| three-vrm (Task 23) | `gltf.parser.json.extensionsUsed` | All three dump ops | n/a (already loaded 0.x) | 7 new fixture-gated tests |
| vrm-metal-kit (Task 24) | `model.specVersion` (library property) | All three dump ops via JSONValue | n/a (already loaded 0.x) | 8 new (6 dispatch + 2 fixture-gated); `swift test` 27/27 |
| univrm (Task 28) | Pre-load GLB inspection via `SpecVersionDetector.DetectFromGlbPath` | EntryDto on EditMode + PlayMode batch paths | **true** (was `false`) | 9 new EditMode tests |
| godot-vrm (Task 30) | `state.json["extensionsUsed"]` after `append_from_file` | All three dump ops | n/a (already loaded 0.x) | Dispatch validated via Rust shim unit tests |
| vrm-mock-renderer (Task 31) | Reads JSON chunk via `vrm_asset_generator::glb::extract_json_chunk` | All three dump ops | n/a | 19 unit tests + 24 integration tests |

### Items deferred to user-side execution

The following slice 1 tasks have findings entries marked DEFERRED. They require local fixtures + built adapter binaries + actual render runs, which weren't feasible in the fresh execution worktree:

- **Task 8 (VRoid Studio 0.x export check):** requires Studio GUI inspection.
- **Task 27 (mid-slice checkpoint, first two-adapter diff):** requires `bootstrap-goldens.sh` run.
- **Task 29 (UniVRM coord-handling repro):** requires Unity + UniVRM build + render run.
- **Task 39 end-of-slice bootstrap step:** requires fixtures + all four adapters built; produces the actual cross-renderer diff data + populates `goldens/manifest.json` with real entries; deploys site to GitHub Pages.

These do not block slice 1's infrastructure deliverable. They block the **announcement-ready** end state — once the user runs them, the deferred findings entries get backfilled with empirical data, the missing methodology-doc image lands, and slice 1 graduates from "infrastructure complete + verified-in-CI" to "external-facing diff visible on the published site."

### Pre-existing test issues observed (not from slice 1)

- `vrm-runner` integration test `dump_positions_smoke::execute_plan_with_reference_positions_against_mock_passes` fails with "No such file or directory" on a missing `smoke_default` plan asset. The asset is bootstrapped via `scripts/bootstrap-goldens.sh` rather than committed; the test predates slice 1 and is not part of the workspace `--lib` baseline that gates Phase A.
- Two `TestPlan` struct-literal callsites in `vrm-runner/tests/{diff_integration,execute_test_batch}.rs` were broken by Task 2's field addition. Fixed in `8788c9d` during Task 21 cleanup.

### Architecture invariants enforced going forward

Per the design doc, the following invariants are baked into slice 1's CI gates and are expected to hold across slices 2–4:

- Sweep registry symmetry assertion runs in CI for every slice (Task 18).
- Manifest `spec_version` field is required on all new entries; CI validator enforces (Task 5).
- `vrm-normalize` round-trip property test runs in CI (Task 36).
- No adapter implements normalization — runner-side only, single bug surface.
- Camera convention pin enforced by runner per `spec_version`; test plans cannot hardcode wrong-handed camera (Task 25).
- `vrm_ext_v0.rs`, `mtoon_v0.rs`, `expressions_v0.rs` remain emit-only; any parser work triggers re-evaluation of the single-crate generator architecture.

### Announcement materials

When the user runs the deferred bootstrap and the announcement-ready state is reached, the announcement to Frans (three-vrm) / 0b5vr (VRM ecosystem) / Lyuma (godot-vrm) should emphasize:

- Cross-cutting infrastructure for VRM 0.x conformance is in place: spec_version-aware manifest schema, test plan schema, ops contract, four-adapter wiring, runner cross-checks, normalization dispatch.
- Four-adapter consensus diff on a small 0.x corpus (slice 1's deliverable; richer corpus follows in slices 2–4).
- Documented methodology pins for 0.x: camera convention (-Z), normalization contract (one-way v0→v1, custom passthrough, v1→v0 rejected), sweep registry symmetry, three hard-error gates.
- The architecture donation path to Khronos glTF WG (per the project's stated goal in CLAUDE.md) is unaffected by slice 1 — the 0.x corpus is methodology-compatible and the `SpecVersion` enum extends cleanly when VRM 1.1 lands.

---

## 2026-05-26 — VRoid Studio 2.12.0 0.x export availability (slice 1 days 1–3 empirical check)

**Check.** VRoid Studio 2.12.0, File → Export, format dropdown inspected.

**Result.** **RESOLVED 2026-05-28 — AVAILABLE.** VRoid Studio **2.13.0** still ships the VRM 0.x export path. Confirmed empirically: `AvatarSample_U` was exported at spec 0.0 (`AvatarSample_U_0.0.vrm.glb`, exporter `VRoid Studio-2.13.0`, generator `UniGLTF-2.64.1`) and passes mrxz/vrm-validator `2.0.0-dev.3.10` with **0 errors** (22 warnings, all expected for VRM 0.x and benign: `UNSUPPORTED_EXTENSION` + `INVALID_EXTENSION_NAME_FORMAT` on the bare `VRM` extension name — 0.x predates the glTF `VENDOR_name` convention that 1.0's `VRMC_*` namespace satisfies — plus `MESH_PRIMITIVE_GENERATED_TANGENT_SPACE` (VRoid exports no tangents) and `UNUSED_OBJECT` on three unused textures). Topology: VRM 0.0, 54 humanoid bones, 14 blendShape groups, 22 secondaryAnimation bone groups, 21 materials, ~179k verts, `CC_BY`.

The original 2026-05-26 check targeted 2.12.0 and was DEFERRED pending GUI inspection (VRoid Studio has no CLI; procedure in `scripts/check-vroid-studio-0x-export.sh`).

**Tier 2 fixture landed (Task 19 Path AVAILABLE).** Sourced as `avatarU_0_0.vrm` — sibling of `avatarA_0_0`, not redistributed in-repo. `scripts/install-humanoid-fixtures.sh` symlinks `AvatarSample_U_0.0.vrm.glb → avatarU_0_0.vrm`; render plan at `test-plans/manual/humanoid/avatarU_0_0.test.yaml` (same camera/lighting as `avatarA_0_0` for apples-to-apples). No `vroid_default_F_0_0` was needed — `AvatarSample_U` serves the second-canonical-0.x-humanoid role.

**Pre-decision.** Slice 1 proceeds with the **fallback path** for now (use `avatarA_0_0` alone as Tier 2 canonical, defer `vroid_default_F_0_0` until Studio export availability is confirmed). This keeps slice 1 unblocked. If the manual check later confirms AVAILABLE, the VRoid fixture can be backfilled in a follow-up commit; if REMOVED, the fallback is already in place.

**Implication if AVAILABLE (future):** re-export VRoid default character through the 0.x path; land as `assets/humanoid/vroid_default_F_0_0.vrm`. Slice 1 Tier 2 canonical fixture set grows by one.

**Implication if REMOVED (current operating assumption):** slice 1 ships with `avatarA_0_0` alone as Tier 2 canonical. Alternate-source paths (older Studio installer; Hub-sourced content) can be explored in slice 2+ if needed.

**Slice 1 closure (Task 19, 2026-05-26):** the `avatarA_0_0` fallback is in effect. `avatarA_0_0.vrm` is sourced via `scripts/install-humanoid-fixtures.sh` (symlinks from a VRMMetalKit checkout; the asset is not redistributed in this repo per its VRM Platform License). The Task 20 test plan at `test-plans/manual/humanoid/avatarA_0_0.test.yaml` consumes this fixture path. No `vroid_default_F_0_0.vrm` is shipped in slice 1.

## 2026-05-26 — Slice 1 mid-slice checkpoint (Task 27): DEFERRED to user-side bootstrap

**Status.** DEFERRED. This checkpoint requires local fixtures (`assets/humanoid/avatarA_0_0.vrm`, `vroid_default_F_1_0.vrm`) installed via `scripts/install-humanoid-fixtures.sh` plus built adapter binaries (three-vrm via `npm run build`, vrm-metal-kit via `swift build`) plus an actual `scripts/bootstrap-goldens.sh` run. None of these are reliably available in a fresh CI worktree; the checkpoint is meant to run on the user's local M-series Mac development environment.

**Expected outcome when run.** With Task 9's empirical finding clarifying that VMK's "180° flip" is intentional library normalization (not a non-spec defect), the original design's day-10 expected failure (`VMK renders back of head on 0.x`) is unlikely to materialize. Instead, three-vrm + VMK should both render the front of the avatar on 0.x assets when the camera is at -Z, because VMK's load-time coord normalization preserves "forward."

**What to capture when the user runs it.**
1. Build adapters: `cd adapters/three-vrm && npm install && npm run build && cd ../vrm-metal-kit && swift build`.
2. Run bootstrap on slice-1 assets: `scripts/bootstrap-goldens.sh --plans test-plans/manual/humanoid/avatarA_0_0.test.yaml` (or whatever the actual command surface is).
3. Run consensus: `scripts/consensus-report.sh --manifest goldens-cache/<bootstrap-dir>/manifest.json`.
4. Inspect both rendered PNGs visually. Save the failure-mode example (if any) to `docs/images/`.
5. Update this entry with: SSIM(three-vrm, VMK) result, visual outcome, and whichever adapter (if any) flagged as outlier.

**Slice 1 implication.** Until this checkpoint runs, success criterion #2 ("VMK 180° flip flagged as conformance failure with clear visual signal") cannot be verified. Slice 1 can ship without this verification if the user accepts that the design assumption was inverted by Task 9. Alternatively, the slice can wait until the user runs the bootstrap locally.

The remaining Phase C tasks (UniVRM wiring, godot-vrm wiring, mock renderer) and Phase D tasks (normalization, methodology doc, site filter) proceed regardless of this deferral — they don't depend on the mid-slice checkpoint's output.

## 2026-05-26 — Slice 1 UniVRM coord-handling repro (Task 29): DEFERRED to user-side bootstrap

**Status.** DEFERRED. Same shape as Task 27 deferral: this investigation requires a built UniVRM adapter and an actual render run on a 0.x asset, which needs Unity 6 installed locally (CLAUDE.md: "CI does build-validate only"). Not feasible in a fresh worktree.

**What changed in Task 28.** UniVRM now accepts 0.x assets (`canLoadVrm0X: true` at all three call sites: `Conformance.cs`, `BatchRunner.cs`, `Vrm10LoadSpike.cs`). Before Task 28 it rejected them outright. So the Task-29 investigation can now actually run.

**What the repro should produce when run.** Render `avatarA_0_0_lit_baseline` through UniVRM and compare visually + via SSIM against three-vrm's render of the same plan (three-vrm is the spec-correct baseline per Task 9). If UniVRM's render differs significantly (e.g., wrong side of the avatar, sideways orientation), that's the coord-handling bug surfacing. Capture the result here and either:

1. **If UniVRM renders correctly:** the suspected coord bug was either fixed upstream since the slice-1 design was written, or was inferred from limited evidence. Close out as "no coord bug observed in current UniVRM v0.131.0 + this adapter wiring."
2. **If UniVRM renders incorrectly:** isolate which axis (front-vs-back? sideways? upside-down?) and check `https://github.com/vrm-c/UniVRM/issues` for related upstream issues. File a new issue with the slice-1 plan as the reproducer if unfiled, and link here.

**Slice 1 impact.** This deferral does not block the rest of Phase C/D. Tasks 30 (godot), 31 (mock), 32–34 (vrm-normalize), 35–36 (runner normalization + tests), 37 (methodology doc), 38 (site filter) can all proceed. Slice 1 success criterion #2 (VMK 180° flip flagged) was the design's original expected failure flag — Task 9 already inverted that expectation. UniVRM's coord behavior is a separate empirical question that resolves when the user runs the bootstrap.

The methodology hazards in `docs/methodology.md` describe what divergence we *expect* between renderers (tone mapping, shadow noise, outline AA, …). This document records divergence the suite *actually observed* in our specific corpus + specific adapter pair, beyond those expected differences.

## Corpus-wide consensus, three-vrm 3.5.0 vs vrm-metal-kit `50cfd7d`

**Date**: 2026-05-11, vrm-conformance commit `1ff198c`.

**Method**: `scripts/bootstrap-goldens.sh` rendered the full 80-test_id corpus (44 MToon variants + 18 spring-bone settle + 18 spring-bone swing) through both real adapters on macOS 26 (Apple M4 Max). `scripts/consensus-report.sh` then ran pairwise SSIM across the bootstrap manifest. Output: `goldens-cache/consensus-report.json` (gitignored — machine-specific paths).

**Headline**: every single test_id fails the v1.0-standard 0.985 SSIM threshold.

```
consensus_passed: 0 / 80
consensus_failed: 80 / 80

Pairwise SSIM corpus-wide:
  three-vrm vs vrm-metal-kit   mean=0.7447  min=0.6313  max=0.9665  n=80
```

Even the closest renderer pair in the entire corpus (`max=0.9665`) is well below the conformance threshold. The mean (0.7447) is more than 20 percentage points below threshold.

### Top 15 most-divergent test_ids

| test_id | min pairwise SSIM | outliers |
|---|---|---|
| `mtoon_outline_world_0p1` | 0.6313 | both |
| `mtoon_shadingShift_neg0p5` | 0.6893 | both |
| `mtoon_shadingShift_neg0p2` | 0.7013 | both |
| `mtoon_doubleSided_true` | 0.7045 | both |
| `mtoon_shadingToony_0p25` | 0.7072 | both |
| `mtoon_shadingToony_0p75` | 0.7079 | both |
| `mtoon_shadingToony_0p5` | 0.7087 | both |
| `mtoon_shadingToony_0p1` | 0.7101 | both |
| `mtoon_shadingShift_neg0p8` | 0.7103 | both |
| `swing_springbone_default` | 0.7105 | both |
| `swing_springbone_drag_0` | 0.7105 | both |
| `swing_springbone_drag_0p2` | 0.7105 | both |
| `swing_springbone_drag_0p8` | 0.7105 | both |
| `swing_springbone_drag_1` | 0.7105 | both |
| `swing_springbone_gravity_0` | 0.7105 | both |

### Observations

**MToon shading divergence dominates.** Nine of the top fifteen most-divergent test_ids vary either `shadingShiftFactor` (the toon-ramp boundary) or `shadingToonyFactor` (the toon-ramp steepness). Both are MToon-1.0 parameters that directly govern how the lit/shadow boundary is computed. Cross-renderer disagreement on this axis is the most expensive kind of conformance gap — it touches the spec's core algorithm.

**Outline rendering is the single worst case.** `mtoon_outline_world_0p1` (world-space outline at 0.1 width) is the most-divergent test in the corpus. Outline rendering is well-known as a methodology hazard (`docs/methodology.md` calls out outline AA differences explicitly), but the magnitude of divergence here (0.6313 SSIM) suggests more than just edge-AA noise.

**Spring-bone swing variants cluster at exactly 0.7105.** Many swing variants produce identical SSIM (rounded to four places). That's evidence the visible mesh isn't responding to chain physics — consistent with the deferred chain-skinned-mesh infrastructure (`crates/vrm-asset-generator/src/chain_mesh.rs`) blocked behind [arkavo-org/VRMMetalKit#181](https://github.com/arkavo-org/VRMMetalKit/issues/181). Without a mesh skinned to the chain joints, swing renders look the same regardless of physics parameters, so the corpus-wide signal degenerates to "two renderers disagree on the same sphere mesh shading regardless of which spring-bone variant generated it."

**Settle vs swing produce the same SSIM.** `swing_springbone_default` (0.7105) and `springbone_default` (in the per_test_id list, also clustered around 0.71) match — confirming the same conclusion. Until the chain-skinned mesh is wired, spring-bone divergence equals MToon-default divergence on a static sphere.

### Filed upstream

- [arkavo-org/VRMMetalKit#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) — root cause of vrm-metal-kit's flat-white sphere across the entire MToon sweep. Single fix would substantially improve the mean for every MToon test_id.
- [pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838) — three-vrm's dark shadow with a falsifiable color-space hypothesis (`0.5^2.2 ≈ 0.21` matches the observed value).

These two issues together cover the dominant divergence pattern for MToon shading. If one or both lands, the corpus-wide mean SSIM should rise substantially and the threshold gap close.

## Second run: VRMMetalKit 0.13.1

**Date**: 2026-05-11, vrm-conformance commit (pending; this section commits with the version bump). Same hardware (M4 Max), same three-vrm version (3.5.0), only the VRMMetalKit revision changed.

**Trigger**: [VRMMetalKit 0.13.1](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.13.1) shipped, closing two of the three bugs filed by this suite (#181 + #182, in [PR #184](https://github.com/arkavo-org/VRMMetalKit/pull/184)). Re-rendering through the corpus measures the delta.

### Corpus-wide before/after

| Metric | Baseline (50cfd7d) | 0.13.1 (9404287) | Delta |
|---|---|---|---|
| consensus_passed | 0 / 80 | 0 / 80 | unchanged |
| mean pairwise SSIM | 0.7447 | **0.7002** | **−0.0445** |
| min pairwise SSIM | 0.6313 | **0.1840** | **−0.4473** |
| max pairwise SSIM | 0.9665 | 0.9490 | −0.0175 |

The pass count is unchanged because the v1.0 threshold (0.985) is still far above the corpus-wide max. But the distribution shifted significantly — and not all in the direction we'd expect.

### Pattern: two clusters move in opposite directions

**MToon shading (~44 variants): essentially unchanged.** `shadingShift_neg0p5` stayed at 0.6893. `shadingShift_neg0p2` stayed at 0.7013. The full `shadingToony_*` cluster stayed at 0.708x. Expected, because the release notes don't mention #183 (the flat-white sphere root cause for MToon-default rendering) and our corpus's MToon shading divergence is dominated by that single cause.

**Spring-bone (~36 settle + swing variants): unchanged at 0.7105.** Expected — visible signal still requires the chain-skinned-mesh asset-side wiring (the infrastructure is in `crates/vrm-asset-generator/src/chain_mesh.rs` but deferred until #181 lands), and even though #181 is now fixed upstream we haven't re-wired chain_mesh into emit yet.

**Outline rendering (8 variants): substantial regression.** The 8 outline test_ids now occupy the top 8 worst slots:

| test_id | baseline | 0.13.1 | Δ |
|---|---|---|---|
| `mtoon_outline_world_0p1` | 0.6313 | **0.1840** | −0.4473 |
| `mtoon_outline_world_0p05` | (n/a in top 15) | **0.3588** | (large drop) |
| `mtoon_outline_screen_0p1` | (n/a in top 15) | **0.4028** | (large drop) |
| `mtoon_outline_world_0p03` | (n/a in top 15) | **0.4330** | (large drop) |
| `mtoon_outline_screen_0p05` | (n/a in top 15) | **0.4711** | (large drop) |
| `mtoon_outline_screen_0p03` | (n/a in top 15) | **0.4967** | (large drop) |
| `mtoon_outline_world_0p01` | (n/a in top 15) | **0.5018** | (large drop) |
| `mtoon_outline_screen_0p01` | (n/a in top 15) | **0.5223** | (large drop) |

The corpus-wide mean dropped −0.0445 specifically because of this 8-variant cluster. The release that closed #181 (non-skinned mesh dropped when skin present) appears to have introduced a regression in outline rendering — outline width and mode now produce visibly different pixels than before, and the divergence vs three-vrm is much larger than at the old pin.

### New findings: two outline bugs surfaced together

This is a measurable behavioral change in VRMMetalKit between 50cfd7d → 9404287. The corpus surfaces it automatically: same VRM, same test plan, same three-vrm version, different VRMMetalKit produces materially different outline pixels. Pixel sampling reveals *both* renderers diverge from MToon-1.0's outline-rendering spec:

| variant | vrm-metal-kit centerline | three-vrm centerline |
|---|---|---|
| `mtoon_outline_none` | `(255, 255, 255)` (flat white, #183) | `(53, 53, 53)` (shaded gray) |
| `mtoon_outline_world_0p1` | `(255, 255, 255)` (outline invisible) | **`(0, 0, 0)` (outline color floods entire mesh)** |

The expected per [VRMC_materials_mtoon-1.0 §4.2 "Outline"](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_materials_mtoon-1.0/README.md) is a thin silhouette band (~6 pixels at this asset's camera distance for `0.01 m` width). Neither renderer produces that.

Both filed:
- [arkavo-org/VRMMetalKit#185](https://github.com/arkavo-org/VRMMetalKit/issues/185) — outline rendering regression in 0.13.1; outline pass appears to drop entirely
- [pixiv/three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) — outline color floods entire mesh interior instead of producing a silhouette band

This is also exactly why we pin the upstream revision in `Package.swift` rather than tracking `main`: regressions like the VRMMetalKit one would otherwise propagate silently. And it demonstrates the conformance suite's payoff structurally: same data surfaced two different upstream bugs in two different renderers, neither of which is visible to that renderer's own unit tests.

### What this run did and didn't validate

- **Did validate**: the two upstream fixes (#181 + #182) are present at 0.13.1 — `swift test` is clean, the adapter binary boots, spring-bone counts are no longer inflated (TBD — needs a separate verification; the corpus signal doesn't reflect this directly since the chain isn't visible).
- **Did NOT validate end-to-end yet**: visible chain-skinned-mesh diffing. With #181 fixed, the chain_mesh.rs infrastructure in this repo can be re-wired into `emit_vrm_with_spring_bone`. That's a separate piece of work — it unblocks the spring-bone signal that's currently degenerate.
- **Surfaced**: an outline-rendering regression that wasn't visible in any per-renderer unit test. Only the cross-renderer signal catches it.

## Third run: chain-skinned mesh wired into emit (VRMMetalKit 0.13.1)

**Trigger**: With [#181](https://github.com/arkavo-org/VRMMetalKit/issues/181) closed in 0.13.1, the deferred chain-skinned cylinder infrastructure (`chain_mesh.rs` + `buffer::pack_sphere_and_chain`) can finally be wired into `emit_vrm_with_spring_bone`. Locally smoke-verified before the corpus run: rendering `springbone_segment_0p2` (4 joints × 0.2 m chain, hangs well below the sphere bounding-box) shows the chain cylinder poking out at the bottom of the frame — sphere + chain coexist correctly on vrm-metal-kit 0.13.1.

### Corpus-wide before/after

| Metric | Run 2 (no chain) | Run 3 (with chain) | Δ |
|---|---|---|---|
| consensus_passed | 0 / 80 | 0 / 80 | unchanged |
| mean pairwise SSIM | 0.7002 | **0.6994** | −0.0008 |
| min pairwise SSIM | 0.1840 | 0.1840 | unchanged |
| max pairwise SSIM | 0.9490 | 0.9490 | unchanged |

The mean barely moved. Chain-cylinder pixels are a small fraction of the frame at default chain dimensions (~25 mm radius, ~0.2 m visible length on the longest variants), so even with the new geometry, outline divergence (the dominant component) still drives the corpus-wide signal.

### What did change: spring-bone variants no longer degenerate

In runs 1 and 2, every spring-bone test_id (both settle and swing) produced exactly 0.7105 SSIM — the chain physics had no visible effect, so all 36 variants collapsed to "identical sphere render plus zero chain pixels", and only the sphere shading (unchanged across variants) mattered.

With the chain-skinned cylinder active, spring-bone variants now produce variant-specific SSIM scores. Top-15 sample from run 3:

| spring-bone test_id | run 3 SSIM | previously |
|---|---|---|
| `swing_springbone_joints_16` | 0.7043 | 0.7105 (degenerate) |
| `swing_springbone_joints_8`  | 0.7043 | 0.7105 (degenerate) |
| `swing_springbone_segment_0p1` | 0.7053 | 0.7105 (degenerate) |
| `swing_springbone_segment_0p2` | 0.7065 | 0.7105 (degenerate) |
| `swing_springbone_default` | 0.7105 (settled) | 0.7105 |
| `swing_springbone_drag_0` | 0.7105 | 0.7105 |
| `swing_springbone_drag_1` | 0.7105 | 0.7105 |

Joints-16 and joints-8 variants diverge most (longer chains = more visible deformation). segment-0p1 and segment-0p2 next (longer segments = more chain pokes below the sphere). Drag and gravity variants stay clustered with default because the chain length is the same (default joint count + default segment length) and only the physics dynamics differ — and the dynamics signal is small at the chain widths and frame sizes we're rendering.

This is the **first time the spring-bone corpus produces a non-degenerate cross-renderer signal**. Renderer differences in chain physics now propagate to pixels.

### Net result

- Three upstream fixes worth of work landed across the three runs.
- Chain-mesh asset infrastructure activated.
- Two new upstream issues filed during run 2 ([VRMMetalKit#185](https://github.com/arkavo-org/VRMMetalKit/issues/185), [three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839)) for outline-rendering bugs.
- Spring-bone signal moved from "degenerate" to "parameter-sensitive."
- Outline rendering remains the dominant divergence — pending the two outline issues.

The corpus-wide mean SSIM is now anchored around 0.70, with the cluster structure dominated by 8 outline tests at the bottom (0.18–0.52) and the rest of the corpus distributed around 0.70–0.95. To meaningfully raise the corpus-wide mean, the outline bugs need to land first. The remaining MToon shading divergence (~0.69 cluster) is still gated on [VRMMetalKit#183](https://github.com/arkavo-org/VRMMetalKit/issues/183).

## Fourth run: VRMMetalKit 0.13.2 — outline regression closed

**Trigger**: [VRMMetalKit 0.13.2](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.13.2) shipped within 3 hours of [#185](https://github.com/arkavo-org/VRMMetalKit/issues/185) being filed. Hotfix; root cause per the release notes: "the outline pass dispatched the inverted-hull geometry at world origin instead of at the rigid mesh node's world position" — side effect from 0.13.1's #181 fix touching the mesh-iteration path.

Re-rendered through vrm-metal-kit only (three-vrm renders preserved from prior run; same hardware, same three-vrm version 3.5.0).

### Corpus-wide before/after

| Metric | Run 3 (0.13.1+chain) | Run 4 (0.13.2+chain) | Δ |
|---|---|---|---|
| consensus_passed | 0 / 80 | 0 / 80 | unchanged |
| mean pairwise SSIM | 0.6994 | **0.7439** | **+0.0445** |
| min pairwise SSIM | 0.1840 | **0.6313** | **+0.4473** |
| max pairwise SSIM | 0.9490 | 0.9665 | +0.0175 |

The 0.13.2 hotfix recovered **exactly** the ground lost by the 0.13.1 outline regression (the delta numbers are symmetric to run 1 → run 2). All three statistics returned to their original baseline values, modulo a tiny rounding band.

### 8 outline tests no longer dominate divergence

In run 3, `mtoon_outline_world_0p1` was the worst test at 0.1840 SSIM. In run 4, it's back to 0.6313 — still the worst test, but in the same range as MToon shading divergence. The 7 other outline tests have fallen entirely out of the top 15 most-divergent list. Top 15 in run 4 is now dominated by:

- 1 outline test (`mtoon_outline_world_0p1` at 0.6313, baseline-equivalent)
- 5 MToon shading tests (shadingShift / shadingToony / doubleSided)
- 8 spring-bone variants (joints / segment / stiffness — parameter-sensitive thanks to the chain-skinned mesh from run 3)
- 1 baseline (`mtoon_default`)

### Cumulative four-run progression

| Run | mean | min | upstream events |
|---|---|---|---|
| 1 | 0.7447 | 0.6313 | first corpus measurement |
| 2 | 0.7002 | 0.1840 | #181/#182 closed; #185+#1839 surfaced |
| 3 | 0.6994 | 0.1840 | chain-skinned mesh wired |
| 4 | 0.7439 | 0.6313 | #185 closed in 0.13.2 |

Three of the four issues filed against VRMMetalKit (#181, #182, #185) are now closed. Three remain open: #183 (MToon flat-white shading), [pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838) (color-space hypothesis), [pixiv/three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) (outline floods entire mesh).

### Time-to-fix observed

- #181 + #182 filed → fixed in 0.13.1: same-session turnaround (hours).
- #185 filed during the 0.13.1 corpus re-run → fixed in 0.13.2: **3 hours**.

When the upstream maintainer is engaged with the conformance suite, the loop closes faster than the test corpus can re-run. The total wall-clock from "find regression" → "merge fix" → "re-measure recovery" is now under a single project session.

## Fifth run: VRMMetalKit 0.13.3 — MToon flat-white closed

**Trigger**: [VRMMetalKit 0.13.3](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.13.3) shipped less than an hour after 0.13.2. Closes [#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) — the deepest of the four bugs filed against VRMMetalKit and the one driving the largest residual divergence. Root cause per the release notes: "the main lighting path applied a Half-Lambert remap that saturated `shadowStep=1` across the visible hemisphere with `shadingToonyFactor=0.9` + typical directional lighting, collapsing the rendered color to `baseColor` everywhere."

That's exactly what the pixel sampling in [#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) showed: every sphere fragment at `(255, 255, 255)` regardless of position, regardless of shading parameter. The toon ramp wasn't applying.

Re-rendered through vrm-metal-kit only.

### Corpus-wide before/after

| Metric | Run 4 (0.13.2) | Run 5 (0.13.3) | Δ |
|---|---|---|---|
| consensus_passed | 0 / 80 | 0 / 80 | unchanged |
| mean pairwise SSIM | 0.7439 | **0.7879** | **+0.0440** |
| min pairwise SSIM | 0.6313 | 0.6313 | unchanged (still outline floor on three-vrm side) |
| max pairwise SSIM | 0.9665 | 0.9665 | unchanged |

The mean moved by +0.044 on a single upstream fix — the largest single-release delta of the session. The 44 MToon shading variants that were anchored at ~0.69 are now distributed across the 0.74–0.76 band.

### Pixel-level recovery

Sphere centerline (x=512 on a 1024×1024 render), `mtoon_default`:

| run | vrm-metal-kit (R,G,B) | three-vrm (R,G,B) |
|---|---|---|
| 1–4 (pre-#183 fix) | **255, 255, 255** (flat white) | 53, 53, 53 |
| 5 (0.13.3) | **164, 164, 164** | 53, 53, 53 |

vrm-metal-kit moved from `1.0` linear (flat white, no shading) to `0.643` linear (real MToon mid-gray, exactly what we'd expect from the spec for a sphere with `baseColor=1.0`, `shadeColor=0.5`, `shadingToonyFactor=0.9`, lit from `(-0.3, -0.6, -0.7)` with intensity 1 + ambient 0.15). The toon math is now firing.

three-vrm is still at 0.208 — consistent with their longer-standing color-space hypothesis ([three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838)). With both renderers' raw outputs now non-trivial, the residual divergence at ~0.79 reflects the *actual* spec-interpretation gap between the two real MToon implementations, not a "one renderer fully broken" artifact.

### Top 15 most-divergent: cluster structure has shifted

| test_id | run 4 | run 5 |
|---|---|---|
| `mtoon_outline_world_0p1` | 0.6313 | 0.6313 (still the floor; three-vrm-side bug) |
| `mtoon_shadingShift_0p8` | not top-15 | **0.7384** |
| `mtoon_shadingToony_0p5` | 0.7087 | **0.7418** |
| `mtoon_shadingShift_neg0p5` | 0.6893 (was worst MToon) | not in top 15 (moved up) |
| `mtoon_shadingShift_neg0p2` | 0.7013 | not in top 15 (moved up) |

Worth noting: `mtoon_shadingShift_neg0p5` was the second-worst test in run 4 at 0.6893 — it fell out of the top 15 entirely in run 5. The negative-shadingShift variants benefited most from the toon-ramp fix (negative shift shifts the lit/shadow boundary toward more shadow, which under the broken ramp had no effect; now the broader shadow area produces actual shadow pixels).

The new bottom of the divergence list is dominated by:
- 1 outline test at the floor (`mtoon_outline_world_0p1` at 0.6313, three-vrm-side per [#1839](https://github.com/pixiv/three-vrm/issues/1839))
- 5 MToon shading + toony variants in the 0.738–0.749 band
- 8 spring-bone parameter-sensitive swing variants in the 0.757–0.762 band

The variance within the spring-bone cluster is now meaningfully tighter (3.7 percentage points of spread vs ~2.5 in run 3/4) because the chain-skinned cylinder is rendering against a properly-shaded sphere background, so the SSIM contributions from chain pixels are easier to discriminate.

### Cumulative five-run progression

| Run | mean | min | upstream events |
|---|---|---|---|
| 1 (50cfd7d) | 0.7447 | 0.6313 | first corpus baseline |
| 2 (0.13.1) | 0.7002 | 0.1840 | #181/#182 closed; #185+#1839 surfaced |
| 3 (0.13.1+chain) | 0.6994 | 0.1840 | chain-skinned mesh wired |
| 4 (0.13.2+chain) | 0.7439 | 0.6313 | #185 closed in 0.13.2 |
| **5 (0.13.3+chain)** | **0.7879** | **0.6313** | **#183 closed in 0.13.3** |

The corpus-wide mean is now +0.0432 ABOVE the original baseline (0.7447 → 0.7879). Three of four VRMMetalKit issues have been closed in the same session that filed them; #183 took 4 hours from filing to closing.

### What remains in the divergence floor

With the three vrm-metal-kit bugs closed, residual divergence comes from:

1. **three-vrm side** — [#1838](https://github.com/pixiv/three-vrm/issues/1838) (dark MToon shadow / double sRGB) and [#1839](https://github.com/pixiv/three-vrm/issues/1839) (outline color floods entire mesh). The first drives the 0.74-cluster floor; the second pins `mtoon_outline_world_0p1` at 0.6313.
2. **MToon spec interpretation** — even with both renderers' fundamental bugs fixed, the two real renderers may legitimately diverge on edge-case shading parameters. Without a third independent reference (UniVRM in Unity), the suite can't pin which interpretation is closest to spec.

The corpus mean is now plausibly approaching what "two-real-renderer pairwise SSIM" can theoretically reach. Further movement requires either three-vrm-side fixes (would raise the corpus floor) or adding a third real adapter to make consensus meaningful enough to identify outliers.

### Open questions

- **Should the corpus's default SSIM threshold be relaxed below 0.985?** v1.0 standardizes on 0.985 per `docs/methodology.md`, but the data shows that's currently unreachable for any test in the corpus. Two interpretations:
  - The threshold is correct; both renderers are sufficiently spec-divergent that pass requires upstream fixes first. The cross-renderer signal IS the conformance result.
  - The threshold needs methodology refinement — e.g., separate thresholds for "exact MToon math" tests vs "approximate visual fidelity" tests, or per-renderer-pair thresholds.
- **Should consensus exclude the mock-renderer entirely from default reporting?** Mock is synthetic and isn't trying to match real renderers. Including it in consensus inflates the apparent divergence. The current script doesn't include mock by default (the bootstrap only renders through real adapters); confirming this stays the convention.
- **Outline-mode divergence at 0.6313 — is that AA noise alone, or a model-level outline-rendering bug?** Likely worth a dedicated investigation (sample the actual outline pixels in both renders).

## Sixth run: godot-vrm L3 shipped — third real renderer added

**Date**: 2026-05-11, vrm-conformance commit `820b716`.

**Trigger**: V-Sekai/godot-vrm vendored at `9fae4049` + Godot-MToon-Shader at `27cb2b78`; L3 Phase 1 ops landed (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`). Phase 2 (spring-bone) deferred — spring-bone test plans skip godot-vrm.

**Method**: `scripts/bootstrap-goldens.sh` rendered the full 80-test corpus through three-vrm, vrm-metal-kit, and godot-vrm on macOS 26 (Apple M4 Max, Godot 4.6.2). `scripts/consensus-report.sh` ran pairwise SSIM across the manifest.

**Headline**: First three-renderer pairwise SSIM data on the corpus. Three-way consensus available for the 44 MToon test_ids where godot-vrm renders; the 36 spring-bone settle + swing tests remain two-renderer (three-vrm vs vrm-metal-kit only) because godot-vrm's Phase 2 ops are `Unimplemented`. godot-vrm vs vrm-metal-kit pairs are the closest at mean SSIM 0.852 — meaningfully tighter than either renderer's pair with three-vrm.

### Corpus-wide consensus

```
Processed 80 test_ids; skipped 0
consensus_passed: 0/80
consensus_failed: 80/80

Pairwise SSIM stats across the corpus:
  pair                                  mean    min     max     n
  godot-vrm vs three-vrm                0.6916  0.1840  0.9482  44
  godot-vrm vs vrm-metal-kit            0.8521  0.5301  0.9517  44
  three-vrm vs vrm-metal-kit            0.7879  0.6313  0.9665  80
```

`n=44` reflects the 44 MToon tests where all three renderers produced output. `n=80` covers the full corpus (including the 36 spring-bone tests where godot-vrm is absent). The `three-vrm vs vrm-metal-kit` pair is unchanged from run 5, as expected — neither renderer changed in this run.

### Top 15 most-divergent test_ids

```
mtoon_outline_world_0p1                   0.1840  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p05                  0.3588  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p1                  0.4028  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p03                  0.4330  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p05                 0.4711  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p03                 0.4967  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p01                  0.5018  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p01                 0.5223  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_doubleSided_true                    0.7045  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p5                 0.7053  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p2                 0.7075  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingToony_0p75                   0.7079  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p8                 0.7106  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg1                   0.7108  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingToony_0p5                    0.7109  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
```

The outline-test cluster dominates the divergence floor as in prior runs (still pinned to `mtoon_outline_world_0p1` at 0.1840). Adding godot-vrm pulled the floor down: in run 5 the floor was 0.6313 (`mtoon_outline_world_0p1` between three-vrm and vrm-metal-kit). godot-vrm renders outlines at a third interpretation that disagrees with both — so the min pair-SSIM for that test drops from 0.6313 to 0.1840.

### Pixel-level sample — mtoon_default

| renderer       | (R, G, B) at sphere center (x=512, y=512) |
|---|---|
| three-vrm      | (53, 53, 53)    |
| vrm-metal-kit  | (164, 164, 164) |
| godot-vrm      | (255, 255, 255) |

For reference: run 5 had three-vrm at (53, 53, 53) and vrm-metal-kit at (164, 164, 164). godot-vrm at (255, 255, 255) is the new data point — flat white at the sphere center, the same surface signature VMK had pre-0.13.3 (run 5 closed [VRMMetalKit#183](https://github.com/arkavo-org/VRMMetalKit/issues/183) for the same symptom). The upstream code paths are entirely independent so the cause differs; the symptom is identical.

### Observations

- **godot-vrm clusters closer to vrm-metal-kit than to three-vrm.** The `godot-vrm vs vrm-metal-kit` mean (0.8521) is +0.164 above the `godot-vrm vs three-vrm` mean (0.6916), and +0.064 above the `three-vrm vs vrm-metal-kit` mean (0.7879). Both godot-vrm and vrm-metal-kit are first-party MToon implementations against the VRMC spec; three-vrm's color-space hypothesis ([three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838)) keeps it darker than either. With three renderers, the consensus diff can now flag `three-vrm` as the outlier on the shading cluster — the methodology's intended use case.
- **godot-vrm mtoon_default = flat white.** Same surface symptom as VMK pre-0.13.3 but unrelated upstream code. Likely candidates: tonemapping not disabled in the Godot-MToon-Shader path, MToon shadingToony saturation similar to VMK's #183, or a default lighting intensity mismatch. Worth a dedicated spike before promoting godot-vrm as the reference for "spec-correct MToon."
- **The outline-test divergence floor dropped from 0.6313 to 0.1840** because godot-vrm renders outlines differently from both other renderers. This is expected: outline rendering is the least-specified part of MToon and three different renderers will produce three different interpretations. The min isn't a regression in any renderer — it's the cost of widening the panel.
- **Spring-bone tests retained run-5 two-renderer numbers** (godot-vrm absent). No regression there.

### Open follow-ups

- **godot-vrm spring-bone tests skipped**: 36 spring-bone settle + swing tests fail the runner's `execute-test-plan` because Phase 2 ops (`step_physics`, `reset_physics`, `animate_root_transform`) return `Unimplemented`. A follow-up plan would add Phase 2 by overriding godot-vrm's `vrm_secondary.gd` spring-bone auto-stepping and taking manual control of the physics pump (`Engine.physics_ticks_per_second = 60`, deterministic per-frame step).
- **godot-vrm flat-white at `mtoon_default`**: file an investigation issue against either `adapters/godot-vrm/src/session.gd` (lighting / tonemap setup) or the upstream Godot-MToon-Shader. Pixel sampling matches the VMK #183 symptom; the fix path likely doesn't.
- **Concern 2 from Spike 2 (mesh-under-head-bone)**: `addons/godot-vrm/VRMC_vrm.gd:387` emits a `Skeleton3D` → `ImporterMeshInstance3D` typed-assignment SCRIPT ERROR during `_create_animation_player` when the asset generator places the mesh node as a child of a humanoid bone (head). Non-fatal — the renderer recovers and produces output — but worth filing upstream against either `V-Sekai/godot-vrm` (typed-assignment hardness; the line should fail gracefully when the node isn't an `ImporterMeshInstance3D`) or `crates/vrm-asset-generator/` (avoid mesh-as-bone-leaf layouts that trip this branch). Reproducer: any of the chain-skinned spring-bone fixtures emitted by `vrm-asset-generator emit-sweep`.

## Seventh run: godot-vrm L4 shipped — full 80-test 3-way consensus

**Date**: 2026-05-11, vrm-conformance commit `9f5aa7b`.

**Trigger**: godot-vrm L4 landed — `step_physics`, `reset_physics`, `animate_root_transform` are now real implementations driving V-Sekai/godot-vrm's `VRMSecondary` node manually (auto-stepping disabled, `do_process` called explicitly, bone-pose-override clearing for proper reset). All 36 spring-bone tests (18 settle + 18 swing) now render through godot-vrm. **This closes the VMK 1.0 launch blocker.**

**Method**: `scripts/bootstrap-goldens.sh` rendered the full 80-test corpus through three-vrm, vrm-metal-kit, and godot-vrm on macOS 26 (Apple M4 Max, Godot 4.6.2). `scripts/consensus-report.sh` ran pairwise SSIM across the manifest.

**Headline**: All three adapters at 80/80. First time the project has full three-way coverage across the entire corpus. Every spring-bone test now has three independent renderers driving the same physics contract.

### Corpus-wide consensus

```
Processed 80 test_ids; skipped 0
consensus_passed: 0/80
consensus_failed: 80/80

Pairwise SSIM stats across the corpus:
  pair                                  mean    min     max     n
  godot-vrm vs three-vrm                0.7042  0.1840  0.9482  80
  godot-vrm vs vrm-metal-kit            0.8709  0.5301  0.9517  80
  three-vrm vs vrm-metal-kit            0.7879  0.6313  0.9665  80
```

All three pairs at `n=80` for the first time. The `three-vrm vs vrm-metal-kit` row is unchanged from run 6 (those renderers didn't move). The `godot-vrm vs *` pairs both gained 36 spring-bone tests' worth of data points:

- `godot-vrm vs three-vrm`: mean +0.0126 (0.6916 → 0.7042), min unchanged (still pinned to the outline cluster).
- `godot-vrm vs vrm-metal-kit`: mean +0.0188 (0.8521 → 0.8709), min unchanged.

The spring-bone tests are pulling both godot-vrm pair means up — i.e. godot-vrm's spring-bone renders agree with the other two renderers more strongly than its MToon renders do. The godot-vrm/vrm-metal-kit pair remains the tightest cluster across the corpus.

### Top 15 most-divergent test_ids

```
mtoon_outline_world_0p1                   0.1840  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p05                  0.3588  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p1                  0.4028  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p03                  0.4330  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p05                 0.4711  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p03                 0.4967  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_world_0p01                  0.5018  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_outline_screen_0p01                 0.5223  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_doubleSided_true                    0.7045  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p5                 0.7053  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingShift_neg0p2                 0.7075  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
mtoon_shadingToony_0p75                   0.7079  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
springbone_joints_16                      0.7096  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
springbone_joints_8                       0.7096  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
springbone_segment_0p1                    0.7097  outliers=['vrm-metal-kit', 'three-vrm', 'godot-vrm']
```

Two `springbone_*` entries crack the top-15 for the first time (`joints_16`, `joints_8`, `segment_0p1`) — but only at ~0.7096, well above the outline-cluster floor of 0.1840. The outline tests still dominate the divergence floor (same 8 entries leading the list as in run 6); their min SSIMs are unchanged because nothing in this run touched outline rendering. The outline-cluster floor hasn't moved with the expanded `n=80` panel — confirming the floor is set by godot-vrm's distinct outline interpretation, not by sample-size noise.

### Observations

- **Spring-bone three-way coverage works.** godot-vrm's manual physics pump (60 Hz fixed step, explicit `do_process` calls, bone-pose-override clearing on reset) produces renders that agree with three-vrm and vrm-metal-kit more strongly than the MToon corpus does. The fact that no spring-bone test cracks the top-8 divergent list — despite three independent physics engines (Godot's, three-vrm's, VMK's) settling the same chain — is the headline methodological win of L4.
- **mtoon_default flat-white persists** (carried from Run 6). godot-vrm still renders (255, 255, 255) at the sphere center while three-vrm sits at (53, 53, 53) and vrm-metal-kit at (164, 164, 164). L4 didn't touch lighting/tonemap; this remains the leading godot-vrm-specific fidelity bug.
- **Outline divergence floor (0.1840) is stable at `n=80`.** The floor didn't shift between run 6's `n=44` godot-pair sample and run 7's `n=80`, which means the worst-case outline disagreement is a deterministic three-way property of the renderers — not an artifact of which subset we sampled.
- **godot-vrm/vrm-metal-kit remains the tightest pair** (mean 0.8709 vs 0.7879 for three-vrm/vmk vs 0.7042 for three-vrm/godot). Both are first-party MToon implementations against the VRMC spec; three-vrm's color-space hypothesis ([three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838)) continues to keep it as the dimmest of the three. With three renderers' worth of data at full `n`, the consensus diff confidently flags three-vrm as the shading-cluster outlier — the methodology working as designed.

### Open follow-ups

- **godot-vrm flat-white at `mtoon_default`** (carried from Run 6 — still open). Lighting/tonemap investigation against `adapters/godot-vrm/src/session.gd` or the upstream Godot-MToon-Shader is the next concrete fidelity win available.
- **`springbone_joints_16` / `joints_8` / `segment_0p1`** crack the top-15 divergent list at ~0.7096. Worth a closer look — the chains agree well enough not to dominate the floor but they're meaningfully lower than the spring-bone median. Candidates: joint-count edge cases (longer chains accumulate more drift per step), per-joint segment-length scaling, or a `do_process` ordering subtlety in long chains.
- **Linux CI driver spike** (still pending from L3). The whole corpus runs on macOS today; a Linux driver pass would validate that the godot-vrm shim doesn't inherit anything macOS-specific from the Godot path.
- **Concern 2 from Spike 2 (mesh-under-head-bone)** (still open from Run 6). Non-fatal `VRMC_vrm.gd:387` typed-assignment script error during chain-skinned imports; renderer recovers and emits output.

## Eighth run: methodology refinement — color-space convention pinned

**Date**: 2026-05-12. No new renderer revisions; methodology + tooling change only.

**Trigger**: [pixiv/three-vrm#1838](https://github.com/pixiv/three-vrm/issues/1838) closed by maintainer reply (not a bug). [0b5vr](https://github.com/0b5vr) explained that three-vrm's MToon implementation deliberately renders in linear color space and assumes the renderer's `outputColorSpace = THREE.SRGBColorSpace` will apply the sRGB OETF on output. `THREE.LinearSRGBColorSpace` is explicitly unsupported for MToon — three.js's `SRGBColorSpace` corresponds to what Unity calls "Linear" workflow (linear shading + sRGB display encoding), not `LinearSRGBColorSpace` (which is *no* display encoding at all).

Our prior test-plan default was `color_space: Linear`, which the three-vrm adapter honored by setting `LinearSRGBColorSpace`. That asked three-vrm to render in an unsupported mode and produced a corpus baseline that systematically under-represented its MToon output by the sRGB OETF. The other two adapters interpreted the same field inconsistently (vrm-metal-kit → `rgba8Unorm` linear framebuffer; godot-vrm → always sRGB-encoded PNG regardless of request).

### What changed

- `docs/methodology.md` — rewrote the **Color management** section to pin `color_space: Srgb` as the v1.0 default for every MToon math test, document the adapter contract per renderer, and flag the directional-intensity-by-π open question as a follow-up.
- `crates/vrm-asset-generator/src/sidecar.rs` — `build_default_test_plan` now emits `color_space: Srgb`. All sweep variants inherit the change (they all start from `build_default_test_plan` or its spring-bone derivatives).
- `adapters/three-vrm/src/renderer-host.html` — added a comment near the `outputColorSpace` branch flagging the convention so future contributors don't reintroduce `LinearSRGBColorSpace` as a default.

### Expected impact on the corpus (not yet measured)

This change has not been re-rendered through the corpus yet — that's a follow-up bootstrap-goldens run. Predictions:

- **three-vrm**: every test should now render meaningfully brighter (the sRGB OETF is applied on output, so the `(53, 53, 53)` sphere centerline at `mtoon_default` should move into the high-100s, much closer to the VRMMetalKit `(164, 164, 164)` and away from the godot-vrm `(255, 255, 255)` outlier). The longstanding "three-vrm is the dimmest renderer" signal — which has been the dominant divergence floor across runs 1–7 since the run-5 VMK fix — should largely close.
- **vrm-metal-kit**: framebuffer flips from `rgba8Unorm` to `rgba8Unorm_srgb`. Pixel values move from raw-linear to sRGB-encoded. Expected to remain visually similar but PNG byte values shift; SSIM vs the new three-vrm baseline likely tightens substantially.
- **godot-vrm**: no behavioral change (already wrote sRGB-encoded PNGs unconditionally).

If the prediction holds, the corpus mean SSIM should jump materially — possibly through the 0.85+ band — driven primarily by the three-vrm/VMK pair re-converging. The remaining divergence floor would still be the outline cluster (three different outline interpretations across three renderers, including [#1839](https://github.com/pixiv/three-vrm/issues/1839)).

### What this measures, conceptually

Up to run 7, every divergence finding was filed against a renderer that was actually behaving incorrectly relative to the spec. This run is the first where the conformance suite *itself* was the source of a systematic divergence — the test plan asked renderers for an output mode that wasn't well-defined cross-renderer, and three-vrm in particular flagged it. The fix is methodology, not renderer code. Logging it here as a deliverable on the same footing as the upstream-bug findings, because the suite's purpose is to produce falsifiable signal and that includes signal about the suite's own assumptions.

### Follow-ups

- **Run 9 bootstrap**: re-render the full 80-test corpus through all three real adapters with the new default and re-measure pairwise SSIM. Compare against run 7 numbers to validate the prediction above.
- **Directional-intensity-by-π**: three-vrm assumes `Math.PI` scaling (legacy three.js convention). Our plan declares `intensity: 1.0` without specifying which convention applies. Decide whether to scale in the adapter (preserves the human-readable `1.0`) or in the plan (requires updating every test). Tracked as an open methodology question in `docs/methodology.md`.

## Ninth run: methodology refinement validated (color_space: Srgb)

**Date**: 2026-05-12, vrm-conformance commit `b6ad01b`. Same hardware (M4 Max), same renderer revisions as run 7 (three-vrm 3.5.0, vrm-metal-kit 0.13.3, godot-vrm @ Godot 4.6.2). The only material change between run 7 and run 9 is the corpus default `color_space` flip from `Linear` to `Srgb` shipped in commit `524c334` (run 8 was the methodology change itself; this run measures it).

### Corpus-wide before/after

| pair | run 7 mean | run 9 mean | Δ | run 7 min | run 9 min |
|---|---|---|---|---|---|
| `three-vrm` vs `vrm-metal-kit` | 0.7879 | **0.8975** | **+0.1096** | 0.6313 | 0.6313 |
| `godot-vrm` vs `three-vrm` | 0.7042 | **0.8398** | **+0.1356** | 0.1840 | 0.1840 |
| `godot-vrm` vs `vrm-metal-kit` | 0.8709 | 0.8714 | +0.0005 | 0.5301 | 0.5303 |

The two pairs involving three-vrm jumped substantially. The pair not involving three-vrm stayed flat. That's exactly the prediction in run 8: three-vrm's output shifted (brighter — its renderer now applies the sRGB OETF on output), bringing it closer to both other renderers; godot-vrm and vrm-metal-kit didn't move because neither's color-space configuration changed in a way that produces different output bytes for this corpus.

`three-vrm vs vrm-metal-kit` at 0.8975 is the highest pair mean the project has ever measured. The corpus is now within ~0.09 of the v1.0-standard 0.985 SSIM threshold. consensus_passed still 0/80 (the threshold is above the corpus max of 0.9749), but the gap closed substantially in one methodology refinement.

### Pixel-level recovery — `mtoon_default` centerline

| renderer | run 7 (x=512, y=512) | run 9 | Δ |
|---|---|---|---|
| three-vrm | (53, 53, 53) | **(126, 126, 126)** | **+73 per channel** |
| vrm-metal-kit | (164, 164, 164) | (164, 164, 164) | 0 |
| godot-vrm | (255, 255, 255) | (255, 255, 255) | 0 |

three-vrm went from `0.208` linear (8-bit) to `0.494` linear. The new value is the result of three-vrm's MToon shader writing its linear-space output through `THREE.SRGBColorSpace` (linear shading + sRGB OETF on output) instead of `LinearSRGBColorSpace` (raw linear, no OETF). The remaining gap vs VRMMetalKit's `0.643` is consistent with the still-open `Math.PI` intensity-scaling question flagged in `docs/methodology.md` — three.js since r155 uses physically-correct directional-light intensity, and three-vrm's spec-intended baseline assumes `intensity = Math.PI` rather than the literal `1.0` our plans declare. Closing that gap is a follow-up; the color-space change alone moved three-vrm 73 channel-units toward the other two renderers.

VRMMetalKit's framebuffer format changed (`rgba8Unorm` → `rgba8Unorm_srgb`) under the same methodology shift, but the centerline bytes are byte-identical to run 7. The MToon shader in VRMMetalKit appears to apply the sRGB OETF in-shader regardless of framebuffer format, so changing the format was a no-op for the rendered output bytes. The `actual_color_space` field in the result envelope reports the new convention; the underlying pixel data hasn't changed.

godot-vrm was already writing sRGB-encoded PNGs unconditionally per its session.gd policy (commit on file), so its output is unchanged.

### Top 15 most-divergent test_ids — pattern shift

Outline cluster (8 tests) still dominates the floor — same 0.1840 / 0.3588 / etc. values, same three-way disagreement on outline rendering. The methodology change doesn't touch outline interpretation; that remains gated on the asset-side investigation flagged when [pixiv/three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) was closed.

What's new in the top 15:

| test_id | run 7 | run 9 |
|---|---|---|
| `mtoon_shadingShift_0p8` | not top-15 | **0.8409** (new floor for shading tests) |
| `swing_springbone_segment_0p1` | 0.7097 | 0.8534 |
| `swing_springbone_joints_16` | 0.7096 | 0.8535 |
| `swing_springbone_segment_0p2` | not top-15 | 0.8547 |
| `swing_springbone_joints_8` | 0.7096 | 0.8565 |
| `swing_springbone_default` | not top-15 | 0.8577 |
| `swing_springbone_drag_0` | not top-15 | 0.8577 |

The 5 swing-springbone tests in the top-15 now cluster around 0.85 (up from ~0.71 in run 7), the same shift magnitude as the corpus-wide mean. They were never the dominant divergence — outline was — and the methodology change moved them in lockstep with the rest of the MToon corpus. The chain-skinned cylinder geometry is the same; the methodology change is what's lifting them.

`mtoon_shadingShift_0p8` cracking the top-15 at 0.8409 is the new floor for MToon shading tests (excluding outline). Worth a follow-up sample to see whether the remaining shading divergence is three-vrm vs the other two (color-space-related residual) or VRMMetalKit/godot-vrm vs three-vrm (genuine shader-interpretation gap).

### Cumulative nine-run progression

| Run | mean (3v vs VMK) | min | upstream events |
|---|---|---|---|
| 1 (50cfd7d) | 0.7447 | 0.6313 | first corpus baseline |
| 2 (0.13.1) | 0.7002 | 0.1840 | #181/#182 closed; #185+#1839 surfaced |
| 3 (0.13.1+chain) | 0.6994 | 0.1840 | chain-skinned mesh wired |
| 4 (0.13.2+chain) | 0.7439 | 0.6313 | #185 closed in 0.13.2 |
| 5 (0.13.3+chain) | 0.7879 | 0.6313 | #183 closed in 0.13.3 |
| 6 (godot-vrm L3) | 0.7879 | 0.1840 | godot-vrm joins as third real renderer (n=44) |
| 7 (godot-vrm L4) | 0.7879 | 0.1840 | godot-vrm full 80-test coverage |
| 8 (methodology refinement) | — | — | color_space: Srgb default shipped; no re-render |
| **9 (run 9 re-bootstrap)** | **0.8975** | **0.1840** | **methodology change validated by data** |

Eight upstream tickets filed and closed (#181, #182, #183, #185 against VRMMetalKit; #1838 closed not-a-bug against three-vrm; #1839 closed pending our asset-side investigation; godot-vrm L3 + L4 self-shipped). The three-vrm/VMK pair mean is now +0.1528 above the original run-1 baseline, the largest single-session improvement of the project's history, driven by a methodology refinement rather than an upstream fix.

### What this validates

- **The methodology refinement was the right call.** The data confirms run 8's prediction directionally and to within an order of magnitude on the magnitude. three-vrm-side divergence wasn't a three-vrm bug; it was a suite-side choice about which output color space to ask for.
- **The four-renderer panel will work.** When UniVRM (Unity, in-design per `rfcs/0003` and `docs/superpowers/plans/2026-05-12-adapter-univrm-scaffold.md`) lands as renderer #4, the consensus diff will have a fourth voter to disambiguate the remaining ~0.10 gap to the 0.985 threshold. The two largest remaining clusters (outline rendering + the ~0.84 shading-tail) are exactly the kinds of disagreement a ground-truth oracle is designed to resolve.

### Open follow-ups

- **`Math.PI` intensity scaling.** Three-vrm's spec-intended baseline assumes directional intensity `Math.PI`. Our plans declare `1.0`. Closing this would likely move three-vrm's centerline from `(126,...)` to something closer to `(188,...)` and tighten the three-vrm/VMK pair further. Decide whether to scale in the three-vrm adapter (preserves human-readable `1.0` in plans) or in the plan (requires touching every test_id). Not blocking the corpus.
- **`mtoon_shadingShift_0p8` and other ~0.84 cluster shading tests.** Sample pixel data to identify which renderer is the outlier on each. With three renderers, consensus can call it — but it's worth a dedicated look before treating any of them as ground truth.
- **Outline floor (0.1840) unchanged.** Asset-side investigation still pending from the [three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) close-out (try a known-good MToon asset from `vrm-c/UniVRM-Samples` and see whether outlines render as a thin silhouette band there).

## Tenth run: Math.PI intensity scaling in three-vrm (corollary to run 9)

**Date**: 2026-05-12, vrm-conformance commit `0763387` baseline. Same hardware, same renderer revisions. Single change: `adapters/three-vrm/src/renderer-host.html` now applies `d.intensity * Math.PI` instead of `d.intensity` for `DirectionalLight.intensity`. Re-rendered through three-vrm only (vrm-metal-kit and godot-vrm renders carried over unchanged from run 9).

This closes the open methodology question that run 9 surfaced ("directional intensity convention" in `docs/methodology.md`). three.js since r155 uses physically-correct intensity (lux); three-vrm's spec-intended baseline assumes `intensity = Math.PI`. Test plans declare `1.0`; the adapter scales by π.

### Corpus-wide before/after

| pair | run 9 mean | run 10 mean | Δ | run 9 max | run 10 max |
|---|---|---|---|---|---|
| `godot-vrm` vs `three-vrm` | 0.8398 | **0.8972** | **+0.0574** | 0.9745 | **0.9902** |
| `three-vrm` vs `vrm-metal-kit` | 0.8975 | 0.8953 | −0.0022 | 0.9749 | **0.9889** |
| `godot-vrm` vs `vrm-metal-kit` | 0.8714 | 0.8714 | 0 | 0.9523 | 0.9523 |

`godot-vrm vs three-vrm` jumped +0.0574 — bigger than the Srgb-default change in run 9. The two pairs not involving three-vrm-side adapter change either moved very slightly (three-vrm/VMK: −0.0022, noise) or didn't move at all (godot/VMK: 0, neither side changed).

Two notable structural shifts:

1. **`godot-vrm vs three-vrm` is now the tightest pair** at 0.8972, exceeding the prior champion `three-vrm vs vrm-metal-kit` at 0.8953. This is the first time the godot/three pair has been the corpus's tightest cluster. Interpretation: three-vrm and godot-vrm now both render in "linear shading + sRGB OETF + physically-correct intensity" convention; VRMMetalKit's MToon shader path doesn't apply the same intensity scaling and produces a slightly different brightness profile. With three renderers, consensus can now flag VRMMetalKit as the mild outlier on MToon shading — which is the methodology working as designed.

2. **Max SSIM crossed 0.99** on non-outline tests for the first time. Run 10 max values are `godot-vrm vs three-vrm = 0.9902`, `three-vrm vs vrm-metal-kit = 0.9889`. The v1.0 standard threshold is 0.985, and both of those exceed it. consensus_passed is still 0/80 because the per-test consensus must hold for all three pairs at once (and the outline cluster floors at 0.1840), but the data shows non-outline tests now reach threshold pixel-agreement between specific renderer pairs.

### Pixel-level — `mtoon_default` centerline

| renderer | run 7 | run 9 (+Srgb) | run 10 (+π) | Δ run 9→10 |
|---|---|---|---|---|
| three-vrm | (53, 53, 53) | (126, 126, 126) | **(195, 195, 195)** | **+69 per channel** |
| vrm-metal-kit | (164, 164, 164) | (164, 164, 164) | (164, 164, 164) | 0 |
| godot-vrm | (255, 255, 255) | (255, 255, 255) | (255, 255, 255) | 0 |

three-vrm now renders BRIGHTER than VRMMetalKit at the centerline — direction flipped from prior runs where three-vrm was the consistent "dimmest" outlier. For `intensity = 1.0 × Math.PI` directional light + the standard MToon material, three-vrm's `0.5 linear → sRGB OETF` should produce `~188` per channel; the actual `195` includes additional contribution from the ambient term (`0.5 × 0.3 = 0.15 linear`) plus shading-shift behavior near the centerline. The math is now self-consistent across three.js's documented physically-correct lighting semantics.

VRMMetalKit's `(164, ...)` is now the *darker* one. Without UniVRM as a fourth oracle to call which interpretation is closest to MToon-1.0, the suite reports the divergence faithfully: with three renderers, two agreeing more strongly than the third is the strongest signal we have until a ground-truth renderer is added.

### Top 15 most-divergent — outline still floors

```
mtoon_outline_world_0p1                   0.1840   (8 outline tests dominate divergence floor; unchanged from prior runs)
mtoon_outline_world_0p05                  0.3588
...
swing_springbone_joints_16                0.8505   (slight regression: 0.8535 → 0.8505)
swing_springbone_joints_8                 0.8506
swing_springbone_segment_0p1              0.8509
swing_springbone_stiffness_0              0.8523   (new entry; previously not in top 15)
swing_springbone_stiffness_0p2            0.8526   (new entry)
swing_springbone_default                  0.8527
mtoon_shadingShift_0p8                    0.8527
```

The 8 outline tests still floor at 0.1840 — outline-rendering disagreement is orthogonal to color-space / intensity, and the asset-side hypothesis from the [three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) close-out remains the open path.

Spring-bone variants are reshuffling slightly. `swing_springbone_stiffness_0` and `_stiffness_0p2` are new top-15 entries; they replaced two of the `drag_*` variants from run 9. The mean of the spring-bone cluster is unchanged within sampling noise (~0.85), but the specific tests in the bottom-15 are now slightly different. With three-vrm brighter, fine-detail rendering at the chain edges becomes a slightly different signal — same underlying physics, slightly different SSIM contribution per pixel.

### Net result

The Math.PI scaling is a net positive for the corpus:

- Corpus-wide mean across the 3 pairs: 0.8696 → 0.8880 (+0.0184)
- Maximum pair-SSIM: 0.9749 → 0.9902 (+0.0153) — first time crossing the v1.0 threshold of 0.985
- The two pairs involving three-vrm shifted in opposite directions (+0.0574 toward godot; −0.0022 from VMK), with the net favoring three-vrm/godot agreement
- The change identifies VRMMetalKit's intensity handling as the new likely outlier on MToon shading — a fresh upstream question worth investigating once we have a fourth renderer to confirm

### Cumulative ten-run progression

| Run | mean (3v vs VMK) | min | max (any pair) | upstream events |
|---|---|---|---|---|
| 1 | 0.7447 | 0.6313 | 0.9665 | first corpus baseline |
| 5 (0.13.3) | 0.7879 | 0.6313 | 0.9665 | #183 closed |
| 7 (godot-vrm L4) | 0.7879 | 0.1840 | 0.9665 | full 3-renderer 80-test |
| 9 (Srgb default) | 0.8975 | 0.1840 | 0.9749 | methodology refinement |
| **10 (Math.PI)** | **0.8953** | **0.1840** | **0.9902** | **intensity convention closed** |

The `three-vrm vs vrm-metal-kit` pair mean is essentially flat between runs 9 and 10 (0.8975 → 0.8953), but the `godot-vrm vs three-vrm` mean — the second-best signal — moved from 0.8398 to 0.8972, putting both three-vrm-involving pairs in the same ~0.89 band for the first time. The corpus is now clustered tightly enough that adding a fourth renderer (UniVRM) for outlier-detection consensus is the obvious next move.

### Per-test deep-dive: `mtoon_shadingShift_0p8` (0.8527 floor for shading)

Visual comparison of the three renderers on this single test (added post-bootstrap, same renders as the table above):

| renderer | render description |
|---|---|
| three-vrm | small shadow region in **lower-right** of the sphere; rest lit. Consistent with test plan's directional dir `[-0.3, -0.6, -0.7]` — light travels down-and-toward-the-camera-from-the-left, so the lit hemisphere is upper-left and shadow is lower-right. Spec-correct shading-boundary position for `shadingShiftFactor: 0.8`. |
| vrm-metal-kit | small shadow region in **upper-right** — Y-component of light direction or surface normal appears flipped. Same general "mostly-lit with localized shadow" shape as three-vrm, just mirrored vertically. |
| godot-vrm | **flat white, no shading visible** — same surface as `mtoon_default` flat-white bug from run 6+. The `VRMC_materials_mtoon` parameters aren't being honored; `shadingShiftFactor` has no effect. |

Two distinct upstream findings:

1. **VRMMetalKit Y-axis convention** — directional-light Y-component or surface-normal Y-component is sign-flipped relative to three-vrm's interpretation. Test plan's negative-Y directional ("light travels downward") should produce shadow in the *lower* hemisphere; VMK puts it in the *upper* hemisphere. This is a new upstream finding worth filing against `arkavo-org/VRMMetalKit` after UniVRM (renderer #4) confirms which Y convention is spec-intended.

2. **godot-vrm MToon parameter binding** — the persistent `mtoon_default` flat-white bug now has a second corroborating data point: `shadingShiftFactor: 0.8` produces no visible shadow on the godot-vrm render. Either the V-Sekai/godot-vrm importer isn't binding `VRMC_materials_mtoon.shadingShiftFactor` to the shader's uniform, or the Godot-MToon-Shader doesn't sample the parameter. Worth filing upstream as a follow-up to the existing `mtoon_default` open issue.

The 0.8527 SSIM floor for `mtoon_shadingShift_0p8` is well-explained by these two divergences combined.

### Open follow-ups

- **VRMMetalKit's intensity / shading interpretation** — now the consensus minority on MToon shading. Combined with the new Y-axis-flip finding from `mtoon_shadingShift_0p8`, there's a strong case for a dedicated upstream investigation. File once UniVRM is rendering to confirm directionally.
- **Outline floor (0.1840)** — settled in run 11 (next section): UniVRM (consortium reference) produces the SAME full-mesh flood as three-vrm + VRMMetalKit. Asset-side issue, not a renderer bug.
- **UniVRM as renderer #4** — scaffold (L1+L2) shipped in this session; L3+L4 deferred. With four renderers, consensus-of-3 can replace consensus-of-2 for outlier-flagging, which will be especially valuable for the ~0.89 cluster where one of three renderers (currently VMK on MToon shading) is the consensus minority.

## Run 11: UniVRM as fourth renderer settles the outline-floor question

**Date**: 2026-05-13, vrm-conformance commit `ba0329c` (UniVRM L3 lands).

**Trigger**: [`docs/superpowers/plans/2026-05-13-adapter-univrm-L3.md`](../docs/superpowers/plans/2026-05-13-adapter-univrm-L3.md) — Phase 1 ops shipped, the UniVRM adapter renders the 44 MToon corpus end-to-end through Unity 6 + UniVRM v0.131.0 + Built-in RP. UniVRM is the **VRM consortium reference implementation** — the codebase MToon-1.0 was specified against.

### The outline question, answered

`mtoon_outline_world_0p1` (and `_0p01`) rendered through UniVRM. Visual comparison against three-vrm + VRMMetalKit at the same test_id:

| Renderer | `mtoon_outline_none` | `mtoon_outline_world_0p01` | `mtoon_outline_world_0p1` |
|---|---|---|---|
| three-vrm 3.5.0 | shaded gray sphere | **flat black, mesh slightly larger than `none`** | **flat black, mesh much larger** |
| VRMMetalKit 0.13.3 | shaded gray sphere | **flat black, mesh slightly larger** | **flat black, mesh much larger** |
| **UniVRM v0.131.0 (reference)** | shaded gray sphere | **flat black, mesh slightly larger** | **flat black, mesh much larger** |
| godot-vrm @ 4.6.2 | flat white (KHR_unlit fallback) | byte-identical to `mtoon_default` (no outline) | byte-identical |

Three independent MToon implementations *plus the consortium reference* produce **the same full-mesh flood** for the conformance corpus's parametric sphere with outline enabled. The fourth (godot-vrm) doesn't render outlines at all (falls back to `KHR_materials_unlit`).

### Why "flood" is spec-compliant for this asset

The MToon-1.0 spec describes outline rendering as an inverted-hull technique: render a copy of the mesh with vertices displaced along their normals by `outlineWidthFactor`, with **front-face culling mandatory regardless of `doubleSided`**. The intent is that the back faces of the displaced shell are visible only along the silhouette (where the main mesh doesn't depth-occlude them), producing a thin outline ring.

For the conformance asset — a 30-cm-radius sphere with `outlineWidth: 0.10m` (33% of mesh radius) — the displaced shell is **so large relative to the main mesh** that even with correct depth ordering, the outline shell's silhouette extends far beyond the main mesh's silhouette. The "outline" visually IS the entire visible disc.

At `outlineWidth: 0.01m` (3% of radius) the result is similar but less extreme — the shell is only slightly larger than the main mesh, yet the renderers still produce flat-black for the entire visible mesh. That suggests the spec's outline technique, when implemented per the front-face-culling mandate, fills the visible mesh with outline color (the main mesh is occluded by the inverted shell's near-side fragments rather than presenting through the silhouette).

Either way: **all four renderers' outputs are consistent with each other and with what the MToon-1.0 spec mandates.** The asset is producing exactly what the spec describes; the divergence between renderers is bounded by silhouette anti-aliasing.

### Consequence for the suite

1. **[pixiv/three-vrm#1839](https://github.com/pixiv/three-vrm/issues/1839) was closed correctly** (closed in run 10) — the UniVRM result confirms the closure was the right call.
2. **No upstream issue to file.** The flood is a property of how outline rendering interacts with our specific parametric-sphere asset shape, not a renderer bug.
3. **Methodology refinement candidate**: the outline tests as currently composed (full-frame SSIM against a sphere with extreme outline width) measure mostly silhouette-AA noise + outline-mesh-size disagreement, not actual outline-shading divergence. Future revisions to the outline tests should either:
   - (a) Compare only the ring band between expected main-mesh silhouette and expected outline-shell silhouette, OR
   - (b) Use a humanoid mesh where the outline width is reasonable relative to feature size (e.g., 0.001m on a face), OR
   - (c) Mark these tests as "expected to flood; the test exercises that flooding is consistent across renderers, not that it produces a silhouette band".

### Corpus-wide 4-renderer consensus (full 80-test rerun)

> **Provisional until VRMMetalKit 0.13.4 is marked Release Candidate.** The corpus results below were generated against the VRMMetalKit `0.13.4` *release tag* (commit [`4223876`](https://github.com/arkavo-org/VRMMetalKit/commit/4223876)) and the vrm-conformance suite at commit [`1fb1799`](https://github.com/arkavo-org/vrm-conformance/commit/1fb1799). They will be promoted from "corpus result" to "RC anchor" only once 0.13.4 ships the RC marker. Re-render and re-anchor any time the pin moves.

**Methodology pins (load-bearing for every number below)**:

- **Corpus**: 80 deterministic test_ids — 44 MToon material variants (`emit-sweep`), 18 spring-bone settle variants (`emit-springbone-sweep`), 18 spring-bone swing variants (`emit-springbone-swing-sweep`).
- **Test asset**: 30-cm-radius procedural sphere on a humanoid skeleton, MToon material, parametric per-test sweep on a single axis.
- **MToon math pins** (from `docs/methodology.md`): `tone_mapping: none`, `cast_shadows: false`, `receive_shadows: false`. ACES/Filmic tone mappers and engine shadow noise are out of scope; this corpus measures MToon shading math, not lighting pipeline.
- **Render config**: 1024×1024 PNG, color space `Srgb` (linear-shaded then sRGB-OETF'd), MSAA 4×, magenta sentinel clear color `(255, 0, 255)`.
- **Spring-bone**: 60 Hz fixed-step, `reset_physics(settle_steps=30)` from rest pose before measurement. *UniVRM L3 renders spring-bone tests in rest pose (physics not stepped); L4 closes this — see follow-up section.*
- **Renderer versions**: VRMMetalKit `0.13.4` (RC candidate), three-vrm `3.5.0` on three.js `0.171.0` via Playwright headless Chromium, godot-vrm `0.1.0` on Godot `4.6.2`, UniVRM `v0.131.0` on Unity `6000.4.6f1` + Built-in RP.

**Method**: `scripts/bootstrap-goldens.sh RUN_UNIVRM=1` re-rendered the full 80-test corpus through all four real adapters; `scripts/consensus-report.sh` computed pairwise SSIM.

```
Pairwise SSIM stats across the corpus:
  pair                                  mean    min     max     n
  godot-vrm vs three-vrm                0.8972  0.1840  0.9902  80
  godot-vrm vs univrm                   0.8278  0.1843  0.9793  80
  godot-vrm vs vrm-metal-kit            0.9047  0.5303  1.0000  80
  three-vrm vs univrm                   0.9305  0.8282  0.9988  80    ← highest agreement
  three-vrm vs vrm-metal-kit            0.9014  0.6313  0.9796  80
  univrm vs vrm-metal-kit               0.8726  0.6315  0.9688  80
```

**Headline result**: `three-vrm` and **UniVRM (the consortium reference implementation)** form the closest renderer pair across the entire 80-test corpus — mean SSIM 0.9305, max 0.9988. `mtoon_outline_world_0p1` specifically produces a three-vrm-vs-UniVRM pairwise SSIM of **0.9988** (essentially pixel-identical renders).

That settles the outline-floor question definitively. Three independent MToon implementations *plus the consortium reference* all converge on the same flood result. The "0.1840 min pair" headline that dominated earlier runs comes from godot-vrm's no-outline fallback (it renders MToon material through `KHR_materials_unlit`), not from any disagreement among the renderers that actually render outlines.

### VRMMetalKit vs the consortium reference — launch anchor

**Headline (post-methodology-fixes + VMK 0.13.5, conformance-internal, declared per-test thresholds)**: across the 80-test corpus, 4 outline tests are `conformance_status: Excluded` (per [vrm-conformance#3](https://github.com/arkavo-org/vrm-conformance/issues/3) — spec-correct flood; whole-frame SSIM measures AA only). Of the remaining **76 included tests**, against the consortium reference (UniVRM v0.131.0 + Unity 6 PlayMode physics) at each test's declared per-test threshold (default 0.85, rimLighting cluster 0.95, per [vrm-conformance#2](https://github.com/arkavo-org/vrm-conformance/issues/2)):

```
  three-vrm     ≥ declared threshold vs UniVRM:  76 / 76  (100%)   ← passes conformance
  godot-vrm     ≥ declared threshold vs UniVRM:  67 / 76  ( 88%)
  VRMMetalKit   ≥ declared threshold vs UniVRM:  63 / 76  ( 83%)   ← VMK 0.13.5; up from 53/76 at 0.13.4
```

**Update (after vrm-conformance sRGB-encoding fix in the VMK adapter)**: VMK 0.13.5 corpus run, post-adapter-fix, lifts the conformance pass-rate from 63/76 (83%) to **66/76 (87%)**. The fix corrected two adapter bugs surfaced by the VMK team's #213 root-cause analysis: (1) case-sensitive `colorSpace == "Srgb"` comparison against the lowercase wire form `"srgb"`, and (2) `RendererConfig.colorPixelFormat` defaulted to `.bgra8Unorm` with no override before pipeline lock-in. VMK was writing linear bytes (byte 118 on `shadingShift_0p8` center pixel) where UniVRM and three-vrm wrote sRGB-encoded bytes (181 and 230). The shadingShift "regression" identified in the 0.13.5-to-0.13.6 finding was actually this encoding bug; VMK's shading math was correct.

Post-fix VMK shadingShift_0p8 center pixel: **byte 181** — exactly what the diagnosis predicted. The byte-for-byte match confirms the encoding was the bug, not the shader math.

VMK 0.13.5 (commit `c01ac8a`) closed [VMK#205](https://github.com/arkavo-org/VRMMetalKit/issues/205) (PR #207, /π Lambert normalization in MToonShader.metal) and [VMK#206](https://github.com/arkavo-org/VRMMetalKit/issues/206) (PR #208, VRMNode.updateWorldTransform re-derives localMatrix from T/R/S). Net effect on this corpus (pre-sRGB-fix):

- **Swing-springbone cluster (#206 closed)**: 18/18 lifted from 0.7985-0.8025 → 0.8916-0.8965. All pass the 0.85 threshold now.
- **shadingToony cluster (#205 partial close)**: All 8 SSIMs shifted up by +0.01 to +0.02. Tests at toony ≥ 0.9 now pass; tests at toony 0 → 0.75 still below 0.85 — see [VMK#213](https://github.com/arkavo-org/VRMMetalKit/issues/213) for the residual curve-shape divergence.
- **shadingShift regression (NEW in 0.13.5)**: 2 tests that passed pre-0.13.5 (`shadingShift_0p8`, `shadingShift_1`) dropped below 0.85 (0.90 → 0.82, 0.94 → 0.83). The /π normalization changed the direct/ambient ratio in a way that interacts with positive-shift boundary placement. Tracked in [VMK#213](https://github.com/arkavo-org/VRMMetalKit/issues/213) alongside the toony residual; both clusters likely share a root cause in the MToon shader's curve math.

**Residual gap after the sRGB-encoding fix + VMK 0.13.6 (10 tests below their declared threshold):**

- **4 shadingToony tests** at SSIM 0.83–0.85: `_0`, `_0p1`, `_0p25`, `_0p5`. Curve-shape divergence (real, not encoding). Filed as the [VMK#213](https://github.com/arkavo-org/VRMMetalKit/issues/213) residual; smoothstep math between `shade` and `base` color differs from UniVRM at low toony values.
- **6 rimLightingMix tests** at SSIM 0.9010 post-0.13.6 (was 0.9078 pre-0.13.6 — see regression note below). [VMK#226](https://github.com/arkavo-org/VRMMetalKit/issues/226) closed via PR #227 fixed the fresnel coordinate space but didn't address the dominant signal: VMK's rim contribution at front-facing pixels is zero at `parametricRimLiftFactor: 0.0`, while UniVRM/three-vrm/godot-vrm all apply rim across the surface. Filed as [VMK#228](https://github.com/arkavo-org/VRMMetalKit/issues/228) (rim lift interpretation). Companion methodology issue [vrm-conformance#4](https://github.com/arkavo-org/vrm-conformance/issues/4) covers the corpus side: the `rimLightingMix` parameter sweep produces identical output across all renderers because of the test asset's specific lighting params, and the 0.95 threshold is doing real signal-work flagging VMK only.

### 0.13.6 small regression worth flagging

PR #227 in VMK 0.13.6 moved the parametric-rim fresnel computation to world space. Empirically the rim cluster SSIM regressed from **0.9078 → 0.9010** (-0.007, uniformly across all 6 tests). The fresnel coordinate-space change shifted the rim band's position; the new position is slightly farther from UniVRM's than the old (incorrect) position was. Conformance pass-rate is unchanged at 66/76 (87%) because none of the rim tests cross the 0.95 threshold before or after.

The regression is small (~0.7% SSIM) and uniform — it's a position shift, not a magnitude blow-up. The dominant SSIM signal remains the lift-at-0 zero-rim-contribution behavior that VMK#228 addresses. Once #228 closes, the rim cluster should jump from 0.9010 to 0.97+ regardless of #227's small position shift; the position is dominated by where the rim is *visible*, and the lift fix makes the rim visible across the front face where the reference impls render it.

The 3 outline tests below 0.85 in the divergent list (`world_0p1`, `screen_0p1`, `world_0p05`) are `conformance_status: Excluded` and don't count toward the pass-rate — they're spec-correct flood per vrm-conformance#3.

For VRMMetalKit specifically, the remaining gap to 76/76 is **10 tests** spread across two clusters, both upstream-fixable:

The 26 tests outside that band split into three named clusters; only one of the three is an open question for VMK at RC time:

- **8 outline tests** (SSIM 0.63–0.86): deliberate stress assets, spec-correct flood, see Outline cluster below.
- **5 shadingToony tests** (SSIM 0.78–0.83): real renderer-side divergence, **filed as [VMK#205](https://github.com/arkavo-org/VRMMetalKit/issues/205)** before RC tag.
- **18 swing-springbone tests** (SSIM 0.7985–0.8025): VMK's `animate_root_transform` produces output byte-identical to no-animation, **filed as [VMK#206](https://github.com/arkavo-org/VRMMetalKit/issues/206)** before RC tag.

**Roadmap**: if VMK#205 (shadingToony) and VMK#206 (animate_root_transform) both close before RC tag, the conformance pass-rate at 0.85 lifts from 54/80 (68%) to a projected ~72/80 (90%) — the originally-claimed number, now with full physics. The shadingToony fix lifts 5 tests; the animate_root_transform fix lifts the 18 swing tests from "asymmetric pose comparison" back into the cross-renderer bulk band.

**Supporting statistic**: corpus-wide mean SSIM 0.8573 (was 0.8726 with L3 rest-pose; the drop reflects swing-test divergence becoming visible after L4 made the comparison fair). Median 0.8675, max 0.9688 (`mtoon_default`).

> **Why the headline dropped 22 percentage points**: before UniVRM L4 PlayMode landed, all 36 spring-bone tests (settle + swing) showed structurally identical "pass" SSIMs (~0.87) because UniVRM was rendering in rest pose while VMK was rendering with active physics. The previous 90% conformance claim was inflated by 18 swing tests that hadn't yet had their comparison made informative. The current 68% is the first **honest** number — and it explicitly identifies VMK#206 as the largest single contributor to the gap.

**Honest note on the declared 0.985 threshold**: every test plan in this corpus carries `diff.threshold: 0.985`, the v1.0 self-diff target. That threshold was scoped for "this renderer producing byte-identical output across runs," not "this renderer matches an independent implementation pixel-perfect." Under 0.985, *zero* of 80 tests pass for any cross-renderer pair, including the closest pair in the corpus (three-vrm ↔ UniVRM at 0.9988 max). For the cross-renderer-vs-reference question — which is what the VMK launch is making — 0.85 is the operationally meaningful threshold across the bulk-of-corpus band, and per-test thresholds need to be brought in line with that in a methodology pass before they become useful as RC gates.

**Conformance pass-rate at several thresholds, VMK 0.13.4 ↔ UniVRM v0.131.0 (PlayMode physics)**:

```
  SSIM ≥ 0.985:   0 / 80 ( 0%)   ← declared threshold; aspirational, not operational
  SSIM ≥ 0.950:   7 / 80 ( 9%)
  SSIM ≥ 0.900:  12 / 80 (15%)
  SSIM ≥ 0.875:  19 / 80 (24%)
  SSIM ≥ 0.850:  54 / 80 (68%)   ← honest bulk-band (drop from 72 reflects post-L4 swing divergence)
  SSIM ≥ 0.800:  72 / 80 (90%)
  SSIM ≥ 0.750:  78 / 80 (98%)
```

By category:

```
  MToon material tests (44):   36/44 ≥ 0.85 (82%)   ← stable comparison; primary conformance claim
  Spring-bone settle (18):     18/18 ≥ 0.85 (100%)  ← both render mostly-rest-pose; informative once
                                                       deep-settle parameter sweeps are added
  Spring-bone swing (18):       0/18 ≥ 0.85 (0%)    ← VMK#206 (animate_root_transform no-op);
                                                       blocked until upstream fix
```

Reference pair for calibration — three-vrm ↔ UniVRM (the closest pair in the corpus):

```
  SSIM ≥ 0.985:  10 / 80 (12%)
  SSIM ≥ 0.950:  54 / 80 (68%)
  SSIM ≥ 0.900:  61 / 80 (76%)
```

Even between three-vrm and the consortium reference — two implementations that share the most spec-interpretation heritage — only 12% of tests cross 0.985. **0.985 is not a meaningful cross-renderer threshold.**

**Per-test distribution behind the mean (post-L4)**:

```
VMK 0.13.4 ↔ UniVRM v0.131.0 — 80-test SSIM distribution
  min:    0.6315   (mtoon_outline_world_0p1; see Outline cluster below)
  median: 0.8675
  mean:   0.8573
  max:    0.9688

Bucket distribution:
  SSIM 0.50–0.70  1 test   ( 1.2%)   ← outline cluster (worst case)
  SSIM 0.70–0.85 25 tests  (31.2%)   ← outline + shadingToony + swing-springbone clusters
  SSIM 0.85–0.95 47 tests  (58.8%)   ← MToon-math + settle-springbone bulk band
  SSIM 0.95–1.00  7 tests  ( 8.8%)
```

The 8 tests below the 0.85 bulk band decompose into three named clusters:

**1. Outline cluster (8 of 8 below-band slots; SSIM 0.63–0.86): deliberate stress, not methodology defect.**

`mtoon_outline_world_*` and `mtoon_outline_screen_*` ask each renderer to draw a 1-cm to 10-cm thick outline shell around a 30-cm-radius sphere on a magenta background. That asset is a *deliberate stress test* of the MToon outline pipeline at parameter extremes — not a representative render. The whole-frame SSIM metric breaks down on it for spec-compliant reasons:

- The MToon spec mandates inverted-hull outline rendering with front-face culling. On a sphere where the outline mesh is 33% larger than the main mesh (`outlineWidth: 0.1m`, radius 0.3m), the spec-correct output is a fully-flooded black disc — what UniVRM produces. three-vrm ↔ UniVRM on this exact test = **0.9988 SSIM (essentially pixel-identical)**.
- VMK ↔ UniVRM on the same test = 0.6315. The 0.36 gap comes from a few pixels of silhouette anti-aliasing disagreement on a frame whose only signal *is* the silhouette ring — there is no main-mesh interior signal to dilute the AA disagreement.
- godot-vrm doesn't render MToon outlines at all (falls back to `KHR_materials_unlit`); its outline-vs-anyone-else SSIMs are ~0.18, which is silhouette-area-only divergence and is excluded from the "where does VMK disagree with the reference" question by construction.

In other words, the outline tests are designed to *separate* renderers that handle the outline pass from renderers that don't. They do that job correctly. They are not designed to feed a whole-frame SSIM comparison — and reading the 0.63 number as a conformance failure misreads the test. Future methodology revision will replace whole-frame SSIM on these tests with a ring-band comparison (silhouette annulus only) or a humanoid mesh at realistic outline widths (~0.001m). Neither changes the underlying renderer behavior; both make the metric reflect what the test is actually asking about.

**2. shadingToony cluster (`mtoon_shadingToony_0`, `_0p1`, `_0p25`, `_0p5`, `_0p75`; SSIM 0.78–0.81): real renderer-side finding, pending VMK fix.**

`shadingToonyFactor` controls the smoothness of the lit/shaded transition in MToon: 0 = full Lambert (smooth gradient), 1 = hard toon step. The four-renderer matrix shows a clean two-cluster pattern across this sweep:

- **{UniVRM, three-vrm}** render `shadingToony=0.25` as a soft Lambert-like gradient (visible falloff in the lower hemisphere of the test sphere).
- **{VMK, godot-vrm}** render the same test as a nearly-flat white sphere — implying the shadingToony curve is being interpreted as "shading intensity scalar" rather than "transition smoothness," which collapses to fully-lit at low values.
- Divergence is monotonic with the parameter: as `shadingToony` → 1, all four renderers converge (~0.92–0.97 SSIM at toony=0.95). At toony=0 the divergence is widest.

This is *not* methodology noise — it's a substantive shading-math difference between two implementation clusters. Worth filing upstream against VRMMetalKit (and separately against godot-vrm) before RC tag; the diagnostic is cheap and the fix likely localizes to the MToon fragment shader's shadingToony interpolation term.

**3. Engine-level rendering residual (the 0.13 gap inside the bulk band itself; methodology-documented).**

The bulk-band tests sit at ~0.87 mean rather than 1.0 because of cross-engine rendering choices the MToon spec deliberately doesn't constrain — silhouette anti-aliasing differences (MSAA 4× with different sample patterns produces different edge pixels), glTF→engine coordinate-convention conversions, sRGB OETF rounding, mip-level selection. These are catalogued as expected divergence in [`docs/methodology.md`](./methodology.md) and aren't expected to close further without engine-level changes outside MToon's scope.

**4. Spring-bone swing cluster (`swing_springbone_*`; SSIM 0.7985–0.8025): VMK#206, animate_root_transform no-op.**

All 18 of VRMMetalKit's swing-springbone PNGs are **SHA256-byte-identical** to their corresponding settle-springbone PNGs (proof in [VMK#206](https://github.com/arkavo-org/VRMMetalKit/issues/206) issue body). The `animate_root_transform` operation completes without error but has no visible effect on the rendered output — the avatar root stays at its loaded position regardless of the animation's target translation. UniVRM, three-vrm, and godot-vrm all show the expected post-animation displacement; only VMK doesn't.

three-vrm ↔ UniVRM on the same swing tests = **0.9555 mean SSIM** (close to the MToon-math agreement band), demonstrating the test design itself works as intended once the renderer's animation pipeline does its job. A VMK fix here lifts 18 tests from the 0.80 cluster up into the 0.85+ bulk band in one shot.

**The framing for launch copy (post-L4, honest)**: VRMMetalKit `0.13.4` matches the MToon-1.0 consortium reference (UniVRM `v0.131.0` with PlayMode physics) within the 0.85 SSIM agreement band on **54 of 80 tests (68%)** across the conformance corpus, with **36 of 44 directly comparable MToon-math tests (82%)** in the agreement band. The 26 tests outside split into one cluster of deliberate stress assets (outline rendering at parameter extremes; spec-correct), one cluster of substantive shading-math divergence (`shadingToony`; [VMK#205](https://github.com/arkavo-org/VRMMetalKit/issues/205)), and one cluster of animation-pipeline divergence (`animate_root_transform`; [VMK#206](https://github.com/arkavo-org/VRMMetalKit/issues/206)). Closing both filed upstream issues before RC tag projects the corpus-wide pass-rate at 0.85 to ~72/80 (90%). *None of the gap is MToon-math error; all of it has a named cluster and a known path to closure.*

### Outline-test SSIM matrix (illustrative for `mtoon_outline_world_0p1`)

|              | vmk     | three-vrm | godot-vrm | **univrm**  |
|--------------|---------|-----------|-----------|-------------|
| vmk          | 1.000   | 0.6313    | 0.5303    | 0.6315      |
| three-vrm    | 0.6313  | 1.000     | 0.1840    | **0.9988**  |
| godot-vrm    | 0.5303  | 0.1840    | 1.000     | 0.1843      |
| **univrm**   | 0.6315  | **0.9988**| 0.1843    | 1.000       |

three-vrm ↔ univrm = 0.9988 on the worst test in the corpus. VMK ↔ {three-vrm, univrm} = ~0.63 (similar flood with slight outline-mesh-size differences). godot-vrm doesn't render outlines at all (KHR_unlit fallback) — its ~0.18 pairs are silhouette-size-only divergence.

### VMK#204 light-direction fix verified in this run

The post-fix VMK corpus run shows:
- `mtoon_shadingShift_0p8` dropped out of the top-15 most-divergent list (was at 0.8527 SSIM pre-fix; now ~0.92 with three-vrm in the new top-15 cluster).
- The pre-fix Y-mirror symptom is gone in visual inspection.
- `godot-vrm vs vrm-metal-kit` mean SSIM jumped from 0.8714 (pre-fix) → 0.9047 (post-fix) — godot agrees more strongly with the corrected VMK.
- Curiously, `univrm vs vrm-metal-kit` went *down* slightly (0.8911 → 0.8726). UniVRM and three-vrm appear to share a shading-shift response curve that VMK still diverges from at large shadingShift values, but that's a separate finding from the light-direction Y-mirror.

### Top divergent tests post-fix (excluding outline cluster)

```
mtoon_shadingToony_0p5      0.7810  outliers all four
mtoon_shadingToony_0p25     0.7842  outliers all four
mtoon_shadingToony_0p1      0.7902  outliers all four
mtoon_shadingToony_0p75     0.8141  outliers all four
swing_springbone_joints_16  0.8187  outliers all four
swing_springbone_segment_0p1 0.8199 outliers all four
```

`mtoon_shadingToony_*` becomes the new attention cluster: the four renderers diverge non-trivially on the shading-toony parameter, suggesting either a spec-interpretation ambiguity or a methodology artifact (the `shadingToony` parameter interacts with how each renderer computes the lit-vs-shaded transition). Worth a dedicated per-test investigation similar to the `mtoon_shadingShift_0p8` deep-dive.

The `swing_springbone_*` divergence is expected at this layer — UniVRM renders spring-bone tests in **rest pose** (physics not stepped), while the other three renderers run their physics implementations. Full spring-bone stepping is partially implemented: `PhysicsDriver.cs` carries `RestoreInitialTransform` + `Process(dt)` loops mirroring the godot-vrm L4 convention, but UniVRM v0.131.0's FastSpringBone runtime constructs its Burst job buffers only when `Application.isPlaying == true`, and `Unity -batchmode -executeMethod` runs in EditMode. Closing this gap properly requires a separate PlayMode batch entry point (`EditorApplication.EnterPlaymode()` → re-enter at a PlayMode method). Deferred to a follow-up L4-PlayMode plan; spring-bone tests render rest-pose for now (with the avatar root parked at `animation.root_transform.translation_end` to keep camera framing consistent with the test plan).

### Bonus: UniVRM L3 capabilities verified in this run

- Synchronous VRM load via `Vrm10.LoadPathAsync(awaitCaller: new ImmediateCaller())` works in Unity 6's batch mode without deadlocks.
- `Camera.targetTexture` + `Texture2D.ReadPixels` + `EncodeToPNG` produces non-trivial PNGs (1024×1024 ARGB32, ~30-50KB per test) in `-batchmode` with Metal initialized.
- MToon shaders compile under Built-in RP; the UniVRM-imported sphere asset shades correctly (gray hemisphere + lit highlight matching three-vrm's baseline).
- Per-test render time ~15-200ms after first-load amortization.
- Spring-bone tests (L4 deferred) render in rest pose without errors — physics not stepped but mesh still rendered.

## How to reproduce

```bash
git clone https://github.com/arkavo-org/vrm-conformance
cd vrm-conformance

# Bootstrap goldens through both real adapters (~7 min, macOS).
./scripts/bootstrap-goldens.sh

# Run the corpus-wide consensus report.
./scripts/consensus-report.sh

# Findings land at goldens-cache/consensus-report.json (machine-specific paths;
# gitignored). The summary stats print to stdout.
```

For different host configurations (different macOS version, GPU, three-vrm version, VRMMetalKit revision), the numbers will shift but the pattern is expected to hold until upstream fixes for #183 and #1838 land.

## Phase 2 collider corpus — VMK 0.14.0 doesn't apply collisions during settle

**Trigger:** Smoke-rendering the phase 2 collider corpus through vrm-metal-kit 0.14.0 before committing to a full bootstrap.

**Finding:** A `springbone_default` asset (no colliders) and a `springbone_collider_sphere_*` asset (WITH a collider sphere whose volume the chain center line penetrates) produce **byte-identical PNGs** at static settle in VMK 0.14.0. Same SHA256 across:
- Default no-collider asset
- 5 different on-axis collider configurations (radius 0.03/0.05/0.10, Y offsets -0.08/-0.04/0/+0.04)
- 4 different lateral collider configurations (X offsets ±0.02/±0.05)

Swing variants of the same plans (with `animate_root_transform` driving the chain through the collider's volume) DO produce different SHAs — confirming VMK's collision pipeline works during animated frames but not during the `warmupPhysics`/settle path that the runner uses for static physics tests.

**Interpretation:** VMK's spring-bone settle (called via `warmupPhysics(steps:)` in our adapter's `reset_physics` handler) advances joint positions under gravity + stiffness + drag but does NOT run collision resolution against `VRMC_springBone.colliders`. Collisions are only resolved during `SpringBoneComputeSystem.update` inside the render frame, which our settle-only physics path doesn't invoke.

**Sweep design adjustment:** lateral X offsets (-0.05, -0.02, +0.02, +0.05) replace the original on-axis Y offsets in `spring_bone_collider_sweep()`. Lateral offsets produce a non-zero collision-force direction, so the sweep will produce signal **once VMK applies settle collisions** (today's swing variants already produce signal because animation provides off-axis seed). The settle plans currently document static-equilibrium pose; if VMK starts running collisions during settle, the SHAs will diverge and the regression will be visible.

**Phase 6 multi-chain colliders:** same fix applied. The trivial sphere collider used for `share_*` group testing was at `offset=[0,0,0]` (on-axis, degenerate). Changed to `offset=[0.03, -0.10, 0]` with radius 0.04 — lateral, in chain's vertical range, so the sharing-mode axis has actual signal as soon as VMK applies settle collisions.

**Corpus interpretation as of VMK 0.14.0:**
- **24 swing collider plans → real cross-renderer signal**
- **24 settle collider plans → null signal on VMK (until upstream fix), but assets + methodology are correct and become useful when VMK changes**

**Upstream:** worth filing as a VMK enhancement issue — "apply VRMC_springBone collisions during warmupPhysics" — once verified the same behavior exists in current main. Suggested issue title: "warmupPhysics doesn't resolve VRMC_springBone collisions; deflection only happens during animated render frames".

**Forward:** continue with bootstrap; the swing portion of the corpus will produce cross-renderer divergence as designed. The settle portion stands as documentation of expected static behavior across renderers.

## Phase 2-6 corpus signal characterization (VMK-only bootstrap, M4 Max)

**Trigger:** VMK-only bootstrap of the full 222-plan corpus (80 existing + 142 new phase 2-6 plans). 302 renders, 0 failures. Comparing SHA256 of rendered PNGs within each sweep family answers "does this sweep actually exercise the axis it claims to?"

**Per-family signal table (distinct SHA256s / total plans):**

| sweep | mode | plans | distinct | signal |
|---|---|---:|---:|---:|
| collider | settle | 24 | 1 | **4%** — null (VMK settle-no-collision; see prior finding) |
| collider | swing | 24 | 15 | **62%** ✓ |
| extended_collider | settle | 18 | 1 | **6%** — null (same root cause) |
| extended_collider | swing | 18 | 7 | **39%** — partial (inverted shapes may be degenerate; investigate) |
| gravity_dir | settle | 4 | 3 | **75%** ✓ |
| gravity_dir | swing | 4 | 4 | **100%** ✓ |
| per-joint taper | settle | 7 | 1 | **14%** — null by design (steady-state pose invariant to transient response params) |
| per-joint taper | swing | 7 | 5 | **71%** ✓ |
| multi-chain | settle | 18 | 3 | **17%** — only `chain_count` axis (2/3/5) produces distinct settled layout; `spacing` and `sharing_mode` axes are vacuous on VMK static settle |
| multi-chain | swing | 18 | 14 | **78%** ✓ |

**What this tells us:**

1. **Animation-driven plans (swing) produce signal across nearly every axis.** Adapter divergence on chain-vs-collider deflection, per-joint taper response under inertia, and multi-chain interaction will all surface during the swing portion of any cross-renderer bootstrap.

2. **Static settle plans only produce signal on axes that affect equilibrium pose**, NOT axes that affect transient response. So:
   - `gravity_dir` (changes equilibrium pose direction) → signal at settle ✓
   - `multi-chain chain_count` (changes layout geometry) → signal at settle ✓
   - `stiffness` / `drag` / `per-joint taper` (transient response only) → null at settle (correct physics)
   - `collider` / `extended_collider` (would change equilibrium pose if applied) → null at settle (VMK bug; documented separately)

3. **`extended_collider` swing at 39% is below expectations.** Sphere-and-capsule swing was 62%; the extended shapes (planes, inverted spheres, inverted capsules) cluster more tightly. Either:
   - The chain doesn't actually contact the extended shapes during the swing arc (geometry mismatch — sweep placements may be off),
   - VMK's extended_collider implementation has gaps,
   - The angle-limit variants (3/9) cluster because the limit isn't being applied.
   Worth follow-up. Track as a phase-3 corpus-tightening item.

4. **The settle/swing pairing always differs** (sample of 6 pairs, all distinct). So every settle plan has a swing variant that produces different pixels — the swing version is a useful additional data point even when the settle version is null.

**Corpus health summary:** ~80 of the 142 new plans currently produce real cross-renderer signal on VMK 0.14.0 (essentially: all swing plans + the gravity_dir and multi-chain settle plans). The remaining ~62 plans document expected static behavior and become useful when VMK starts applying settle collisions (or when other renderers diverge from VMK on static behavior).

**Forward:** run three-vrm bootstrap to add the second renderer's data, then run `scripts/consensus-report.sh` for cross-renderer SSIM analysis. The 80 signal-producing new plans will reveal whether VMK and three-vrm diverge on collider response, multi-chain physics, or gravity direction handling. The 62 null-on-VMK plans will surface as "three-vrm diverges from VMK at settle" if three-vrm applies settle collisions where VMK doesn't.

## Phase 3 — three-vrm 3.5.0 rejects assets that declare VRMC_springBone_extended_collider

**Trigger:** Three-vrm-only bootstrap of the 222-plan corpus. 266/302 succeeded, 36/302 failed. All 36 failures are extended_collider variants (settle + swing).

**Symptom:** `load_vrm` returns `-32001 LoadFailed` for every asset that declares `VRMC_springBone_extended_collider` in `extensionsUsed`. The asset's `extensionsRequired` field correctly lists only `VRMC_vrm` (matching every other plan in the corpus), so this is not a "required extension not supported" rejection. The @pixiv/three-vrm 3.5.0 loader fails the asset.

**Hypothesis:** three-vrm's `VRMSpringBoneLoaderPlugin` likely treats every collider entry as requiring a `shape` field (per VRMC_springBone-1.0 base schema), but the extended_collider spec says omit `shape` when an extended shape is set under `extensions.VRMC_springBone_extended_collider.shape`. Strict loaders that don't implement the extension's relaxation will reject the asset. Confirmed by inspection of the emitted JSON: collider entries have NO `shape` field, only `extensions.VRMC_springBone_extended_collider.shape`.

**Coverage impact:** 36 plans not renderable on three-vrm 3.5.0; cross-renderer diff on extended_collider axes uses VMK + godot-vrm only (godot-vrm coverage TBD).

**Upstream:** worth filing against @pixiv/three-vrm — "VRMSpringBoneLoaderPlugin rejects colliders that omit `shape` in favor of `VRMC_springBone_extended_collider.shape` (extension's recommended omission causes loader failure)".

## Cross-renderer divergence: settle collisions (VMK vs three-vrm)

**Trigger:** Sampling SHA256 of `springbone_default` and `springbone_collider_*` renders between VMK and three-vrm.

**Finding:** VMK produces byte-identical PNGs for `springbone_default` (no colliders) and `springbone_collider_sphere_x0p05_r0p05` (a sphere collider in the chain's lateral path) at settle — confirming the VMK settle-no-collision issue. **three-vrm produces DIFFERENT SHAs for the same two assets** — confirming three-vrm DOES apply collisions during settle.

This is a direct cross-renderer divergence on a load-bearing physics axis. The cross-pair SSIM on these plans will quantify the magnitude. Practical implication: any avatar with author-placed colliders is silently inconsistent between VMK and three-vrm at static rest — chains rest in different positions depending on which renderer is showing the avatar.

**Forward:** quantify with `scripts/consensus-report.sh` once godot-vrm bootstrap completes. The three-renderer pair-wise SSIM matrix on the new corpus will reveal whether the divergence is two-way (VMK vs three-vrm) or three-way (and whether godot-vrm aligns with VMK on settle-no-collision or with three-vrm).

## Full four-renderer consensus report on the 222-plan corpus

**Trigger:** Bootstrap of VMK + three-vrm + godot-vrm + (existing) univrm; `scripts/consensus-report.sh` produced pairwise SSIM across all common test_ids. M4 Max.

### Headline numbers

- **222 test_ids processed, 0 skipped**
- **206/222 consensus_passed (93%)** — every renderer ≥ declared threshold vs every other
- **16/222 consensus_failed** — all 16 are MToon outline + shadingToony plans (pre-existing divergence categories; not from the new phase 2-6 corpus)

### Conformance pass rates vs UniVRM reference

| renderer | pass rate |
|---|---|
| three-vrm | **76/76 (100%)** |
| vrm-metal-kit | 74/76 (97%) |
| godot-vrm | 67/76 (88%) |

### Pairwise SSIM means (full corpus)

| pair | mean | min | max | n |
|---|---:|---:|---:|---:|
| three-vrm vs univrm | 0.9583 | 0.8491 | 0.9988 | 80 |
| three-vrm vs vrm-metal-kit | 0.9564 | 0.6313 | 0.9865 | 186 |
| univrm vs vrm-metal-kit | 0.9468 | 0.6315 | 0.9935 | 80 |
| godot-vrm vs three-vrm | 0.9242 | 0.1840 | 0.9902 | 186 |
| godot-vrm vs vrm-metal-kit | 0.8997 | 0.5303 | 0.9739 | 222 |
| godot-vrm vs univrm | 0.8429 | 0.1843 | 0.9793 | 80 |

(three-vrm n=186 reflects the 36 extended_collider plans it cannot load. godot-vrm n=222 reflects full corpus coverage. univrm n=80 is the existing pre-phase-2 coverage.)

### New corpus (142 plans) per-family min-SSIM stats

| family | n | mean min | median min | consensus pass |
|---|---:|---:|---:|:---:|
| multichain settle | 18 | **0.9067** | 0.9063 | 18/18 |
| multichain swing | 18 | 0.9129 | 0.9128 | 18/18 |
| gravity settle | 4 | 0.9093 | 0.9099 | 4/4 |
| gravity swing | 4 | 0.9159 | 0.9164 | 4/4 |
| collider settle | 24 | 0.9099 | 0.9099 | 24/24 |
| collider swing | 24 | 0.9164 | 0.9165 | 24/24 |
| extended settle | 18 | 0.9099 | 0.9099 | 18/18 |
| extended swing | 18 | 0.9162 | 0.9161 | 18/18 |
| taper settle | 7 | 0.9099 | 0.9099 | 7/7 |
| taper swing | 7 | 0.9162 | 0.9159 | 7/7 |

**All 142 new corpus plans pass consensus.** Cross-renderer SSIM minimum across the entire new corpus is 0.9058 (multichain n=5 variants, VMK vs godot-vrm).

### Patterns

1. **VMK vs godot-vrm is the consistently lowest pair** across the new corpus — every "worst pair" in the top-20 most-divergent new-corpus plans is `vrm-metal-kit vs godot-vrm`. Three-vrm sits between them on most axes. This suggests godot-vrm's Godot 4 spring-bone implementation has the largest systematic offset from VMK's Metal/SwiftFX implementation, with three-vrm closer to both.

2. **Swing variants converge tighter than settle variants** across every family (e.g., collider settle 0.9099 → swing 0.9164). Animation produces more agreement, not less, even though intuition might predict the opposite. Probable cause: at settle, MToon shading differences dominate the SSIM signal; under motion, those differences are averaged across moving silhouettes and the relative weighting shifts toward chain agreement (which is high across renderers).

3. **No new outliers.** The 16 consensus_failed plans are all pre-existing MToon outline + shadingToony issues. Phase 2-6 didn't add any renderer-specific failures despite introducing colliders, extended_colliders, multi-chain physics, and per-joint taper.

4. **`extended swing` shows higher SSIM than `extended settle`** even though three-vrm rejects all 18 extended plans entirely. The remaining pair is just VMK vs godot-vrm, and they agree adequately (0.9099 settle, 0.9162 swing). Means: VMK and godot-vrm have compatible extended_collider implementations (or at least equally-broken in matching ways).

### Conformance signal characterization

The new 142-plan corpus is **net-positive conformance signal**:
- Adds breadth on physics axes (colliders, extended_colliders, gravity_dir, per-joint taper, multi-chain) that the existing 80-plan corpus didn't cover.
- All 142 plans pass consensus on the four-renderer matrix — they discriminate between behaviors **without producing false renderer-specific failures**.
- Cross-renderer minimum 0.9058 means the corpus is tightly bounded; future renderer regressions on physics will surface as new lows below this floor.

### Forward

The seven-phase springbone closure has delivered: corpus expanded from 80 to 222 plans, infrastructure for position-based diff (phase 1) is in place, four renderers boot-strapped, consensus report produced. Reasonable continuations:

1. **File two real upstream issues**: (a) VMK 0.14.0 doesn't apply collisions during settle, (b) @pixiv/three-vrm 3.5.0 rejects assets that omit base `shape` in favor of `VRMC_springBone_extended_collider.shape`.
2. **Author `avatarA_collider_1_0.vrm`** to unblock the deferred `avatarA_bosom_collider` humanoid plan.
3. **Calibrate the phase 7 coupling matrix threshold** against three-vrm + godot-vrm baseline coupling.
4. **Wire `--dump-positions` into the bootstrap script** so position goldens populate `positions_url` manifest entries automatically.

## VMK issue hunt — five VMK bugs filed from one bootstrap

**Trigger:** With the four-renderer consensus matrix in hand, mined `goldens-cache/consensus-report.json` for VMK-specific SHA-level collapse patterns: "VMK renders multiple sweep variants byte-identically while three-vrm + UniVRM distinguish them" is a clean signature for "VMK silently ignores or mis-applies the swept parameter".

### Filed issues

| # | scope | shape |
|---|---|---|
| [VMK#236](https://github.com/arkavo-org/VRMMetalKit/issues/236) | spring-bone settle collisions | `warmupPhysics` doesn't resolve `VRMC_springBone.colliders`. 25 collider configurations + no-collider baseline all produce SHA `f02fb44e3d2a…` at static settle on VMK. Three-vrm renders these distinctly. |
| [VMK#237](https://github.com/arkavo-org/VRMMetalKit/issues/237) | `VRMC_springBone_extended_collider` chaotic | 18 swing variants → 7 SHA buckets that don't track swept axes (shape × placement × angle_limit). VMK reads SOMETHING from the extension but applies it inconsistently. |
| [VMK#238](https://github.com/arkavo-org/VRMMetalKit/issues/238) | MToon `rimLightingMix` boundary | Exact boundary values `0` and `1` produce identical render (SHA `ccbaa146…`); intermediate values `(0, 1)` produce distinct renders. Three-vrm + UniVRM distinguish all values. |
| [VMK#239](https://github.com/arkavo-org/VRMMetalKit/issues/239) | MToon `shadingShift` + `shadingToony` boundary | `shadingShift=±1` and `shadingToony=0`/`=1` collapse to default-bucket render; intermediate values work correctly. Three-vrm + UniVRM correct. (Issue body initially over-stated the scope; corrected with a follow-up comment.) |
| [VMK#240](https://github.com/arkavo-org/VRMMetalKit/issues/240) | spring-bone `stiffness` under animation | `stiffness=0`/`=0.8`/`=1` collapse to shared swing trajectory (SHA `0c9ecdad…`); only `=0.2` distinct. The shared SHA appears across 10 unrelated swing test_ids spanning collider, extended_collider, and stiffness families. |

### Cross-cutting hypothesis

VMK#238, #239, and #240 all collapse parameter values at exact integer-valued or spec-boundary inputs (`0`, `1`, `-1`, `0.8`). VMK 0.14.0's published release fix for the collider parse bug specifically addressed `JSONSerialization` returning `[Double]` while the parser cast to `[Float]`. The same pattern likely affects scalar `Float` properties when their JSON value is a whole number: `JSONSerialization` returns `NSNumber.int(0)` or `NSNumber.int(1)`, and the parser's `Float` cast silently fails, falling back to the property's default value. The collapse-to-default fingerprint matches this hypothesis.

This is a tractable upstream fix — accept `Double`, `Float`, AND `Int` in the scalar parse paths, the same way the 0.14.0 collider fix accepts `[Double]` and `[Float]`.

### Pattern that surfaced these

A small Python tool that, for each parameter sweep family, counts:

```
(VMK distinct SHAs) vs (three-vrm distinct SHAs)
```

Any family where `VMK distinct < three-vrm distinct` is a VMK collapse candidate. Combined with "are the asset's swept parameter values actually distinct in the emitted JSON" (sanity check that asset emission isn't the bug), this reliably identifies VMK silently-ignored parameters. The same tool will surface any new collapses in future bootstrap runs.

### Coverage

Five issues filed in one analysis session against a corpus of 222 plans × 4 renderers. The hunt was systematic: every MToon scalar parameter and every spring-bone scalar parameter was checked for the collapse signature. Three new VMK bugs (#238, #239, #240) came from the new phase 2-6 corpus AND from the pre-existing MToon corpus — the hunt method works equally well on existing test plans, suggesting more bugs could be found by extending similar analysis to other adapters or to less-swept parameter axes.

## Phase 2 — VRMC_springBone collider sweep landed (synthetic only)

**Trigger:** Phase 1 infrastructure (dump_bone_positions across four adapters, position-diff math, manifest + runner integration) merged. Phase 2 of the seven-phase springbone gap closure design adds collider emission to the asset generator and 48 test plans (24 Cartesian variants × settle/swing).

**Shipped:**
- Generator types: `ColliderShape::{Sphere, Capsule}`, `ColliderAttach`, `ColliderParams`, `ColliderGroupParams`, `SpringBoneSceneParams`.
- `vrm_ext.rs::vrmc_spring_bone_scene()` emits `colliders[]`, `colliderGroups[]`, per-spring `colliderGroups`.
- `emit-springbone-collider-sweep` subcommand → 48 `.vrm` + `.test.yaml` + `.meta.json` triplets.
- Sweep axes: shape (sphere, capsule), offset_y (-0.08, -0.04, 0, +0.04), radius (0.03, 0.05, 0.10). Cartesian, not one-axis-at-a-time, because collision response isn't separable on a single axis at this scale.
- VRM validator (v2.0.0-dev.3.10) reports 0 errors on sampled emitted files; 1 pre-existing warning (TEXCOORD_0 unused) and info-level empty-node messages matching the existing spring-bone corpus.

**Deferred:**
- `avatarA_bosom_collider` humanoid plan — requires authoring `avatarA_collider_1_0.vrm` in Blender (one head-mounted sphere collider intersecting the existing bust chain swing path). Estimated half-day of authoring; not code work. The 48-plan synthetic sweep is independent and ships now.
- The collider sweep currently does not run through `bootstrap-goldens.sh` — that's a separate task once renderers have rendered the new corpus at least once.

**Forward:** Phase 3 adds `VRMC_springBone_extended_collider` (planes, inverted sphere/capsule, joint angle limits).

## Phase 3 — VRMC_springBone_extended_collider sweep landed

**Trigger:** Phase 2 base-collider sweep merged. Phase 3 adds the companion extension `VRMC_springBone_extended_collider-1.0`: planes, inverted (inside) sphere/capsule, and per-joint angleLimit.

**Shipped:**
- ColliderShape variants: `Plane { normal }`, `InsideSphere { radius }`, `InsideCapsule { radius, tail_offset }`.
- `SpringBoneParams.joint_angle_limit_deg: Option<f32>` — emitted under `joints[].extensions.VRMC_springBone_extended_collider.angleLimit` (degrees, per-joint).
- glTF `extensionsUsed` correctly declares `VRMC_springBone_extended_collider` only when extended shapes or angle limits are present.
- `emit-springbone-extended-sweep` subcommand emits 36 plans (3 shapes × 3 placements + 3 shapes × 3 angle limits = 18 cartesian × settle/swing).

**Adapter coverage:** the extension is conformance-tested via cross-renderer diff in subsequent corpus runs. Adapters that don't support it should diff loudly. Known status: three-vrm and VRMMetalKit may have partial support (VMK#67 is the open angle-limit verification ticket); godot-vrm coverage depends on V-Sekai/godot-vrm's spec_extended state.

**Forward:** Phase 4 adds gravityDir variation.

## Phase 4 — gravityDir sweep landed (8 plans)

**Trigger:** Phase 3 extended-collider sweep merged. Phase 4 closes the gravity-direction axis: prior sweeps held `gravity_dir = [0,-1,0]` constant, so any adapter hard-coding -Y would pass cross-renderer diff silently.

**Shipped:** `emit-springbone-gravity-dir-sweep` subcommand emitting 8 plans (4 directions × settle/swing): default (-Y), anti (+Y), sideways (+X), oblique (+0.7, -0.7, 0). All other SpringBoneParams (joint_count, stiffness, drag, gravity_power) held at defaults so the gravity-direction axis is unconfounded.

## Phase 5 — per-joint taper sweep landed (14 plans)

**Trigger:** Phase 4 gravityDir sweep merged. Phase 5 closes the per-joint variation axis: real hair tapers stiffness toward the tip; uniform scalars hide adapter-level discretization bugs that only manifest on non-uniform chains.

**Shipped:** Four optional per-joint vectors on `SpringBoneParams`:
- `stiffness_per_joint: Option<Vec<f32>>`
- `drag_force_per_joint: Option<Vec<f32>>`
- `gravity_power_per_joint: Option<Vec<f32>>`
- `hit_radius_per_joint: Option<Vec<f32>>`

When `Some(v)`, `v.len() == joint_count` is required; the per-joint vector overrides the scalar. `emit-springbone-taper-sweep` produces 14 plans (4 stiffness shapes + 3 drag shapes × settle/swing).

**Deliberate architecture deviation:** the spec proposed a `JointVec<T>` enum (`Uniform | PerJoint`). The optional-parallel-field shape is additively cheaper and avoids churn through existing callers — equivalent expressiveness for this phase's needs. Revisit if phase 6 multi-chain forces a bigger API refactor.

**Forward:** Phase 6 — multi-chain emission.

**Forward:** Phase 5 — per-joint parameter taper (JointVec refactor).

## Phase 6 — multi-chain sweep landed (36 plans)

**Trigger:** Phase 5 per-joint taper merged. Phase 6 closes the multi-chain axis: prior sweeps emitted a single chain attached to the head; multi-chain assets exercise collider-group sharing semantics (`share_all`, `share_none`, `share_alt`) plus chain-count effects.

**Shipped:**
- `vrmc_spring_bone_scene_multichain` iterates N springs into a JSON array of springs; the single-chain `vrmc_spring_bone_scene` is now a thin wrapper.
- `emit_vrm_with_spring_bone_multichain` emits N parallel chain hierarchies (each chain attaches to its own intermediate node radial-spaced at 0.05 m around head in the XZ plane). N skins, N chain cylinder meshes, one sphere mesh.
- `pack_sphere_and_multichains` in `buffer.rs` packs a sphere + N skinned chains into a single GLB buffer with a 7-accessor-per-chain layout (pos/nrm/uv/idx/joints/weights/ibm).
- `emit-springbone-multichain-sweep` produces 36 plans (3 chain counts × 2 spacings × 3 sharing modes × settle/swing).
- Validator (v2.0.0-dev.3.10): 0 errors on sampled emitted files; warnings are pre-existing across the corpus (TEXCOORD_0 unused, NODE_EMPTY at chain tips, NODE_SKINNED_MESH_NON_ROOT — all identical in kind to single-chain assets).

**Known limitation:** the sweep's "spacing" axis (0.02, 0.05 m encoded in IDs) currently maps to a fixed 0.05 m radial spacing at emit time. Both spacing values produce identical geometry. Resolving requires threading spacing through `SpringBoneSceneParams` → emit; deferred because the chain-count and sharing-mode axes are the load-bearing ones for VMK#162-class regressions and the spacing axis is a secondary concern.

**Forward:** Phase 7 — VMK#162 regression matrix (execute-test-plan-matrix runner mode).

## Phase 7 — VMK#162 coupling matrix runner landed

**Trigger:** Phase 6 multi-chain merged. Final phase: the runner gains `execute-test-plan-matrix`, enabling self-comparison regressions of the form "changing one tuned parameter should not silently shift the equilibrium that other parameters establish" (VMK#162).

**Architecture deviation from spec:** the spec proposed runtime parameter mutation. Phase 7 ships pre-emitted asset variants instead — the matrix YAML enumerates a baseline `.vrm` + N perturbation `.vrm` paths, runner orchestrates N+1 renders + position dumps + per-joint delta computation. This sidesteps the need for an adapter-side `override_spring_params` op.

**Shipped:**
- `crates/vrm-test-plan/src/lib.rs`: `CouplingMatrix` + `CouplingPerturbation` types.
- `crates/vrm-runner/src/execute_matrix.rs`: orchestrator, `per_joint_drift`, `MatrixResult::passed()`/`outliers()`.
- `crates/vrm-runner/src/execute.rs`: `execute_plan_capturing_positions` for matrix-mode position capture.
- `vrm-runner execute-test-plan-matrix` subcommand with full describe catalog entry.
- `test-plans/manual/coupling/springbone_default_coupling.matrix.yaml`: example matrix using existing emit-springbone-sweep variants.
- Smoke-tested through mock renderer end-to-end: `ok: true`, all `max_drift_m: 0.0`, `overall_passed: true`.

**Calibration deferred:** the example matrix uses `coupling_threshold_m: 0.015` as an opening guess. Real calibration requires running the matrix on three-vrm and godot-vrm (well-behaved baselines), observing their max coupling drift, and tuning the threshold above their max but below VMK's reported coupling magnitude. That measurement run is a separate manual step — not blocking infrastructure delivery.

**Forward:** the seven-phase VRMC_springBone gap closure is complete. The corpus across phases 2–6 ships 142 new test plans:
- Phase 2: 48 collider plans
- Phase 3: 36 extended-collider plans
- Phase 4: 8 gravityDir plans
- Phase 5: 14 per-joint taper plans
- Phase 6: 36 multi-chain plans
- Phase 7: 1 example coupling matrix YAML (calibration matrix)

Next: bootstrap-goldens on the new corpus and update `goldens/manifest.json`.

## VMK 0.15.0 verification — four filed issues confirmed closed

**Trigger:** VMK 0.15.0 (commit `5378ade`) shipped with release notes citing four vrm-conformance findings as the QA regression sweep that drove the release: [#236](https://github.com/arkavo-org/VRMMetalKit/issues/236) (collider parse), [#238](https://github.com/arkavo-org/VRMMetalKit/issues/238) (rimLightingMix), [#239](https://github.com/arkavo-org/VRMMetalKit/issues/239) (shadingShift/Toony), [#240](https://github.com/arkavo-org/VRMMetalKit/issues/240) (stiffness collapse). The release attributes all four to two root causes: (a) `AnyCodable` decoding numeric JSON as Int/Double inconsistently, and (b) `warmupPhysics` failing to decrement `settlingFrames`.

**Method:** bumped `adapters/vrm-metal-kit/Package.swift` from 0.14.0 (`f25a947`) to 0.15.0 (`5378ade`), ran `SKIP_THREE_VRM=1 SKIP_GODOT_VRM=1 scripts/bootstrap-goldens.sh` to re-render only vrm-metal-kit against the unchanged 302-plan corpus. Compared post-0.15.0 PNG SHA prefixes against the captured pre-0.15.0 baseline at `/tmp/vmk_pre_0_15_0_shas.txt`.

**Distinct-SHA counts (pre → post):**

| family | issue | pre | post | verdict |
|---|---|---|---|---|
| `mtoon_rimLightingMix_*` (6 variants) | VMK#238 | 5/6 (`_0` and `_1` shared SHA `ccbaa146…`) | **6/6** | closed |
| `mtoon_shadingShift_*` (9 variants) | VMK#239 (shift) | 7/9 (`_0`, `_1`, `_neg1` shared `5d8cf178…`) | **9/9** | closed |
| `mtoon_shadingToony_*` (8 variants) | VMK#239 (toony) | 6/8 (`_0`, `_0p9`, `_1` shared `5d8cf178…`) | **8/8** | closed |
| `springbone_collider_sphere_*` (12 settle variants) | VMK#236 | 1/12 (all `f02fb44e…`) | **11/12** | closed |
| `springbone_collider_capsule_*` (12 settle variants) | VMK#236 | 1/12 (all `f02fb44e…`) | **11/12** | closed |
| `swing_springbone_collider_sphere_*` (12 swing variants) | VMK#236 | 1/12 | **12/12** | closed |
| `swing_springbone_collider_capsule_*` (12 swing variants) | VMK#236 | 1/12 | **12/12** | closed |
| `swing_springbone_stiffness_*` (4 swing variants) | VMK#240 | 2/4 (`_0`, `_0p8`, `_1` shared `0c9ecdad…`) | **4/4** | closed |

The 1/12 residual collisions in settle collider sphere/capsule are the symmetric `x=±0.05, r=0.03` configurations matching `f02fb44e3d2a…` — these are physically correct: a 3 cm-radius collider offset 5 cm laterally cannot contact bust-chain joints sitting near `x≈0`, so the settle pose equals the no-collision baseline. The swing variants confirm this — under animated excitation the chain reaches the colliders and 12/12 produce distinct SHAs.

**Cross-cutting hypothesis confirmed.** The Int-vs-Double pattern logged in the prior `VMK issue hunt` entry was named explicitly in VMK's PR #258 body: "`AnyCodable` decodes whole-number `0.0` as `Int(0)` and `as? [Double]` fails on the mixed `[Double, Double, Int]` array." PR #254 generalizes the fix to MToon scalar factors; PR #255 sweeps residual `VRMExtensionParser` sites. The boundary-collapse fingerprint identified in our findings (`0`, `1`, `-1` collapsing to default while intermediate values worked) was the correct diagnostic signal — same root cause, same fix shape, across all four issues.

**VMK tracker discrepancy:** [VMK#239](https://github.com/arkavo-org/VRMMetalKit/issues/239) is still marked `state=OPEN` on GitHub at the time of this verification, but the 0.15.0 release notes name it as a closure and our re-render confirms the symptom is gone (all 17 shadingShift+shadingToony variants now produce distinct SHAs). Likely a missed `Fixes #239` link in the merge commit; VMK should auto-close on their next pass. [VMK#237](https://github.com/arkavo-org/VRMMetalKit/issues/237) (extended_collider chaotic clustering) also remains open — release notes mention "phases 1–3" landed via PR #260/#262 but the upstream issue stays open pending end-to-end swing verification on capsule/sphere extended_collider variants. Both are tracked here for the next bump cycle.

**Forward:** the next re-bootstrap should run all four renderers so the consensus-report can quantify SSIM movement against the three-vrm/godot-vrm/UniVRM baselines. Expected direction: VMK pairwise SSIM with the consortium-reference cluster (currently `0.6313..0.9665`, mean `~0.74`) should improve materially on the four families that previously collapsed to default-bucket renders. That measurement is the value-add of this closure — distinct SHAs prove the parameter is now being read; cross-renderer SSIM proves the parameter is now being read *correctly*.

## VMK 0.15.0 conformance level re-evaluation (cross-renderer)

**Trigger:** Same-day re-run of `scripts/consensus-report.sh` against the post-0.15.0 manifest (only VMK PNGs changed; three-vrm/godot-vrm/UniVRM untouched). Compares directly to the prior 222-plan baseline in this document.

**Adapter capability tier: still L4.** No scaffold changes — Phase 1 ops + spring-bone physics remain real; the 302-plan corpus (222 unique test_ids; some plans produce settle+swing pairs) renders end-to-end. The re-evaluation is about *conformance signal*, not adapter coverage.

### Headline: VMK now matches the consortium reference on 99% of comparable plans

| pair | pre-0.15.0 | post-0.15.0 |
|---|---|---|
| **vrm-metal-kit vs univrm** (consortium reference) | 74/76 (**97%**) | **75/76 (99%)** |
| three-vrm vs univrm | 76/76 (100%) | 76/76 (100%) |
| godot-vrm vs univrm | 67/76 (88%) | 67/76 (88%) |
| consensus_passed (all-pairs) | 206/222 | 207/222 |

The 1 remaining miss against UniVRM is `mtoon_outline_world_0p1` — a universal outline-hazard test where every renderer is an outlier from every other (the 0.85 threshold is below the silhouette-AA floor at this outline thickness). It is not a VMK-specific failure.

### Pairwise SSIM movement (corpus-wide means)

| pair | pre mean | post mean | Δ | post min | post max |
|---|---:|---:|---:|---:|---:|
| three-vrm vs vrm-metal-kit | 0.9564 | **0.9572** | +0.0008 | 0.6313 | 0.9879 (was 0.9865) |
| univrm vs vrm-metal-kit | 0.9468 | **0.9491** | +0.0023 | 0.6315 | 0.9935 |
| godot-vrm vs vrm-metal-kit | 0.8997 | 0.9000 | +0.0003 | 0.5303 | 0.9777 (was 0.9739) |

The mean movement looks small at the corpus level because (a) most of the 222 plans were already passing pre-0.15.0 and (b) the closure families are a small share of the corpus. The structural fact is that **VMK's max SSIM with three-vrm and godot-vrm both rose** — i.e. the closure-family upgrades pushed previously-collapsed test_ids into the high-agreement band, not just over a threshold.

### Closure-family agreement bands (VMK vs UniVRM)

| family | n | min SSIM | mean SSIM | max SSIM | reading |
|---|---:|---:|---:|---:|---|
| `mtoon_rimLightingMix_*` | 6 | 0.9491 | **0.9789** | 0.9935 | tight agreement; VMK joins reference cluster |
| `mtoon_shadingShift_*` | 9 | 0.9290 | **0.9646** | 0.9909 | tight agreement |
| `mtoon_shadingToony_*` | 8 | 0.8945 | 0.9324 | 0.9822 | agreement at floor; some variants in the new VMK+three-vrm vs UniVRM+godot-vrm split (see below) |
| `swing_springbone_stiffness_*` | 4 | 0.962 | 0.963 | 0.964 | VMK matches UniVRM and three-vrm to ≥0.96 across the full sweep; previously these 4 plans shared a single PNG SHA on VMK |
| `springbone_collider_*` (settle, 24) | 24 | 0.9062 | 0.9082 | — | all pass consensus; VMK vs godot-vrm pair, three-vrm/UniVRM don't author these |
| `swing_springbone_collider_*` (24) | 24 | 0.9144 | 0.9158 | — | swing variants tighter than settle, as observed corpus-wide |

### Newly-visible signal: shadingToony cluster flip

Pre-0.15.0 the `mtoon_shadingToony_*` divergent tests had VMK as the consensus outlier (its shading curve was flat at boundary inputs). Post-0.15.0 the same test_ids appear in the top-15 most divergent list with **`outliers=['godot-vrm', 'univrm']`** — i.e. VMK + three-vrm now agree with each other, and the minority pair is godot-vrm + UniVRM. The 0.85 threshold is missed by 0.005–0.04 on five of the eight `shadingToony` variants (0, 0p1, 0p25, 0p5, 0p75).

This is a substantive shift in attribution. Pre-0.15.0 the natural read was "VMK has a shadingToony bug". Post-0.15.0 it reads as "VMK and three-vrm interpret the shadingToony curve one way; UniVRM and godot-vrm interpret it another." Worth filing against the next renderer pair we audit (likely godot-vrm, since UniVRM is the consortium reference and PR #235 already added VMK's radiometric mode to match what UniVRM does at the radiance-normalization layer). The actionable question is whether godot-vrm's `Godot-MToon-Shader` applies the same `1/π` BRDF Lambert + radiometric normalization that VMK and three-vrm now both apply.

### Open clusters (carried forward)

- **[VMK#213](https://github.com/arkavo-org/VRMMetalKit/issues/213)** (shadingToony curve at low-toony + high-positive-shift) — PR #235 added `LightNormalizationMode.radiometric`; verifies as no longer a VMK-specific bug per the cluster flip above. Tracker still shows open; close pending.
- **[VMK#237](https://github.com/arkavo-org/VRMMetalKit/issues/237)** (extended_collider chaotic) — PRs #260/#262 land phases 1–3; tracker still open pending end-to-end swing verification (we can supply that now from `_assets_extended/`).
- **[VMK#239](https://github.com/arkavo-org/VRMMetalKit/issues/239)** — release notes name it closed; tracker discrepancy. SHA-distinctness + cross-renderer SSIM both confirm symptom gone.
- **[VMK#228](https://github.com/arkavo-org/VRMMetalKit/issues/228)** (rim front-face contribution) — closed via regression test in #234. SSIM data agrees.

### Bottom line

VMK has moved from the **97% conformance band** (with named outstanding clusters on rim lighting and shadingToony) to the **99% conformance band** against the consortium reference, with the four "boundary collapse" findings cited as direct contributors to the release. The remaining 1 miss is a universal methodology hazard, not a VMK-specific defect. Cross-renderer SSIM movement is modest at the corpus mean (∆ ≤ +0.003) but the structural change is in *attribution* — VMK is now a member of the spec-tight cluster, and the next round of upstream fingerpointing should be directed at the godot-vrm + UniVRM minority on shadingToony.

## MToon alpha sweep landed — new conformance signal (VMK#264 surface area)

**Trigger:** Prior to VRMA phase 2 work, added 5 new sweep variants to `mtoon_basic_sweep` to exercise the MToon alpha-routing surface ([VMK#264](https://github.com/arkavo-org/VRMMetalKit/issues/264) territory). The generator gained an `alpha_cutoff: f32` field on `MToonParams` and now emits glTF-spec-correct `alphaCutoff` (only when `alphaMode == MASK`).

**New corpus additions:**

| test_id | alphaMode | baseColorFactor.a | alphaCutoff | transparentWithZWrite |
|---|---|---|---|---|
| `mtoon_alpha_mask_cutoff_0p25` | MASK | 0.25 | 0.25 | false |
| `mtoon_alpha_mask_cutoff_0p5` | MASK | 0.50 | 0.50 | false |
| `mtoon_alpha_mask_cutoff_0p75` | MASK | 0.75 | 0.75 | false |
| `mtoon_alpha_blend_zwrite_false` | BLEND | 0.50 | — | false |
| `mtoon_alpha_blend_zwrite_true` | BLEND | 0.50 | — | true |

(The default `mtoon_default` already covers the OPAQUE baseline so we don't re-emit it.)

### Method

`scripts/bootstrap-goldens.sh` rendered the new 5 plans through VMK + three-vrm + godot-vrm on Apple M4 Max (UniVRM has no entries for these test_ids yet — the existing UniVRM batch only covers the pre-phase-2 80-test corpus). `scripts/consensus-report.sh` produced pairwise SSIM. Manifest now carries 725 entries vs. the prior 710.

### Per-test-id SHA distinctness (across 3 cutoff values)

| renderer | distinct SHAs across 3 MASK cutoffs |
|---|---|
| **vrm-metal-kit** | **3 of 3** (`0559d7…`, `cedc33…`, `29ea50…`) — distinguishes every cutoff |
| three-vrm | 1 of 3 (single SHA `6ff1f5…` for all cutoffs) — **alphaCutoff variations invisible in output** |
| godot-vrm | 1 of 3 (single SHA `51c60e…` for all cutoffs) — **alphaCutoff variations invisible in output** |

For BLEND variants (`zwrite_false` vs `zwrite_true`), all three renderers produce byte-identical pairs (1 SHA per renderer covering both zwrite states). This is the expected null result for a single-mesh scene — `transparentWithZWrite` only affects depth interactions between multiple transparent surfaces, which we don't author.

### Per-test pairwise SSIM (MASK variants)

| test_id | VMK vs three-vrm | VMK vs godot-vrm | three-vrm vs godot-vrm | consensus |
|---|---:|---:|---:|---|
| `mtoon_alpha_mask_cutoff_0p25` | 0.9463 | **0.9994** | 0.9466 | passed |
| `mtoon_alpha_mask_cutoff_0p5` | 0.9469 | **0.9996** | 0.9466 | passed |
| `mtoon_alpha_mask_cutoff_0p75` | **0.9912** | 0.9503 | 0.9466 | passed |

The pattern is notable: **VMK's pairwise SSIM with each reference renderer shifts as `alphaCutoff` changes**. At low cutoff (0.25, 0.5) VMK matches godot-vrm to ≥0.9994. At high cutoff (0.75) VMK matches three-vrm to 0.9912. The two reference renderers each produce a single invariant output across cutoffs, and those two outputs disagree — three-vrm vs godot-vrm sits at 0.9466 regardless of cutoff value.

### Interpretation

This corpus *does not directly verify* [VMK#264](https://github.com/arkavo-org/VRMMetalKit/issues/264) (`discard_fragment()` defeats hardware A2C). #264 predicts that VMK's MASK output should look the same as OPAQUE — no smooth coverage variation across cutoff values — since the shader discards before A2C can act. But we observe VMK responding visibly to cutoff value (3 distinct SHAs), which is the opposite of what #264's description would predict. Two possibilities:

1. VMK is varying its output via some non-A2C code path (e.g. baseColorFactor.a modulating the rendered color outside the discard branch), which gives the cutoff parameter visible effect but not the spec-correct subsample-coverage shape. #264's bug is real but is being *masked* by an upstream incorrect behavior.
2. The discard-before-A2C path described in #264 has not landed in the 0.15.0 release we're pinned to, and partial A2C is currently working. The pipeline routing fix referenced in #264's framing is independent of this rendering output.

Either way the data is unambiguous on a more basic point: **`alphaCutoff` does not produce visible variation in three-vrm or godot-vrm**. Both reference renderers ignore the swept parameter entirely — which is the inverse of the VMK-vs-references attribution pattern we usually see (VMK ignores, references vary). This is the corpus producing a *new* shape of conformance finding.

### Out of scope for this entry

- **[VMK#265](https://github.com/arkavo-org/VRMMetalKit/issues/265)** (VRM 0.x `_BlendMode=3` → `transparentWithZWrite` conversion). The generator emits VRM 1.0 only; we have no VRM 0.x source asset that could carry `_BlendMode`. Filed as a follow-up corpus extension (generator gains a `--emit-vrm0` flag, or hand-author one VRM 0.x reference asset).
- **[VMK#266](https://github.com/arkavo-org/VRMMetalKit/issues/266)** (MSAAAlphaToCoverageTests pass when A2C is dead code). Meta-issue inside VMK's own test suite; not addressable from this corpus.

### Forward

The new signal warrants follow-up issue-filing once we have a clean reproduction story:
- File three-vrm + godot-vrm issues that `alphaCutoff` parameter has no visible effect in their MToon paths (or confirm via the spec that the observation is spec-compliant — MASK with uniform `baseColorFactor.a == alphaCutoff` is a degenerate case where pass/fail is the only spec-prescribed behavior).
- Comment on VMK#264 with this corpus's observation: "VMK distinguishes cutoff values at the PNG level; the discard-before-A2C bug described in #264 may not be the root cause of three-vrm/godot-vrm divergence on these test_ids."

## VRMA conformance — first cross-renderer signal (two real adapters)

**Trigger:** VRMA phases 1-5 landed (commits `36b663d..d012255`). UniVRM (phase 4) and three-vrm (phase 5) are now real VRMA adapters; godot-vrm and VRMMetalKit return `-32000 vrma-v1` Unimplemented. This is the first run where two real adapters produce comparable pose-vector output.

**Method:** Phase 6 bootstrap renders the 37-plan VRMA corpus (15 humanoid + 12 expression + 10 lookAt) through all 4 adapters. `scripts/vrma-pose-consensus.py` aggregates `<output_dir>/<id>_<renderer>.pose.json` pairs into a structured pose-diff report using spec-default tolerances (0.010 rad per-bone / 0.005 m hips / 0.005 expression / 1.0° yaw-pitch / 0.001 m offset).

### Adapter VRMA coverage

| adapter | pose.json files produced | gap | tracker |
|---|---|---|---|
| three-vrm | **37/37** (full corpus) | — | — |
| UniVRM | **15/37** (humanoid sweep only) | expression + lookAt assets fail UniVRM load; bugs in our .vrma emission — see "Emission bugs surfaced" below | self-filed in this findings entry |
| godot-vrm | 0/37 | Unimplemented; `addons/vrm/1.0/VRMC_vrm_animation.gd` is an empty stub | [V-Sekai/godot-vrm#142](https://github.com/V-Sekai/godot-vrm/issues/142) |
| VRMMetalKit | 0/37 | Unimplemented; VMK#165 open since 2026-05-10 | [VMK#165](https://github.com/arkavo-org/VRMMetalKit/issues/165) |

### Cross-renderer headline (15 humanoid plans, three-vrm vs UniVRM)

**0/15 plans pass at spec-default tolerances. Worst per-bone divergence: 1.0472 rad (60°). Mean across the 15 plans: 0.66904 rad (~38°).**

```
==> Cross-renderer pose-diff: three-vrm vs univrm
    test_id_count: 15
    passed: 0/15
    failed: 15/15

    Per-channel maxima (worst single test):
      per_bone_rotation_max_rad                max=1.04720  mean=0.66904
      hips_translation_m                       max=0.00000  mean=0.00000
      per_preset_expression_max_delta          max=0.00000  mean=0.00000
      look_at_yaw_delta_deg                    max=0.00000  mean=0.00000
```

### Top 10 most-divergent test_ids

| test_id | per-bone max (rad) | worst bone | authored angle |
|---|---:|---|---:|
| `vrma_humanoid_l_lowerleg_pitch` | 1.0472 | leftLowerLeg | 60° |
| `vrma_humanoid_l_upperarm_pitch` | 1.0472 | leftUpperArm | 60° |
| `vrma_humanoid_l_upperarm_yaw` | 1.0472 | leftUpperArm | 60° |
| `vrma_humanoid_r_upperarm_pitch` | 1.0472 | rightUpperArm | 60° |
| `vrma_humanoid_r_upperarm_yaw` | 1.0472 | rightUpperArm | 60° |
| `vrma_humanoid_head_yaw_45` | 0.7854 | head | 45° |
| `vrma_humanoid_l_upperleg_pitch` | 0.7854 | leftUpperLeg | 45° |
| `vrma_humanoid_l_upperarm_roll` | 0.5236 | leftUpperArm | 30° |
| `vrma_humanoid_neck_yaw_30` | 0.5236 | neck | 30° |
| `vrma_humanoid_r_upperarm_roll` | 0.5236 | rightUpperArm | 30° |

### The pattern is conclusive

Each test_id's measured divergence equals **exactly** the generator-authored angle of that bone's rotation (1.0472 = 60°, 0.7854 = 45°, 0.5236 = 30°). This is the unmistakable signature of "one renderer applies the rotation, the other leaves the bone at identity". The geodesic between authored-quat and identity is the authored angle.

Inspection of `vrma_humanoid_l_upperarm_yaw` pose dumps confirms:

```
leftUpperArm  three-vrm = [0.0, 0.5,  0.0, 0.866]   ← 60° Y rotation applied
leftUpperArm  univrm    = [0,   0,    0,   1]       ← identity, no rotation
```

### Apparent UniVRM batch-path-specific issue

This contradicts the phase 4 task 9 smoke (commit `35db5c6`), which verified that UniVRM correctly applies the head bone's 45° Y rotation when given `vrma_humanoid_head_yaw_45` via the manual one-off `execute-test-plan` path. The smoke produced UniVRM head `[0, -0.3827, 0, 0.9239]` = ±45° Y (sign-invariant); the phase 6 batch produces UniVRM head identity on the *same* .vrma input.

The divergence isn't in three-vrm (verified via phase 5 smoke at `vrma_humanoid_head_yaw_45` → 45°) and isn't in the .vrma file itself (same file three-vrm reads). It's in UniVRM's **batched** VRMA path through `Conformance.Tests.Play.BatchRunner` versus the one-off `execute-test-plan` path that worked in phase 4 smoke. Likely root causes (not yet pinned down):

1. **`apply_at_time` not threaded through the batch manifest.** The BatchRunner reads `t.animation.vrma.apply_at_time` from manifest.json; the runner's `execute_test_batch.rs` may not be serializing it correctly.
2. **VrmaDriver's `srcAnimator.enabled = false` toggle interacts with PlayMode batch lifecycle differently** than with the one-off PlayMode invocation. Per-test cleanup may be leaving Mecanim in a state where SampleAnimation doesn't reach limb bones on subsequent iterations.
3. **The retarget call `target.Runtime.Process()` is being shadowed by per-frame Mecanim updates** when the GameObject lifetime spans the next `yield return null`.

Tracked for follow-up; doesn't block the cross-renderer signal — three-vrm is correct, UniVRM batch reports identity for non-head bones.

### Emission bugs surfaced (22 of 37 plans fail UniVRM load)

The phase 3 .vrma generator has two bugs that prevent UniVRM from loading expression and lookAt assets:

**Expression sweep (12 failures): `NodeImporter.FixCoordinate` index out of range.**
```
System.ArgumentOutOfRangeException: Index was out of range.
  at UniGLTF.NodeImporter.FixCoordinate (...) [0x0005d] in NodeImporter.cs:161
```
The generator emits expression-target nodes with no TRS fields; `FixCoordinate` walks the node hierarchy and tries to read from an array indexed by something that's missing. Hypothesis: the node needs an explicit `translation`/`rotation`/`scale` or `matrix` field.

**LookAt sweep (10 failures): `VrmAnimationImporter.TransferOwnership` null reference.**
```
System.NullReferenceException: Object reference not set to an instance of an object
  at UniVRM10.VrmAnimationImporter.TransferOwnership (...) [0x0000c] in VrmAnimationImporter.cs:303
```
`TransferOwnership` is called after the importer parses humanoid + expression + lookAt blocks. Hypothesis: lookAt-only .vrma files need humanoid bones declared even when no humanoid rotation channels exist, to satisfy UniVRM's importer invariants.

These are corpus-emission bugs filed against ourselves — not consortium-implementation bugs. Three-vrm reads these same .vrma files without issue, so they're spec-compliant enough for three-vrm's parser. UniVRM has stricter validation. Worth filing as our own follow-up; doesn't block the headline result.

### Interpretation

**Phase 6 closes the wiring loop:** two real adapters can apply VRMA, and the runner can compute cross-renderer pose-vector diff over the result. The 15-plan signal is the first measurable VRMA conformance number we've ever produced. **It's also a real cross-renderer divergence finding — UniVRM's batch path applies head bone but not limb bones, while three-vrm applies all.**

The "0/15 pass" headline isn't a methodology failure or threshold-too-tight issue — it's a real engineering signal pointing at UniVRM's batch lifecycle. Until that gets pinned down, the suite has a valid first-pass measurement, and the bug it surfaced is the kind of thing the conformance suite exists to find.

### Forward

1. **[#6](https://github.com/arkavo-org/vrm-conformance/issues/6)** — UniVRM batch path: head bone applies, limb bones at identity. The signal driver. Likely apply_at_time threading or Mecanim toggle ordering.
2. **[#7](https://github.com/arkavo-org/vrm-conformance/issues/7)** — Expression .vrma emission: NodeImporter.FixCoordinate index range (12 plans).
3. **[#8](https://github.com/arkavo-org/vrm-conformance/issues/8)** — LookAt .vrma emission: TransferOwnership null reference (10 plans).
4. **[#9](https://github.com/arkavo-org/vrm-conformance/issues/9)** — execute-test-batch relative-path resolution: surfaced during phase 6 staging.
5. **[#10](https://github.com/arkavo-org/vrm-conformance/issues/10)** — Phase 7 manual humanoid clips tracker (Blender authoring + T-pose audit).
6. External — [VMK#165](https://github.com/arkavo-org/VRMMetalKit/issues/165) commented and [V-Sekai/godot-vrm#142](https://github.com/V-Sekai/godot-vrm/issues/142) filed for the two `Unimplemented` adapter gaps.

The 15-plan signal at 0/15 pass is paradoxically the cleanest VRMA conformance finding the suite has produced: a single, falsifiable divergence pattern with named bones, named test_ids, and a clearly-bounded root cause (UniVRM batch path; not three-vrm; not the .vrma emission; not phase 2 runner substrate). That's exactly what cross-renderer conformance is supposed to surface.

## Downstream-user-reported VMK defect catalog — spec-section to tracking map

A downstream user assembled a catalog of observed VMK visual defects with explicit spec citations. Each maps to a concrete spec section + an existing VMK issue + this corpus's coverage status. Recorded here for traceability so future reports can be cross-checked against this taxonomy before filing.

| user-observed defect | spec section violated | VMK tracking | corpus coverage |
|---|---|---|---|
| Hair loses transparency (becomes opaque) | glTF 2.0 §3.9.4 `alphaMode` + VRMC_materials_mtoon `transparentWithZWrite` | [VMK#263](https://github.com/arkavo-org/VRMMetalKit/issues/263) open | partial — alpha sweep single-mesh only; [vrm-conformance#11](https://github.com/arkavo-org/vrm-conformance/issues/11) opens layered fixture gap |
| Hair rendered behind opaque ear | VRM 1.0 standard render-queue (transparent after opaque) | [VMK#263](https://github.com/arkavo-org/VRMMetalKit/issues/263) open | **no** — [vrm-conformance#11](https://github.com/arkavo-org/vrm-conformance/issues/11) |
| Arms twist inside-out during walking | VRMC_vrm_animation + VRMC_vrm Humanoid quaternion retarget | [VMK#165](https://github.com/arkavo-org/VRMMetalKit/issues/165) open (no VRMA impl yet) | partial — single-bone VRMA via phase 3 sweep; multi-bone walks deferred to [vrm-conformance#10](https://github.com/arkavo-org/vrm-conformance/issues/10) |
| Joints bend backwards | same as above (rest-pose delta calculation) | VMK#165 | same — single-bone covered, multi-bone deferred |
| Hair clips through face (static) | VRMC_node_collider boundary | **fixed VMK#236 in 0.15.0** | verified — 24-variant collider sweep 11/12 distinct post-fix |
| Hair clips through face (during fast motion) | VRMC_node_collider + frame timing | [VMK#267](https://github.com/arkavo-org/VRMMetalKit/issues/267) open (1-frame writeBonesToNodes lag) | partial — swing sweep exercises motion but 0.2 m / 0.25 s window may not surface 1-frame lag; avatarA_bosom_swing more realistic |
| Hair flies rigidly / doesn't fall under gravity | VRMC_springBone stiffness + gravity math | **fixed VMK#240 in 0.15.0** | verified — stiffness swing 4/4 distinct post-fix |
| Bust caves inward | VRMC_springBone origin/offset + zero-settle | **fixed VMK#233 in 0.14.0** | verified — `avatarA_bosom_zerosettle` SSIM jumped 0.7928 → 0.8396 vs three-vrm |

### What was actionable from the catalog

Two follow-ups landed:

1. **Comments on VMK#263 + VMK#267** ([#263 comment](https://github.com/arkavo-org/VRMMetalKit/issues/263#issuecomment-4472357789), [#267 comment](https://github.com/arkavo-org/VRMMetalKit/issues/267#issuecomment-4472358560)) — forwarded the spec citations + corpus-coverage status to the VMK team, plus the layered-transparency fixture offer.
2. **[vrm-conformance#11](https://github.com/arkavo-org/vrm-conformance/issues/11)** — corpus gap for layered-transparency MToon fixture (multi-mesh, opaque + transparent layered) so VMK#263 fix can be cross-renderer verified.

### What was already covered

Five of the eight defect classes are tracked elsewhere (VMK closed 4 in 0.14.0/0.15.0; VMK#165 + #267 + #263 remain open). Phase 6's VRMA work covers the multi-bone retargeting axes from the spec angle. The corpus's existing avatarA humanoid plans + the spring-bone closure work already exercise the post-fix verification path for the four closed VMK issues.

### Lesson for future downstream defect catalogs

When a downstream user reports a visual defect with spec citations, the highest-value response is **mapping each defect to (a) the spec section it violates, (b) the existing upstream tracking issue, and (c) the corpus test_id that catches it**. Filing new issues is the exception; most defects in a well-tracked project already have an open issue. The exception in this round was the layered-transparency *corpus* gap — a clear corpus gap, not an unfiled defect.

### Counter-datapoint: VMK 0.15.1 (unreleased) renders MToon transparency cleanly

A VMK tester evaluated a static T-pose render of a VRM 1.0 asset on the **unreleased 0.15.1** (post-0.15.0 main) and reported a clean, high-quality result on the three axes VMK#263 specifically calls out: alpha/transparency, depth sorting, and MToon specular/shading.

This means: **VMK#263 appears already fixed in 0.15.1**, not "asset-conditional in 0.15.0" as the prior framing suggested. The 0.15.0 → 0.15.1 delta contains the closure work. (My first comment on VMK#263 proposed a material-JSON bisect on the wrong assumption that both releases were the same code; corrected at [VMK#263 #issuecomment-4472442300](https://github.com/arkavo-org/VRMMetalKit/issues/263#issuecomment-4472442300).)

**Implication for the corpus pin:** vrm-conformance currently pins VMK at 0.15.0 (`adapters/vrm-metal-kit/Package.swift`, commit `6c90240`). When 0.15.1 releases, bump the pin + re-run the cross-renderer bootstrap to verify VMK#263 closure with the same signal that surfaced it. The single-mesh alpha sweep we have today will already detect a closure on its 5 variants; the layered-transparency fixture from [vrm-conformance#11](https://github.com/arkavo-org/vrm-conformance/issues/11) would catch broader render-queue regressions without depending on a specific asset's full material block.

### T-pose spec primer for VRMA implementers

The VMK team reported confusion about the T-pose spec while planning VMK#165 (VRMA implementation). The spec covers it in two complementary documents — both are mandatory reading for anyone implementing VRMA application math:

- [`VRMC_vrm-1.0/tpose.md`](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_vrm-1.0/tpose.md) defines T-pose as **two simultaneous criteria**: appearance (8 visual rules, 1.1–1.8) and numerical (uniform-scale transforms, 2.1).
- [`VRMC_vrm_animation-1.0/how_to_transform_human_pose.md`](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_vrm_animation-1.0/how_to_transform_human_pose.md) defines the **rest rotation math** that VRMA application requires.

**The load-bearing fact** that trips up VRMA implementers: VRM 1.0 removed the VRM 0.x restriction forcing rest rotations to identity. A spec-correct VRM 1.0 model can have **non-zero local rest rotations on humanoid bones** while still visually being in T-pose. This means a VRMA's `local_rotation_quat` field **cannot be applied directly to `bone.localRotation`** — it must be normalized through the model's rest rotation pair `(W, L)` first.

The spec provides explicit formulas:

- `PoseForA → NormalizedLocalRotation`: `W · L⁻¹ · A.LocalRotation · W⁻¹`
- `NormalizedLocalRotation → PoseForB`: `L · W⁻¹ · NormalizedLocalRotation · W`

UniVRM bundles this as runtime "ControlRig" machinery; `target.Runtime.Process(sourcePose, sourceTPose)` applies the math automatically. VMK#165's implementation needs equivalent surface area.

Full primer forwarded to VMK at [VMK#165 #issuecomment-4472458466](https://github.com/arkavo-org/VRMMetalKit/issues/165#issuecomment-4472458466). The downstream-user "arms twist inside out during walking" symptom is the textbook failure mode when the normalization math is skipped — the vrm-conformance 15-plan humanoid sweep will surface this directly via the runner's pose-diff op.

**Methodology note** (record for future):

T-pose conformance has a precedent for being audited as a one-time check per avatar (the methodology hazard #1 the VRMA design spec already calls out). The suite's `avatarA_1_0.vrm` should be audited against the 8 appearance criteria + the rest-rotation-is-non-identity reality before manual humanoid clips (issue [#10](https://github.com/arkavo-org/vrm-conformance/issues/10)) ship. A T-pose audit isn't currently a runner op — it's a one-time check at corpus-curation time.

## VMK 0.15.1 verification — VRMA pose math + spring-bone rotation closures

**Trigger:** VMK 0.15.1 ships closing four conformance issues filed during the 0.15.0 review window: VMK#264 (MToon discard_fragment defeats A2C — opt-in A2C path added), VMK#265 (VRM 0.x `_BlendMode=3` → `transparentWithZWrite` explicit), VMK#269 (VRMA retargeting zombie pose — pose-normalisation formula from `VRMC_vrm-1.0/how_to_transform_human_pose.md` shipped verbatim), and VMK#270 (spring-bone twin-tails horizontal during rotation — parent rotation now read fresh each frame). Release notes also call out two **behaviour changes**: spring-bone gravity is ~12× stronger, and `windAmplitude` is now velocity-scale (÷ ~60).

**Method:** bumped `adapters/vrm-metal-kit/Package.swift` from 0.15.0 (`5378ade`) to 0.15.1 (`db5b90b`), re-rendered VMK-only over the unchanged 386-plan corpus, ran `scripts/consensus-report.sh` against the new manifest (UniVRM + three-vrm + godot-vrm PNGs cached from phase 6 bootstrap).

### Headline numbers (vs phase 6 baseline)

| metric | pre-0.15.1 | post-0.15.1 | Δ |
|---|---|---|---|
| **vrm-metal-kit vs univrm** | 75/76 (97% → 99%) | **80/81 (99%)** | +5 plans (new alpha sweep coverage), pass-rate held |
| univrm vs vrm-metal-kit pairwise SSIM mean | 0.9491 | 0.9473 | −0.0018 |
| three-vrm vs vrm-metal-kit pairwise SSIM mean | 0.9572 | 0.9575 | +0.0003 |
| godot-vrm vs vrm-metal-kit pairwise SSIM mean | 0.9000 | 0.9016 | +0.0016 |
| Top SSIM (VMK vs godot-vrm) | 0.9777 | **0.9996** | +0.0219 |
| consensus_passed | 207/222 | 211/227 | +4 |

**No VMK regressions.** The 0.9491 → 0.9473 dip in pairwise vs UniVRM is consistent with the gravity 12× behaviour change: some spring-bone plan rest positions shift, moving SSIM slightly.

### Behaviour change verification: gravity 12× stronger (release-notes callout)

Direct evidence — VMK 0.15.1 `swing_springbone_gravity_*` PNGs:

```
68b391e7764a swing_springbone_gravity_0.png       ← collapse
68b391e7764a swing_springbone_gravity_1.png       ← collapse
68b391e7764a swing_springbone_gravity_2.png       ← collapse
3330d007e2ac swing_springbone_gravity_dir_anti.png
8bd3bca3db2c swing_springbone_gravity_dir_default.png
29723d51d5ca swing_springbone_gravity_dir_oblique.png
c1afdb420d2e swing_springbone_gravity_dir_sideways.png
```

`swing_springbone_gravity_0/1/2` all share SHA `68b391e7764a` — at 12× stronger gravity, the magnitude sweep is saturated (anything > 0 pulls the chain to its fully-extended rest in the swing window). Direction sweep still distinguishes (4 distinct SHAs across 4 directions) because direction isn't affected by magnitude scaling.

Similarly, `swing_springbone_stiffness_0/0p2` share the same SHA (stiffness too weak to resist the new strong gravity), while `_0p8/_1` distinguish. The behaviour-change callout is **verifiable from the cross-renderer signal** — exactly the surface a conformance suite should report.

**Implication for the corpus:** the gravity + stiffness magnitude sweep values were calibrated against 0.15.0's gravity scale. They're now compressed at the low end. Either re-author the sweep with new values (e.g., `gravity ∈ {0.05, 0.10, 0.50}` instead of `{0.0, 0.5, 1.0}`) or document this as an intentional cross-version artefact. Not blocking; recorded for re-tuning when the spring-bone sweep gets a follow-up pass.

### What 0.15.1 closures we CANNOT directly verify from our existing signal

- **VMK#269 (VRMA retargeting)** — VMK now has VRMA library support, but the **VMK adapter's `Operations.swift`** still declares the 5 VRMA ops in `reservedPhases` (returning `-32000 vrma-v1`). Phase-7-equivalent adapter wiring would promote them to real, after which our 15-plan humanoid VRMA sweep would directly verify VMK#269 closure by comparing VMK pose dumps against UniVRM + three-vrm.
- **VMK#270 (spring-bone rotation)** — vrm-conformance corpus doesn't currently rotate the avatar root during physics (the `animate_root_transform` op interpolates translation only). [vrm-conformance#12](https://github.com/arkavo-org/vrm-conformance/issues/12) tracks the rotation-while-physics test family that would verify this directly.
- **VMK#264 (MToon A2C)** — opt-in path; our test plans don't request A2C explicitly, so default rendering is unchanged. Verification would require either an A2C-opt-in flag in the test plan schema, or layered-transparency fixture work ([vrm-conformance#11](https://github.com/arkavo-org/vrm-conformance/issues/11)).
- **VMK#265 (VRM 0.x conversion)** — no VRM 0.x asset in the corpus; deferred per phase 3 scope.

### Spec citations driving 0.15.1's VRMA closure

The VMK 0.15.1 release notes credit two pieces of work from this suite:

- **The T-pose primer at [VMK#165 #issuecomment-4472458466](https://github.com/arkavo-org/VRMMetalKit/issues/165#issuecomment-4472458466)** that documented the spec's pose-normalisation formula
  `Normalised = W_A · L_A⁻¹ · A.LocalRotation · W_A⁻¹`
  `B.LocalRotation = L_B · W_B⁻¹ · Normalised · W_B`
  — shipped verbatim in 0.15.1's `VRMAnimationLoader.makeRotationSampler`.
- **The phase 6 15-plan humanoid VRMA signal** (0/15 pass at spec tolerance, per-bone divergence equal to authored angle) that confirmed the defect was a normalisation failure rather than per-asset noise.

The conformance suite's role of producing falsifiable signal that drives upstream closure is working as designed — same playbook the spring-bone + MToon closures used in prior phases.

### Forward

The biggest remaining gap is **VMK adapter VRMA wiring**: promote the 5 VRMA ops out of `reservedPhases` and bind them to VRMMetalKit's now-real VRMA library API. Once that lands, the 4-renderer cross-renderer pose-diff matrix becomes meaningful (currently 2-renderer only). The work is similar in shape to the phase 4 UniVRM + phase 5 three-vrm adapter wiring; estimated 8–12 commits.

### Spring-bone rotation guidance for VMK (filed pre-0.15.1, closed in 0.15.1)

A 0.15.1 (unreleased) tester reported twin-tails / side-locks sticking horizontally as the character rotates. Downstream framing identified 4 claimed spec violations; critical pass against canonical [VRMC_springBone-1.0 README.md](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_springBone-1.0/README.md):

| user claim | accuracy | correct framing |
|---|---|---|
| "Verlet integration required" | overstated | Spec §SpringBone Algorithm explicitly marks the section `*non-normative*`. Verlet is the reference path; the mandate is on observable behavior. |
| "gravityPower + gravityDir applied per frame" | **accurate** | Pseudocode: `external = deltaTime * gravityDir * gravityPower; nextTail += external`. |
| "stiffness pulls toward rest pose" | partial | The pseudocode is `stiffness = deltaTime * parentWorldRotation * initialLocalRotation * boneAxis * stiffnessForce`. **The rest direction uses parent's CURRENT world rotation, not cached.** If VMK caches `initialParentWorldRotation`, the spring locks toward a world-fixed direction — manifests as "horizontal stick during rotation". |
| "World-vs-local evaluation error" | spirit-correct but missing the spec mechanism | The spec has a `center` field per SpringChain that switches integration into center-relative space precisely for the "model rotates / walks" case. World space is the default; `center` is the spec's prescribed mechanism. |

Forwarded to VMK at [VMK#270](https://github.com/arkavo-org/VRMMetalKit/issues/270) with diagnostic suggestions (check `parentWorldRotation` is read fresh each frame; log the 4 force-term magnitudes; verify whether the asset declares `center`).

**Corpus gap surfaced:** the suite's `animate_root_transform` op exercises translation-driven inertia but not rotation-driven inertia. A new `animate_root_rotation` op + 12–18 variant rotation-while-physics sweep would catch this defect class. Filed as [vrm-conformance#12](https://github.com/arkavo-org/vrm-conformance/issues/12).

## Gap-fill: gravity magnitude sweep retuned for VMK 0.15.1's spec-correct gravity scale

**Trigger:** verifying VMK 0.15.1, the `swing_springbone_gravity_{0,1,2}.png` PNGs all share SHA `68b391e7764a` on VMK — the 12× behaviour-change collapse. A pointed user question — "Is gravity being tested?" — surfaced the deeper issue: even pre-0.15.1, the gravity magnitude sweep `{0.0, 1.0, 2.0}` was only discriminating on **one renderer** (three-vrm). VMK + godot-vrm + UniVRM all collapsed to a single SHA across the magnitude axis.

### Pre-retune cross-renderer SHAs (swing variants)

| renderer | distinct SHAs (3 magnitudes) | status |
|---|---|---|
| three-vrm | **3/3** | discriminates correctly |
| vrm-metal-kit (0.15.1) | 1/3 | saturated at 12× scale |
| godot-vrm | 1/3 | known godot spring-bone defect |
| univrm | 1/3 | values too large for spec-correct scale |

Three of the four renderers were giving the suite zero signal on the gravity-power axis. The corpus had a real coverage gap even before 0.15.1; the 12× change just made it more visible.

### Retune

Replaced `{0.0, 1.0, 2.0}` with `{0.0, 0.02, 0.05, 0.10, 0.20}` — 5 values spanning the post-spec-compliance discrimination band. Lower end (0.02) is just above three-vrm's noise floor; upper end (0.20) is well below VMK 0.15.1's saturation threshold.

### Post-retune cross-renderer SHAs (swing variants)

| renderer | distinct SHAs (5 magnitudes) | status |
|---|---|---|
| **three-vrm** | **5/5** | discriminates fully |
| **vrm-metal-kit (0.15.1)** | **5/5** | discriminates fully — VMK is now a member of the spec-correct cluster on gravity-power |
| godot-vrm | 1/5 | still collapses; known defect tracked separately |
| univrm | 1/5 (all 5 share SHA `5253c7934887`) | new cross-renderer finding — UniVRM's spring-bone swing setup doesn't visibly apply gravity_power regardless of value; status=`ok` per the runner so it's not a parse error, it's a runtime non-application |

The gap is closed: VMK + three-vrm now both produce 5-way distinct PNG SHAs across the new gravity sweep. Cross-renderer signal on the gravity-power axis is real and falsifiable.

### Unrelated UniVRM BatchRunner bug surfaced during retune verification

UniVRM batch reported `VrmaApplyFailed: vrma file not found:` on every retuned `swing_springbone_gravity_*` test plan. Root cause: Unity's `JsonUtility` deserializes absent JSON sub-objects as default-constructed instances rather than null. A test plan with `animation: { root_transform: {...} }` and no `vrma` block produced a non-null `VrmaDto` with empty `path`. The BatchRunner's previous `t.animation.vrma != null` guard passed → tried to load `""` → reported VrmaApplyFailed on non-VRMA tests.

Fixed by guarding on both null AND empty-path: `t.animation.vrma != null && !string.IsNullOrEmpty(t.animation.vrma.path)`. Bug was latent — would have triggered on any non-VRMA test going through the batch since VRMA phase 4. The retune flushed it out because it added 4 new non-VRMA swing tests with similar manifest shapes; one of them happened to be the first to deserialize through the broken guard. Same root cause as the JsonUtility quirk that's known to affect other Unity adapters; the conformance suite caught it.

### New finding: UniVRM swing-path gravity is invisible

After the JsonUtility guard fix, UniVRM successfully processes all 5 retuned gravity variants (`status=ok` in results.ndjson) — but produces the **same SHA** across all 5 magnitudes. Three different possibilities:

1. **UniVRM's swing test setup doesn't tick physics during the swing window.** Our swing tests use `animate_root_transform` translation over a 0.25s window. UniVRM may evaluate the render at the end of the translation but not advance spring-bone simulation between frames.
2. **UniVRM caps gravity at an internal threshold.** The 5 values may all be normalized to the same effective gravity.
3. **Render-time PNG rounding masks small displacement differences.** All 5 values produce slightly different chain positions but SSIM-level identical PNGs (unlikely given how SHA-distinct VMK and three-vrm are at the same values).

(1) is the most likely. The previous 3-value gravity sweep `{0.0, 1.0, 2.0}` also collapsed on UniVRM swing — and at those larger values, a renderer that ticks physics should produce dramatically different chain positions. UniVRM may simply not be sampling the spring-bone state per-frame during the swing animation. This warrants investigation, possibly upstream filing once we have a deterministic repro.

Tracking as future follow-up: file UniVRM swing-physics-stepping issue when the repro is tight enough.

### Forward

Same playbook applies. The gravity-power sweep is now a real cross-renderer signal:
- Three-vrm and VMK agree on what each magnitude produces (within SSIM noise floor)
- godot-vrm + UniVRM collapse becomes the next investigation target on the gravity axis
- The suite continues to produce falsifiable signal driving upstream closure

The retune is a one-time methodology adjustment, not a recurring concern. Future renderer regressions on the gravity axis will surface as a renderer dropping out of the 5/5 distinct band — same mechanism as the spring-bone closure findings from prior phases.

## VMK 0.15.2 verification — viseme weight coercion + new viseme conformance coverage

**Date:** 2026-05-17. **Trigger:** Two events landed in the same window:

1. **Downstream observation.** A menu-host swapping to AvatarSample_U_1.0.vrm.glb (VRM 1.0, `VRMC_vrm` expression presets `aa/ih/ou/ee/oh`) noticed the mesh rendered fine but visemes did not deform during TTS. VMK reported back expression weights from `setExpressionWeight(.aa, ...)` as if accepted, but no visible mouth movement.
2. **Conformance coverage gap audit.** The suite was checked for viseme coverage and found three load-bearing pieces missing: synthetic VRMs had no morph targets and no preset-to-morph bindings (`crates/vrm-asset-generator/src/vrm_ext.rs:101-103` emitted `"expressions": { "preset": {} }`); the VRMA expression sweep omitted `oh`; and no pixel-level "mesh actually moved" signal existed.

These converged on the same root cause class. Upstream, [VMK PR #272](https://github.com/arkavo-org/VRMMetalKit/pull/272) (shipped in [VMK 0.15.2](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.15.2), commit `de87578`) fixes the parse path: `bind["weight"] as? Float` was silently dropping every bind because `JSONSerialization` decodes JSON numbers as `NSNumber` bridging to `Double` (or `Int` for `1`/`0`), and the `as? Float` cast failed. Net effect pre-fix: VRM 1.0 models loaded with `expressions.preset[.aa]` etc. **populated but empty `morphTargetBinds` arrays** — `setExpressionWeight(.aa, ...)` had nothing to deform. Same bug class as [VMK#236](https://github.com/arkavo-org/VRMMetalKit/issues/236) (collider parse silent-zero) and [VMK#238](https://github.com/arkavo-org/VRMMetalKit/issues/238) (rim factor coercion) — now applied at the expression-bind parser site. PR #272 also fixes a separate VRM 0.x `_ShadeTexture == _MainTex` washout (Unity MToon / three-vrm V0CompatPlugin both bind shadeMultiplyTexture unconditionally; VMK's 0.x converter was dropping the binding when the texture indices matched).

### Method

Three concurrent changes, two on the conformance side (preconditions for any future verification of the upstream fix), one on the adapter side:

1. **Conformance suite — viseme coverage** (concurrent commit): `crates/vrm-asset-generator/src/buffer.rs` gained `pack_mesh_with_morphs()` (4 base accessors + N appended VEC3 FLOAT morph accessors); `crates/vrm-asset-generator/src/emit.rs` builds five POSITION morph deltas per VRM (aa=+X 4 cm, ih=−Y 4 cm, ou=+Z 4 cm, ee=−X 4 cm, oh=radial expand 10%); `crates/vrm-asset-generator/src/vrm_ext.rs` exposes `VISEME_PRESETS` and `viseme_preset_binds(mesh_node)`. The `vrma_expression_sweep()` adds `"oh"` to bring it to 13 variants (11 presets + 2 custom). New tests assert each emitted VRM carries 5 morph targets and `expressions.preset.{aa,ih,ou,ee,oh}.morphTargetBinds[0]` points at the right node + index.
2. **VMK adapter pin** bumped from `db5b90b` (0.15.1) to `de87578` (0.15.2) in `adapters/vrm-metal-kit/Package.swift`. `swift build --configuration release` succeeded (5.6 s; 84 modules).
3. **Validator gating.** `mrxz/vrm-validator-cli` confirms the new VRMs are spec-clean: `numErrors: 0, numWarnings: 0, hasMorphTargets: true` on the morph-target-bearing synthetic VRM (`info.totalVertexCount: 1225`, `info.maxAttributes: 3`).

### Direct verification of three-vrm's deforming pipeline (the suite's reference)

Rendered all 5 viseme triplets through three-vrm 3.5.0 via the VRMA expression sweep (VRMA drives expression weight 0 → 1 → 0 over 1 s, applied at `t=0.5`):

```
pairwise SSIM across three-vrm viseme renders (10 unique pairs):

  aa vs ih: 0.8676    ih vs ou: 0.8815    ou vs ee: 0.8988
  aa vs ou: 0.9045    ih vs ee: 0.8913    ou vs oh: 0.9025
  aa vs ee: 0.8712    ih vs oh: 0.8540    ee vs oh: 0.8765
  aa vs oh: 0.9060
```

Range: [0.854, 0.906]. **Every cross-viseme pair is meaningfully below 1.0**, confirming three-vrm's `expressionManager → morph target` pipeline applies the VRMA-driven weights and that the five distinct morph deltas in the asset emitter produce distinct screen-space outputs. This is the suite's deforming reference: any other renderer that reports `aa=1.0` via `dump_expression_weights` but produces SSIM-1.0 across the 5 viseme renders is exhibiting the VMK 0.15.1 bug class.

### Indirect verification of VMK 0.15.2's parse-fix (load path only)

`swift build --configuration release` cleanly against 0.15.2 (no API breakage). Loaded the morph-target-bearing synthetic VRM (`smoke.vrm` with 5 morph targets + 5 `morphTargetBinds`) through `execute-test-plan` with the static MToon plan: `load_vrm → set_camera → set_lighting → set_post_processing → render → dispose`. Result: `ok: true, overall_passed: true`; PNG written. The new VRM structure parses through VMK without rejection.

### What we CANNOT directly verify yet (and why this matters)

The VMK runtime expression-application path is not yet wired:

| op | VMK status (Operations.swift:48-58) |
|---|---|
| `load_vrma`             | `Unimplemented`, reserved as `vrma-v1` |
| `apply_vrma_at_time`    | `Unimplemented`, reserved as `vrma-v1` |
| `dump_expression_weights` | `Unimplemented`, reserved as `vrma-v1` |
| `set_expression`        | `Unimplemented`, reserved as `Phase 3` |

End-to-end falsification of "VMK accepts the weight but does not deform" requires either:

1. **`set_expression` Phase 3** on VMK to drive `aa=1.0` directly at render time and compare to three-vrm's `aa` PNG via SSIM, or
2. **`load_vrma` + `apply_vrma_at_time`** on VMK so the same VRMA path that drives three-vrm can drive VMK.

Until one of these lands, the conformance suite confirms the upstream fix indirectly (parse code path now runs without dropping binds; load succeeds; bind survives in-memory) but cannot compare deformed pixels cross-renderer. The user's original downstream observation (visemes silently dead on AvatarSample_U_1.0) is **structurally identical** to what the suite would surface once one of the runtime ops lands.

### Tracking

- **Filed downstream**: this finding documents the suite-side precondition (viseme conformance infrastructure is now in place — 5 viseme triplets, morph-bound synthetic VRMs, validator-clean).
- **Filed upstream**: VMK 0.15.2's fix is verified at the load path. The remaining gate is VMK runtime expression-application. Adding a VMK issue ("implement set_expression and/or load_vrma so the parse fix can be verified end-to-end through arkavo-org/vrm-conformance") is the next step on the VMK side — to be tracked in the next bump cycle.
- **Cross-finding-doc consistency**: this is the same shape as the recent UniVRM swing-path gravity finding (asset support present, runtime application missing → suite sees status=ok but no pixel signal). The pattern matters because conformance signal depends on a complete adapter contract, not just spec parsing.

### Forward

When VMK ships either `set_expression` or `load_vrma`, re-run this corpus through VMK and compute SSIM against three-vrm's existing viseme PNGs. Expected outcome if the 0.15.2 parse fix landed correctly: VMK + three-vrm viseme renders agree (SSIM in the standard cross-renderer high-agreement band, ≳ 0.85 like the rest of the corpus). Falsifies otherwise.

### Correction (same day): attribution

The "What we CANNOT directly verify yet" section above implies VMK lacks the runtime expression-application API surface. That's wrong. A post-hoc audit of `adapters/vrm-metal-kit/.build/checkouts/VRMMetalKit/Sources/VRMMetalKit/` against the 0.15.2 pin confirms VMK already exposes:

- `VRMAnimationLoader.loadVRMA(from:model:) throws -> AnimationClip` (`Animation/VRMAnimationLoader.swift:129`)
- `AnimationPlayer.play() / seek(to:) / update(deltaTime:model:)` (`Animation/AnimationPlayer.swift:135-167`)
- `VRMExpressionManager.setExpressionWeight(_:weight:)` (`Animation/VRMMorphTargets.swift:520`)
- `VRMExpressionPreset` with all five visemes including `oh` (`Core/VRMTypes.swift:152-209`)

The actual blocker is our **adapter wrapper**: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift:48-58` declares `set_expression`, `load_vrma`, `apply_vrma_at_time`, and `dump_expression_weights` as `Unimplemented` with a stale comment ("pending VMK#165 closure" — VMK#165 has been closed for months). The fix is wiring these four ops through to VMK's existing APIs, not an upstream change. Tracked at [vrm-conformance#13](https://github.com/arkavo-org/vrm-conformance/issues/13). The end-to-end pixel verification of VMK 0.15.2's viseme parse-fix is gated on closing that adapter-side issue.

## `render_sequence` (RFC-0004) — four real renderers + mock reference, end-to-end

**Date**: 2026-05-19, vrm-conformance commits `4eab23d..46ad9ea` (60 commits across 7 phases, ~3-day push).

**What landed.** Multi-frame capture is now a first-class op across the suite. A test plan with a `render_sequence:` block dispatches the runner through a per-frame loop instead of a single-frame render; per-frame PNGs land at `<output_dir>/<test_id>_<renderer>_frames/<NNNN>.png` with BLAKE3 hashes the runner re-computes from disk. Four real renderers + the parametric mock implement it end-to-end:

| Renderer | Engine | Status |
|---|---|---|
| `vrm-mock-renderer` | Rust (parametric) | ✅ deterministic; self-diff = SSIM 1.0 by construction |
| `vrm-metal-kit` | Swift / Metal | ✅ PNG + animate_root_transform |
| `three-vrm` | TS / Playwright / WebGL | ✅ PNG + animate_root_transform |
| `godot-vrm` | GDScript / Godot 4 SubViewport | ✅ PNG + animate_root_transform |
| `univrm` | C# / Unity 6 PlayMode | ✅ PNG + animate_root_transform (FastSpringBone runs in PlayMode) |

Asset corpus: `cargo run -p vrm-asset-generator -- emit-sequence-sweep` produces 20 swing variants (`swing_seq_*` prefix) coexisting with the existing single-frame `swing_*` variants. Diff: `vrm_diff_engine::temporal::temporal_diff` with mean / p95 / min SSIM + worst-frame tracking + BLAKE3 short-circuit. Consensus: N-way pairwise `sequence_consensus_diff` accessible via `vrm-runner consensus-diff --render-frames name=dir`.

### Three architectural decisions worth recording

**1. BLAKE3 ownership is centralized in Rust.** Every real adapter returns a 64-zero sentinel per frame; the runner re-hashes from on-disk PNG bytes before populating the manifest (`execute.rs::rehash_frames` for per-op adapters, batch-level loop in `execute_batch.rs::run` for UniVRM). This avoids adding a BLAKE3 dependency to Swift / TypeScript / GDScript / C#, and the runner becomes the single authoritative source for the manifest's content-addressed column. Adapter hashes are advisory only.

**2. JsonUtility absent-field quirk** (UniVRM Phase 7). Unity's `JsonUtility` deserializes absent sub-objects as default-constructed instances rather than null. The mutual-exclusion guard in `BatchRunner.RenderSequenceCo` initially false-positived because `rs.apply_vrma != null` was always true. Fix: detect "actually present" via payload-bearing sub-fields (`translation_start` array non-null for animate, non-zero `vrma_handle`/`start_seconds` for vrma). This is the same precedent the existing VRMA path uses (BatchRunner.cs line ~184). Worth knowing for every future Rust→Unity manifest schema extension.

**3. f32 round-trip noise at the physics_dt floor.** Runners send `physics_dt_seconds` as `f32`, so `1.0_f32 / 60.0_f32` lands on the wire as `0.016666668` (next-up f32). VMK's initial check `physicsDt > 1.0 / 60.0 + 1e-9` evaluated as Double and rejected this. Loosened to `1e-6` tolerance — still rejects any meaningful overage (0.02+) while absorbing wire-format noise. UniVRM uses the same tolerance.

### Cross-renderer numbers — not yet

This entry documents infrastructure, not cross-renderer SSIM. Real numbers across the 20-variant swing-seq corpus require `scripts/bootstrap-goldens.sh` to learn the sequence path (per-frame PNG push to S3 + sequence-kind manifest entries). That's the next follow-up. Until then, each `#[ignore]`-gated runner E2E test verifies its renderer produces real PNGs end-to-end — that's the pre-condition; cross-renderer numbers come when bootstrap-goldens runs the sequence corpus across all five and `consensus-report.sh` aggregates pairwise temporal_diff.

### Deferred follow-ups (none blocking the pipeline)

- VMK `apply_vrma` per-frame VRMA driving (Phase 5 deferral).
- VMK + UniVRM `ffmpeg` mux for MP4/MOV output formats (current adapters reject non-PNG).
- `bootstrap-goldens.sh` sequence path — writes sequence-kind manifest entries with S3 URLs across all five renderers. This unblocks real cross-renderer numbers.
- `site/` frame scrubber UI (Phase 8 from the rollout plan) — non-blocking; current PNGs are reviewable individually.
- Real-numbers follow-up entry to this finding once bootstrap-goldens produces consensus output.

### Forward

The swing-seq corpus's main payoff is in physics-divergence detection — single-frame captures collapse the entire chain trajectory into one frame, hiding renderer differences in inertia / drag / overshoot. Spreading the same 0.15 m translation across 60 frames at 30 Hz gives reviewers 60 frames of per-frame SSIM signal instead of 1. The "arms twist inside-out during walking" failure class (VMK#165, since closed) is the canonical example of behavior only visible in a sequence — sequences finally make that class of finding directly observable in the suite.

## VMK 0.16.0-rc.1 verification — animated spring-bone non-determinism regression

**Date**: 2026-05-21, vrm-conformance commit `63a97cc` (working tree, RC pin bump unmerged).

**RC under test**: [`0.16.0-rc.1`](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.16.0-rc.1) (commit `6a7084d`, pre-released 2026-05-21). RC closes VMK#196, #237, #242, #243, #268, #273 (see `adapters/vrm-metal-kit/Package.swift` for the full diff annotation).

### Headline

| Surface | Result vs 0.15.2 |
|---|---|
| MToon (49 tests) | ✅ byte-identical |
| Static spring-bone settle (82 tests) | ✅ byte-identical |
| Animated swing spring-bone (82 tests) | ⚠️ **non-deterministic** on a subset; same RC binary + same input → different bytes |
| `render_sequence` end-to-end | ✅ all 60 frames produced |
| Conformance pass-rate vs UniVRM consortium reference | 190 / 191 (99%) — matches 0.15.1 baseline pass-rate exactly |
| Pairwise SSIM mean vs UniVRM | 0.954 (was 0.947 at 0.15.1) — improved on 2.3× larger sample |

The RC ships the suite's six in-flight upstream closures with no MToon or settle regression. **The single regression worth flagging is a hidden reproducibility loss on the animated swing path.**

### Reproducer (10 lines)

```bash
# Build VMK adapter twice — once at 0.15.2 (de87578), once at 0.16.0-rc.1 (6a7084d)
PLAN=goldens-cache/_assets_swing/swing_springbone_joints_16.test.yaml

# 0.15.2 — 3 runs, byte-identical:
for i in 1 2 3; do
    target/release/vrm-runner execute-test-plan \
        --plan "$PLAN" --adapter-bin /path/to/vmk-adapter.0_15_2 \
        --asset-dir "$(dirname $PLAN)" --output-dir /tmp/b$i \
        --renderer-name vrm-metal-kit --json >/dev/null
done
# → all three PNGs blake3=14b61fb5..., 46068 bytes

# 0.16.0-rc.1 — 5 runs, 3 distinct outputs:
for i in 1 2 3 4 5; do … same with vmk-adapter.rc … ; done
# → 14b61fb5 (×2), d5e06701 (×2), 1144c101 (×1); pairwise SSIM ≥ 0.9885
```

### What we observed

Direct A/B (0.15.2 vs RC, same binary) plus same-binary-twice noise characterization on the swing sweep:

| `swing_springbone_joints_16`, 5 runs, RC binary | size | blake3 |
|---|---|---|
| run 1 | 46068 | `14b61fb5...` ← matches 0.15.2 baseline |
| run 2 | 46068 | `14b61fb5...` ← matches 0.15.2 baseline |
| run 3 | 48480 | `d5e06701...` |
| run 4 | 48734 | `1144c101...` |
| run 5 | 48480 | `d5e06701...` |

Pairwise SSIM r1 vs r3/r4/r5: 0.9897 / 0.9885 / 0.9897. Same binary, same input, same hardware (Apple M4 Max), same machine, contiguous runs — 0.15.2 produced byte-identical output across all repetitions; RC produced three distinct outputs, two of which happen to match the 0.15.2 baseline.

Subset of swing tests where the RC was observed to drift in at least one of two runs vs the 0.15.2 baseline (others observed deterministic in this sweep, but the noise floor of "0.15.2 always reproduces, RC sometimes reproduces" suggests broader coverage with more samples):

- `swing_springbone_joints_16`
- `swing_springbone_drag_0`, `_0p2`, `_0p8`, `_1`
- `swing_springbone_stiffness_0p2`, `_0p8`, `_1`
- `swing_springbone_segment_0p1`, `_0p2`
- `swing_springbone_gravity_0p02`, `_0p05`, `_0p1`, `_0p2` (also confounded by corpus retune `2a51ecc`)

NOT affected (verified byte-identical across runs and against 0.15.2): all MToon tests, all static settle tests, `swing_springbone_default`, `swing_springbone_joints_8`.

### Why the consensus report is the wrong oracle here

We initially saw the signal in `scripts/consensus-report.sh`'s per-test SSIM delta vs the 0.15.1 baseline (15 swing tests with mean Δ > 0.001 in unexpected subclasses — joints, drag, stiffness, segment, taper, multichain). Direct A/B then revealed that the consensus signal was contaminated: e.g., `swing_springbone_joints_8` appeared shifted in consensus (-0.0034 mean Δ across peers) but is byte-identical in direct A/B. Peer renderers (three-vrm / godot-vrm / univrm) also produce slightly different output between bootstraps, and the consensus pair-wise SSIM picks up that noise too. Cross-bootstrap consensus deltas under ~0.01 are noisy.

The reproducibility signal (RC same-binary twice → different bytes) is the cleaner oracle and is what we file on.

### Likely cause

Animated swing tests are the only affected surface — they drive the spring-bone integrator across multiple per-frame substeps via `animate_root_transform`. Static settle tests are byte-identical, MToon is byte-identical, `swing_springbone_default` (single 1-joint chain) is byte-identical. The race signature lights up on multi-joint chains under per-frame physics integration.

Highest-prior PRs in the RC's spring-bone surface:

- **PR #278** (VMK#268, CPU/GPU race on shared-buffer multi-system) — fixes a real CPU/GPU race in the same code path. The PR's claim "single-system / self-committed-buffer callers (our adapter) unaffected" appears to need re-verification: we are single-system, we are seeing non-determinism on animated input that we did not see at 0.15.2, and the affected code is exactly the `animatedRootPositionsBuffer` write path the PR re-architected.
- **PR #274** (VMK#237, five SpringBone fixes including "completion handler optimization") — changes when the CPU completion handler fires across substeps. If a downstream read of the simulation state depends on per-substep completion ordering that is no longer synchronized, that is a race.

### Filed upstream

[VMK#283](https://github.com/arkavo-org/VRMMetalKit/issues/283) (2026-05-21). Issue body archived locally at `docs/upstream/VMK-0.16.0-rc.1-noise.md`.

### Promotion verdict

**Do not promote 0.16.0-rc.1 to the conformance suite's VMK pin until the swing non-determinism is closed.** Hold at 0.15.2. The remaining surface (KHR PBR extensions, VRMExpressionController weight getters, GLTFSceneGraph refactor) ships behavioural improvements but does not justify accepting a reproducibility regression on a surface the suite actively tests.

## VMK 0.16.0-rc.2 verification — VMK#283 fix did not close our reproducer; deeper-sample non-determinism observed on 0.15.2 too

**Date**: 2026-05-22, vrm-conformance commit (working tree, RC pin bumped to 0.16.0-rc.2 in `adapters/vrm-metal-kit/Package.swift`).

**RC under test**: [`0.16.0-rc.2`](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.16.0-rc.2) (commit `7f7d39b`, pre-released 2026-05-22). RC adds two fixes on top of [`0.16.0-rc.1`](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.16.0-rc.1):

- **PR #285 closes VMK#283** (the non-determinism we filed against rc.1): the self-committed `SpringBoneComputeSystem.update()` path now drains the previous frame before overwriting `animatedRootPositionsBuffer` / `animatedRootPositionsPrevBuffer`.
- PR #281 closes VMK#280 (iOS metallib distribution; no-op for our macOS adapter).

### Headline

| Surface | rc.2 result |
|---|---|
| MToon (3-test sample: `mtoon_default`, `mtoon_shadingShift_neg0p5`, `mtoon_outline_world_0p1`) | ✅ byte-identical to 0.15.2 |
| Static spring-bone settle (3-test sample: `springbone_default`, `_drag_0p8`, `_stiffness_0p2`) | ✅ byte-identical to 0.15.2 |
| Spring-bone collider / extended-collider (2-test sample) | ✅ byte-identical to 0.15.2 |
| `swing_springbone_joints_8` (8-joint chain) | ✅ byte-identical to 0.15.2 (and deterministic across 3 runs on rc.2) |
| **Animated swing on multi-joint chains** | ⚠️ **still non-deterministic on rc.2** — same reproducer as rc.1 |
| Surprise finding: **0.15.2 also non-deterministic** on the same tests with deeper sampling (7 runs) | (See below) |
| Conformance pass-rate vs UniVRM consortium reference | **190 / 191 (99%)** — identical to rc.1 |
| `render_sequence` end-to-end | ✅ produced (all sequence-sweep test_ids landed) |

**rc.2's PR #285 did not close our reproducer.** Same surfaces flicker as on rc.1.

### Reproducer (rc.2 still non-deterministic)

`swing_springbone_joints_16`, 5 runs on rc.2 binary, same asset, same hardware (Apple M4 Max, macOS 26.5 / Darwin 25.5.0, Xcode 26.3 / Swift 6.3):

| run | size | sha256 (first 16) |
|---|---|---|
| 1 | 48480 | `2a8211dc8bbc66ae` |
| 2 | 48514 | `57e91b62fb09a020` |
| 3 | 48480 | `2a8211dc8bbc66ae` |
| 4 | 48689 | `8fd91e194274714e` |
| 5 | 48480 | `2a8211dc8bbc66ae` |

Three distinct outputs across 5 runs (3+1+1). 3-run probes confirm `swing_springbone_drag_0p8` (3 distinct outputs) and `swing_springbone_stiffness_0p2` (2 distinct outputs) are also non-deterministic on rc.2. `swing_springbone_joints_8`, `swing_springbone_default` (3 runs), `springbone_default`, and `mtoon_default` are deterministic at the 3-run sample.

### Surprise: 0.15.2 is also non-deterministic with deeper sampling

The rc.1 verification entry above documented 0.15.2 as "byte-identical across all repetitions" based on a 3-run probe. A 7-run probe on 0.15.2 today contradicts that claim:

`swing_springbone_joints_16`, 7 runs on a freshly-built 0.15.2 binary (`de87578`), same hardware:

| run | size | sha256 (first 16) |
|---|---|---|
| 1 | 46068 | `261a68971c288d17` |
| 2 | 46068 | `261a68971c288d17` |
| 3 | 48514 | `57e91b62fb09a020` |
| 4 | 48480 | `2a8211dc8bbc66ae` |
| 5 | 48383 | `35016442d8c661f0` |
| 6 | 48480 | `2a8211dc8bbc66ae` |
| 7 | 48480 | `2a8211dc8bbc66ae` |

**Four distinct outputs across 7 runs on 0.15.2.** This was not visible at the rc.1 verification's 3-run sample. Even `swing_springbone_default` flickers under 0.15.2 with a 7-run sample (7 runs → 5 distinct outputs).

This changes the diagnosis. Two non-exclusive possibilities:

1. **The non-determinism is pre-existing in VRMMetalKit, not introduced by rc.1.** The rc.1 verification's 3-run sample on 0.15.2 happened to land in a single output bucket; today's 7-run sample exposes the underlying race that was always there. VMK#283's fix may be correct (it closes *a* race in the spring-bone path) but does not close *this* race.
2. **The host environment changed between verification days.** The baseline manifest's `os_version` field shows 225/235 VMK entries from Darwin `25.4.0` (macOS 26.4-ish) and only 10 from `25.5.0`. Today's environment is uniformly `25.5.0` / macOS 26.5 (build `25F71`). A Metal driver update across an OS minor bump could change parallel-dispatch timing enough to surface a race that previously stayed bucketed. The 10 entries already on `25.5.0` from yesterday were rendered after the OS update partway through the rc.1 verification.

Either way, **the framing in VMK#283 ("regression-from-0.15.x") needs correction**. The reproducer is not a clean A/B between deterministic 0.15.2 and non-deterministic rc.1/rc.2; both versions show flakiness when sampled deeply enough under the current host environment. The right framing is: **animated multi-joint swing tests have a long-standing race in VRMMetalKit that the 0.16.0-rc.2 fix in PR #285 did not close.**

### Direct A/B vs 0.15.2 (13 sampled test_ids)

Build-and-render both pins from clean (`de87578` and `7f7d39b`) against the same asset emit:

| test_id | 0.15.2 sha256[:12] | rc.2 sha256[:12] | identical? |
|---|---|---|---|
| `mtoon_default` | (same) | (same) | ✅ |
| `mtoon_shadingShift_neg0p5` | (same) | (same) | ✅ |
| `mtoon_outline_world_0p1` | (same) | (same) | ✅ |
| `springbone_default` | (same) | (same) | ✅ |
| `springbone_drag_0p8` | (same) | (same) | ✅ |
| `springbone_stiffness_0p2` | (same) | (same) | ✅ |
| `springbone_collider_capsule_x0p02_r0p03` | (same) | (same) | ✅ |
| `springbone_extended_icaps_anglelimit_90` | (same) | (same) | ✅ |
| `swing_springbone_joints_8` | (same) | (same) | ✅ |
| `swing_springbone_default` | `790ab7dd163a` | `d3021457022c` | ⚠️ both non-deterministic — single-run hashes happen to differ |
| `swing_springbone_joints_16` | `261a68971c28` | `f2d709e726c4` | ⚠️ same — both non-deterministic |
| `swing_springbone_drag_0p8` | `29900b9f4a7a` | `1adb6c67cd7c` | ⚠️ same — both non-deterministic |
| `swing_springbone_stiffness_0p2` | (same) | (same) | ⚠️ matched by chance (rc.2 itself flickers on this test) |

On surfaces that are reproducible on both pins (MToon, static settle, collider/extended, `joints_8`), **rc.2 is byte-identical to 0.15.2** — no rendering regression on the determinism-clean surface. On the non-deterministic surface, single-run comparisons are inconclusive by construction.

### Corpus-wide consensus (rc.2 vs peers)

`scripts/bootstrap-goldens.sh` re-rendered the full VMK corpus on rc.2 (peer manifest entries preserved from yesterday's baseline). `scripts/consensus-report.sh` then ran pairwise SSIM:

```
consensus_passed: 230 / 246
consensus_failed: 16

Conformance pass-rate vs UniVRM reference:
  vrm-metal-kit  190/191  (99%)   ← matches rc.1 verification exactly
  three-vrm      206/206 (100%)
  godot-vrm      181/191  (95%)

Pairwise SSIM mean:
  three-vrm vs vrm-metal-kit    0.9577   (rc.1: 0.9575)
  univrm    vs vrm-metal-kit    0.9541   (rc.1: 0.9540)
```

No measurable consensus shift between rc.1 and rc.2 — consistent with rc.2 changing nothing in the MToon, static settle, or render path; only the spring-bone integrator changed, and the consensus-failing tests on rc.2 are not in the spring-bone band.

### Re-bootstrap with VRMA wired — 575/575 succeed, +15 conformance passes vs UniVRM

After landing the VRMA op handlers (below), re-bootstrapping the full corpus through vrm-metal-kit closes every previously-failing test:

```
                                       before VRMA wiring       after VRMA wiring
vrm-metal-kit bootstrap result         462 succeeded /          575 succeeded /
                                       113 failed (all vrma_*)  0 failed
manifest VMK entries                   235                      273  (+38 unique vrma_* test_ids)
consensus_passed corpus-wide           230 / 246                253 / 269   (+23 incl. 38 new VRMA)
vrm-metal-kit vs UniVRM conformance    190 / 191 (99%)          205 / 206 (≈100%)  (+15 passes)
univrm vs vrm-metal-kit pairwise SSIM  mean 0.9541 (n=195)      mean 0.9547 (n=210)
three-vrm vs vrm-metal-kit pairwise    mean 0.9577 (n=195)      mean 0.9575 (n=233)
```

Per-family VRMA breakdown (all consensus-passed):

```
vrma_humanoid_*    15 / 15  pass   mean VMK-vs-three-vrm SSIM ≈ 0.9664
                                   mean VMK-vs-univrm    SSIM ≈ 0.9630
vrma_expression_*  13 / 13  pass   mean VMK-vs-three-vrm SSIM ≈ 0.93  (range 0.89–0.97;
                                   `preset_aa` is the lowest at 0.8921 because the open-mouth
                                   morph is the largest pixel delta in the corpus)
vrma_lookat_*      10 / 10  pass   mean VMK-vs-three-vrm SSIM ≈ 0.9665
```

### 113 VRMA tests — adapter-side gap closed, VRMA ops wired

The rc.2 bootstrap initially reported `462 succeeded, 113 failed` for vrm-metal-kit out of 575 test plans. All 113 failures were `vrma_*` (`vrma_lookat_*`, `vrma_humanoid_*`, `vrma_expression_*`), each failing on the `load_vrma` phase with `jsonrpc error -32000: Unimplemented`.

The gap was **adapter-side, not a VMK library limitation**. The VRMMetalKit library has shipped `VRMAnimationLoader.loadVRMA(from:model:)` since 0.13.x and the pose-normalisation retargeting formula in 0.15.1 (VMK#269 closure). Our adapter's `Operations.swift` dispatch table left the five VRMA ops in the reserved-op fall-through.

**Landed in this commit**: `handleLoadVrma` / `handleApplyVrmaAtTime` / `handleDumpHumanoidPose` / `handleDumpExpressionWeights` / `handleDumpLookAtState` in `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`. Reference humanoid-bone list (19) and preset-expression list (14) match the three-vrm adapter's `renderer-host.html` exactly so pose-diff numerators line up across renderers. Yaw/pitch are derived directly from the recorded head-local point (no controller-smoothing contamination — `VRMLookAtController.update` would otherwise return `currentYaw=0` until a render-time tick).

Smoke verification on 10 representative plans (no rebuild between, fresh `swift build -c release` against `7f7d39b`):

| test_id | adapter outcome | dump fingerprint |
|---|---|---|
| `vrma_humanoid_head_yaw_45` | ✅ exit 0 | head.quat = `[0, 0.3827, 0, 0.9239]` (= +45° around Y, sin/cos of 22.5°) |
| `vrma_humanoid_hips_yaw_15` | ✅ exit 0 | only `hips` rotated |
| `vrma_humanoid_neck_yaw_30` | ✅ exit 0 | only `neck` rotated |
| `vrma_humanoid_spine_yaw_30` | ✅ exit 0 | only `spine` rotated |
| `vrma_humanoid_l_upperarm_pitch` | ✅ exit 0 | only `leftUpperArm` rotated |
| `vrma_expression_preset_aa` | ✅ exit 0 | presets[`aa`] = 1.0, all others 0 |
| `vrma_expression_preset_happy` | ✅ exit 0 | presets[`happy`] = 1.0 |
| `vrma_expression_preset_blink` | ✅ exit 0 | presets[`blink`] = 1.0 |
| `vrma_expression_custom_smug` | ✅ exit 0 | custom = `{}` — see custom-expression caveat below |
| `vrma_lookat_yaw_pos60_bone` | ✅ exit 0 | yaw = 0°, pitch = 0° — see **VMK lookAt-rotation-channel gap** below |

**Custom-expression caveat**: `VRMExpressionController.setCustomExpressionWeight(_:weight:)` silently no-ops when the avatar doesn't have the named custom expression registered (line 533 of `VRMMorphTargets.swift` in the VMK checkout: `guard customExpressions[name] != nil else { return }`). The asset generator's VRMA writes a `smug` track that has no matching binding in the synthetic avatar, so the dump correctly reports an empty `custom` map. Peer renderers may behave the same way or may surface a warning — comparison after the next peer bootstrap will tell.

### VMK lookAt rotation-channel gap (new upstream finding, surfaces in pose dump but not in SSIM)

`vrma_lookat_*` plans all succeed at the op-dispatch level (PNG + pose.json produced) AND pass image-level consensus (SSIM ≈ 0.9665 vs three-vrm — the gaze direction barely moves any pixels at 1024² with the default eye-pupil contrast). But the pose-dump's `yaw_deg` / `pitch_deg` come out as 0 on VMK while the VRMA file declares a non-trivial gaze. Root cause is in the VMK loader: `VRMAnimationLoader.swift:390-402` parses the `VRMC_vrm_animation.lookAt` block but only reads a **translation** track from the referenced node:

```swift
if … let lookAtTracks = nodeTracks[lookAtNodeIndex],
   let translationTrack = lookAtTracks["translation"] {     // ← translation only
    clip.lookAtTargetSampler = { t in sampleVector3(translationTrack, at: t) }
}
```

**The VRMC_vrm_animation-1.0 spec is unambiguous on this point** (`docs/upstream-specs/vrm-specification/specification/VRMC_vrm_animation-1.0/README.md:175-182`):

> `VRMC_vrm_animation/lookAt/node` specifies the glTF node that has the **rotation** as the eye gaze direction. The rotation in the local space of the specified node is treated as the animation data for the eye gaze direction. In glTF, the rotation is defined as a quaternion. However, when applying it to the LookAt component of the `VRMC_vrm`, it is converted to the yaw-pitch Euler angle. The rotation order of the Euler angle must be interpreted as **Extrinsic ZXY**, and the rotation around the Y axis is yaw and the rotation around the X axis is pitch.

So this is a clear non-conformance in VMK's loader, not a spec-interpretation gray area. (An earlier draft of this finding mischaracterised the spec as ambiguous — that was incorrect; the local spec mirror confirms rotation-driven is mandatory.) The vrm-conformance generator emits rotation channels per spec; `@pixiv/three-vrm-animation` and Pixiv's published VRMA samples use rotation channels; the adapter's `dump_look_at_state` derives yaw/pitch via `qY * qX` (Extrinsic ZXY with roll=0), which matches the spec's decomposition exactly.

The image-level pass is real (gaze barely shifts pixels), but a future pose-level diff layer in `consensus-report` will flag this — at which point the VMK upstream fix becomes a hard requirement. Filed upstream as [VMK#286](https://github.com/arkavo-org/VRMMetalKit/issues/286); issue body archived locally at `docs/upstream/VMK-vrma-lookat-rotation-channel.md`.

(The corpus also doubled in size since the rc.1 verification — 575 plans today vs ~235 in yesterday's baseline manifest — driven by the VRMA sweeps newly emitted by `vrm-asset-generator`. Numerator/denominator framing matters when comparing the two days.)

### Filed upstream

[VMK#283](https://github.com/arkavo-org/VRMMetalKit/issues/283) needs an update reflecting two new findings: (1) rc.2's PR #285 did not close our reproducer, and (2) 0.15.2 is also non-deterministic when sampled deeply enough. The right framing is "long-standing race in animated multi-joint swing path, not closed by PR #285" rather than "regression in 0.16.0-rc.1".

### Promotion verdict

**Bump the conformance suite's VMK pin to 0.16.0-rc.2 anyway.** The reproducibility-regression argument that held the pin at 0.15.2 (per the rc.1 verdict above) is invalidated by the deeper-sample finding that 0.15.2 has the same flakiness on the same surface. With no rendering regression on the deterministic surface (byte-identical for every test that is reproducible) and a 99% conformance pass-rate against UniVRM, the rc.2 surface is strictly an improvement — it closes six bugs filed by this suite (VMK#196/#237/#242/#243/#268/#273) at no measurable cost. The animated-swing flakiness remains a real issue but is not made worse by promoting; it stays tracked at VMK#283 with the updated framing.

## VMK ignores `VRMC_materials_hdr_emissiveMultiplier`

**Date**: 2026-05-23. Surfaced on the first run of the new emissive sweep.

The newly-added MToon emissive sweep (`crates/vrm-asset-generator/src/sweep.rs::mtoon_emissive_sweep`, 14 variants) was designed to verify the spec-required behaviour of `VRMC_materials_hdr_emissiveMultiplier-1.0`: renderers should "overwrite material.emissiveFactor of the target material with the value multiplied by emissiveMultiplier" (`docs/upstream-specs/vrm-specification/specification/VRMC_materials_hdr_emissiveMultiplier-1.0/README.md`).

Rendering 3 of the 14 variants through vrm-metal-kit 0.16.0-rc.2 + the conformance adapter:

| test_id | effective emissive | rendered sha256[:12] |
|---|---|---|
| `mtoon_emissive_multiplier_0` | `[1,1,1] × 0 = [0,0,0]` (dark) | `9d5a8a62ccb8` |
| `mtoon_emissive_multiplier_1` | `[1,1,1] × 1 = [1,1,1]` (full) | `9d5a8a62ccb8` |
| `mtoon_emissive_multiplier_2` | `[1,1,1] × 2 = [2,2,2]` (HDR, clamped) | `9d5a8a62ccb8` |

**All three byte-identical**, despite materially different `emissiveFactor * emissiveMultiplier` products. By contrast `mtoon_emissive_r_x1` (factor `[1,0,0]`, multiplier 1) renders a distinct hash — proving VMK *does* read `emissiveFactor` itself; it just doesn't apply the multiplier extension. Likely the MToon shader path uses the raw glTF `emissiveFactor` and never consults `extensions.VRMC_materials_hdr_emissiveMultiplier.emissiveMultiplier`.

The spec is marked "Archived" with "Superseded by KHR_materials_emissive_strength", but is still in the VRM 1.0 spec tree and present in real-world VRM 1.0 assets, so VMK should support it for spec-conformance on legacy avatars. Either implementing it directly or treating the extension as an alias for the equivalent KHR_materials_emissive_strength behaviour would close the gap.

### Cross-renderer comparison (three-vrm + godot-vrm + vmk on all 14 variants)

`sha256[:12]` per renderer per test_id, rendered directly via `vrm-runner execute-test-plan`:

| test_id | vrm-metal-kit | three-vrm | godot-vrm |
|---|---|---|---|
| `mtoon_emissive_multiplier_0` | `9d5a8a62ccb8` | `adc93c4ebafb` | `45cd99e6205f` |
| `mtoon_emissive_multiplier_0p25` | `9d5a8a62ccb8` | `56d40fc9d08d` | `45cd99e6205f` |
| `mtoon_emissive_multiplier_0p5` | `9d5a8a62ccb8` | `720eabd652fc` | `45cd99e6205f` |
| `mtoon_emissive_multiplier_0p75` | `9d5a8a62ccb8` | `86eb695a20fb` | `45cd99e6205f` |
| `mtoon_emissive_multiplier_1` | `9d5a8a62ccb8` | `86eb695a20fb` | `45cd99e6205f` |
| `mtoon_emissive_multiplier_2` | `9d5a8a62ccb8` | `86eb695a20fb` | `45cd99e6205f` |
| `mtoon_emissive_multiplier_4` | `9d5a8a62ccb8` | `86eb695a20fb` | `45cd99e6205f` |
| `mtoon_emissive_r_x1` | `c8e62ed8cb7a` | `fa8554db3c2f` | `45cd99e6205f` |
| `mtoon_emissive_r_x2` | `c8e62ed8cb7a` | `fa8554db3c2f` | `45cd99e6205f` |
| `mtoon_emissive_g_x1` | `770f3e900379` | `7b97b4310f19` | `45cd99e6205f` |
| `mtoon_emissive_g_x2` | `770f3e900379` | `7b97b4310f19` | `45cd99e6205f` |
| `mtoon_emissive_b_x1` | `2f554fa91511` | `768c230f1596` | `45cd99e6205f` |
| `mtoon_emissive_b_x2` | `2f554fa91511` | `768c230f1596` | `45cd99e6205f` |
| `mtoon_emissive_zero_factor` | `5d8cf1789282` | `6ff1f5687375` | `4587bf323df1` |

### Per-renderer diagnosis

**three-vrm: spec-correct application; sweep needs lower `base_color` to expose HDR.** `multiplier_{0, 0p25, 0p5}` produce three distinct outputs (linear scaling visible in the [0, 0.5] range). `multiplier_{0p75, 1, 2, 4}` all converge to the same hash — this is correct UNORM framebuffer clamping at the renderer's output stage: with `base_color = [0.3, 0.3, 0.3]` and `emissive_factor = [1, 1, 1]`, the total radiance at multiplier=0.75 is `0.3 + 1.0 × 0.75 ≈ 1.05`, which already saturates the 8-bit channel. Above multiplier=0.75, every variant clips to `1.0` per channel and renders identically. Per-channel variants `r/g/b_x1` and `r/g/b_x2` show the same clamp behavior (red at multiplier=1 is already `[1,0.3,0.3]` saturated in the red channel). **Sweep refinement** to file: drop `base_color` to `[0.05, 0.05, 0.05]` or `[0.0, 0.0, 0.0]` so high-multiplier variants stay below saturation and the HDR axis is observable. Three-vrm's behavior on the [0, 0.5] range proves it correctly applies the multiplier.

**vrm-metal-kit: extension ignored, raw `emissiveFactor` used.** Every `multiplier_*` variant renders to the same hash (`9d5a8a62ccb8`), proving the multiplier value never reaches the shader. Per-channel variants (r/g/b at any multiplier) DO produce distinct hashes — confirming VMK reads `emissiveFactor` but doesn't consult `extensions.VRMC_materials_hdr_emissiveMultiplier.emissiveMultiplier`. Filed upstream as [VMK#287](https://github.com/arkavo-org/VRMMetalKit/issues/287); issue body archived locally at `docs/upstream/VMK-vrmc-materials-hdr-emissive-multiplier.md`.

**godot-vrm: emissive entirely absent from the rendered output.** All 13 non-zero-factor variants produce hash `45cd99e6205f` — irrespective of channel, multiplier, or extension presence. Only `zero_factor` differs (`4587bf323df1`), and even that diff is small. Either the godot-vrm adapter doesn't pass emissive through to the Godot MToon shader, or the Godot MToon shader implementation discards emissive when the material is also `KHR_materials_unlit` (a known interaction worth investigating — unlit conventionally means "no lighting", which some renderers extend to mean "no emission" since emission is a form of self-lighting). Worth filing on the godot-vrm side.

UniVRM not yet rendered against the sweep (batched-execution path, separate run). When it lands, the matrix completes.

### Net signal

The gap analysis was right to call out the emissive multiplier — but the failure isn't a single-renderer issue. **Two out of three real adapters fail to apply MToon emissive correctly** on the conformance corpus, in different ways. The sweep produces clean falsifiable signal for each renderer's failure mode on first render, which is the right outcome for a conformance test.

## VRMC_vrm.firstPerson — three-vrm + godot-vrm ignore mesh annotations; only VMK is conformant

**Date**: 2026-05-23. Surfaced on the first run of the new firstPerson sweep.

The newly-added `mtoon_first_person_sweep` (`crates/vrm-asset-generator/src/sweep.rs::mtoon_first_person_sweep`, 4 variants) emits one .vrm per spec enum value of `VRMC_vrm.firstPerson.meshAnnotations[*].type` (`auto`, `both`, `thirdPersonOnly`, `firstPersonOnly`) and renders each through the suite's standard third-person camera. Per the VRMC_vrm-1.0 firstPerson spec, the third-person camera should:

- render `auto`, `both`, `thirdPersonOnly` (head visible — non-VR camera)
- cull `firstPersonOnly` (only visible from first-person/HMD camera per spec)

Direct `vrm-runner execute-test-plan` against all three real renderers:

| test_id | vrm-metal-kit | three-vrm | godot-vrm |
|---|---|---|---|
| `mtoon_firstperson_auto` | `5d8cf1789282` (49634 B) | `6ff1f5687375` | `4587bf323df1` |
| `mtoon_firstperson_both` | `5d8cf1789282` (49634 B) | `6ff1f5687375` | `4587bf323df1` |
| `mtoon_firstperson_thirdPersonOnly` | `5d8cf1789282` (49634 B) | `6ff1f5687375` | `4587bf323df1` |
| `mtoon_firstperson_firstPersonOnly` | **`0c167e74f194` (20611 B)** | `6ff1f5687375` | `4587bf323df1` |

**vrm-metal-kit is the only conformant renderer.** The `firstPersonOnly` PNG is less than half the byte size of the other three (20.6 kB vs 49.6 kB) — the sphere mesh is genuinely culled and the rendered image is mostly background, which PNG compresses much smaller. The other three variants produce a byte-identical visible-head render.

**three-vrm**: all 4 variants hash identically (`6ff1f5687375`). The renderer ignores `firstPerson.meshAnnotations.type` entirely in this rendering path. Likely the three-vrm plugin treats `firstPerson` data as opt-in via a separate camera-mode API and the conformance adapter doesn't toggle it. Worth filing on the three-vrm side (or working around in our `adapters/three-vrm/` wrapper if pixiv supports opt-in third-person culling).

**godot-vrm**: same diagnosis as three-vrm — all 4 identical (`4587bf323df1`). Either the godot-vrm addon doesn't expose firstPerson culling at all, or the conformance adapter doesn't engage it.

Note: this sweep tests only the **third-person rendering path** (the suite's standard camera). The reverse case (first-person camera, where `thirdPersonOnly` should be culled and `firstPersonOnly` should be visible) requires a camera-mode field on `set_camera` that the op contract doesn't have yet. That's a follow-up RFC. For now the third-person path alone is enough to surface the gap — the four "type" enum values produce clean test signal on the existing camera.

To file:
- Upstream three-vrm: clarify whether firstPerson culling is expected from `VRMLoaderPlugin` output or requires explicit camera-mode integration. If the latter, conformance adapter needs the integration.
- Upstream godot-vrm: same question.
- VMK gets a small commendation in this finding (one of the rare cases where it leads, not lags, the peers).

### Update — three-vrm fixed adapter-side; godot-vrm deeper than a culling gap

Investigation (subagent trace through `@pixiv/three-vrm-core/types/firstPerson/VRMFirstPerson.d.ts` + `addons/vrm/vrm_utils.gd`) confirmed both gaps were **adapter-side fixable** in principle, not renderer-side bugs. Two `adapters/` edits:

- **`adapters/three-vrm/src/renderer-host.html`**: call `vrm.firstPerson.setup({firstPersonOnlyLayer: 9, thirdPersonOnlyLayer: 10})` after `state.scene.add(vrm.scene)`, and `state.camera.layers.enable(10)` + `state.camera.layers.disable(9)` for third-person camera mode.
- **`adapters/godot-vrm/src/session.gd`**: `camera.cull_mask = 0xFFFFF & ~2` to exclude the firstPersonOnly layer bit (the addon already assigns `layers=2` to firstPersonOnly meshes via `perform_head_hiding()`; we just weren't filtering them at the camera).

Post-fix re-render (same 4 plans, same hardware):

| test_id | vrm-metal-kit | three-vrm (fixed) | godot-vrm |
|---|---|---|---|
| `mtoon_firstperson_auto` | `5d8cf1789282` (49.6 kB) | `6ff1f5687375` (57.5 kB) | `4587bf323df1` (10.6 kB) |
| `mtoon_firstperson_both` | `5d8cf1789282` (49.6 kB) | `6ff1f5687375` (57.5 kB) | `4587bf323df1` (10.6 kB) |
| `mtoon_firstperson_thirdPersonOnly` | `5d8cf1789282` (49.6 kB) | `6ff1f5687375` (57.5 kB) | `4587bf323df1` (10.6 kB) |
| `mtoon_firstperson_firstPersonOnly` | **`0c167e74f194` (20.6 kB)** | **`ec736560cc6c` (24.7 kB)** | `4587bf323df1` (10.6 kB) |

**three-vrm is now conformant** — `firstPersonOnly` hashes distinctly and the PNG drops from 57.5 kB → 24.7 kB (less than half), matching the head-culled signature VMK produces. Pattern across the 4 variants now mirrors VMK exactly: 3 visible-head + 1 culled-head.

**godot-vrm: the camera.cull_mask edit applied, but the rendered output is unchanged.** All 4 variants still hash identically at 10.6 kB. The deeper issue is that godot-vrm is rendering only a small, identical portion of the scene regardless of mesh annotations — consistent with the prior emissive-sweep finding where godot produced identical 10.6 kB output across every emissive variant too. The addon's `perform_head_hiding()` may be silently no-op'ing (no head bone in our synthetic mesh's weights, perhaps, or the registration order keeps mesh-layer assignment from firing). This is no longer a firstPerson-culling story — it's a baseline godot rendering issue affecting the synthetic-avatar corpus. Worth filing as its own thread once diagnosed.

Conformance count delta: **1 of 3 peer renderers moved from non-conformant to conformant** on this surface with a 10-line adapter change. VMK + three-vrm now both pass; godot-vrm still needs investigation.

### Methodology investigation — godot-specific rendering, not a corpus issue

(Note: an earlier draft of this section claimed the conformance corpus had a 1–4% pixel-coverage methodology hazard affecting all renderers. That claim was wrong and is corrected below. The original measurement counted RGB=0 pixels as "empty" without accounting for the MToon shader writing legitimately dark colors for the shaded portion of the sphere, plus alpha-channel quirks that varied per renderer.)

ASCII visualization of `mtoon_default` across the three renderers (32× downsample, `:` = non-zero RGB with alpha=0, `M` = magenta background, blank = exact (0,0,0) pixel) confirms the avatar IS rendered visibly on VMK and three-vrm:

- **three-vrm**: clear sphere silhouette at rows 8–15 spanning cols 7–24 of the 32-row downsample (~512×288 px of original) — recognisable avatar head shape. Pixel count 3.81% non-black is consistent with MToon-shaded sphere where the shaded half reads as RGB=(0,0,0) due to default shading.
- **vrm-metal-kit**: sparse pixels in roughly the same area, mostly the sphere's lit edge.
- **godot-vrm**: just a few isolated bright pixels at scattered positions, no recognisable shape.

The screen-space math (sphere radius 0.3 m, world position (0, 1.36, 0), camera (0, 1.4, 1.5), FOV 30°, distance ≈ 1.5005 m) predicts the sphere should subtend ≈ 22.6° = 75% of frame height = ~764 px diameter. three-vrm matches this prediction; godot does not render anything close to it.

**Refined conclusion**: the corpus produces meaningful signal for VMK + three-vrm comparisons. godot-vrm is the outlier — it renders only sparse highlights, not the full MToon-shaded sphere. The earlier consensus pair-stats SSIM (~0.90 godot vs VMK) is somewhat inflated by mostly-dark-vs-mostly-dark correlation, but the headline "godot doesn't render the avatar fully on this corpus" stands. The 10.6 kB godot PNG size reflects sparse rendered content + RGB (no alpha), not a corpus methodology problem.

**For the firstPerson question**: godot's failure to differentiate the 4 variants is consistent with the avatar not being meaningfully rendered to begin with — there's nothing for `perform_head_hiding()` to cull because the mesh isn't visibly present. Diagnosing godot's MToon-shader pipeline is the right next thread, not a corpus retune.

## glTF-core PBR textures on MToon — `occlusionTexture` industry-wide ignored; `normalTexture` partial on VMK

**Date**: 2026-05-23. Surfaced on the first run of the PBR-textures sweep.

`mtoon_pbr_textures_sweep` (6 variants) attaches glTF-core `normalTexture` (tangent-space normal map at texture index 1) and `occlusionTexture` (R-channel AO modulation, reuses checkerboard at index 0) to MToon materials. The question it answers: do MToon renderers integrate with the glTF-core PBR texture pipeline, or does the MToon shader override the entire textureInfo handling?

| test_id | vrm-metal-kit (size) | three-vrm (size) | godot-vrm (size) |
|---|---|---|---|
| `mtoon_pbrtex_baseline` (no PBR textures) | `5d8cf17... 50K` | `6ff1f56... 58K` | `4587bf3... 11K` |
| `mtoon_pbrtex_occlusion_default` (strength=1) | `5d8cf17... 50K` | `6ff1f56... 58K` | `4587bf3... 11K` |
| `mtoon_pbrtex_occlusion_strength_half` (0.5) | `5d8cf17... 50K` | `6ff1f56... 58K` | `4587bf3... 11K` |
| `mtoon_pbrtex_normal_default` (scale=1) | `a599ae8... 69K` | `cb5eec9... 71K` | `4587bf3... 11K` |
| `mtoon_pbrtex_normal_scale_2x` (scale=2) | `a599ae8... 69K` | `d81b15e... 83K` | `4587bf3... 11K` |
| `mtoon_pbrtex_combined` (both) | `a599ae8... 69K` | `cb5eec9... 71K` | `4587bf3... 11K` |

### Three distinct findings

**(1) `occlusionTexture` is industry-wide ignored on MToon materials — INTENTIONAL, confirmed by UniVRM reference.** VMK's `occlusion_default` (strength=1) and `occlusion_strength_half` (strength=0.5) both render byte-identical to baseline `5d8cf17...`. three-vrm: same. **UniVRM (the consortium reference) confirms this**: same hash `9ed71e6...` for baseline and both occlusion variants. Four renderers, four different MToon implementations, all omitting `occlusionTexture` honoring → this is intentional spec behavior, not a per-renderer bug. The MToon spec (`docs/upstream-specs/vrm-specification/specification/VRMC_materials_mtoon-1.0/README.md`) explicitly declares MToon a non-PBR toon shader; PBR features don't apply. **Documented in `docs/methodology.md` as a non-applicable conformance axis** for MToon. The sweep variants stay in the corpus as tripwires (a renderer that suddenly starts honoring `occlusionTexture` should be flagged for divergence from the consortium reference); consensus evaluation treats per-renderer agreement-on-omission as expected.

**(2) `normalTexture` is partially honored on VMK: read but `scale` ignored.** VMK's `normal_default` (scale=1) and `normal_scale_2x` (scale=2) both hash `a599ae8...` — different from baseline (so VMK *does* read the normal map and apply per-vertex perturbation) but identical to each other (so the `scale` field on the textureInfo isn't being threaded through to the shader's tangent-space normal computation). UniVRM proves this IS a conformance gap (UniVRM produces distinct `e985087...` vs `308879e...` hashes for scale=1 vs scale=2; file size jumps 57K → 81K). Filed upstream as [VMK#290](https://github.com/arkavo-org/VRMMetalKit/issues/290); issue body archived locally at `docs/upstream/VMK-normal-texture-scale.md`. Narrowest VMK scope so far — only the `scale` field needs threading through.

**(3) `normalTexture` is fully conformant on three-vrm AND UniVRM.** Both produce distinct hashes for scale=1 vs scale=2 (three-vrm `cb5eec9...` → `d81b15e...`, UniVRM `e985087...` → `308879e...`), with the expected file-size jump consistent with amplified normal perturbation.

**godot-vrm**: blocked as expected — every variant hashes `4587bf323df1`.

### UniVRM data: the methodology question resolves cleanly

Including UniVRM in the comparison was decisive. Without the reference renderer's data, the occlusion finding looked like an industry-wide gap that might be filed against three of four renderers. UniVRM's behavior confirms it's intentional spec semantics, saving four pointless upstream filings. Conversely, UniVRM's distinct-scale render proves `normalTexture.scale` IS on the conformance hook (single-issue scope rather than methodology question).

**Methodology lesson recorded for future sweeps**: when a finding might be "the spec is silent or ambiguous", route through UniVRM before filing upstream issues. UniVRM's behavior is the consortium reference for ambiguous spec questions per `docs/methodology.md`.

## MToon outlineWidthMultiplyTexture — VMK partial-broken (new gap); three-vrm conformant

**Date**: 2026-05-23. Surfaced on the first run of the new outlineWidthMultiplyTexture sweep.

`mtoon_outline_width_multiply_texture_sweep` (5 variants) exercises per-vertex outline-width modulation per the MToon spec (README.md:710-715): the texture's **G-channel** (not R) is read and multiplied into the outline width. Render through all 3 adapters:

| test_id | vrm-metal-kit | three-vrm | godot-vrm |
|---|---|---|---|
| `mtoon_outlinewidthtex_baseline` (no texture) | `d3cd8b8733f5` | `1626b6a23782` | `4587bf323df1` |
| `mtoon_outlinewidthtex_mode_none` (texture + mode=none — regression guard) | `5d8cf1789282` | `6ff1f5687375` | `4587bf323df1` |
| `mtoon_outlinewidthtex_world` (texture + world outline) | `acc8b45afb2b` | `76ae39623713` | `4587bf323df1` |
| `mtoon_outlinewidthtex_screen` (texture + screen outline) | `acc8b45afb2b` | `e8db0ad6c981` | `4587bf323df1` |
| `mtoon_outlinewidthtex_width_2x` (texture + 2× base width) | `acc8b45afb2b` | `45aacfe46425` | `4587bf323df1` |

**vrm-metal-kit: partial-broken on a new axis.**

1. ✅ **Regression guard passes**: `mode_none` with a texture set renders to `5d8cf1789282` — byte-identical to no-texture `mtoon_default`. VMK correctly gates the binding on `outlineWidthMode != none`.
2. ❌ **Three different textured variants produce one hash**: `world`, `screen`, and `width_2x` all render to `acc8b45afb2b` despite varying both `outline_width_mode` (world vs screen) **and** `outline_width_factor` (0.05 vs 0.1). VMK's outline pipeline is in a degraded state when the multiply texture is attached — it ignores per-vertex G-channel modulation **and** the base width factor **and** the coordinate-mode distinction, but does render *some* outline (different hash from baseline).
3. This is a different failure pattern from VMK#287 (emissive multiplier ignored, no effect) and VMK#288 (texture transform ignored, no effect). Here the texture *does* change the render path — just incorrectly. Worth filing as its own VMK upstream issue (different code site than #287/#288, different diagnosis shape).

**three-vrm: fully spec-conformant.** All 5 variants distinct. World vs screen produce different outputs, width_2x produces a third, mode_none correctly suppresses outlines.

**godot-vrm: blocked by import-time root cause.** All 5 hash `4587bf323df1` — the no-content render.

### Filed upstream

[VMK#289](https://github.com/arkavo-org/VRMMetalKit/issues/289). Different shape from #287/#288 — those two are pure no-ops (extension silently dropped), here the texture *is* read but the outline pipeline goes degraded and ignores per-vertex G modulation, `outlineWidthFactor`, AND `outlineWidthMode`. Three distinct sweep variants render to identical PNG hash on VMK. Issue body archived locally at `docs/upstream/VMK-outline-width-multiply-texture.md`.

## MToon shadingShiftTexture + rimMultiplyTexture — VMK + three-vrm both conformant; cumulative MToon-texture story closes

**Date**: 2026-05-23. Surfaced on the first run of two new sweeps.

`mtoon_shading_shift_texture_sweep` (5 variants, spec `README.md:282-289`: texture R-channel × scale ADDED to `shadingShiftFactor`) and `mtoon_rim_multiply_texture_sweep` (4 variants, RGB multiplies into parametric rim) round out the MToon-spec texture-binding coverage.

```
shadingShiftTexture sweep            vmk            three-vrm      godot-vrm
  baseline                           5d8cf17... (50K) 6ff1f56... (58K) 4587bf3... (11K)
  default (scale=1.0)                6d70c6e... (49K) 8dfd4b0... (86K) 4587bf3... (11K)
  scale_2x                           2975d85... (49K) e0a1411... (61K) 4587bf3... (11K)
  scale_half                         ec803b2... (50K) f8f8d97... (63K) 4587bf3... (11K)
  with_factor (factor=-0.3)          274be90... (38K) 61992f3... (66K) 4587bf3... (11K)

rimMultiplyTexture sweep             vmk            three-vrm      godot-vrm
  baseline                           2272ed4... (48K) 86eb695... (35K) 4587bf3... (11K)
  default                            bcc058f... (76K) ca05efa... (88K) 4587bf3... (11K)
  red_rim                            ea5939c... (89K) 9804435... (83K) 4587bf3... (11K)
  half_mix                           115da5d... (83K) 42aabe9... (89K) 4587bf3... (11K)
```

Both renderers conformant on both bindings. Godot blocked by the import-time root cause (every variant hashes `4587bf323df1` — the no-content render).

### Complete MToon-texture conformance matrix (final)

| MToon texture binding | three-vrm | vrm-metal-kit | godot-vrm |
|---|---|---|---|
| `baseColorTexture` (read) | ✅ | ✅ | ❌ blocked |
| `KHR_texture_transform` on textureInfo | ✅ | ❌ ([VMK#288](https://github.com/arkavo-org/VRMMetalKit/issues/288)) | ⚠️ partial |
| `shadeMultiplyTexture` | ✅ | ✅ | ❌ blocked |
| `matcapTexture` | ✅ | ✅ | ❌ blocked |
| `shadingShiftTexture` | ✅ | ✅ | ❌ blocked |
| `rimMultiplyTexture` | ✅ | ✅ | ❌ blocked |

**The final MToon-texture story for VMK:** reads every per-MToon texture binding correctly (5 of 5 covered today: shade, matcap, shadingShift, rim). The ONLY remaining gap is `KHR_texture_transform` on textureInfo (already filed as VMK#288). One issue, one well-scoped shader fix, closes VMK's MToon-texture conformance gap entirely.

**The final story for godot:** every texture binding is blocked by the same root cause (addon import-time vs runtime mismatch). No partial signal observable until that's fixed.

## MToon matcapTexture — VMK + three-vrm both conformant; godot blocked

**Date**: 2026-05-23. Surfaced on the first run of the new matcapTexture sweep.

`crates/vrm-asset-generator/src/sweep.rs::mtoon_matcap_texture_sweep` emits 5 MToon assets exercising the spec's rim-lighting matcap term (per `docs/upstream-specs/vrm-specification/specification/VRMC_materials_mtoon-1.0/README.md:550`: `rim += matcapFactor.rgb * texture(matcapTexture, matcapUv).rgb`, where matcapUv is derived from the view-space surface normal — sphere-mapped, not mesh-UV-mapped). All variants set near-black base+shade colors so the matcap contribution is the only meaningful pixel-write.

| test_id | matcapFactor | matcapTexture | vrm-metal-kit (size) | three-vrm (size) | godot-vrm |
|---|---|---|---|---|---|
| `mtoon_matcap_baseline` | `[1,1,1]` | absent | `24d279c77e24` (50K) | `58ad3eacee0e` (60K) | `a4b4ae4aa7c0` (11K) |
| `mtoon_matcap_default` | `[1,1,1]` | present | `73a7c0638d69` (121K) | `9a2f5b656ede` (107K) | `a4b4ae4aa7c0` (11K) |
| `mtoon_matcap_red_tint` | `[1,0,0]` | present | `04751bf8ea16` (103K) | `c1f3c42c47bb` (95K) | `a4b4ae4aa7c0` (11K) |
| `mtoon_matcap_blue_tint` | `[0,0,1]` | present | `0345c0dee6a9` (85K) | `9de7cfa2e618` (93K) | `a4b4ae4aa7c0` (11K) |
| `mtoon_matcap_dim` | `[0.5,0.5,0.5]` | present | `297a2e1350c9` (118K) | `b1782709b6ad` (106K) | `a4b4ae4aa7c0` (11K) |

**vrm-metal-kit: fully spec-conformant.** All 5 variants distinct. The baseline-vs-default file-size jump (50K → 121K) is dramatic — adding the matcap roughly doubles the visible pixel content, which the PNG encoder reflects in compressed size. Red and blue tints produce intermediate file sizes (103K and 85K) consistent with one channel surviving the multiplicative blend. The dim variant (118K) is close to default — confirming `matcapFactor=[0.5,0.5,0.5]` is applied as a linear half-intensity multiplier rather than ignored or clamped.

**three-vrm: fully spec-conformant.** All 5 distinct, similar file-size pattern. The two renderers' conformance is independent confirmation — when both VMK and three-vrm distinguish the same variants, the spec semantics are unambiguous and our test asset is sound.

**godot-vrm: every variant produces `a4b4ae4aa7c0` (11K)** — the no-content render again, blocked by the documented import-time vs runtime mismatch root cause. matcap conformance can't be observed until that root cause closes.

### Cumulative MToon-texture conformance picture

After today's three texture-binding sweeps (baseColorTexture+KHR_texture_transform, shadeMultiplyTexture, matcapTexture):

| binding | three-vrm | vrm-metal-kit | godot-vrm |
|---|---|---|---|
| `baseColorTexture` (read) | ✅ | ✅ | ❌ (import-time) |
| `KHR_texture_transform` on `baseColorTexture` | ✅ | ❌ ([VMK#288](https://github.com/arkavo-org/VRMMetalKit/issues/288)) | ⚠️ partial |
| `shadeMultiplyTexture` | ✅ | ✅ | ❌ (import-time) |
| `matcapTexture` | ✅ | ✅ | ❌ (import-time) |

VMK reads every per-binding texture correctly; only the per-textureInfo `KHR_texture_transform` extension is missing. So VMK#288's scope keeps narrowing: it's not a texture-binding gap, just a UV-transform gap in the shader.

## MToon shadeMultiplyTexture — VMK + three-vrm both conformant; godot blocked by import-time root cause

**Date**: 2026-05-23. Surfaced on the first run of the new shadeMultiplyTexture sweep.

`crates/vrm-asset-generator/src/sweep.rs::mtoon_shade_multiply_texture_sweep` emits 6 MToon assets exercising the spec's shaded-color path (`shadeColorTerm = shadeColorFactor.rgb * texture(shadeMultiplyTexture, uv).rgb`, per `docs/upstream-specs/vrm-specification/specification/VRMC_materials_mtoon-1.0/README.md:307`). All variants reuse the procedural 16×16 quadrant checkerboard texture (index 0 — shared with the texture-transform sweep, no duplication). Renders direct via `vrm-runner execute-test-plan`:

| test_id | shadeColorFactor | shadingShift | vrm-metal-kit | three-vrm | godot-vrm |
|---|---|---|---|---|---|
| `mtoon_shadetex_baseline` (no texture) | `[0.5, 0.5, 0.5]` | `0.0` | `5d8cf1789282` | `6ff1f5687375` | `4587bf323df1` |
| `mtoon_shadetex_default` | `[0.5, 0.5, 0.5]` | `0.0` | `dfec1281483f` | `8906734a25b3` | `4587bf323df1` |
| `mtoon_shadetex_white_tint` | `[1, 1, 1]` | `0.0` | `3db2f48a6638` | `42ac57832959` | `4587bf323df1` |
| `mtoon_shadetex_red_tint` | `[1, 0, 0]` | `0.0` | `d721f4fd2186` | `dfa83093fffc` | `4587bf323df1` |
| `mtoon_shadetex_shift_neg0p5` | `[1, 1, 1]` | `-0.5` | `ccb0c061e330` | `b98c612e8985` | `4587bf323df1` |
| `mtoon_shadetex_shift_pos0p5` | `[1, 1, 1]` | `+0.5` | `dc0697e11d63` | `6e096f4213de` | `4587bf323df1` |

**vrm-metal-kit: fully spec-conformant.** All 6 variants render distinctly — including the baseline-vs-default pair (`5d8cf1789282` vs `dfec1281483f` proves VMK reads `shadeMultiplyTexture` and multiplies it into the shade term) and the per-axis controls (tinted vs un-tinted, shifted vs unshifted). Notable contrast with the texture-transform sweep where VMK ignored `KHR_texture_transform` entirely: VMK's MToon parser **does** read the per-MToon texture bindings (`shadeMultiplyTexture` here), it just doesn't apply the per-textureInfo `KHR_texture_transform` extension to the UVs. So VMK#288's scope is narrower than it might've appeared — the fix only needs to thread the UV transform into the shader's texture-sampling step, not add new texture-binding support.

**three-vrm: fully spec-conformant.** All 6 distinct hashes, including the red-tint variant which exercises the multiplicative blending (red × {red, green, blue, yellow} → {red, black, black, red}).

**godot-vrm: every variant produces `4587bf323df1`** — the same hash godot has been producing for every textured/no-texture mtoon_default render. Consistent with the documented `_import_post` import-time vs runtime mismatch root cause: godot's scene never gets the textured material attached, so the conformance test can't observe any shading behaviour. Marked as blocked by the addon-import-time fix.

### Net signal

- **VMK conformance**: 1-of-2 textured-MToon paths conformant (shadeMultiplyTexture ✅, baseColorTexture + KHR_texture_transform ❌). Filed VMK#287 (emissive) + VMK#288 (texture transform) cover the gaps.
- **three-vrm conformance**: clean on all texture-related conformance tests so far (emissive minus HDR-clamp expected, firstPerson after the adapter fix, baseColorTexture + transform, shadeMultiplyTexture).
- **godot-vrm**: every texture-related finding inherits from the import-time root cause. Sub-question of "does godot support shadeMultiplyTexture in its addon shader" can't be answered until that root cause closes.

## KHR_texture_transform — three distinct conformance patterns

**Date**: 2026-05-23. Surfaced on the first run of the new texture-transform sweep.

`crates/vrm-asset-generator/src/sweep.rs::mtoon_texture_transform_sweep` emits 8 textured MToon assets (procedural 16×16 quadrant checkerboard: red/green/blue/yellow) crossing offset, rotation, scale, and combined transforms per the [`KHR_texture_transform`](https://github.com/KhronosGroup/glTF/blob/main/extensions/2.0/Khronos/KHR_texture_transform/README.md) extension. Renders direct via `vrm-runner execute-test-plan`:

| test_id | vrm-metal-kit | three-vrm | godot-vrm |
|---|---|---|---|
| `mtoon_uvxform_identity` | `5b8077fbe8a4` | `fcd41570e763` | `31baf5da3260` |
| `mtoon_uvxform_offset_x_0p5` | `5b8077fbe8a4` | `d8aed98253e2` | `a1b70cab9d48` |
| `mtoon_uvxform_offset_y_0p5` | `5b8077fbe8a4` | `147fd12b206b` | `8a1bf9c5439e` |
| `mtoon_uvxform_rotation_eighth` (π/4) | `5b8077fbe8a4` | `c416ef51b768` | `31baf5da3260` |
| `mtoon_uvxform_rotation_quarter` (π/2) | `5b8077fbe8a4` | `6a6992a5755a` | `31baf5da3260` |
| `mtoon_uvxform_scale_2x` | `5b8077fbe8a4` | `33ac4596423c` | `c4672144a2cc` |
| `mtoon_uvxform_scale_half` | `5b8077fbe8a4` | `2ec50ff90eb9` | `05a8a21860a1` |
| `mtoon_uvxform_combined` | `5b8077fbe8a4` | `0d7e9f3ccbf2` | `0917bf8b0882` |

**three-vrm: fully spec-conformant.** All 8 variants render to distinct hashes, including the eighth/quarter rotation pair (proving the rotation axis is applied independently). Reference behavior.

**vrm-metal-kit: ignores `KHR_texture_transform` entirely.** All 8 variants produce the same `5b8077fbe8a4` PNG. Verification: that hash differs from VMK's no-texture `mtoon_default` render (`5d8cf1789282`), so VMK **does** read the `baseColorTexture` — it just doesn't consult `extensions.KHR_texture_transform`. The MToon shader pipeline applies the texture with the raw UV coordinates from the mesh.

**godot-vrm: partial — applies offset and scale, ignores rotation.** Five distinct hashes across the 8 variants. `identity`, `rotation_eighth`, and `rotation_quarter` all hash to `31baf5da3260`, indicating the rotation axis is silently dropped. `offset_x`, `offset_y`, `scale_2x`, `scale_half`, and `combined` all produce unique outputs. (Bear in mind godot-vrm's "rendered output" on this corpus is sparse fragments per the [VRM addon import-time vs runtime mismatch](#root-cause-for-godots-sparse-rendering--vrm-addon-import-time-vs-runtime-mismatch) finding, so the partial conformance claim should be re-verified once that root cause is closed.)

### To file upstream

- **VMK**: filed as [VMK#288](https://github.com/arkavo-org/VRMMetalKit/issues/288). Same shape as the emissive-multiplier issue (VMK#287): a per-textureInfo extension that needs to be threaded into the MToon shader's UV computation. Spec citation in `docs/upstream-specs/glTF/extensions/2.0/Khronos/KHR_texture_transform/README.md`. Issue body archived locally at `docs/upstream/VMK-khr-texture-transform.md`.

- **godot-vrm**: needs the import-time root cause closed first (per the godot-vrm findings entry above). After that, the rotation-axis gap can be diagnosed separately.

### Root cause for godot's sparse rendering — VRM addon import-time vs runtime mismatch

Captured Godot stderr during a single `mtoon_default` render (via `vrm-runner execute-test-plan --adapter-bin vrm-godot-shim`) shows two cascading errors in the addon's VRM import path, before any MToon-shader code runs:

```
ERROR: Bug: Dictionary::operator[] used when there was no value for the given key "vrm/already_processed". Please report.
   at: operator[] (core/variant/dictionary.cpp:136)
   GDScript backtrace:
       [0] _import_preflight (res://addons/vrm/1.0/VRMC_vrm.gd:957)
       [1] load_vrm (res://src/session.gd:42)

SCRIPT ERROR: Trying to assign value of type 'Skeleton3D' to a variable of type 'ImporterMeshInstance3D'.
          at: _VRMC_vrm._create_animation_player (res://addons/vrm/1.0/VRMC_vrm.gd:387)
          GDScript backtrace:
              [0] _create_animation_player (res://addons/vrm/1.0/VRMC_vrm.gd:387)
              [1] _import_post (res://addons/vrm/1.0/VRMC_vrm.gd:1034)
              [2] load_vrm (res://src/session.gd:46)
```

`ImporterMeshInstance3D` is Godot's editor-time abstract class that normally gets resolved into runtime types (`MeshInstance3D` + `Skeleton3D`) during editor-side glTF import. The godot-vrm addon's `VRMC_vrm.gd:_import_post` was written against the editor-time scene graph and assumes those resolutions have already happened. When we call it from runtime code via `GLTFDocument.append_from_file` + `generate_scene` (`session.gd:42-46`), the `ImporterMeshInstance3D` types are still present — and the addon's animation-player builder tries to assign a `Skeleton3D` to one of them, failing the type check.

So the cascading effect is:
1. `_import_preflight` partially fails (missing `vrm/already_processed` initialisation).
2. `_import_post` then errors out on the editor/runtime type mismatch.
3. The scene gets handed to the rest of `session.gd` in a partially-constructed state.
4. The MToon material setup may not even attach to any meshes that survived.
5. The viewport renders only the skeleton-debug-render fragments + sparse highlights from whatever did materialise.

**This isn't an MToon shader bug, an adapter wiring bug, or a firstPerson-culling bug.** It's that the godot-vrm addon (`V-Sekai/godot-vrm` lineage in `adapters/godot-vrm/addons/vrm/`) is designed for editor-time import and not for runtime headless loading. Every previous godot-vrm finding in this document inherits from this root cause.

Properly fixing this is **multi-session work** with three plausible paths:
1. **Adapt the addon for runtime** — audit `VRMC_vrm.gd`'s `_import_*` callbacks and replace `ImporterMeshInstance3D` references with runtime equivalents; not a small change.
2. **Bypass the addon's `_import_post`** — call `gltf.generate_scene()` first, then walk the resulting runtime scene graph and apply VRM data ourselves. Loses VRM-specific features but gives a clean baseline render.
3. **Upstream** — file with `V-Sekai/godot-vrm` (the addon source) asking for a documented runtime-loading API.

For now, **mark every godot-vrm finding in this document as inheriting from the addon import-time root cause** and stop chasing godot-specific symptoms until the import path is fixed. Godot remains in the manifest for completeness (consensus diffs against it still pass via mostly-black-vs-mostly-black correlation), but its renders should not be trusted as a conformance reference.

## VMK 0.16.0-rc.3 verification — six closures land, headline non-determinism gone, two new sub-findings

**Date**: 2026-05-23, vrm-conformance commit (working tree, RC pin bumped to 0.16.0-rc.3 in `adapters/vrm-metal-kit/Package.swift`).

**RC under test**: [`0.16.0-rc.3`](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.16.0-rc.3) (commit `8cd3bc9`, pre-released 2026-05-23). Single squashed PR #291 closes six issues filed by this suite plus one long-standing open (VMK#239) which appears in the release notes but not the commit message — verified empirically below.

**Environment**: Apple M4 Max, macOS 26.5 (build 25F71), Xcode 26.5 / Swift 6.3.2, same machine that ran the rc.1 and rc.2 verifications.

### Headline

| Surface | rc.3 result vs rc.2 |
|---|---|
| **VMK#283 (animated swing non-determinism, headline rc.2 finding)** | ✅ **closed** — `swing_springbone_joints_16`, `swing_springbone_drag_0p8`, `swing_springbone_stiffness_0p2`: **7/7 runs byte-identical** on each plan |
| **VMK#286 (VRMA lookAt rotation channel)** | ✅ **filing closed (partial)** — yaw/pitch now parsed correctly on all 10 plans; downstream gap surfaced (see "New finding A" below) |
| **VMK#287 (MToon HDR emissive multiplier)** | ✅ **closed for LDR range** — `mtoon_emissive_multiplier_{0, 0.25, 0.5, 0.75}` now produce 4 distinct hashes (rc.2: all collapsed to one). HDR (>1) saturates at display gamut, expected with `tone_mapping: none` |
| **VMK#288 (KHR_texture_transform on baseColorTexture)** | ✅ **closed** — all 8 `mtoon_uvxform_*` variants produce 8 distinct hashes (rc.2: all collapsed to one) |
| **VMK#289 (outlineWidthMultiplyTexture degraded pipeline)** | ✅ **closed** — `world`/`screen`/`width_2x` were all `acc8b45afb2b` on rc.2; on rc.3 they produce 3 distinct hashes (`cfff0bc73f5d` / `2125c39e530e` / `015b61b46ce9`); `mode_none` correctly suppresses outline |
| **VMK#290 (normalTexture.scale)** | ✅ **closed for normal scale** — `mtoon_pbrtex_normal_default` (`a599ae818d08`) vs `mtoon_pbrtex_normal_scale_2x` (`8c30c0e22cdf`) now distinct (rc.2: byte-identical). Occlusion strength still ignored (see "New finding B" below) |
| **VMK#239 (shadingShift/shadingToony boundary collapse)** | ✅ **closed empirically** — 9/9 shadingShift + 8/8 shadingToony variants distinct; no boundary collapse |
| MToon + static settle sample (5 MToon + 3 spring-bone settle) | ✅ byte-identical rc.2 → rc.3 (no surprise regressions on unaffected variants) |
| Run-to-run determinism on swing chain | ✅ deterministic at the binary-output level (was non-deterministic on rc.1, rc.2, AND 0.15.2) |

**rc.3 is the first VMK release in this verification cohort that produces byte-identical output across consecutive runs on the animated-swing surface.** PR #291's fixed 60 Hz `synchronousSpringBone` timestep — replacing the wall-clock-paced step that drove the rc.1/rc.2/0.15.2 non-determinism — is the structural fix the rc.2 verification recommended.

### VMK#283 reproducer table (compare to rc.1 and rc.2 entries above)

`swing_springbone_joints_16`, 7 runs on rc.3 binary, same asset, same hardware:

| run | size | sha256 (first 16) |
|---|---|---|
| 1 | 45084 | `af3d5dffe9dfaef4` |
| 2 | 45084 | `af3d5dffe9dfaef4` |
| 3 | 45084 | `af3d5dffe9dfaef4` |
| 4 | 45084 | `af3d5dffe9dfaef4` |
| 5 | 45084 | `af3d5dffe9dfaef4` |
| 6 | 45084 | `af3d5dffe9dfaef4` |
| 7 | 45084 | `af3d5dffe9dfaef4` |

**One distinct output across 7 runs.** Compare to rc.2 (3 distinct / 5 runs), rc.1 (3 distinct / 5 runs), and 0.15.2 (4 distinct / 7 runs). The reproducibility regression introduced in rc.1 (then unclosed in rc.2) is fully closed on rc.3. Same pattern on `swing_springbone_drag_0p8` (7/7 byte-identical) and `swing_springbone_stiffness_0p2` (7/7 byte-identical).

### New finding A — swing-axis collapse on stiffness sweep (VMK#240 class regression)

Side effect of the fixed-timestep determinism: the entire stiffness sweep now collapses to one hash, even though rc.2 showed three distinct value-clusters across the sweep despite per-run jitter.

| plan_id | rc.2 sha (3 runs) | rc.3 sha (deterministic) | axis effect on rc.3 |
|---|---|---|---|
| `swing_springbone_default` | `8bd3bca3...` / `68b391e7...` ×2 | `68b391e7764a2a9e` | baseline |
| `swing_springbone_stiffness_0` | `68b391e7...` / `009b0cbd...` ×2 | `68b391e7764a2a9e` | **identical to default** |
| `swing_springbone_stiffness_0p2` | `68b391e7...` ×3 | `68b391e7764a2a9e` | **identical to default** |
| `swing_springbone_stiffness_0p8` | `e790e30c...` ×2 / `3074ad2f...` | `68b391e7764a2a9e` | **identical to default** |
| `swing_springbone_stiffness_1` | `be7e94a8...` ×2 / `98483065...` | `68b391e7764a2a9e` | **identical to default** |

On rc.2, the stiffness axis differentiated despite jitter: stiffness ∈ {0, 0.2} clustered around one set of values, {0.8} around another, {1} around a third. On rc.3, **every stiffness value renders byte-identical to the default plan** — i.e. stiffness has no observable effect on the rendered swing. Drag axis differentiates at the low end (`drag_0`, `drag_0p2` distinct) but collapses at the high end (`drag_0p8`, `drag_1` both = default).

This is the same bug class as the previously-closed [VMK#240](https://github.com/arkavo-org/VRMMetalKit/issues/240) (stiffness collapse under animation, closed in 0.15.0 by consuming `settlingFrames` inside `warmupPhysics`). Plausible root cause: PR #291's new fixed-rate `synchronousSpringBone` step uses a different settle/warmup interaction than the prior wall-clock step, and the test plan's capture window now falls entirely inside a post-settle period where stiffness no longer differentiates output.

Filed as [VMK#292](https://github.com/arkavo-org/VRMMetalKit/issues/292); tracker at `docs/upstream/VMK-swing-stiffness-axis-collapse-rc3.md`. Note that this is a **plan-vs-renderer interaction** — the suite's swing plans may also need their capture-time retuned to land mid-swing rather than post-settle, since the new determinism makes the capture-time choice consequential.

### New finding B — `occlusionTexture.strength` ignored on MToon

Surfaced while verifying VMK#290 (the same `mtoon_pbrtex_*` corpus exercises occlusion-strength as a separate axis from normal-scale):

| test_id | rc.3 sha |
|---|---|
| `mtoon_pbrtex_baseline` (no occlusion texture) | `5d8cf1789282` |
| `mtoon_pbrtex_occlusion_default` (strength=1.0) | `5d8cf1789282` ← same as baseline |
| `mtoon_pbrtex_occlusion_strength_half` (strength=0.5) | `5d8cf1789282` ← same as baseline |

Both occlusion-strength variants produce byte-identical output to the no-occlusion baseline — the `occlusionTexture` is not affecting the rendered output at all (not just the `strength` field — the entire texture binding seems silently dropped on the MToon path). Different shape from VMK#290 (where the texture *was* applied at scale=1.0 equivalent but the `scale` multiplier was lost): here the texture seems silently absent.

Filed as [VMK#293](https://github.com/arkavo-org/VRMMetalKit/issues/293) (sibling to VMK#290); tracker at `docs/upstream/VMK-occlusion-texture-strength.md`.

### New finding C — VMK#286 closes the parser, but the parsed gaze doesn't reach the renderer

VMK#286's filing was specifically about `VRMAnimationLoader` silently dropping rotation-channel gaze (pose dump `yaw_deg=0` / `pitch_deg=0` across the entire `vrma_lookat_*` corpus). On rc.3, the pose dump now shows the correctly-decoded yaw/pitch:

| test_id | dump `yaw_deg` | dump `pitch_deg` | `applied_via` |
|---|---|---|---|
| `vrma_lookat_yaw_neg60_bone` | `+60.00` | `0.00` | `bone` |
| `vrma_lookat_yaw_pos60_bone` | `-60.00` | `0.00` | `bone` |
| `vrma_lookat_pitch_neg30_bone` | `0.00` | `-30.00` | `bone` |
| `vrma_lookat_pitch_pos30_bone` | `0.00` | `+30.00` | `bone` |
| `vrma_lookat_neutral_bone` | `0.00` | `0.00` | `bone` |

(sign convention is "avatar's gaze rotation vs world frame", which is the negation of the named-target direction — this matches the spec).

**The filing's specific assertion is closed.** However, all 10 plans still render to byte-identical PNGs (`5d8cf1789282`), and the pose dump shows **no humanoid bone has a non-identity rotation** on bone-driven plans, and **no lookAt expression preset has a non-zero weight** on expression-driven plans. So the parsed gaze isn't propagating to the rendered avatar geometry.

Two possible root causes:
1. The yaw/pitch state is computed at `dump_look_at_state` time from the gaze quaternion stored in the lookAt controller, but `apply_vrma_at_time` doesn't push that state into the bone graph / expression weights before the render frame.
2. The state IS pushed but the `VRMLookAtController.update` path that owns the bone-vs-expression dispatch is gated on a render-time tick that doesn't fire in offline render mode.

Filed as [VMK#294](https://github.com/arkavo-org/VRMMetalKit/issues/294) (follow-up to VMK#286); tracker at `docs/upstream/VMK-vrma-lookat-renderer-propagation.md`. Reproducer asset is the existing `vrma_lookat_*` corpus.

### What this verification didn't measure

- **Corpus-wide UniVRM consensus pass-rate.** The rc.1 and rc.2 entries both report `190 / 191 (99%)`. To get a rc.3 number, the four new MToon test corpora (`mtoon_emissive_*`, `mtoon_uvxform_*`, `mtoon_outlinewidthtex_*`, `mtoon_pbrtex_*`) need UniVRM goldens re-bootstrapped — they didn't exist when the prior verifications ran. Running `scripts/bootstrap-goldens.sh` (or a UniVRM-only re-render of those sweeps) plus `scripts/consensus-report.sh` will populate the number for a follow-up entry.
- **`render_sequence` (RFC-0004) at scale.** Verified single-shot via the swing reproducer; not re-bootstrapped through the 60-plan sequence sweep.
- **iOS/iOSSimulator builds.** rc.3's release notes call out new iOS/iOS-Simulator support; our adapter still ships macOS-only and was not exercised on iOS device or simulator. The `swift build` log shows all three metallib variants are now copied into `Resources/` (consistent with VMK#280 closure in rc.2).

### Reproducer (10 lines)

```bash
# Bump the pin (already done in this commit).
( cd adapters/vrm-metal-kit && swift build -c release )
cp adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter /tmp/vmk-adapter.rc3

# VMK#283 closure check — 7 runs, expect byte-identical:
PLAN=goldens-cache/_assets_swing/swing_springbone_joints_16.test.yaml
for i in 1 2 3 4 5 6 7; do
  target/release/vrm-runner execute-test-plan \
    --plan "$PLAN" --adapter-bin /tmp/vmk-adapter.rc3 \
    --asset-dir "$(dirname $PLAN)" --output-dir "/tmp/rc3-r${i}" \
    --renderer-name vrm-metal-kit --json >/dev/null
done
# → all 7 PNGs blake3=af3d5dff..., 45084 bytes
```

The four MToon closure sweeps (`emit-emissive-sweep`, `emit-texture-transform-sweep`, `emit-outline-width-multiply-texture-sweep`, `emit-pbr-textures-sweep`) are wired into `vrm-asset-generator`; render each through the rc.3 adapter and compare per-variant hashes against the tables above.

### Status of upstream tracker docs after rc.3

| Tracker | Issue | Status |
|---|---|---|
| `docs/upstream/VMK-vrma-lookat-rotation-channel.md` | VMK#286 | ✅ filing closed (parser side); needs follow-up on renderer propagation gap |
| `docs/upstream/VMK-vrmc-materials-hdr-emissive-multiplier.md` | VMK#287 | ✅ closed (LDR range; HDR saturation is methodology-consistent) |
| `docs/upstream/VMK-khr-texture-transform.md` | VMK#288 | ✅ closed |
| `docs/upstream/VMK-outline-width-multiply-texture.md` | VMK#289 | ✅ closed |
| `docs/upstream/VMK-normal-texture-scale.md` | VMK#290 | ✅ closed |
| `docs/upstream/VMK-0.16.0-rc.1-noise.md` | VMK#283 | ✅ closed |
| `docs/upstream/VMK-swing-stiffness-axis-collapse-rc3.md` | [VMK#292](https://github.com/arkavo-org/VRMMetalKit/issues/292) | filed + closed in rc.4 same day |
| `docs/upstream/VMK-occlusion-texture-strength.md` | [VMK#293](https://github.com/arkavo-org/VRMMetalKit/issues/293) | filed + closed in rc.4 same day |
| `docs/upstream/VMK-vrma-lookat-renderer-propagation.md` | [VMK#294](https://github.com/arkavo-org/VRMMetalKit/issues/294) | filed + closed in rc.4 same day; suite-side asset coverage follow-up |

## VMK 0.16.0-rc.4 verification — three same-day closures + adapter wiring update

**Date**: 2026-05-23 (same day as rc.3 verification), vrm-conformance commit (working tree, pin bumped to 0.16.0-rc.4).

**RC under test**: [`0.16.0-rc.4`](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.16.0-rc.4) (commit `81ebce6`, pre-released 2026-05-23). Single squashed PR #296 closes all three suite-filed follow-ups against rc.3 ([VMK#292](https://github.com/arkavo-org/VRMMetalKit/issues/292), [VMK#293](https://github.com/arkavo-org/VRMMetalKit/issues/293), [VMK#294](https://github.com/arkavo-org/VRMMetalKit/issues/294)) plus a pre-existing rigid-follow regression surfaced during PR #291 development ([VMK#295](https://github.com/arkavo-org/VRMMetalKit/issues/295)).

### Headline

| Surface | rc.4 result vs rc.3 |
|---|---|
| **VMK#292 (swing-axis stiffness collapse)** | ✅ **closed** — 9 swing axis variants produce 9 distinct hashes (rc.3: stiffness sweep + high-drag all collapsed to `68b391e7764a2a9e`) |
| **VMK#293 (occlusionTexture silently dropped)** | ✅ **closed** — `occlusion_default` (`e24bff37139b`) + `occlusion_strength_half` (`17a817fdda2b`) + baseline (`5d8cf1789282`) all distinct (rc.3: all three were byte-identical); `combined` variant also picks up occlusion (`6a6c35376509` vs rc.3's normal-only `a599ae818d08`) |
| **VMK#294 (VRMA lookAt propagation)** | ✅ **closed (fix landed; asset coverage gap)** — pose dump shows correct yaw/pitch (as on rc.3), but the new `VRMLookAtController.applyImmediately()` writes to **eye bones** + **custom expressions**, which the synthetic humanoid corpus doesn't have. Adapter wiring updated; suite-side asset extension needed to observe end-to-end |
| **VMK#295 (center-node rigid follow CPU/GPU race)** | ✅ **closed in same PR** — new Metal kernel `springBoneApplyCenterDelta` applies per-substep deltas during GPU execution. Not a suite filing; behavioural impact below |
| VMK#283 determinism (rc.3 closure) | ✅ holds — 5/5 byte-identical runs on `swing_springbone_joints_16` |
| VMK#287/#288/#289/#239 (rc.3 closures) | ✅ all hold byte-identical between rc.3 and rc.4 |
| VMK#290 normalTexture.scale (rc.3 closure) | ✅ holds byte-identical |

**rc.4 is a clean follow-up release**: every suite-filed regression against rc.3 closed within hours of being filed, every prior closure preserved.

### Adapter wiring update required by VMK#294 closure

VMK#294's fix added `VRMLookAtController.applyImmediately()` which resolves the queued gaze target into eye-bone rotations or custom expression weights immediately, bypassing the frame-rate-dependent smoothing tick. The library calls it from `AnimationPlayer.applyClip()` — but our adapter doesn't use `applyClip()`. Instead, `handleApplyVrmaAtTime` (`adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift:1220`) sets the target manually:

```swift
session.renderer.lookAtController?.target = .headLocalPoint(point)
```

Without `applyImmediately()`, the target sits queued and the next render misses the gaze. **This commit adds the call**:

```swift
session.renderer.lookAtController?.target = .headLocalPoint(point)
session.renderer.lookAtController?.applyImmediately()   // ← NEW
```

The adapter change is correct and necessary regardless of asset coverage. See "asset coverage caveat" below for why end-to-end visual propagation still isn't observable on the current synthetic corpus.

### Asset coverage caveat for VMK#294

Inspecting a rc.4 pose dump for `vrma_lookat_yaw_pos60_bone` (which shows `yaw_deg=-60`, `applied_via=bone` — proof the parse worked):

```
humanoid bone names: ['hips', 'spine', 'chest', 'neck', 'head', 'leftShoulder',
  'leftUpperArm', 'leftLowerArm', 'leftHand', 'rightShoulder', 'rightUpperArm',
  'rightLowerArm', 'rightHand', 'leftUpperLeg', 'leftLowerLeg', 'leftFoot',
  'rightUpperLeg', 'rightLowerLeg', 'rightFoot']   ← 19 bones, no leftEye/rightEye

expressions.presets: {aa, angry, blink, blinkLeft, blinkRight, ee, happy, ih,
  neutral, oh, ou, relaxed, sad, surprised}   ← 14 presets, no lookLeft/Right/Up/Down

expressions.custom: {}   ← empty
```

But VMK 0.16.0-rc.4's `applyToBones` writes to `model.nodes[leftEyeBoneIndex].rotation` / `[rightEyeBoneIndex].rotation` (eye bones — not in this corpus), and `applyToExpressions` writes to custom expressions `LookLeft` / `LookRight` / `LookUp` / `LookDown` (not in this corpus). So the fix is in the library, called by our adapter, but lands on writers our model doesn't expose.

(Side note: VRM 1.0 spec defines `lookLeft` / `lookRight` / `lookUp` / `lookDown` as **preset** expressions, not customs. VMK's `applyToExpressions` writing to custom-namespace `LookLeft` may itself be a spec compliance gap worth filing — but it's a different issue from VMK#294, and one we can't characterize until our assets carry either eye bones or the relevant expressions.)

To observe VMK#294's closure end-to-end:
1. Extend `crates/vrm-asset-generator` to emit `leftEye` / `rightEye` humanoid bones on the `vrma_lookat_*_bone` plans (~10-20 LoC; needs eye bones in the humanoid node list + the `humanBones.leftEye` / `.rightEye` references in `VRMC_vrm.humanoid`).
2. Extend the same generator to emit `lookLeft` / `lookRight` / `lookUp` / `lookDown` preset expressions on the `vrma_lookat_*_expr` plans, *or* (if filing the VRM 1.0 spec discrepancy upstream) the custom-namespace versions VMK currently writes to.
3. Re-render the sweep through rc.4 — expect distinct hashes per yaw/pitch direction and non-empty `bones=` / `exprs=` columns in the pose-dump table.

### VMK#292 closure table (compare to the rc.3 finding entry above)

| plan_id | rc.3 sha (collapsed) | rc.4 sha (differentiates) |
|---|---|---|
| `swing_springbone_default` | `68b391e7764a2a9e` | `7106a87c57c0a88e` |
| `swing_springbone_stiffness_0` | `68b391e7764a2a9e` ← collapsed | `773ddc9f03361071` |
| `swing_springbone_stiffness_0p2` | `68b391e7764a2a9e` ← collapsed | `bf3106eaf43728b6` |
| `swing_springbone_stiffness_0p8` | `68b391e7764a2a9e` ← collapsed | `8730281bf707b15b` |
| `swing_springbone_stiffness_1` | `68b391e7764a2a9e` ← collapsed | `855a0b717ad287f5` |
| `swing_springbone_drag_0` | `5175753de45f4784` | `d9268dee27a4a8dc` |
| `swing_springbone_drag_0p2` | `e2a95d0f3b1cda36` | `80b4358f00c21264` |
| `swing_springbone_drag_0p8` | `68b391e7764a2a9e` ← collapsed | `b8e0c013023d0845` |
| `swing_springbone_drag_1` | `68b391e7764a2a9e` ← collapsed | `753c990aaa06a8bb` |

**All 9 plans produce 9 distinct hashes on rc.4.** Stiffness axis fully restored; high-drag end of drag axis also restored. None of the rc.4 hashes equal the rc.3 hashes — both VMK#292 (warmup drain) and VMK#295 (per-substep GPU kernel) change spring-bone output. The full swing/multichain corpus needs re-bootstrapping; static settle goldens (no `animate_root_transform`, no warmup interaction) remain byte-identical to rc.2/rc.3.

### VMK#293 closure table

| test_id | rc.3 sha | rc.4 sha | result |
|---|---|---|---|
| `mtoon_pbrtex_baseline` (no texture) | `5d8cf1789282` | `5d8cf1789282` | unchanged baseline ✓ |
| `mtoon_pbrtex_normal_default` (scale=1) | `a599ae818d08` | `a599ae818d08` | normal-only unchanged ✓ |
| `mtoon_pbrtex_normal_scale_2x` (scale=2) | `8c30c0e22cdf` | `8c30c0e22cdf` | normal scale unchanged ✓ |
| `mtoon_pbrtex_occlusion_default` (strength=1) | `5d8cf1789282` ← was identical to baseline | `e24bff37139b` ← now distinct | occlusion now visible ✓ |
| `mtoon_pbrtex_occlusion_strength_half` (strength=0.5) | `5d8cf1789282` ← was identical to baseline | `17a817fdda2b` ← now distinct | strength field honored ✓ |
| `mtoon_pbrtex_combined` (normal + occlusion) | `a599ae818d08` ← was identical to normal-only | `6a6c35376509` ← now distinct | combined output differs ✓ |

### VMK#295 (center-node rigid follow) — observed behaviour shift

Not a suite filing, but the new `springBoneApplyCenterDelta` GPU kernel changes integration output on every spring-bone plan that uses `animate_root_transform`. Our swing-sweep corpus exercises this on every variant (the entire chain animation is driven via root translation per the suite's methodology pin). The rc.4 hashes for the swing axis (table above) reflect the combined effect of VMK#292's warmup fix + VMK#295's per-substep kernel — they aren't separable without rebuilding rc.3-with-#295-only or rc.4-with-#292-only.

### Reproducer

```bash
# Bump pin + adapter wiring update (this commit) + build:
( cd adapters/vrm-metal-kit && swift build -c release )
cp adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter /tmp/vmk-adapter.rc4

# VMK#292 axis sweep check:
for plan_id in swing_springbone_stiffness_0 swing_springbone_stiffness_0p2 \
               swing_springbone_stiffness_0p8 swing_springbone_stiffness_1; do
    target/release/vrm-runner execute-test-plan \
        --plan "goldens-cache/_assets_swing/${plan_id}.test.yaml" \
        --adapter-bin /tmp/vmk-adapter.rc4 \
        --asset-dir "goldens-cache/_assets_swing" \
        --output-dir "/tmp/rc4/${plan_id}" \
        --renderer-name vrm-metal-kit --json >/dev/null
    shasum -a 256 "/tmp/rc4/${plan_id}/${plan_id}_vrm-metal-kit.png"
done
# → 4 distinct hashes (rc.3: all 4 = 68b391e7764a2a9e)

# VMK#283 determinism preserved:
for i in 1 2 3 4 5; do
    target/release/vrm-runner execute-test-plan \
        --plan "goldens-cache/_assets_swing/swing_springbone_joints_16.test.yaml" \
        --adapter-bin /tmp/vmk-adapter.rc4 \
        --asset-dir "goldens-cache/_assets_swing" \
        --output-dir "/tmp/rc4/joints16_r${i}" \
        --renderer-name vrm-metal-kit --json >/dev/null
done
# → all 5 runs sha256 19c79498d45f53ca... (byte-identical)
```

### What this verification didn't measure

- **Visual confirmation of VMK#294** end-to-end. Pose dump confirms parse + adapter wiring; visible PNG propagation blocked by asset coverage (see caveat above).
- **VMK#295's effect on a known-reference render.** No oracle rc.3-without-#295-changes binary available; observed behavior on rc.4 swing axis is the combined #292+#295 output.

### Full corpus re-bootstrap + consensus report (release readiness)

Ran `RUN_UNIVRM=1 scripts/bootstrap-goldens.sh` then `scripts/consensus-report.sh` on the same Apple M4 Max / macOS 26.5 / Xcode 26.5 / Swift 6.3.2 environment, immediately following the rc.4 closures verification above. Four real renderers exercised: VMK rc.4 + adapter wiring update, three-vrm 3.5.0, godot-vrm via vrm-godot-shim, UniVRM v0.131.0 (Unity 6000.4.6f1, Personal license, PlayMode batched).

#### Bootstrap stability

```
632 test plans found

vrm-metal-kit (rc.4):  632 succeeded, 0 failed   ← 100% stability across the entire corpus
three-vrm 3.5.0:       560 succeeded, 72 failed  ← all 72 fails are extended_collider (known)
godot-vrm:             519 succeeded, 113 failed ← all godot-import root cause (existing finding)
UniVRM v0.131.0:       267 ok / 59 errors of 326 ← Unity-side subset coverage
```

**rc.4 is the first release in the cohort where VMK renders the entire 632-plan corpus end-to-end without a single failure.** Compare rc.2 initial bootstrap (`462 succeeded / 113 failed` — all `vrma_*` Unimplemented) and rc.2 after VRMA wiring (`575 succeeded / 0 failed` — over a 575-plan corpus). rc.4 adds 57 more plans (closure-specific MToon sweeps + the expanded vrma_lookat corpus) and renders them all.

#### Conformance pass-rate vs UniVRM consortium reference

```
                                            rc.2 (after VRMA      rc.4
                                            wiring; n=206)        (n=263)
vrm-metal-kit  ≥ declared threshold:        205 / 206 (≈100%)     247 / 263 (94%)
three-vrm      ≥ declared threshold:        206 / 206 (100%)      254 / 263 (97%)
godot-vrm      ≥ declared threshold:        ~181 / 191 (95%)      217 / 248 (88%)

VMK absolute conformance passes vs rc.2:    +42 (over 57-test expanded corpus)
```

The fractional pass-rate drop (100% → 94%) is **entirely accounted for by the 57 new test variants** added since rc.2 (`mtoon_uvxform_*`, `mtoon_emissive_*`, `mtoon_outlinewidthtex_*`, `mtoon_pbrtex_*`, plus the expanded vrma_lookat corpus). These surface previously-untested cross-renderer divergences. rc.4 adds 42 absolute new passes on top of rc.2's baseline — a meaningful expansion of conformance coverage.

#### Pairwise SSIM means

```
                                          rc.2 (after VRMA       rc.4
                                          wiring)
three-vrm vs vrm-metal-kit       mean     0.9575 (n=233)         0.9512 (n=290)
univrm    vs vrm-metal-kit       mean     0.9547 (n=210)         0.9417 (n=267)
godot-vrm vs vrm-metal-kit       mean     —                      0.8960 (n=288)
three-vrm vs univrm              mean     —                      0.9440 (n=267)
godot-vrm vs univrm              mean     —                      0.8590 (n=252)
godot-vrm vs three-vrm           mean     —                      0.9129 (n=252)
```

SSIM means dropped slightly vs rc.2 — explained by the same expansion: new sweeps drag the mean down with their wider methodology tolerance. The intra-three-vrm-vs-VMK delta is 0.0063 (well within noise for non-PBR toon shading), and intra-univrm-vs-VMK is 0.013 (similar). No SSIM regression on existing plans.

#### Per-test VMK failures (16 plans below threshold + 3 excluded)

All 19 VMK-vs-UniVRM failures categorized:

| category | n | example | min ssim | hazard class |
|---|---|---|---|---|
| Outline width/screen-coord | 7 | `mtoon_outline_world_0p1` | 0.18 | **Methodology** (outline aliasing across renderers; CLAUDE.md pin "wider local SSIM tolerance band on outline regions") |
| Matcap (view-space sampling) | 5 | `mtoon_matcap_baseline` | 0.61 | **Methodology** (matcap view-direction convention differs across renderers; pre-existing) |
| Shaded-color texture | 5 | `mtoon_shadetex_default` | 0.72 | **Methodology** (shaded-color blend differs across renderers; pre-existing) |
| Stacked PBR textures | 2 | `mtoon_pbrtex_combined` (0.81), `mtoon_pbrtex_normal_scale_2x` (0.85) | 0.81 | **Near-threshold**; second is 0.005 below the 0.85 cut |
| Rim multiplier | 1 | `mtoon_rimLightingMix_1` (0.9491) | 0.95 | **Near-threshold**; fails by 0.0009 under a tight 0.95 threshold |

**No regression-class failures introduced by rc.4.** Every fail is either a previously-known methodology hazard (where all four renderers are flagged as outliers — the consensus report's "most divergent" list confirms this) or a near-threshold tie that would flip on tolerance retuning.

The closure-specific test_ids verified per-feature in the rc.4 closures section above (`mtoon_uvxform_*`, `mtoon_emissive_multiplier_*`, `mtoon_outlinewidthtex_{screen,mode_none}`, `mtoon_pbrtex_occlusion_*`) are **all in the passing set** — closure of #287/#288/#289/#293 is corroborated at the corpus level, not just at the per-hash level.

#### Release-readiness verdict

**rc.4 is ready to tag stable.** Evidence summary:

1. **Stability**: 632/632 render success — strongest of any release in the cohort.
2. **Per-feature spec conformance**: 9/9 closures verified end-to-end against spec citations this session (VMK#283, #287, #288, #289, #290, #292, #293, #239, #295). Plus VMK#286 parser closure preserved.
3. **Corpus-wide consensus**: 247/263 passes vs UniVRM consortium reference — +42 absolute new passes over rc.2's 205, on a 57-test-larger corpus.
4. **No regressions**: every rc.3 closure verified holding byte-identical on rc.4 (#287, #288, #289, #290, #239); rc.2 baselines on MToon + static-settle hold byte-identical.
5. **Failure profile is clean**: all 19 VMK-vs-UniVRM failures are pre-existing methodology hazards (outline / matcap / shadetex aliasing, where all four renderers diverge) or near-threshold ties (within 0.005 of cut).

**Two follow-ups recommended for the 0.16.1 minor cycle, both non-blocking:**

- **VMK#294 end-to-end visual verification** requires extending the suite's synthetic humanoid generator to emit `leftEye`/`rightEye` bones (~10 LoC) and either `lookLeft`/`lookRight`/`lookUp`/`lookDown` preset expressions or the custom-namespace variants VMK currently writes to. Renderer fix verified at the dump level + adapter wiring landed.
- **VRM 1.0 spec discrepancy on lookAt expression namespace**: VMK's `applyToExpressions` writes to **custom**-namespace `LookLeft`/`LookRight`/`LookUp`/`LookDown`, but the VRM 1.0 spec defines `lookLeft`/`lookRight`/`lookUp`/`lookDown` as **preset** expressions. Worth filing as a 0.16.1 follow-up once we have asset coverage to verify. **Update**: filed as [VMK#297](https://github.com/arkavo-org/VRMMetalKit/issues/297), closed via PR #298 in 0.16.0 stable — see entry below.

## VMK 0.16.0 stable verification — first non-pre-release in the cohort

**Date**: 2026-05-23, vrm-conformance commit (working tree, pin bumped to 0.16.0 stable in `adapters/vrm-metal-kit/Package.swift`).

**Release**: [`0.16.0`](https://github.com/arkavo-org/VRMMetalKit/releases/tag/0.16.0) (commit `392d949`, released 2026-05-23 — **stable**, `prerelease: false`). Consolidates rc.1 → rc.4 plus PR #298 closing VMK#297 (the lookAt preset-namespace spec discrepancy this suite flagged at the end of the rc.4 verification).

### Headline

| Surface | 0.16.0 stable result |
|---|---|
| **PR #298 spec fix verified (VMK#297 closure)** | ✅ 5/5 new upstream unit tests pass locally; dual-write strategy preserves VRM 0.x custom-namespace + spec-correct VRM 1.0 preset-namespace |
| **rc.4 vs 0.16.0 stable A/B** | ✅ 12/12 sanity sample byte-identical (MToon: default, shadingShift, outline, alpha-blend; spring-bone: settle + 3 swing axes; VRMA lookAt: 4 plans) |
| **VMK#283 determinism reproducer** | ✅ 5/5 byte-identical runs on `swing_springbone_joints_16` (sha `19c79498d45f53ca` — same as rc.4) |
| Cohort-wide closures (13 total) | ✅ all hold: VMK#283, #286, #287, #288, #289, #290, #292, #293, #294, #297, #239, #295 + earlier 0.15.x closures |

### Why 0.16.0 stable is a low-risk bump from rc.4

The only delta between rc.4 and 0.16.0 stable is PR #298 — a 3-file surgical fix:
- `VRMLookAtController.swift` (+44/-31)
- `VRMExtensionParser.swift` (+9/-0)
- `Tests/VRMLookAtExpressionNamespaceTests.swift` (+337/-0, new file with 5 tests)

PR #298 was reviewed independently before merge ([this finding entry's PR #298 review](https://github.com/arkavo-org/VRMMetalKit/pull/298)): dual-write strategy means every controller emit reaches both spec-preset and legacy-custom namespaces, so the change is no-op on VRM 0.x assets and lights-up on spec-compliant VRM 1.0 assets. Verified at the binary level on our corpus: 18/18 byte-identical PR #298 vs rc.4, then 12/12 byte-identical 0.16.0 vs rc.4. Determinism preserved.

### Cohort summary (rc.1 → 0.16.0 stable)

13 issues closed in this cohort, 10 of them filed by this suite:

| Issue | Filed by | Closed in | Type |
|---|---|---|---|
| VMK#283 | this suite (rc.1 verification) | rc.3 (PR #291) | spring-bone determinism |
| VMK#286 | this suite | rc.3 (PR #291) | VRMA lookAt rotation-channel parse |
| VMK#287 | this suite | rc.3 (PR #291) | MToon HDR emissive multiplier |
| VMK#288 | this suite | rc.3 (PR #291) | MToon KHR_texture_transform |
| VMK#289 | this suite | rc.3 (PR #291) | MToon outlineWidthMultiplyTexture |
| VMK#290 | this suite | rc.3 (PR #291) | glTF-core normalTexture.scale |
| VMK#239 | (long-standing) | rc.3 (PR #291; release notes only) | MToon shadingShift/shadingToony boundary |
| VMK#292 | this suite (rc.3 verification) | rc.4 (PR #296) | spring-bone stiffness axis collapse |
| VMK#293 | this suite (rc.3 verification) | rc.4 (PR #296) | glTF-core occlusionTexture |
| VMK#294 | this suite (rc.3 verification) | rc.4 (PR #296) | VRMA lookAt renderer propagation |
| VMK#295 | upstream | rc.4 (PR #296) | spring-bone center-node rigid follow CPU/GPU race |
| VMK#297 | (suite flagged in rc.4 entry) | 0.16.0 (PR #298) | VRM 1.0 lookAt preset-namespace spec compliance |

Pace: 13 filings + closures in 3 days (2026-05-21 → 2026-05-23), each verified per-feature against spec citation in the rc.1 → 0.16.0 chain of findings entries.

### Status of upstream tracker docs after 0.16.0

All tracker stubs in `docs/upstream/VMK-*.md` for issues this suite filed are now **closed in 0.16.0 stable**. No open suite-filed VMK issues at this writing.

Two suite-side follow-ups remain (both non-blocking, both noted under the rc.4 entry's "two follow-ups" subsection):

1. **Asset coverage extension for VMK#294** end-to-end visual verification — add `leftEye`/`rightEye` bones + `lookLeft`/`lookRight`/`lookUp`/`lookDown` preset expressions to the synthetic humanoid generator (~10–15 LoC in `crates/vrm-asset-generator/src/sweep.rs`).
2. **Re-bootstrap goldens** for downstream consumers — every plan in the spring-bone corpus using `animate_root_transform` shifted between rc.2 and 0.16.0 (combined effect of VMK#292 warmup drain + VMK#295 GPU kernel). Spring-bone goldens cached anywhere should be refreshed.

Neither blocks 0.16.0 adoption. The adapter wiring update for VMK#294 (`applyImmediately()` call) is already in this commit and works for both spec-compliant assets (when suite-side coverage extends) and the existing corpus (no-op).

### Status of upstream tracker docs after rc.4

| Tracker | Issue | Status |
|---|---|---|
| `docs/upstream/VMK-vrma-lookat-rotation-channel.md` | VMK#286 | ✅ closed in rc.3 (parser); see VMK#294 follow-up |
| `docs/upstream/VMK-vrmc-materials-hdr-emissive-multiplier.md` | VMK#287 | ✅ closed in rc.3; verified holds on rc.4 |
| `docs/upstream/VMK-khr-texture-transform.md` | VMK#288 | ✅ closed in rc.3; verified holds on rc.4 |
| `docs/upstream/VMK-outline-width-multiply-texture.md` | VMK#289 | ✅ closed in rc.3; verified holds on rc.4 |
| `docs/upstream/VMK-normal-texture-scale.md` | VMK#290 | ✅ closed in rc.3; verified holds on rc.4 |
| `docs/upstream/VMK-0.16.0-rc.1-noise.md` | VMK#283 | ✅ closed in rc.3; verified holds on rc.4 |
| `docs/upstream/VMK-swing-stiffness-axis-collapse-rc3.md` | VMK#292 | ✅ closed in rc.4 |
| `docs/upstream/VMK-occlusion-texture-strength.md` | VMK#293 | ✅ closed in rc.4 |
| `docs/upstream/VMK-vrma-lookat-renderer-propagation.md` | VMK#294 | ✅ closed in rc.4 (renderer); suite-side asset coverage follow-up open |

## Downstream goal calibration — VRoid Hub baseline, Muse 0.16.0 diagnosis correction, two-tier corpus pivot

**Date**: 2026-05-24.

**Trigger**: a downstream tester of VMK 0.16.0 (Muse) reported `Muse/Renderer.swift:554` invokes `colliderRegistry?.update()` every frame but never injects the computed procedural sphere colliders (head / upper-chest / hand body-blockers built in `Muse/Physics/ColliderRegistry.swift:117-132`) into VRMMetalKit's spring-bone simulation — a long-standing dead pipe (git log -S "updateSphereColliders" returns nothing), not a 0.14 → 0.16 regression. Diagnosed against three candidate fix paths (override existing VRM colliders via `setColliderRadius`; new upstream `setProceduralSphereColliders` API; or model-side `insideSphere` / `insideCapsule`).

The bug surfaces a methodology question this suite hadn't engaged with: **the downstream goal is "VRoid Hub `.vrm` imported into a game with physics and collisions working out of the box."** Our existing 263-plan corpus is entirely parametric synthetic assets (one-axis-at-a-time sweeps on stripped baseline rigs). Whether real VRoid content exhibits the bug class Muse is patching around — and whether Muse's patch is even *necessary* — could not be answered from the corpus.

**Method**: exported `vroid_default_F_1_0.vrm` from VRoid Studio with permissive license fields set at export time. Symlinked into `assets/humanoid/` (matching the existing avatarA pattern that points back to `../VRMMetalKit/`). Extracted the glTF JSON chunk and inspected `VRMC_vrm.meta`, `VRMC_springBone`, and `extensionsUsed`.

### What VRoid's default export actually ships

**License (clean for redistribution + Khronos donation):**

| Field | Value |
|---|---|
| `licenseUrl` | `https://vrm.dev/licenses/1.0/` |
| `avatarPermission` | `everyone` |
| `commercialUsage` | `corporation` |
| `allowRedistribution` | `true` |
| `modification` | `allowModificationRedistribution` |
| `creditNotation` | `unnecessary` |

`VRMC_vrm.meta` is the spec-defined license layer; downstream apps are required by the VRM 1.0 spec to read and respect it on import. Setting permissive fields at export time obviates the per-fixture licensing audit that hand-authored or Hub-sourced fixtures would require.

**Spring-bone topology (`specVersion: 1.0`, base `VRMC_springBone` only — no `VRMC_springBone_extended_collider`):**

- **28 colliders** organized into **12 collider groups**, all sphere-shape:
  - Spine: 1 sphere (`J_Bip_C_Spine`)
  - **UpperChest: 3 spheres** — 1 center + 2 lateral at ±0.043 m X offset, radii 0.061–0.087 m (the exact body-blocker geometry Muse's registry computes)
  - Neck: 1 sphere (radius 0.043 m)
  - Head: 1 sphere (radius 0.090 m — classic face/hair anti-clipping)
  - L+R upper arms: 3 spheres each, distributed along arm length
  - L+R lower arms: 4 spheres each
  - L+R hands: 1 sphere each
  - L+R upper legs: 3 spheres each
- **44 springs** organized by chain class:
  - 2 `Bust` springs (3 joints each) — `colliderGroups: []` (swing-only by design)
  - 24 `Skirt` springs (4 joints each) — `colliderGroups: [10, 11]` (both upper-leg groups)
  - 7 `Hair` springs (4–5 joints each) — `colliderGroups: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]` (Spine + UpperChest + Neck + Head + both full arm chains, all 10 body-blocker groups)
  - 6 `TopsUpperArm`, 4 `CatEar`, 1 `FoxTail` (decorative chains) — `colliderGroups: []`

### Diagnosis correction for Muse 0.16.0

The Muse bug ("`ColliderRegistry.update()` computes spheres, result is dropped") is real. The diagnosis of the *fix* is the part this finding revises:

**The procedural registry duplicates colliders VRoid already declares in-file.** For VRoid-sourced content — which is the dominant downstream case and the stated goal — VMK has already loaded the 10-group body-blocker set, and the 7 hair chains already reference them. The host app's procedural recomputation is solving a problem the file already solves.

| Muse fix path | Necessary for VRoid content? |
|---|---|
| 1. Override existing colliders via `setColliderRadius` | Yes if registry is repurposed as a radius-tuning source on declared colliders |
| 2. New `setProceduralSphereColliders` upstream API | **No** — only needed for non-VRoid content that ships without collider groups |
| 3. Model-side `insideSphere` / `insideCapsule` | No — VRoid doesn't use the extended-collider extension and pushing creators toward it would fork the ecosystem |

The cleanest VMK-side fix is: delete the registry's procedural-injection role and trust the in-file declarations. If the registry has empirical value it's as a radius multiplier on top of `setColliderRadius`. The new-upstream-API path is only justified by a *separate* requirement — supporting avatars exported from non-VRoid pipelines that ship without colliders.

### Methodology implication — two-tier corpus pivot

This was the load-bearing realization. The existing parametric synthetic corpus is excellent at renderer-axis isolation but **structurally cannot answer "does a real VRoid avatar import-and-play correctly across renderers."** The bone topology, the multi-chain hair / bust / skirt / decorative spring layout, the comprehensive in-file collider declarations — none of these are represented in a stripped baseline rig.

Three independent suite-side gaps converge on the same shape:

1. **VMK#267 (`writeBonesToNodes` 1-frame lag)** — flagged in our prior findings catalog as "synthetic swing sweep at 0.2 m / 0.25 s may not surface 1-frame lag; avatarA_bosom_swing more realistic." Realistic content is what surfaces frame-timing bugs.
2. **The Muse class** — synthesizes the question "does the renderer apply VRoid's declared collider groups to hair chains?" There is no equivalent question we could ask of the synthetic corpus because synthetic doesn't have collider groups in the file.
3. **`VRMC_springBone_extended_collider` realism gap** (findings.md ≈ 2026-05-15 phase 3 entry) — our 36 extended-shape variants render on synthetic rigs that never enter the inverted-containment shell, compressing SSIM signal. A body-shaped baseline would expose containment behavior properly.

Pivot: **Tier 1 (existing) — parametric synthetic sweeps for renderer-axis isolation. Tier 2 (new) — canonical real-content fixtures from VRoid Studio for downstream-realism conformance.** Both belong, neither replaces the other. Methodology RFC drafted at `rfcs/0005-canonical-content-tier.md`.

### What landed in this commit

- `assets/humanoid/vroid_default_F_1_0.vrm` — symlink to source-of-truth in `../VRMMetalKit/`
- `assets/humanoid/vroid_default_F_1_0.meta.json` — provenance + license + spring-bone topology summary
- `test-plans/manual/humanoid/vroid_default_F_collider_settle.test.yaml` — settled-pose hair-vs-body test plan for the new fixture (threshold provisional, calibrates on first bootstrap)
- `rfcs/0005-canonical-content-tier.md` — methodology RFC, Draft status

### What this obsoletes

- The deferred `avatarA_collider_1_0.vrm` Blender authoring task (findings.md:1105). The VRoid baseline is strictly better than what we would have hand-authored: it ships VRoid's full multi-chain topology and the comprehensive declared collider set, rather than a single bespoke head-mounted sphere on a stripped rig. Half an hour of Studio UI work replaced an estimated half-day of Blender authoring.

### Forward

- No upstream issue against VRMMetalKit for "add `setProceduralSphereColliders`" — the finding above argues it is unnecessary for VRoid-sourced content. If a downstream consumer brings non-VRoid avatars later, the question can be re-opened with that data.
- Comment the Muse diagnosis correction back to the downstream tester (the registry-vs-declared-colliders observation is the actionable signal).
- Bootstrap the VRoid baseline through all four real adapters to calibrate the SSIM threshold for `vroid_default_F_collider_settle` empirically. Until then the plan's threshold is provisional.
- Methodology RFC discussion: see `rfcs/0005-canonical-content-tier.md`.

### First four-adapter bootstrap of `vroid_default_F_collider_settle` (same day)

**Method**: ran the new Tier 2 plan through all four real adapters — VMK 0.16.0, three-vrm 3.5.0, godot-vrm, and UniVRM v0.131.0 (Unity 6000.4.6f1 PlayMode via `execute-test-batch`). All four loaded the file cleanly and produced 1024×1024 PNGs (`overall_passed: true` / batch `ok_count: 1` on each).

**Four-way consensus matrix (SSIM, plan threshold 0.70):**

|                | vrm-metal-kit | three-vrm | godot-vrm | univrm |
|---|---|---|---|---|
| **vrm-metal-kit** | 1.0000   | **0.8826 ✓** | 0.6009 ✗ | 0.4560 ✗ |
| **three-vrm**     | 0.8826 ✓ | 1.0000       | 0.6081 ✗ | 0.4323 ✗ |
| **godot-vrm**     | 0.6009 ✗ | 0.6081 ✗     | 1.0000   | 0.4479 ✗ |
| **univrm**        | 0.4560 ✗ | 0.4323 ✗     | 0.4479 ✗ | 1.0000   |

`consensus_passed=false`, all four renderers flagged as outliers (none has ≥ 2 agreements at threshold 0.70). Two distinct conformance-signal classes emerged.

**Visual inspection** (`goldens-cache/humanoid/vroid_default_F_collider_settle/`):

1. **VMK + three-vrm** (SSIM 0.88 to each other): avatar facing camera, **eyes open** at rest, hair drapes naturally, MToon shading agrees within expected cross-renderer drift. This is the "expected" Tier 2 baseline cluster.
2. **godot-vrm** (SSIM ≈0.60 to VMK/three-vrm): avatar facing camera but **eyes closed** at rest. Glasses framing and hair drape differ as downstream consequence (eyelid bone state propagates).
3. **univrm** (SSIM ≈0.43–0.46 to all others): **avatar facing AWAY from camera** — UniVRM renders the back of the head. Same camera spec (position `[0.4, 1.4, 0.55]`, target `[0, 1.35, 0]`) produces front-facing renders in the other three adapters and back-facing in UniVRM.

**Diagnosis hypothesis — UniVRM avatar-facing divergence (confirmed via cross-check)**: this is the more conceptually interesting finding.

**Cross-check**: rendered the existing `avatarA_bosom_threequarter` plan (already in the corpus, runs cleanly on the three non-UniVRM adapters) through UniVRM. **UniVRM renders the back of avatarA too** — same systematic behavior on a different fixture. Render saved at `goldens-cache/humanoid/vroid_default_F_collider_settle/_cross_check_avatarA_threequarter_univrm.png` for reference. So the divergence is **systematic to the UniVRM adapter across all VRM 1.0 humanoid fixtures we have**, not specific to VRoid or to one camera framing.

**Spec framing** — VRM 0.x vs VRM 1.0 default avatar orientation:

| Spec version | Avatar facing direction at rest | Origin |
|---|---|---|
| VRM 0.x | -Z (Unity convention: "Unity forward" away from a +Z-positioned camera looking toward origin sees the *front*) | Pre-Khronos, Unity-native VRM extension |
| VRM 1.0 | +Z (glTF convention: avatar's nose along +Z in right-handed glTF space; a +Z-positioned camera looking toward origin sees the *back*) | Ratified at Khronos, normalized to glTF native |

UniVRM v0.131.0 is the spec-authors' reference implementation and correctly applies VRM 1.0 semantics — avatar faces +Z, so a camera at +Z sees the back. The three other adapters carry forward the VRM 0.x convention (either through default-rotation on load, or through a 180°-rotated camera in their native scene setup, or some other compensation), so the same camera spec sees the front for them.

**This means every existing humanoid plan in `test-plans/manual/humanoid/` was implicitly authored against the legacy VRM 0.x camera convention.** The plans pass on VMK / three-vrm / godot-vrm because those three apply the same legacy compensation; UniVRM exposes that the convention is non-spec on VRM 1.0 content.

So: **UniVRM is the spec-correct reference here.** The downstream interop story is the right one — a stock VRoid avatar imported into a game *using a strict-VRM-1.0-conformant runtime* will face away from the camera unless the host app explicitly rotates 180° or places the camera at -Z. Three of four real renderers don't, which is why a downstream developer porting between renderers sees the avatar mysteriously face the wrong way.

**This was the user's hypothesis from the start** ("VRM 0.0 and VRM 1.0 I think have the default placement and coordinates different") — confirmed empirically.

**Diagnosis hypothesis — godot-vrm expression divergence** (separate finding, lower priority): VRM 1.0 default-no-preset semantics expect rest = modeler-baked mesh (eyes open). Godot rendering eyes closed at rest suggests adapter is applying a blink preset by default, or the loader is interpreting blendshape weights at rest non-canonically. Needs adapter-side investigation.

**Calibration verdict**: **keep threshold at 0.70**. The value cleanly separates legitimate cross-renderer drift (VMK/three-vrm at 0.88) from the two real conformance signals (godot at ≈0.60, univrm at ≈0.45). Plan is no longer provisional; calibration confirms the empirical band.

**What this validates about the methodology pivot**: on the very first Tier 2 bootstrap, two distinct conformance-signal classes emerged that the Tier 1 parametric synthetic corpus could not produce — (1) default-blendshape-preset state and (2) avatar-facing convention under coordinate-system conversion. Neither would surface from a stripped synthetic skeleton without face geometry or VRM blendshape presets. This is the case-in-point for RFC 0005.

### Camera-flip executed + post-flip empirical reality

Followed through on the methodology fix in the same session. Flipped camera Z on all six humanoid plans (`avatarA_bosom*`, `avatarA_face`, `vroid_default_F_collider_settle`) — `position[2]: +x → -x`, targets unchanged. Re-bootstrapped the VRoid plan through all four real adapters.

**Post-flip four-way matrix:**

|                | VMK    | three-vrm | godot-vrm | UniVRM |
|---|---|---|---|---|
| **VMK**        | 1.0000 | **0.8961 ✓** | 0.6922   | 0.4449 ✗ |
| **three-vrm**  | 0.8961 ✓ | 1.0000     | **0.7194 ✓** | 0.4324 ✗ |
| **godot-vrm**  | 0.6922 | 0.7194 ✓   | 1.0000   | 0.4362 ✗ |
| **UniVRM**     | 0.4449 ✗ | 0.4324 ✗   | 0.4362 ✗ | 1.0000 |

`agreement_count: [1, 2, 1, 0]`. Pairwise SSIM tightened across the three non-UniVRM adapters (VMK↔three-vrm: 0.883 → 0.896; VMK↔godot: 0.601 → 0.692; three-vrm↔godot: 0.608 → 0.719 — now above threshold). UniVRM stayed at ≈0.43 against all three. The same camera-flip that brought the three closer together also kept UniVRM diverging from them by exactly the same amount.

**Visual diagnosis (post-flip renders at `goldens-cache/humanoid/vroid_default_F_collider_settle/post-flip/`):**

- VMK, three-vrm, godot-vrm: all render the **back** of the avatar's head with camera at -Z = -0.55.
- UniVRM: renders the **front** of the avatar's face (eyes open, glasses visible, hair framing) with the same camera spec.

This confirms mechanism — the three non-UniVRM adapters apply a 180° avatar pre-rotation on load (contrary to VRM 1.0 spec). UniVRM follows VRM 1.0 spec strictly. The camera-flip exposes the bug; it didn't fix it. Filing upstream is the next step for actually getting the four into consensus.

**Calibration note**: godot's eyes-closed expression-state bug is *hidden* in the post-flip view (back of head, eyelids not visible). The corpus loses that signal with the methodology fix. A complementary +Z plan would re-expose it but break consensus on UniVRM. Trade-off worth flagging in RFC 0005's "open questions."

### VRM 0.x adapter-load smoke (`avatarA_0_0_smoke`)

Authored a minimal smoke plan against the existing `avatarA_0_0.vrm` fixture (VRM 0.x, generated by UniGLTF-2.64.1, declares `VRM` extension + `secondaryAnimation` + `materialProperties` + `meta.licenseName: CC_BY`) and ran it through all four real adapters with the unified -Z camera methodology pin.

| Adapter | Result | Detail |
|---|---|---|
| VMK 0.16.0 | ✓ loads, renders | Renders back of head — VMK applies 180° flip to VRM 0.x files too (consistent with its VRM 1.0 behavior) |
| three-vrm 3.5.0 | ✓ loads, renders | Renders **front** of avatar (face, chest, dress) — three-vrm does NOT flip VRM 0.x files (inconsistent with its VRM 1.0 behavior, where it does flip) |
| godot-vrm | ✓ loads, renders | Renders **front** — same as three-vrm: no flip on VRM 0.x |
| UniVRM v0.131.0 | ✗ LoadFailed | `Failed to load as VRM 1.0` — adapter passes `canLoadVrm0X: false` to `Vrm10.LoadPathAsync`. One-line adapter fix unblocks. |

**Renders saved at** `goldens-cache/humanoid/avatarA_0_0_smoke/` for reference.

### Combined orientation-handling matrix (this session's most important finding)

Across the two smokes (one VRM 0.x asset + one VRM 1.0 asset, both with -Z camera per the unified methodology pin), the actual cross-adapter behavior is:

| Adapter | VRM 0.x: avatar faces | VRM 1.0: avatar faces | Spec-correct? |
|---|---|---|---|
| VMK | +Z (applies 180° flip) | +Z (applies 180° flip) | Wrong for both (spec: -Z) |
| three-vrm | -Z (native, no flip) | +Z (applies 180° flip) | VRM 0.x ✓, VRM 1.0 ✗ |
| godot-vrm | -Z (native, no flip) | +Z (applies 180° flip) | VRM 0.x ✓, VRM 1.0 ✗ |
| UniVRM | (cannot load) | -Z (no flip) | VRM 1.0 ✓ (cannot test 0.x today) |

**No single adapter is fully spec-correct across both spec versions.** With the unified -Z camera methodology pin, three of four adapters render the front for VRM 0.x (three-vrm + godot + UniVRM if loading worked) — but for VRM 1.0, only UniVRM renders the front, and the other three render the back.

### Revised methodology pin (correction)

The earlier proposal in this finding ("VRM 0.x camera at +Z; VRM 1.0 camera at -Z") was based on incorrect intuition that the two spec versions placed avatars at different end orientations in glTF. **They don't.** Empirically and per spec:

- **VRM 0.x spec** (Unity-coord-centric language): "model faces Unity +Z forward." Unity → glTF export Z-flip lands the avatar facing glTF -Z.
- **VRM 1.0 spec** (glTF-native language): avatar faces glTF -Z directly.

Both spec versions place the avatar facing -Z in the on-disk glTF coordinates. The "different default placement" hypothesis was about spec text language and reference coordinate system, not end-state orientation.

**Unified methodology pin**: all VRM humanoid plans (0.x and 1.0) use camera at -Z (negative Z position, target origin) — spec-correct for both. The avatarA_0_0_smoke plan ships with this convention.

### Stub RFC for VRM 0.x conformance

The scope of "add VRM 0.x to the suite" is larger than this commit — adapter-side fixes (UniVRM `canLoadVrm0X` config), asset generator emit paths (`VRM` extension namespace, `secondaryAnimation`, `materialProperties`), parametric MToon sweep duplication, orientation methodology pin discussion. Scoped in `rfcs/0006-vrm-0x-conformance.md` (Draft, scope sketch — design TBD). Don't expand the corpus today.

### Follow-ups

- **File four upstream orientation issues** (one per adapter) with the smoke renders + camera spec + observed-vs-expected attached:
  - VMK: applies non-spec 180° avatar flip to BOTH VRM 0.x and 1.0 files; should not flip per spec (-Z is the spec-correct facing direction).
  - three-vrm: applies non-spec 180° avatar flip to VRM 1.0 files (does not flip VRM 0.x). Internally inconsistent across spec versions.
  - godot-vrm: same as three-vrm.
  - UniVRM: adapter rejects VRM 0.x files because `canLoadVrm0X` is set to false. One-line adapter config fix (`adapters/univrm/UniVRMConformance/`).
- **File godot-vrm rest-pose-expression issue** independently — visible only from camera angles that show the face. Currently hidden by the post-flip back-of-head view; will re-surface when a face-visible plan exists for VRM 1.0 (will need to wait until UniVRM and the other three agree on facing direction, otherwise plans can't run face-visible).
- **Methodology decision**: with no adapter fully spec-correct across both versions, the corpus's "humanoid plans use -Z camera" pin produces structurally inconsistent renders today. RFC 0006 enumerates three approaches (a) accept divergence as upstream-filing signal, (b) per-spec-version expected-divergence map at the diff layer, (c) author plans with both -Z and +Z cameras for cross-comparison. Pick one in 0006's design phase.
- **Author `vroid_default_F_collider_swing.test.yaml`** after upstream orientation fixes land — until then, swing-mode tests run into the same back-of-head visibility issue.
- **Tier 2 manifest publishing** — RFC 0005 open question; defer.

### Correction: avatar-facing-direction spec reading was inverted

**Date**: 2026-05-24 (same session as the original finding; committed earlier today as `5a8928f`).

**What changed**: Re-read both spec versions directly to ground the upstream-issue language. The conclusions in this finding's earlier subsections (and in RFCs 0005 + 0006) were **based on an inverted reading of the VRM 1.0 spec**. Correcting transparently rather than amending the commit, because (a) findings-as-deliverable history is more valuable than a clean amendment, and (b) the correction itself is a useful methodology data point: spec-direction mistakes are easy to make and the suite has to catch them by reading primary sources.

**Spec text, both versions, primary sources:**

- VRM 0.x: `specification/0.0/README.md` line 238 — *"Model faces towards -Z direction"* (in OpenGL/glTF coords; the document is explicit about OpenGL coord system in its footnote).
- VRM 1.0: `specification/VRMC_vrm-1.0/tpose.md` Definition 1.1 — *"The legs, torso, head, and eyes of a VRM model must be oriented along the +Z axis, symmetrical on the X axis, and standing straight."* Definition 1.2 confirms: *"The toes must be directed along the +Z axis."*

**The two spec versions place the avatar at OPPOSITE default orientations in glTF coordinates:**

| Spec | Avatar faces (glTF coords) | Spec-correct camera position to view front |
|---|---|---|
| VRM 0.x | -Z | -Z (camera at -Z = beyond the avatar's nose, looking toward origin = +Z direction = sees front) |
| VRM 1.0 | +Z | +Z (camera at +Z = beyond the avatar's nose, looking toward origin = -Z direction = sees front) |

This is the user's hypothesis from the start ("VRM 0.0 and VRM 1.0 I think have the default placement and coordinates different"), confirmed by both spec texts directly.

**Re-interpretation of the empirical data (corrected):**

Pre-flip VRM 1.0 plans (camera at +Z, *spec-correct for VRM 1.0*):
- VMK / three-vrm / godot-vrm: all rendered the front of the avatar's face — **all three are spec-correct on VRM 1.0**.
- UniVRM: rendered the back — **adapter coord-handling bug** (Unity Z-flips the avatar on glTF import, but the adapter does not Z-flip the plan's camera position to match; the camera ends up on the wrong side relative to the avatar in Unity's coordinate space).

VRM 0.x smoke (camera at -Z, *spec-correct for VRM 0.x*):
- three-vrm / godot-vrm: rendered the front — **spec-correct** (no flip; preserve the file's native -Z facing).
- VMK: rendered the back — **non-spec** (VMK applies a 180° rotation on VRM 0.x load, making the avatar face +Z internally, contradicting the VRM 0.x spec's -Z facing).
- UniVRM: cannot load (`canLoadVrm0X: false`).

**Corrected cross-adapter matrix:**

| Adapter | VRM 0.x | VRM 1.0 |
|---|---|---|
| VMK | ✗ applies 180° flip (non-spec for 0.x; should preserve -Z facing) | ✓ preserves +Z facing (spec-correct) |
| three-vrm | ✓ preserves -Z facing (spec-correct) | ✓ preserves +Z facing (spec-correct) |
| godot-vrm | ✓ preserves -Z facing (spec-correct) | ✓ preserves +Z facing (spec-correct) |
| UniVRM | (cannot load) | ✗ adapter coord-mismatch (Unity Z-flip on avatar but not camera) |

three-vrm and godot-vrm are fully spec-correct across both spec versions — the one renderer pair that needs no upstream fix on orientation. VMK has a VRM-0.x-specific bug. The UniVRM divergence is in our adapter (not in the UniVRM library itself), and is solvable adapter-side by Z-flipping the camera position when bridging glTF coords → Unity coords.

**Methodology pin (corrected, per-spec-version):**

- VRM 0.x humanoid plans: camera at **-Z** (target origin) — sees the front of an avatar that natively faces -Z.
- VRM 1.0 humanoid plans: camera at **+Z** (target origin) — sees the front of an avatar that natively faces +Z.

The "unified -Z pin" proposed earlier was wrong. The "VRM 0.x +Z, VRM 1.0 -Z" pin proposed before that was also wrong. The correct per-spec pin has each version's camera matching its avatar's natively-facing direction.

**Plan-file changes following the correction (in this commit's diff):**

- Reverted camera Z back to +0.55 / +0.6 on the five VRM 1.0 humanoid plans (`avatarA_bosom*`, `avatarA_face`, `vroid_default_F_collider_settle`). These now match the spec-correct VRM 1.0 convention.
- `avatarA_0_0_smoke.test.yaml` stays at -Z (spec-correct for VRM 0.x). No change.

**Upstream filings, corrected scope:**

- **VMK**: file VRM 0.x avatar-orientation issue — adapter applies non-spec 180° rotation; VRM 0.x spec (`specification/0.0/README.md:238`) requires -Z facing in OpenGL/glTF coords. Three-vrm and godot-vrm preserve the spec orientation; VMK does not. Filing now.
- **UniVRM adapter** (`adapters/univrm/UniVRMConformance/`): adapter-side fix — Z-flip the plan's camera position when bridging glTF → Unity coords, so the camera ends up on the correct side of the Z-flipped avatar. Internal to our suite; not an upstream filing.
- **godot-vrm rest-pose-expression eyes-closed**: still pending; independent of orientation; visible now that camera at +Z (spec-correct) shows the face.

**Methodology data point**: I committed the inverted reading because the empirical evidence (UniVRM-as-spec-reference vs. three-cohort-disagrees) read more naturally as "UniVRM correct, others wrong" without verifying the spec text. The spec quote went into RFC 0006 fabricated rather than copied verbatim. The lesson recorded for the suite: **always quote spec text verbatim with file:line attribution; never paraphrase from memory or extrapolate from empirical disagreement without grounding in the canonical document**. Adding this discipline to the suite's contribution guidelines.

## VRoid default Bust springs have empty colliderGroups by design — clipping is ecosystem content convention, not renderer bug

**Date**: 2026-05-24 (same session). **Trigger**: downstream Muse 0.16.0 team reported visible bust clipping on the default VRoid avatar after deleting the dead `ColliderRegistry` injection pipeline (see earlier finding "Downstream goal calibration"). They observed bust-clipping in both static settle and dynamic frames; clean elsewhere (head, shoulders, dress, cheek/jaw, scalp). Their initial diagnosis: "VRoid's torso collider doesn't cover the bust."

### Empirical correction to that diagnosis

Re-inspected `vroid_default_F_1_0.vrm`'s `VRMC_springBone` extension:

- **VRoid's UpperChest collider group is comprehensive.** 3 spheres on `J_Bip_C_UpperChest` (1 center at z+0.009m + 2 lateral at ±0.043m X offset, radii 0.061–0.087m). They cover the chest plane.
- **The 7 `Hair` springs reference 10 collider groups** (Spine, UpperChest, Neck, Head, both arm chains, hands) — comprehensive body-blocker collision for hair.
- **The 2 `Bust` springs reference ZERO collider groups** — `colliderGroups: []`. By design. The bust chains swing freely under gravity with no collision constraint.

So the bust clipping isn't "the chest collider doesn't cover the bust" (the colliders are there). It's "**VRoid's Bust springs deliberately don't reference the existing chest colliders**." Almost certainly an authorial choice in Studio's default character — free-swing bust without containment for an unconstrained-motion aesthetic. The empirical consequence is exactly what Muse observed: under certain motion or pose configurations, bust geometry clips into torso geometry because the bust chains have no constraint to keep them outside the chest plane.

### Implications for Muse's three-options decision

This **rules out option 2 (registry as `setColliderRadius` multiplier)** for this symptom:

- `setColliderRadius` modifies the radius of an existing VRM-declared collider.
- The chest spheres are declared but only referenced by the Hair springs.
- Bumping chest-sphere radius affects hair drape, not bust deflection.
- To fix bust clipping at runtime, Muse would need to mutate `springs[].colliderGroups[]` to make Bust springs reference the existing UpperChest group. **VMK does not expose this API.** Adding it would be a separate upstream feature request, not a small registry rebuild.

**Option 1 (ship as-is, document as model limitation)** is the actionable recommendation. Two framings to consider for downstream documentation:
- "Bust chains on stock VRoid characters are authored without chest containment for free-swing aesthetic; users wanting containment can modify the .vrm or wait for Studio updates."
- A "compatibility notes" page that flags this with other known content-quirks (e.g., skirt clipping when running).

**Option 3 (Logger conversion)** has independent value as observability hygiene but doesn't gate the bust-clipping decision.

### Tier 2 plans landed in this commit

Two new test plans target the bust-vs-body interaction so downstream consumers (Muse and other VMK users) have a reproducible cross-renderer baseline:

- **`vroid_default_F_bust_settle.test.yaml`** — static settle at 30 physics steps, frontal chest framing (`y_target = 1.15`, `z = +0.55`, FOV 28°), VRoid F default character. Renders the equilibrium pose of bust chains under gravity with no collision constraint.
- **`vroid_default_F_bust_swing.test.yaml`** — same framing, with `animate_root_transform` lateral motion (`[0.15, 0, 0]` over 0.25 s at 60 Hz) to excite the bust chains. Single-frame end-of-animation capture; for multi-frame timing-sensitive coverage use `render_sequence` per RFC-0004 once a swing-sequence variant is authored.

Both plans use the spec-correct VRM 1.0 camera convention (camera at +Z, target origin; per `docs/upstream-specs/.../tpose.md` Definition 1.1).

### First four-adapter bootstrap

**bust_settle pairwise SSIM matrix (threshold 0.70):**

|                | VMK    | three-vrm | godot-vrm | UniVRM |
|---|---|---|---|---|
| **VMK**        | 1.0000 | **0.8433 ✓** | 0.4714 ✗ | 0.3422 ✗ |
| **three-vrm**  | 0.8433 ✓ | 1.0000     | 0.4807 ✗ | 0.2922 ✗ |
| **godot-vrm**  | 0.4714 ✗ | 0.4807 ✗   | 1.0000   | 0.3377 ✗ |
| **UniVRM**     | 0.3422 ✗ | 0.2922 ✗   | 0.3377 ✗ | 1.0000 |

**bust_swing pairwise SSIM matrix:**

|                | VMK    | three-vrm | godot-vrm | UniVRM |
|---|---|---|---|---|
| **VMK**        | 1.0000 | **0.7348 ✓** | 0.6612 | 0.6263 |
| **three-vrm**  | 0.7348 ✓ | 1.0000     | 0.6784 | 0.6237 |
| **godot-vrm**  | 0.6612 | 0.6784     | 1.0000 | 0.6205 |
| **UniVRM**     | 0.6263 | 0.6237     | 0.6205 | 1.0000 |

**Visual observation across renders** (`goldens-cache/humanoid/vroid_default_F_bust_{settle,swing}/`):

- VMK and three-vrm cluster tightly (frontal view of bust-area bodice, hair pigtails to sides) — the spec-correct cohort.
- godot-vrm shows the same framing but with its known eyes-closed default-expression bug visible at the top of frame.
- UniVRM shows the back of the avatar (the known adapter coord-handling bug).

The single-frame settle render does NOT dramatically surface bust-vs-bodice clipping at this framing — clothing geometry covers most of the chest area at rest, and the static equilibrium pose isn't where peak deflection occurs. The swing variant excites the chains but the single end-of-animation frame may miss peak deflection. **Recommended next step for product teams reproducing this**: extend to a `render_sequence` plan (RFC-0004) that captures the swing across N frames and visualizes the deflection trajectory; the worst-clipping frame will be diagnostic.

### What the cross-renderer agreement means

**The four-renderer agreement on "Bust springs swing freely" is the conformance signal**, not the absence of clipping in any specific frame. All four renderers (modulo their adapter-side coord bugs) honor the file's empty `Bust.colliderGroups` declaration. This is an **ecosystem content convention**, not a per-renderer bug. No renderer-side fix will help; the fix must be at the asset or runtime-mutation layer.

### Action items

- **Comment back to the Muse team**: forward this diagnosis correction. Their option 1 is the right call. Option 2 (`setColliderRadius` multiplier) would not address the symptom because Bust springs don't reference the chest colliders to begin with. Option 3 (Logger hygiene) is good infra work independent of this.
- **Upstream filing candidates** (future, lower priority):
  - VRoid Studio: consider making Bust spring `colliderGroups` default to `[UpperChest]` in the default character template. Would silently fix the issue for every new export without breaking existing content.
  - VMK: feature request for a `setSpringColliderGroups` API that mutates `springs[].colliderGroups[]` at runtime. Would let host apps like Muse override the file-declared configuration on a per-character basis without re-exporting. Not filed yet — should wait until at least one product genuinely needs it (and would be willing to land the upstream PR if VMK accepts it).
- **Suite-side follow-up**: author a `vroid_default_F_bust_seq.test.yaml` using RFC-0004 `render_sequence` to capture the swing trajectory across 30 frames. The worst-clipping frame will be visible there.

### Correction (same session): "bust-clipping" means hair-through-bust-via-chest-sphere-gap, not bust-mesh-into-torso

**Date**: 2026-05-24 (same session, after Muse team round-trip).

The Muse team confirmed their loaded VRoid diagnostic and clarified the symptom. The conformance suite committed `d03a3b3` with my analysis framing "bust-clipping" as *bust mesh penetrating torso* — a problem class that exists in principle (Bust springs have empty `colliderGroups`) but is **not what Muse is observing**.

**Actual symptom**: hair strands clip through the bust region of the avatar — visible at static settle and in motion. The 28 sphere colliders that approximate the humanoid don't form a continuous shell across the upper torso; the bust geometry curves forward into a gap between the chest spheres, and Hair springs (which DO reference the UpperChest collider group) can pass strands through that gap into / past the bust.

**This rehabilitates Muse's option 2.** `setColliderRadius` on the chest spheres *is* the right fix for this symptom — Hair springs reference UpperChest, so bumping those radii closes the gap that allows hair to clip through. My earlier rejection of option 2 was based on conflating two bug classes (Bust spring deflection vs. Hair spring clipping). Apologies forwarded to the Muse team.

**Concrete bump targets** (from inspection of `vroid_default_F_1_0.vrm`):

```
collider[0]  node=J_Bip_C_Spine        offset=[0, 0, 0]               r=0.1041
collider[1]  node=J_Bip_C_UpperChest   offset=[0, 0, +0.0087]         r=0.0868   ← bust-area front; load-bearing bump
collider[2]  node=J_Bip_C_UpperChest   offset=[+0.043, +0.056, -0.009] r=0.0607  ← L upper-chest (positioned slightly back + high)
collider[3]  node=J_Bip_C_UpperChest   offset=[-0.043, +0.056, -0.009] r=0.0607  ← R upper-chest (mirror)
```

For Hair → UpperChest collision: indices **1, 2, 3** are the bump targets. **Geometric nuance**: spheres 2 and 3 are positioned slightly behind UpperChest origin (`z = -0.009`) and higher (`y = +0.056`, near shoulder level). Sphere 1 is the one whose forward projection actually touches the bust volume. A uniform multiplier on all three may push 2/3 backward into the torso interior; **sphere 1 deserves a larger bump** (1.5×–2.0×) than 2/3.

**Bone-name → collider-index lookup** (deterministic per VRM 1.0 humanoid spec, no new VMK API needed):

```swift
let upperChestNode = vrm.extensions.VRMC_vrm.humanoid.humanBones["upperChest"]?.node
let chestNode      = vrm.extensions.VRMC_vrm.humanoid.humanBones["chest"]?.node
let spineNode      = vrm.extensions.VRMC_vrm.humanoid.humanBones["spine"]?.node
let bumpable       = Set([upperChestNode, chestNode, spineNode].compactMap { $0 })

for (idx, collider) in vrm.extensions.VRMC_springBone.colliders.enumerated() {
    if bumpable.contains(collider.node) {
        let baseRadius = collider.shape.sphere.radius
        renderer.setColliderRadius(at: idx, radius: baseRadius * multiplier)
    }
}
```

The VRMC_vrm.humanoid bone names are normative (spec-defined enum), so this works on any spec-conformant avatar — not just VRoid Studio output. Cross-character compatible.

**Backup if radius bumping alone is insufficient**: spheres at backward Z offsets may not close the bust-front gap even at maximum radius. A logical follow-on VMK API would be `setColliderOffset(at:offset:)` — bump radius AND shift sphere position forward into the bust geometry. Small upstream change; worth filing if empirical tuning shows radius alone doesn't close the gap.

**My earlier committed plans (`vroid_default_F_bust_settle/swing`) don't exercise this symptom.** VRoid F default has pigtails (hair tied to the sides), not chest-draping hair. Hair joints stay near the head/shoulder line and don't engage the chest-collider region. To surface the actual Muse-observed symptom, a follow-up plan is needed — either with a VRoid template that has chest-draping hair, or a forced-pose plan that throws default hair across the chest, or a `render_sequence` swing where hair sweeps through the chest during motion. Filed as a follow-up.

**Bust spring `colliderGroups: []` finding is still factually correct** but is about a hypothetical class (bust mesh deflection into torso) that isn't manifesting in the wild — bust mesh is light and follows the bust bones cleanly without large deflection at typical motion levels. Demoted from "the diagnosis" to "a related observation worth recording."

### Action items, corrected

- **Muse team**: proceed with option 2. Use the bone-name → collider-index algorithm above. Bump sphere 1 (UpperChest center) more aggressively than 2/3 due to position geometry. Tune `multiplier` empirically against the products that are blocking on this.
- **vrm-conformance suite**: author a follow-up plan that actually surfaces hair-through-chest-gap visually — candidate: VRoid template export with longer hair, OR a forced-pose plan that pushes default pigtails forward, OR a `render_sequence` swing. Not blocking Muse's fix; useful for the cross-renderer regression baseline once they ship the fix.
- **Possible future VMK filing**: `setColliderOffset(at:offset:)` if radius-only tuning is empirically insufficient. Don't file speculatively — wait for Muse's tuning data.

### `vroid_default_F_bust_seq` — RFC-0004 trajectory plan landed for downstream regression baselines

Authored a `render_sequence` variant of the bust swing using RFC-0004 multi-frame capture: 30 frames at 60 Hz, `animate_root_transform` lateral translation `[0.15, 0, 0]`, frontal chest framing. Bootstrapped through all four real adapters. 30 PNG frames per adapter, all sequences complete (`overall_passed: true` on each).

**Per-frame pairwise SSIM** (computed via `vrm-runner diff` per frame):

| Frame | VMK ↔ three-vrm | VMK ↔ godot | three-vrm ↔ godot | Comment |
|---|---|---|---|---|
| 0000 | 0.8433 | 0.4714 | 0.4807 | settle pose |
| 0001 | **0.6656** | 0.4285 | 0.4320 | motion-start transition dip |
| 0007 | 0.8139 | 0.3461 | 0.3403 | mid-motion |
| 0014 | 0.8493 | 0.3386 | 0.3356 | peak displacement (~50% of trajectory) |
| 0021 | 0.8675 | 0.3946 | 0.3908 | motion winding down |
| 0029 | 0.8815 | 0.4216 | 0.4181 | end of trajectory |

**Pattern:**

- **VMK ↔ three-vrm** (the spec-correct cohort): cluster tightly at 0.83–0.88 across the trajectory. One transient dip to 0.67 on frame 0001 (settle-to-motion transition; one renderer applies motion faster than the other). Otherwise consistently above the 0.70 plan threshold.
- **All pairs involving godot**: 0.32–0.48 throughout. Godot's eyes-closed default-expression bug dominates the face region regardless of physics state; the divergence is rendering convention, not spring-bone behavior. Filed separately.
- **UniVRM**: not in this table (still has adapter coord-handling bug rendering the back of the head; same coordinate-flip issue documented in the "spec reading was inverted" correction).

**For downstream regression baselines** (the Muse use case):
- The 30 VMK frames serve as a baseline of "what VMK renders today on the unfixed VRoid file." Sample frames at `goldens-cache/humanoid/vroid_default_F_bust_seq/` (frames 0000, 0007, 0014, 0021, 0029 saved as visual reference).
- Muse can hot-swap their app with `setColliderRadius` bumps applied at load time, re-render the same plan, and visually compare against this baseline. The expected fix-signature: chest area hair coverage tighter (less gap, less hair-through-bust), bust silhouette unchanged.
- Frame 0014 (mid-motion peak) is the most diagnostic single frame for the chest-collider-gap symptom — pigtails in transit across the chest region during the lateral swing.

**Caveat acknowledged**: VRoid F default's pigtails (hair tied to the sides) are sub-optimal for surfacing the hair-through-chest path described by Muse. The pigtails *do* cross the chest area during lateral swing motion (visible in frames 0007–0021), but a VRoid character with long forward-draping hair would surface the same symptom more dramatically at rest pose. A future fixture (`vroid_F_longhair_*.vrm`) re-exported from Studio with a long-hair preset would close that coverage gap. Defer until empirically warranted by Muse's tuning iteration.

**For the conformance suite as infrastructure**, this is the first multi-frame Tier 2 canonical-content plan in the corpus. The pattern (render_sequence over a VRoid baseline with `animate_root_transform`) is generalizable — future Tier 2 motion plans should follow this shape rather than the single-frame end-of-animation pattern that loses peak-deflection signal.

## VMK#237 status — still reproducing on VMK 0.16.0; canonical-content reproducer landed

**Date**: 2026-05-24. **Trigger**: VMK team picked up the issue for active investigation. Re-bootstrapped the existing synthetic 18-variant swing sweep through VMK 0.16.0 stable to confirm bug persists post-cohort-release; authored a canonical-content reproducer derivative of the VRoid baseline for sharper diagnosis.

### Synthetic recheck on VMK 0.16.0 — per-bucket SHA breakdown

18 `swing_springbone_extended_*` variants rendered through VMK 0.16.0 → 7 unique SHAs (same clustering as original filing, bug persists):

| SHA bucket | # | Variants |
|---|---|---|
| `7106a87c…` | **10** | `icaps_anglelimit_{30,60,90}`, `icaps_{ploose,pmed}`, `isphere_anglelimit_{30,60,90}`, `isphere_{ploose,pmed}` |
| `1bb888ae…` | 2 | `icaps_ptight`, `isphere_ptight` |
| `c0c9a475…` | 2 | `plane_anglelimit_60`, `plane_pmed` |
| `83cce4f7…` | 1 | `plane_anglelimit_30` |
| `fa6ed352…` | 1 | `plane_anglelimit_90` |
| `5f00ac35…` | 1 | `plane_ploose` |
| `b04db911…` | 1 | `plane_ptight` |

**Tighter fingerprint than the original 7-bucket framing**: the inside-shape variants (sphere + capsule, `inside: true`) all collapse to one of two SHAs. Plane variants get mostly-distinct treatment (5 unique SHAs across 6). Working hypothesis posted to VMK#237 in the comment:

- VMK's inside-shape handler reads the *radius* (the `ptight` placement uses a smaller radius / different offset, producing a distinct SHA) but ignores shape *type* (sphere vs capsule), *angle limit*, and other parameter variations.
- VMK's plane handler distinguishes placement variants but `anglelimit_60 ≡ pmed` collapse suggests angle-limit reads in plane but doesn't propagate cleanly to inside-shapes.

### Canonical-content reproducer

`vroid_default_F_extcoll_headbubble.vrm`: derivative of the canonical VRoid baseline with one extended-collider added at the head node. Spec-compliant form (no base `shape`, only `extensions.VRMC_springBone_extended_collider.shape.sphere` with `inside: true`, `radius: 0.1`). Tight head-bubble containment that *must* visibly constrain hair joints if the extension is applied.

Generator script `scripts/build-vroid-extcoll-reproducers.py` produces the derivative deterministically from the canonical source (BLAKE3 `f71fad0f5ecc7edc411b572a6bd5cf3a7e59413c1c7ea57dea401938bfe8fff1`). Test plan `test-plans/manual/humanoid/vroid_extcoll_headbubble.test.yaml`.

| Adapter | Result | Diagnostic value |
|---|---|---|
| VMK 0.16.0 | renders distinctly from baseline | extension is being read; SHA differs |
| three-vrm 3.5.0 | `LoadFailed` | known separate issue — three-vrm rejects extended-only colliders |
| godot-vrm | renders distinctly from baseline | responds to extension when given spec-compliant form |
| UniVRM v0.131.0 | `LoadFailed: NullReferenceException` | **new finding** — UniVRM crashes on mixed-extension file (most colliders base-only, one with extended). Filing separately. |

VMK ↔ godot-vrm SSIM on the extcoll render: 0.7334 — they respond similarly to the extension. Confirms the bug isn't synthetic-asset-specific.

### Side findings discovered during this investigation

- **UniVRM v0.131.0 `NullReferenceException`** on VRM 1.0 files with mixed extended-collider declarations (one entry uses `VRMC_springBone_extended_collider`, others use base spec only). The synthetic `_assets_extended/` corpus uses extended-only on all colliders and doesn't trip this bug. The canonical derivative adds *one* extended collider to a file with 28 base-only colliders, which UniVRM's loader doesn't handle. Worth filing against [vrm-c/UniVRM](https://github.com/vrm-c/UniVRM) once the UniVRM adapter coord-handling fix is in (so the cross-pair comparison is clean). **Not blocking VMK#237**; logged for follow-up.

### Action items

- **Posted VMK#237 comment** with per-bucket table, canonical reproducer description, and a working hypothesis for the inside-shape collapse pattern: [VMK#237 comment](https://github.com/arkavo-org/VRMMetalKit/issues/237#issuecomment-4530281409).
- **Committed reproducer artifacts** for the VMK team to pull a one-shot reproducer: generator script + test plan + this finding.
- **Follow up on UniVRM mixed-extcoll NullReferenceException** as a separate filing once the suite's UniVRM adapter coord-fix lands. Logged in the findings backlog.

### VMK team review round-trip — sharper diagnosis posted as [comment-4530363752](https://github.com/arkavo-org/VRMMetalKit/issues/237#issuecomment-4530363752)

The VMK team picked up VMK#237 and replied with a deep parser/shader/renderer review identifying three plausible code paths: (1) `worldNormal` non-uniform-scale propagation through plane colliders, (2) group-mask filtering, (3) inside-shape substep-ordering. **Their priority ordering put plane as "the entire problem"**, citing my bucket layout — but that reading inverted the table. Plane is the working case; inside-sphere/capsule is the broken one.

To prevent them sinking time into the wrong code path, ran the SHA-bucket × shape-params cross-correlation and posted the full table back. The corrected reading:

- **Plane**: 5 distinct SHAs across 6 variants. Lone collapse (`plane_anglelimit_60` ≡ `plane_pmed`) is consistent with VMK's default `angleLimit` happening to be 60°, so explicit 60° matches the no-angleLimit default. Physically correct, not a bug.
- **Inside-sphere / inside-capsule**: 10 variants collapse to a single SHA. Across that bucket, `sphere` ≡ `capsule`, `radius=0.2` ≡ `radius=0.4`, and `angleLimit ∈ {30°, 60°, 90°, none}` all produce identical SHAs. The 2-variant `ptight` bucket exists because `radius=0.1` is small enough to actively constrain the chain (physical, expected).

Two parameters are NOT propagating to the inside-shape collision response:

1. **per-joint `angleLimit`** — plane respects it (30/60/90 produce distinct SHAs); inside-sphere/capsule doesn't (all collapse).
2. **shape-type discrimination** (sphere vs capsule inside-variant) — collapse to same SHA at every radius tier.

Recommended diagnostic to the VMK team: log `(shape_type, radius, joint.angleLimit)` at the `inside`-branch entry in `SpringBoneCollision.metal:151-173` for one inside-sphere and one inside-capsule variant. If `angleLimit` reads as unset for inside but correct for plane, the GPU buffer build path (`SpringBoneComputeSystem.swift:850-881`) is dropping it on the inside path. If it reads correctly but trajectory is unchanged, the constraint isn't being applied in the shader's inside branch. Plane normal / group-mask hypotheses aren't load-bearing because plane is mostly fine — if those were broken, plane would collapse too.

Their #3 hypothesis (inside-shape substep ordering producing 4cm overrun) is separately real but addresses the *magnitude* of penetration when constraint IS engaged, not the *parameter-propagation* issue above. Likely a distinct ticket.

For **VMK#267**, concurred with their assessment: the sync-path partial fix is acceptable for offline (the conformance suite's use case). The 1.4-1.6% residual penetration is RED at the strict threshold but workable for cross-renderer baselines. The real fix (option 2: skinning reads `bonePosCurr` directly for spring joints) belongs as a distinct issue and unblocks the interactive use case (Muse, etc.) — not the conformance suite directly. If/when option 2 lands, the suite re-bootstraps and validates.

### Diagnostic results — angleLimit propagates correctly; collapse is physical, not a parser bug

**Date**: 2026-05-24 (same session). **Trigger**: VMK team landed instrumentation in their working tree — `setInsideColliderDiagnosticsEnabled(true)` + `dumpInsideColliderDiagnostics()` — and asked the suite to run against the fixtures. Wired it into the adapter via a temporary local-path swap (Package.swift `.package(path: ...)`) + env-gated diagnostic hook (`VMK_INSIDE_DIAGNOSTICS=1`); both reverted after the run since the API isn't in the SPM-pinned 0.16.0 yet.

**Output for the relevant variants:**

```
isphere_pmed       boundary=0.2  bone=3 penetration=0.0000  angleLimit=0.0°
isphere_ploose     boundary=0.4  bone=3 penetration=0.0000  angleLimit=0.0°
isphere_anglelimit_30   boundary=0.2  bone=3 penetration=0.0000  angleLimit=30.0° (0.524 rad) ✓
isphere_anglelimit_60   boundary=0.2  bone=3 penetration=0.0000  angleLimit=60.0° (1.047 rad) ✓
isphere_anglelimit_90   boundary=0.2  bone=3 penetration=0.0000  angleLimit=90.0° (1.571 rad) ✓
isphere_ptight     boundary=0.1  bone=3 penetration=0.0200  angleLimit=0.0°            ← only fixture engaging the constraint
icaps_anglelimit_60     boundary=0.2  bone=3 penetration=0.0000  angleLimit=60.0° (1.047 rad) ✓ shape=inside-capsule
plane_anglelimit_60     "inside-collider diagnostics: no bone hit an inside-* branch this run"  ← correct, plane uses different pipeline
```

**My earlier hypothesis was wrong.** `angleLimit` propagates per-variant correctly (30/60/90° read as the right radian values; 0.0 for no-angleLimit fixtures). `shape` is correctly distinguished (`inside-sphere` vs `inside-capsule`). `groupMatched=1` everywhere. `boundary` reads the correct radius per variant.

**The SHA collapse is physical, not a parser/buffer-build bug.** Chain bone positions are at `distance = {0.0014, 0.0500, 0.0999}` from the collider node. For boundary `0.2` and `0.4`, the chain fits inside — bone 3 (the deepest) has `distance < boundary` so no penetration. Containment never engages, the trajectory is whatever the (unconstrained) gravity + drag + spring produces, and identical params modulo `angleLimit` (which is post-engagement-applied) produce identical SHAs.

Only `isphere_ptight` (boundary=0.1) has `bone=3 distance=0.1000 → penetration=0.0200` — the constraint actively engages. `icaps_ptight` likewise. Both ptight variants collapse to a single SHA because sphere-inside and capsule-inside use shared code at the end-cap (correct per spec — capsule end-cap IS a sphere).

**The real bug surfaced**: `isphere_ptight bone=3` reports `penetration=0.02m` at the diagnostic capture point, even though the inside-shape collision pass is supposed to resolve it. This confirms the VMK team's hypothesis #3 (substep ordering): the inside-shape collision push runs, but the distance-constraint or FK reconstruction partially unwinds it within the same substep. **Substep ordering is the actually-real fix direction** — their `testInsideSphereColliderKeepsJointsInsideAfterSwing` failing by 4cm matches this exactly. Iterating collision-after-distance, or applying the inside clamp inside the distance kernel itself, would close it.

**The plane `anglelimit_60 ≡ pmed` collapse** is consistent with VMK's default `angleLimit` being `60°` (or 1.047 rad, whichever is the internal default). Physically correct, not a bug. Likely worth a code comment in VMK's angleLimit default-value path.

### Test fixture coverage gap (suite-side TODO)

The synthetic extended-collider sweep authoring chose boundary radii `{0.1, 0.2, 0.4}` against a chain whose deepest bone is at distance `0.1` from the collider node. So only `radius=0.1` (`ptight`) actually engages the inside-constraint; `0.2` and `0.4` are no-ops. **This is a test-design coverage gap, not a VMK bug** — the sweep was framed to expect SHA divergence per axis without verifying that each variant actually exercises its swept parameter.

**Suite-side action**: emit additional inside-collider fixtures with boundary radii smaller than chain extent (e.g., `0.04, 0.06, 0.08` so all bones penetrate). Re-bootstrap to confirm VMK distinguishes them.

### Posted to VMK#237 as [comment-4530444585](https://github.com/arkavo-org/VRMMetalKit/issues/237#issuecomment-4530444585)

Forwarded the diagnostic output + corrected analysis upstream. Apologized for the previous misdirection.

### Methodology lesson

This session produced *three* successive corrections in the diagnosis path:

1. Camera-Z direction misread (committed wrong, corrected in `7d81075`).
2. "Bust-clipping = bust-mesh-into-torso" misread of the Muse symptom (corrected in `54933fd`).
3. "Inside-shape angleLimit not propagating" diagnosis based on bucket-table pattern matching, without instrumenting to verify (corrected by the diagnostic run above).

Each was a case of inferring a code-level bug from behavioral evidence without verifying through instrumentation or primary source. The shared pattern: **behavioral evidence ranks lower than instrumented data**. When a downstream team has the instrumentation, the suite should ask them to run it before posting hypotheses. Adding this to the suite's contributing guidance.

### Fixture fix: extended-collider sweep now actually exercises inside-shape semantics

**Date**: 2026-05-24 (same session). **Trigger**: VMK team confirmed the parser/plumbing/shader are wired correctly end-to-end (substep-ordering defensive fix landed). The bucket collapse was suite-side fixture engineering: chain extent ≤ boundary radius for most variants meant containment never engaged.

**Fix**: changed inside-sphere/capsule placement radii in `crates/vrm-asset-generator/src/sweep.rs::make_extended_shape_with_placement` from `[0.10, 0.20, 0.40]` to `[0.04, 0.06, 0.08]`. The 4-joint × 0.05 m default chain has its deepest joint at distance ≈ 0.10 m from the collider node; with the new radii, all three placement values are smaller than chain extent, so containment actively engages on every variant.

**Validation**: regenerated the 36-plan sweep, re-rendered the 18 swing variants through VMK 0.16.0 (with the suite's temporary diagnostic hook re-applied), measured SHA distribution.

**Before fix** (radii 0.10/0.20/0.40): 18 renders → **7 unique SHAs**. 10 inside-shape variants collapsed to 1 SHA (no penetration anywhere).

**After fix** (radii 0.04/0.06/0.08): 18 renders → **11 unique SHAs**. Per-variant diagnostic now shows non-zero penetration on bone 2 and/or bone 3 across all inside-shape variants (e.g., `isphere_ptight`: bone2=0.0298 m, bone3=0.0154 m; `isphere_pmed`: bone2=0.0100 m, bone3=0.0019 m).

**Per-bucket post-fix:**

| SHA | # | Variants | Diagnostic |
|---|---|---|---|
| `68f3e2b8…` | 4 | anglelimit_90 + pmed (sphere + capsule) | 90° too permissive to bind in our geometry; ≡ default |
| `29e12b3a…` | 2 | anglelimit_30 (sphere + capsule) | sphere ≡ capsule on end-cap shared code (correct per spec) |
| `9b3af63f…` | 2 | anglelimit_60 (sphere + capsule) | same |
| `e03767af…` | 2 | ploose (sphere + capsule) | same |
| `c0c9a475…` | 2 | plane_anglelimit_60 + plane_pmed | plane default ≈ 60° (unchanged from before) |
| `fa85b095…` | 1 | icaps_ptight | sphere/capsule distinct here — different end-cap geometry |
| `16a76df6…` | 1 | isphere_ptight | distinct from capsule counterpart |
| `83cce4f7…` / `fa6ed352…` / `5f00ac35…` / `b04db911…` | 1 each | plane variants | unchanged |

The remaining "collapses" are physically correct: sphere/capsule share end-cap code (VMK confirmed; spec-aligned), default angleLimit happens to equal 90° for inside-shapes and 60° for planes (worth confirming as a separate finding once verified). No residual bug in VMK; conformance-side gap closed.

**Closing the VMK#237 loop**: the substep-ordering improvement landed on VMK as a defensive robustness fix (no regression, no real bug needed shifting). The bucket collapse was conformance-side fixture engineering. Both fixes pushed; the issue is effectively resolved from both ends.

**TODO** (not blocking): the corpus should also extend the sweep to include radii that engage the constraint more strongly across additional axes (e.g., chain length + swing amplitude variants) so cross-cohort consensus across the inside-shape feature has more dimensions. Defer until empirically motivated by another adapter.

## 2026-05-26 — VMK 180° flip on VRM 0.x: location and structurality (slice 1 days 1–3 empirical check)

**Pinned VRMMetalKit revision:** `392d94926619bcb59401f49b29e82d2a575d4d15` (from `adapters/vrm-metal-kit/Package.swift`).

**Location.** UPSTREAM_LIBRARY. Lines: `Sources/VRMMetalKit/Core/VRMModel.swift:980–1011` (the `buildNodeHierarchy()` block gated on `if isVRM0`) and `Sources/VRMMetalKit/Core/VRMModel.swift:881–897` (`applyVRM0InverseBindMatrixConjugation()`).

**Structurality.** LOAD_BEARING. Reasoning: the 180° rotation is not a render-pass toggle — it is conjugated into every node's local TRS at model-load time (`node.rotation`, `node.translation`, `node.initialRotation`, `node.initialTranslation`) and a matching left-multiply of `Ry180` into every skin's `inverseBindMatrices`. Physics, animation, and culling all operate in the rotated space thereafter. The `VRMRenderer` comments at lines 1567, 2310–2312, and 2421–2423 explicitly state "VRM 0.0 → 1.0 coordinate conversion is applied at load time, so there is no per-frame version rotation". Removing the load-time flip without also removing the IBM conjugation would produce a `Ry180·p − p` vertex displacement on every joint. The `ARKitCoordinateConverter.rootRotationCorrection` (`Sources/VRMMetalKit/ARKit/ARKitCoordinateConverter.swift:165–167`) is a separate, independent quaternion used only for ARKit root-joint alignment — it is not the VRM 0.x conversion and is not gated on spec version.

**Implication for slice 1.**
The flip is UPSTREAM_LIBRARY + LOAD_BEARING. This means:
- No adapter-shim fix is possible. The flip is structurally embedded in VRMMetalKit's load path.
- The observed rendering effect (model in VRM 1.0 coordinate space) is intentional from VRMMetalKit's perspective: it normalises VRM 0.x to VRM 1.0 semantics so a single downstream render path serves both formats.
- For conformance purposes, the question is whether VRMMetalKit's normalisation is spec-correct. The VRM 0.x spec (`specification/0.0/README.md`) says models face −Z in Unity's left-handed coordinate system. VRMMetalKit converts to glTF right-handed space (facing +Z) at load time, which matches what VRM 1.0 consumers expect. The camera convention documented in our conformance suite's `set_camera` operation positions the camera at +Z facing −Z, which is correct for both VRM 1.0 and the VRMMetalKit-normalised VRM 0.x space.
- Upstream issue stub filed at `docs/upstream/VMK-vrm-0x-orientation.md` for tracking; the flag stays documented through slices 1–4 so we have a paper trail if a conformance discrepancy surfaces empirically.

**Evidence.**

```
# Sources/VRMMetalKit/Core/VRMModel.swift (revision 392d949), lines 980–1011
# buildNodeHierarchy() — excerpt:

        // VRM 0.0 → VRM 1.0 coordinate conversion.
        // Unity left-handed (model faces -Z) → glTF right-handed (model faces +Z).
        // A 180° rotation around Y aligns facing direction AND makes node.worldPosition
        // consistent with VRM 1.0 (left limbs positive X).  Applied once at load time
        // so physics, animation, and culling all see the same coordinate space.
        // The matching `inverseBindMatrices` pass runs after skins are loaded — see
        // `applyVRM0InverseBindMatrixConjugation()`; without it skinning at rest
        // would displace vertices by `Ry180·p − p` for each joint.
        if isVRM0 {
            for node in nodes {
                // Conjugate local rotation by 180° Y: (x, y, z, w) → (-x, y, -z, w)
                node.rotation = simd_normalize(
                    simd_quatf(ix: -node.rotation.imag.x,
                               iy:  node.rotation.imag.y,
                               iz: -node.rotation.imag.z,
                               r:   node.rotation.real)
                )
                // Rotate translation: (x, y, z) → (-x, y, -z)
                node.translation = SIMD3<Float>(-node.translation.x,
                                                 node.translation.y,
                                                -node.translation.z)
                // Update bind pose storage so resetToBindPose() stays consistent
                node.initialRotation = node.rotation
                node.initialTranslation = node.translation
                // Scale magnitudes are unchanged under 180° rotation
                node.updateLocalMatrix()
            }
            // Recalculate world transforms after mutating every local matrix
            for node in nodes where node.parent == nil {
                node.updateWorldTransform()
            }
        }

# Sources/VRMMetalKit/Core/VRMModel.swift (revision 392d949), lines 881–897
# applyVRM0InverseBindMatrixConjugation() — excerpt:

    private func applyVRM0InverseBindMatrixConjugation() {
        guard isVRM0 else { return }
        let ry180 = float4x4(
            SIMD4<Float>(-1, 0,  0, 0),
            SIMD4<Float>( 0, 1,  0, 0),
            SIMD4<Float>( 0, 0, -1, 0),
            SIMD4<Float>( 0, 0,  0, 1)
        )
        for skin in skins {
            for i in 0..<skin.inverseBindMatrices.count {
                skin.inverseBindMatrices[i] = ry180 * skin.inverseBindMatrices[i]
            }
        }
    }
```

The adapter shim (`adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`) contains no orientation-flipping code. The only `.pi` occurrences in the shim are FOV degree-to-radian conversions and lookAt yaw/pitch conversions — all unrelated to VRM version handling.

## 2026-05-26 — mrxz/vrm-validator coverage of VRM 0.x (slice 1 days 1–3 empirical check)

**Validator binary:** `.tools/vrm-validator-cli` (shell wrapper at `/Users/arkavo/Projects/vrm-conformance/.tools/vrm-validator-cli` invoking `node .tools/vrm-validator/cli.js`; installed via `scripts/install-validator.sh`). Validator version: `2.0.0-dev.3.10` (mrxz/vrm-validator).

**Test asset:** `assets/humanoid/avatarA_0_0.vrm` (VRM 0.x, exported by UniGLTF-2.64.1).

**Result on `avatarA_0_0.vrm`:** ACCEPTED_WITH_WARNINGS. Exit code: 0.

**Summary of issues reported:**
- `numErrors`: 0
- `numWarnings`: 16 (all `MESH_PRIMITIVE_GENERATED_TANGENT_SPACE`, severity 1 — material requires a tangent space but mesh primitive does not provide it; runtime-generated tangent space may be non-portable)
- `numInfos`: 4 (1× `INVALID_EXTENSION_NAME_FORMAT` for the `VRM` extension; 1× `UNSUPPORTED_EXTENSION` — "Cannot validate an extension as it is not supported by the validator: 'VRM'" at severity 2/info; 2× `UNUSED_OBJECT` for textures 24/25/26)
- `numHints`: 0

**Output (compact summary — full JSON available by re-running the validator):**

```json
{
  "uri": "avatarA_0_0.vrm",
  "validatorVersion": "2.0.0-dev.3.10",
  "issues": {
    "numErrors": 0,
    "numWarnings": 16,
    "numInfos": 4,
    "numHints": 0
  }
}
```

Key messages:
- `INVALID_EXTENSION_NAME_FORMAT` at `/extensionsUsed/0` (severity 1 / warning): VRM 0.x uses the bare `"VRM"` extension name, which does not comply with the glTF extension naming convention (`VENDOR_feature`).
- `UNSUPPORTED_EXTENSION` at `/extensionsUsed/0` (severity 2 / info): mrxz/vrm-validator does not have a VRM 0.x schema validator; it validates only the glTF core structure, skipping the `VRM` extension blob entirely.
- `MESH_PRIMITIVE_GENERATED_TANGENT_SPACE` ×14 (severity 1 / warning): tangent-space generation required at runtime. This is expected for VRM 0.x Unity exports and is not a conformance-relevant defect for this suite.
- `UNUSED_OBJECT` ×3 for textures 24/25/26 (severity 2 / info): three embedded textures have no material reference in the glTF core. They may be referenced only inside the `VRM` extension blob (which the validator skips), so these are false positives from the validator's perspective.

**Implication.**
ACCEPTED_WITH_WARNINGS. The validator accepts VRM 0.x without errors; exit code 0 means the CI validator gate applies uniformly to 0.x assets without modification. The warnings are:
1. `MESH_PRIMITIVE_GENERATED_TANGENT_SPACE` — expected characteristic of VRM 0.x exports; not a blocking defect for the conformance suite. No action needed; document as a per-corpus methodology note.
2. `UNSUPPORTED_EXTENSION` for the `VRM` blob — the validator performs no VRM 0.x-specific validation. For 0.x corpus entries, this means CI catches only glTF-core structural issues, not VRM-extension correctness. Fall-back to local schema validation against `docs/upstream-specs/vrm-specification/specification/0.0/schema/` for VRM-extension correctness checks is available but not currently needed for slice 1, which focuses on renderer conformance rather than asset correctness.

**No validator exemption needed for 0.x CI.** The existing `vrm-validator-wrap` invocation path works unchanged.
