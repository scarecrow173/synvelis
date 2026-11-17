// sv519 inventory helper: parse docs/** frontmatter relations and classify
// resolvability using the same rules as route_graph.js / doc_status.js.
// Read-only: writes JSON inventory to stdout or a file.
import fs from "node:fs";
import path from "node:path";

const cwd = process.cwd();
const CANONICAL_DOC_DIRS = [
  "docs/ideas", "docs/discovery", "docs/specs", "docs/designs",
  "docs/plans", "docs/tasks", "docs/test-specs", "docs/adr",
  "docs/impl/ir", "docs/impl/exp",
];
const NON_REF_KEYS = new Set(["changes", "test-spec-skip"]);
const EXTERNAL = /^(?:[a-z][a-z\d+.-]*:|\/\/)/i;

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const e of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const f = path.join(dir, e.name);
    if (e.isDirectory()) walk(f, out);
    else if (e.isFile() && e.name.toLowerCase().endsWith(".md") && !/^(?:readme|index)\.md$/i.test(e.name)) out.push(f);
  }
  return out;
}

function parseFrontmatter(raw) {
  const m = raw.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return {};
  const data = {};
  let currentKey = null;
  let inRelations = false;
  let relations = null;
  for (const line of m[1].split(/\r?\n/)) {
    if (/^\S/.test(line)) { inRelations = false; currentKey = null; }
    const item = line.match(/^(\s*)-\s*(.*)$/);
    if (item && inRelations && currentKey) {
      if (!Array.isArray(relations[currentKey])) relations[currentKey] = [];
      relations[currentKey].push(parseScalar(item[2]));
      continue;
    }
    const kv = line.match(/^(\s*)([A-Za-z0-9_-]+):\s*(.*)$/);
    if (!kv) continue;
    const indent = kv[1].length;
    const key = kv[2];
    const rest = kv[3];
    if (indent === 0) {
      currentKey = key;
      inRelations = key === "relations";
      if (inRelations) relations = {};
      else data[key] = rest === "" ? null : parseScalar(rest);
      continue;
    }
    if (!inRelations) continue;
    if (indent === 2) {
      currentKey = key;
      relations[key] = [];
      if (rest && rest !== "[]") {
        relations[key] = parseScalar(rest);
      }
      continue;
    }
  }
  if (relations) data.relations = relations;
  return data;
}

function parseScalar(s) {
  s = s.trim();
  if (s === "[]") return [];
  if (s.startsWith("[") && s.endsWith("]")) {
    const inner = s.slice(1, -1).trim();
    if (!inner) return [];
    return inner.split(",").map((part) => parseScalar(part.trim()));
  }
  if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) return s.slice(1, -1);
  if (/^-?\d+$/.test(s)) return Number(s);
  if (s === "true") return true;
  if (s === "false") return false;
  if (s === "null" || s === "~") return null;
  return s;
}

const records = [];
const files = CANONICAL_DOC_DIRS.flatMap((d) => walk(path.join(cwd, d)));
for (const abs of files) {
  const rel = path.relative(cwd, abs).split(path.sep).join("/");
  const raw = fs.readFileSync(abs, "utf8");
  const fm = parseFrontmatter(raw);
  const rels = fm.relations && typeof fm.relations === "object" ? fm.relations : {};
  for (const [field, vals] of Object.entries(rels)) {
    if (NON_REF_KEYS.has(field)) continue;
    const list = Array.isArray(vals) ? vals : [vals];
    for (const v of list) {
      if (typeof v !== "string" || !v.trim()) continue;
      records.push({ source: rel, field, value: v.trim(), id: fm.id ?? null });
    }
  }
}

// route_graph resolution replica
const ids = new Map();
for (const f of files) {
  const rel = path.relative(cwd, f).split(path.sep).join("/");
  const fm = parseFrontmatter(fs.readFileSync(f, "utf8"));
  if (fm.id) {
    if (!ids.has(fm.id)) ids.set(fm.id, []);
    ids.get(fm.id).push(rel);
  }
}
const docPaths = new Set(files.map((f) => path.relative(cwd, f).split(path.sep).join("/")));

function localTarget(ownerAbs, value) {
  const trimmed = value.trim();
  const ownerCandidate = path.resolve(path.dirname(ownerAbs), trimmed);
  const rootCandidate = path.resolve(cwd, trimmed);
  const docRel = trimmed.startsWith("./") || trimmed.startsWith("../") || trimmed === "." || trimmed === "..";
  const preferred = docRel ? ownerCandidate : rootCandidate;
  const fallback = docRel ? rootCandidate : ownerCandidate;
  const chosen = fs.existsSync(preferred) ? preferred : fallback;
  const exists = fs.existsSync(chosen) && fs.statSync(chosen).isFile();
  return { path: path.relative(cwd, chosen).split(path.sep).join("/"), exists };
}

const out = [];
for (const r of records) {
  const v = r.value;
  const external = EXTERNAL.test(v);
  const byId = ids.get(v) ?? [];
  let status;
  if (external) status = "external";
  else if (byId.length === 1) status = "id";
  else if (byId.length > 1) status = "ambiguous-id";
  else {
    const ownerAbs = path.join(cwd, r.source);
    const t = localTarget(ownerAbs, v);
    if (docPaths.has(t.path)) status = "path-doc";
    else if (t.exists) status = "path-file-nondoc";
    else status = "broken";
  }
  out.push({ ...r, status });
}

const summary = {};
for (const o of out) summary[o.status] = (summary[o.status] || 0) + 1;
console.error("summary:", JSON.stringify(summary));
const broken = out.filter((o) => o.status === "broken");
console.error("broken count:", broken.length);
fs.writeFileSync(process.argv[2] || "evidence/sv519/inventory_all.json", JSON.stringify(out, null, 2));
