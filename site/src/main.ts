import {
  collectRenderers,
  groupByTest,
  loadManifest,
  s3ToHttps,
  type ManifestEntry,
} from "./manifest";

type SpecVersionFilter = "all" | "0.x" | "1.0";

const app = document.getElementById("app");
if (!app) {
  throw new Error("#app element missing from index.html");
}

void main(app);

async function main(root: HTMLElement): Promise<void> {
  try {
    const manifest = await loadManifest();
    renderWithFilter(root, manifest.entries);
  } catch (err) {
    renderError(root, err);
  }
}

function renderWithFilter(root: HTMLElement, allEntries: ManifestEntry[]): void {
  let currentFilter: SpecVersionFilter = "all";

  const filterBar = buildFilterBar(currentFilter, (newFilter) => {
    currentFilter = newFilter;
    updateFilterBar(filterBar, currentFilter);
    const filtered = filterEntries(allEntries, currentFilter);
    render(content, filtered);
  });

  const content = document.createElement("div");
  content.id = "content";

  clear(root);
  root.append(filterBar, content);

  render(content, filterEntries(allEntries, currentFilter));
}

function filterEntries(entries: ManifestEntry[], filter: SpecVersionFilter): ManifestEntry[] {
  if (filter === "all") return entries;
  return entries.filter((e) => (e.spec_version ?? "1.0") === filter);
}

function buildFilterBar(
  initial: SpecVersionFilter,
  onChange: (v: SpecVersionFilter) => void,
): HTMLElement {
  const bar = document.createElement("div");
  bar.className = "filter-bar";

  const label = document.createElement("span");
  label.className = "filter-label";
  label.textContent = "Spec:";
  bar.append(label);

  const chips: { value: SpecVersionFilter; text: string }[] = [
    { value: "all", text: "All" },
    { value: "0.x", text: "VRM 0.x" },
    { value: "1.0", text: "VRM 1.0" },
  ];

  for (const chip of chips) {
    const btn = document.createElement("button");
    btn.className = `chip${initial === chip.value ? " active" : ""}`;
    btn.dataset.value = chip.value;
    btn.textContent = chip.text;
    btn.addEventListener("click", () => onChange(chip.value));
    bar.append(btn);
  }

  return bar;
}

function updateFilterBar(bar: HTMLElement, active: SpecVersionFilter): void {
  for (const btn of bar.querySelectorAll<HTMLButtonElement>("button.chip")) {
    btn.classList.toggle("active", btn.dataset.value === active);
  }
}

function clear(root: HTMLElement): void {
  while (root.firstChild) {
    root.removeChild(root.firstChild);
  }
}

function renderError(root: HTMLElement, err: unknown): void {
  const message = err instanceof Error ? err.message : String(err);
  clear(root);
  const box = document.createElement("div");
  box.className = "error";
  const heading = document.createElement("h2");
  heading.textContent = "Failed to load manifest";
  const detail = document.createElement("p");
  detail.textContent = message;
  const hint = document.createElement("p");
  hint.className = "hint";
  hint.textContent =
    "This is expected before any submissions have landed. Once goldens/manifest.json is published, refresh.";
  box.append(heading, detail, hint);
  root.append(box);
}

function render(container: HTMLElement, entries: ManifestEntry[]): void {
  clear(container);

  if (entries.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "Manifest loaded but contains no entries yet.";
    container.append(empty);
    return;
  }

  const manifest = { version: 1, entries };
  const groups = groupByTest(manifest);
  const renderers = collectRenderers(manifest);

  // Stable test ordering: first-seen in the manifest.
  const testIds = Array.from(groups.keys());

  for (const testId of testIds) {
    const section = renderTestSection(testId, groups.get(testId) ?? [], renderers);
    container.append(section);
  }
}

function renderTestSection(
  testId: string,
  entries: ManifestEntry[],
  renderers: string[],
): HTMLElement {
  const section = document.createElement("section");
  section.className = "test";

  const heading = document.createElement("h2");
  heading.className = "test-id";
  heading.textContent = testId;
  section.append(heading);

  const grid = document.createElement("div");
  grid.className = "grid";

  // Index entries by renderer for O(1) lookup; in the unlikely case of
  // multiple submissions per (test, renderer) we take the most recent.
  const byRenderer = new Map<string, ManifestEntry>();
  for (const entry of entries) {
    const existing = byRenderer.get(entry.renderer_name);
    if (!existing || entry.submitted_at > existing.submitted_at) {
      byRenderer.set(entry.renderer_name, entry);
    }
  }

  for (const renderer of renderers) {
    const entry = byRenderer.get(renderer);
    grid.append(entry ? renderCell(entry) : renderMissingCell(renderer));
  }

  section.append(grid);
  return section;
}

function renderCell(entry: ManifestEntry): HTMLElement {
  const cell = document.createElement("article");
  cell.className = "cell";

  const header = document.createElement("div");
  header.className = "cell-header";

  const rendererLabel = document.createElement("h3");
  rendererLabel.className = "renderer";
  rendererLabel.textContent = entry.renderer_name;
  header.append(rendererLabel);

  const specBadge = document.createElement("span");
  const specVersion = entry.spec_version ?? "1.0";
  specBadge.className = `spec-version-badge spec-version-${specVersion === "0.x" ? "v0" : "v1"}`;
  specBadge.textContent = specVersion;
  header.append(specBadge);

  cell.append(header);

  const img = document.createElement("img");
  img.loading = "lazy";
  img.decoding = "async";
  img.src = s3ToHttps(entry.image_url);
  img.alt = `${entry.renderer_name} render of ${entry.test_id}`;
  cell.append(img);

  const meta = document.createElement("p");
  meta.className = "meta";
  meta.textContent = formatMetadata(entry);
  cell.append(meta);

  return cell;
}

function renderMissingCell(rendererName: string): HTMLElement {
  const cell = document.createElement("article");
  cell.className = "cell cell-missing";

  const rendererLabel = document.createElement("h3");
  rendererLabel.className = "renderer";
  rendererLabel.textContent = rendererName;
  cell.append(rendererLabel);

  const placeholder = document.createElement("div");
  placeholder.className = "placeholder";
  placeholder.textContent = "no submission";
  cell.append(placeholder);

  return cell;
}

function formatMetadata(entry: ManifestEntry): string {
  const shortHash = entry.git_hash.slice(0, 7);
  return `${entry.renderer_version} · ${entry.os} · ${entry.gpu_model} · ${shortHash}`;
}
