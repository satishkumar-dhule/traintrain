const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const response = await axios.get(`https://www.railyatri.in/pnr-status/2522658044`, {
      headers: { 'User-Agent': 'Mozilla/5.0' }
    });
    const $ = cheerio.load(response.data);
    console.log("Full text of container:", $('.pnr-search-result').text().replace(/\s+/g, ' ').substring(0, 500));
    console.log("All tables:", $('table').text().replace(/\s+/g, ' ').substring(0, 500));
  } catch (err) {
    console.error(err.message);
  }
}
test();
