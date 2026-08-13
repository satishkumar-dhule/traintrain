const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const response = await axios.get(`https://www.railyatri.in/pnr-status/2522658044`, {
      headers: { 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36' }
    });
    const $ = cheerio.load(response.data);
    console.log("Train Info:", $('.pnr-search-result-info').first().text().trim());
    console.log("Boarding:", $('.boarding-station').text().trim());
    
    // Let's dump a bit of the DOM to see where the data is
    console.log("HTML snippet:", $('div.pnr-search-result-info').html() || "NOT FOUND");
  } catch (err) {
    console.error(err.message);
  }
}
test();
