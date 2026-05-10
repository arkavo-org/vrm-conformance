# RFC 0002: Anti-fraud and submission integrity for golden renders

- **Status:** Draft
- **Author(s):** Paul Flynn
- **Date:** 2026-05-10

## Summary

Renderer maintainers submit golden PNGs via PR. Without safeguards, a bad actor could submit doctored images to make their renderer "look correct." This RFC defines the multi-layered policy that protects submission integrity without imposing a re-rendering burden on our CI.

## Motivation

The PR-submitted-renders model is the only model that scales. CI re-rendering Unity / Godot / WebGL workloads is impractical and creates a credential / licensing surface we do not want. The cost of that decision is a trust gap; this RFC closes it pragmatically.

## Detailed design

Three layers of defense:

### 1. Strict submission metadata

CI rejects any PR whose `goldens/manifest.json` entries are missing or malformed. Required fields per submission:

```json
{
  "test_id": "mtoon_basic_shadingShift_neg0.5",
  "renderer_name": "vrm-metal-kit",
  "renderer_version": "0.5.2",
  "git_hash": "a1b2c3d4...",
  "os": "macos",
  "os_version": "14.4.1",
  "gpu_vendor": "Apple",
  "gpu_model": "M2 Pro",
  "driver_version": "Metal 3",
  "build_flags": "release",
  "image_url": "s3://arkavo-vrm-conformance/...",
  "image_blake3": "blake3:...",
  "submitted_at": "2026-05-10T12:34:56Z"
}
```

These provide traceability; a fraudulent submission must also fabricate consistent metadata. `image_blake3` uses BLAKE3 to align with the project-wide content-addressing convention.

### 2. Spot-check audit cadence

Maintainers periodically (target: monthly) sample N renders from a recently-submitted column and re-render locally on matching hardware. Mismatches escalate to a public audit. The audit cadence is documented but the sample is private — surprise is the point.

### 3. Consensus reference mode

For tests using `diff.reference_renderer: consensus`, a 3-of-5 majority defines the baseline and outliers are flagged automatically. One outlier renderer cannot shift the baseline. This is a partial defense (it presumes ≥3 honest renderers per test) but compounds with #1 and #2.

### Reproducibility statement

`CONTRIBUTING.md` commits submitters to: *"renderer + asset + test plan + build hash should produce this PNG within tolerance T on the same hardware class."* This is a public statement, not a CI-enforced check. It exists so that audits have a reference contract to invoke when escalating.

## Alternatives considered

- **CI-side re-rendering.** Rejected as cost-prohibitive and credential-prohibitive.
- **Trusted-builder attestation (Sigstore / SLSA).** Interesting; deferred. The Phase 1 cost-to-benefit is too high. Revisit if fraud actually occurs.
- **Multi-party submission (require ≥2 maintainers to submit independently for the same renderer).** Rejected on coordination cost; effectively halts maintainer adoption.

## Open questions

- What is the appropriate sample size and cadence for the spot-check audit? Defer until we have ≥3 active renderer adapters submitting.
- Does the manifest schema need a signature field for cryptographic attestation? Defer; keep the door open by leaving the schema extensible.

## References

- Original handover §11 Open Question 7.
