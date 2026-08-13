const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const response = await axios.get(`https://www.railyatri.in/pnr-status/2522658044`, {
      headers: { 'User-Agent': 'Mozilla/5.0' }
    });
    const $ = cheerio.load(response.data);
    const chartStats = $('.chart-stats').parent().html() || $('.chart-stats').html() || "NOT FOUND";
    console.log("Passenger Block:", chartStats);
  } catch (err) {
    console.error(err.message);
  }
}
test();
