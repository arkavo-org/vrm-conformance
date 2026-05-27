// Manifest types — wire shape mirrors the Rust schema in
// `crates/vrm-s3/src/manifest.rs`. The Rust side flattens
// `SubmissionMetadata` into the entry, so all metadata fields are
// top-level alongside `test_id`.

export interface ManifestEntry {
  test_id: string;
  spec_version: "0.x" | "1.0"; // Task 4 / Task 6 migration
  renderer_name: string;
  renderer_version: string;
  git_hash: string;
  os: string;
  os_version: string;
  gpu_vendor: string;
  gpu_model: string;
  driver_version: string;
  build_flags: string;
  image_url: string; // s3://bucket/key form
  image_blake3: string; // "blake3:<hex>" form
  byte_size: number;
  submitted_at: string; // RFC 3339 ISO 8601
}

export interface Manifest {
  version: number;
  entries: ManifestEntry[];
}

/**
 * Convert an `s3://bucket/key` URL to a public HTTPS URL.
 *
 * The default mapping is `https://<bucket>.s3.amazonaws.com/<key>`. This can
 * be overridden at build time via the `VITE_S3_BASE_URL` env var, which (if
 * set) is treated as a base URL prefix; the full URL becomes
 * `<VITE_S3_BASE_URL>/<key>` (the bucket is dropped, as the override is
 * assumed to already encode the bucket / CDN host).
 *
 * Inputs that are already HTTP(S) URLs are returned as-is. Anything else
 * is returned unchanged so the `<img>` 404 surfaces visibly rather than
 * being silently rewritten into nonsense.
 */
export function s3ToHttps(url: string): string {
  if (url.startsWith("http://") || url.startsWith("https://")) {
    return url;
  }
  if (!url.startsWith("s3://")) {
    return url;
  }
  const rest = url.slice("s3://".length);
  const slash = rest.indexOf("/");
  if (slash < 0) {
    return url;
  }
  const bucket = rest.slice(0, slash);
  const key = rest.slice(slash + 1);

  const override = import.meta.env.VITE_S3_BASE_URL as string | undefined;
  if (override && override.length > 0) {
    const trimmed = override.endsWith("/") ? override.slice(0, -1) : override;
    return `${trimmed}/${key}`;
  }
  return `https://${bucket}.s3.amazonaws.com/${key}`;
}

/**
 * Fetch the manifest from `goldens/manifest.json` (relative to the site
 * root, so it works under both `/` and a project-page subpath).
 *
 * Throws on non-2xx responses or JSON parse failure; callers should
 * catch and render a friendly error message.
 */
export async function loadManifest(): Promise<Manifest> {
  const res = await fetch("goldens/manifest.json", { cache: "no-cache" });
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} ${res.statusText}`);
  }
  const data = (await res.json()) as Manifest;
  if (typeof data !== "object" || data === null || !Array.isArray(data.entries)) {
    throw new Error("Manifest is missing `entries` array");
  }
  return data;
}

/**
 * Group manifest entries by `test_id`, preserving first-seen order
 * so the rendered page is stable across loads.
 */
export function groupByTest(manifest: Manifest): Map<string, ManifestEntry[]> {
  const groups = new Map<string, ManifestEntry[]>();
  for (const entry of manifest.entries) {
    const existing = groups.get(entry.test_id);
    if (existing) {
      existing.push(entry);
    } else {
      groups.set(entry.test_id, [entry]);
    }
  }
  return groups;
}

/**
 * Collect the set of all renderer names appearing anywhere in the
 * manifest, in first-seen order. Useful so each test row can render
 * a "no submission" placeholder for renderers that didn't submit
 * for that particular test.
 */
export function collectRenderers(manifest: Manifest): string[] {
  const seen = new Set<string>();
  const order: string[] = [];
  for (const entry of manifest.entries) {
    if (!seen.has(entry.renderer_name)) {
      seen.add(entry.renderer_name);
      order.push(entry.renderer_name);
    }
  }
  return order;
}
