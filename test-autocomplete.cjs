const https = require('https');

function get(url) {
  return new Promise((resolve) => {
    https.get(url, { headers: { 'User-Agent': 'Mozilla/5.0' } }, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => resolve(data));
    }).on('error', () => resolve(null));
  });
}

async function test() {
  console.log("RY Trains:", (await get("https://www.railyatri.in/m/train-autocomplete?query=129")).substring(0, 200));
  console.log("RY Stations:", (await get("https://www.railyatri.in/m/station-autocomplete?query=ndls")).substring(0, 200));
  console.log("CT Trains:", (await get("https://www.confirmtkt.com/pnr/ajax/autocomplete-train?q=129")).substring(0, 200));
}
test();
