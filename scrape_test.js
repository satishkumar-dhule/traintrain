const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const res = await axios.get('https://www.railyatri.in/pnr-status/1234567890', {
      headers: {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)'
      }
    });
    const $ = cheerio.load(res.data);
    console.log($('body').text().substring(0, 500));
  } catch (e) {
    console.error(e.message);
  }
}
test();
