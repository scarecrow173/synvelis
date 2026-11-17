// repair-manifest.json を適用して frontmatter relation 値を修復する。
// relations ブロック内で relation key を追跡し、該当 relation の
// `- "value"` 行だけを置換/削除する。適用後に relation 内の重複値を除去する。
import fs from 'node:fs';

const manifest = JSON.parse(fs.readFileSync('evidence/sv519/repair-manifest.json', 'utf8'));

function yamlUnescape(s) {
  return s.replace(/\\"/g, '"').replace(/\\\\/g, '\\');
}
function yamlEscape(s) {
  return s.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

let applied = 0, removed = 0, unmatched = [];
const touchedFiles = [];

for (const [file, relations] of Object.entries(manifest)) {
  const lines = fs.readFileSync(file, 'utf8').split('\n');
  // relations ブロック範囲を特定
  const relStart = lines.findIndex((l) => /^relations:\s*$/.test(l));
  if (relStart < 0) { unmatched.push(`${file}: no relations block`); continue; }
  let relEnd = lines.length;
  for (let i = relStart + 1; i < lines.length; i++) {
    if (/^[A-Za-z_][A-Za-z0-9_-]*:/.test(lines[i]) || /^---\s*$/.test(lines[i])) { relEnd = i; break; }
  }
  // ブロック内を走査: relation key と item 行を収集
  // item: ^(\s+)-\s*"(.*)"\s*$  → { idx, indent, value }
  // relKey: ^\s+([A-Za-z0-9_-]+):\s*(\[.*\])?\s*$ → 現在の relation
  const items = []; // {idx, relation, indent, raw, value}
  let curRel = null;
  for (let i = relStart + 1; i < relEnd; i++) {
    const line = lines[i];
    const km = line.match(/^\s+([A-Za-z0-9_-]+):\s*(\[.*\])?\s*$/);
    if (km && !line.trimStart().startsWith('-')) { curRel = km[1]; continue; }
    const im = line.match(/^(\s+)-\s*"(.*)"\s*$/);
    if (im && curRel) {
      items.push({ idx: i, relation: curRel, indent: im[1], raw: im[2], value: yamlUnescape(im[2]) });
    }
  }
  // 置換/削除を適用（後ろから処理して index ずれを防ぐ）
  const replacements = []; // {idx, newLines[] | null}
  for (const [relation, valueMap] of Object.entries(relations)) {
    for (const item of items.filter((it) => it.relation === relation)) {
      const action = valueMap[item.value];
      if (!action) continue;
      if (action.action === 'remove') {
        replacements.push({ idx: item.idx, newLines: null });
        removed++;
      } else {
        const newLines = action.values.map((v) => `${item.indent}- "${yamlEscape(v)}"`);
        replacements.push({ idx: item.idx, newLines });
        applied += action.values.length;
      }
      delete valueMap[item.value]; // 消化済みマーク
    }
  }
  // 未適用チェック
  for (const [relation, valueMap] of Object.entries(relations)) {
    for (const v of Object.keys(valueMap)) unmatched.push(`${file}|${relation}|${v.slice(0, 60)}`);
  }
  if (!replacements.length) continue;
  // index 降順で適用
  replacements.sort((a, b) => b.idx - a.idx);
  for (const r of replacements) {
    if (r.newLines === null) lines.splice(r.idx, 1);
    else lines.splice(r.idx, 1, ...r.newLines);
  }
  // relation ブロック内の重複 item 値を除去（最初の出現を残す）
  // 再スキャンして dedupe
  const relStart2 = lines.findIndex((l) => /^relations:\s*$/.test(l));
  let relEnd2 = lines.length;
  for (let i = relStart2 + 1; i < lines.length; i++) {
    if (/^[A-Za-z_][A-Za-z0-9_-]*:/.test(lines[i]) || /^---\s*$/.test(lines[i])) { relEnd2 = i; break; }
  }
  const seen = new Map(); // relation -> Set(values)
  let curRel2 = null;
  const toDelete = [];
  for (let i = relStart2 + 1; i < relEnd2; i++) {
    const line = lines[i];
    const km = line.match(/^\s+([A-Za-z0-9_-]+):\s*(\[.*\])?\s*$/);
    if (km && !line.trimStart().startsWith('-')) { curRel2 = km[1]; if (!seen.has(curRel2)) seen.set(curRel2, new Set()); continue; }
    const im = line.match(/^\s+-\s*"(.*)"\s*$/);
    if (im && curRel2) {
      const v = yamlUnescape(im[1]);
      if (seen.get(curRel2).has(v)) toDelete.push(i);
      else seen.get(curRel2).add(v);
    }
  }
  for (let i = toDelete.length - 1; i >= 0; i--) lines.splice(toDelete[i], 1);
  fs.writeFileSync(file, lines.join('\n'));
  touchedFiles.push(file);
}

console.log(`applied entries: ${applied}, removed entries: ${removed}, files: ${touchedFiles.length}`);
if (unmatched.length) {
  console.error('UNMATCHED:', unmatched.length);
  for (const u of unmatched) console.error(' ', u);
  process.exit(1);
}
