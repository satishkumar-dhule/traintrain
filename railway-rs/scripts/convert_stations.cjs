// Converts the original GeoJSON stations file into the flat format consumed by
// railway-rs: data/stations.json = [{code,name,state,zone}]
// Pseudo-codes (XX-/YY- prefixes) are dropped - they are not real stations.
const fs = require('fs');
const path = require('path');

const src = process.argv[2] || path.join(__dirname, '..', '..', 'data', 'stations.json');
const out = process.argv[3] || path.join(__dirname, '..', 'data', 'stations.json');

const raw = JSON.parse(fs.readFileSync(src, 'utf8'));
const features = raw.features || raw;

const stations = [];
const seen = new Set();
for (const f of features) {
  const p = (f.properties || {});
  const code = String(p.code || '').trim().toUpperCase();
  if (!/^[A-Z0-9]{2,}$/.test(code)) continue;
  if (code.startsWith('XX-') || code.startsWith('YY-')) continue;
  if (seen.has(code)) continue;
  seen.add(code);
  stations.push({
    code,
    name: String(p.name || code).trim(),
    state: String(p.state || '').trim(),
    zone: String(p.zone || '').trim(),
  });
}

stations.sort((a, b) => a.code.localeCompare(b.code));
fs.writeFileSync(out, JSON.stringify(stations));
console.log(`wrote ${stations.length} stations to ${out}`);
