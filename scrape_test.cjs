const axios = require('axios');
const cheerio = require('cheerio');

async function test() {
  try {
    const res = await axios.get('https://www.railyatri.in/time-table/12951', {
      headers: { 'User-Agent': 'Mozilla/5.0' }
    });
    const $ = cheerio.load(res.data);
    const nextData = $('#__NEXT_DATA__').html();
    const data = JSON.parse(nextData);
    const tt = data.props.pageProps.trainTimeTable;
    
    const schedule = {
        train_number: tt.train_number,
        train_name: tt.train_name,
        route_description: `${tt.source_station} to ${tt.destination_station}`,
        running_days: [],
        stops: tt.routeGroup.flatMap(rg => rg.routesummary).map(stop => ({
            code: stop.station_code,
            name: stop.station_name,
            arrival: stop.sta_min ? Math.floor(stop.sta_min / 60) + ":" + (stop.sta_min % 60).toString().padStart(2, '0') : "--:--",
            departure: stop.std_min ? Math.floor(stop.std_min / 60) + ":" + (stop.std_min % 60).toString().padStart(2, '0') : "--:--",
            day: stop.day
        })),
        source: "Public Website Scrape (RailYatri)",
        cache_ttl: 300,
        notice: "Live data extracted from public website."
    };
    
    console.log(schedule.stops.slice(0, 3));
  } catch (e) {
    console.error(e.message);
  }
}
test();
