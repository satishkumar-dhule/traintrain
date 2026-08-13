const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const response = await axios.get('https://www.railyatri.in/live-train-status/12951', {
      headers: { 'User-Agent': 'Mozilla/5.0' }
    });
    const $ = cheerio.load(response.data);
    const nextData = $('#__NEXT_DATA__').html();
    if (nextData) {
      console.log(JSON.parse(nextData).props?.pageProps?.ltsData);
    }
  } catch (e) {
    console.error(e.message);
  }
}
test();
