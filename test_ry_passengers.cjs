const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const response = await axios.get(`https://www.railyatri.in/pnr-status/2522658044`, {
      headers: { 'User-Agent': 'Mozilla/5.0' }
    });
    const $ = cheerio.load(response.data);
    console.log("Passenger HTML:", $('.passenger_chart_table').html() || $('.pnr-search-result-info').parent().html() || "NOT FOUND");
  } catch (err) {
    console.error(err.message);
  }
}
test();
