// Fetches the real NTES train master list (train_data.js) and converts it to
// data/trains.json = [{number,name}] - real trains only, never invented.
const fs = require('fs');
const path = require('path');

const out = process.argv[2] || path.join(__dirname, '..', 'data', 'trains.json');
const ts = Date.now();
const url = `https://enquiry.indianrail.gov.in/mntes/javascripts/train_data.js?v=${ts}`;

async function main() {
  const res = await fetch(url, { headers: { 'User-Agent': 'Mozilla/5.0' } });
  if (!res.ok) throw new Error(`train_data.js returned ${res.status}`);
  const text = await res.text();
  const m = text.match(/var\s+arrTrainList\s*=\s*\[(.*)\]/s);
  if (!m) throw new Error('arrTrainList not found in response');
  const raw = JSON.parse(`[${m[1]}]`);
  const trains = raw.map((s) => {
    const idx = s.indexOf('- ');
    const number = (idx > 0 ? s.slice(0, idx) : s).trim();
    const name = (idx > 0 ? s.slice(idx + 2) : s).trim();
    return { number, name };
  }).filter((t) => /^\d{5}$/.test(t.number) || /^\d{7,8}$/.test(t.number));
  fs.writeFileSync(out, JSON.stringify(trains));
  console.log(`wrote ${trains.length} trains to ${out} (source ${text.length} bytes)`);
}

main().catch((e) => { console.error(e.message); process.exit(1); });
