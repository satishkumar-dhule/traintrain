const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const response = await axios.get(`https://www.railyatri.in/pnr-status/2522658044`, {
      headers: { 'User-Agent': 'Mozilla/5.0' }
    });
    const $ = cheerio.load(response.data);
    const bodyText = $('body').text().replace(/\s+/g, ' ');
    const pnrIndex = bodyText.indexOf('2522658044');
    console.log("Around PNR:", bodyText.substring(Math.max(0, pnrIndex - 100), pnrIndex + 500));
    
    // Find all class names containing 'pass' or 'chart'
    const classes = new Set();
    $('*').each((i, el) => {
      if (el.attribs && el.attribs.class) {
        classes.add(el.attribs.class);
      }
    });
    console.log("Classes:", Array.from(classes).filter(c => c.includes('pass') || c.includes('chart') || c.includes('info')).join(', '));
  } catch (err) {
    console.error(err.message);
  }
}
test();
