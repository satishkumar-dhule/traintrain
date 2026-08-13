const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const response = await axios.get(`https://www.railyatri.in/pnr-status/2522658044`, {
      headers: { 'User-Agent': 'Mozilla/5.0' }
    });
    const $ = cheerio.load(response.data);
    // Find all rows that look like passenger data
    $('.pnr-search-result-info').parent().children().each((i, el) => {
       console.log($(el).attr('class'), $(el).text().substring(0, 100).replace(/\s+/g, ' '));
    });
    console.log("Passenger info wrapper:", $('.passenger-info').html() || $('.pnr-search-result-pass').html() || "NOT FOUND");
  } catch (err) {
    console.error(err.message);
  }
}
test();
