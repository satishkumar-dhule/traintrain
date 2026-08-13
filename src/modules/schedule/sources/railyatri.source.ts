import { DataSource } from '../../core/AgentAggregator';


export class RailyatriScheduleSource implements DataSource<any> {
  name = 'Railyatri';

  async fetch(train: string): Promise<any> {
    
    const axios = (await import('axios')).default;
    const cheerio = await import('cheerio');
    
    const response = await axios.get(`https://www.railyatri.in/time-table/${train}`, {
      headers: { 'User-Agent': 'Mozilla/5.0' }
    });
    
    const $ = cheerio.load(response.data);
    const nextData = $('#__NEXT_DATA__').html();
    if (!nextData) throw new Error("Train schedule not found on Railyatri.");
    
    const data = JSON.parse(nextData);
    const tt = data.props?.pageProps?.trainTimeTable;
    
    if (!tt || !tt.routeGroup) throw new Error("Invalid train data received.");

    const stops = tt.routeGroup.flatMap((rg: any) => rg.routesummary).map((stop: any) => ({
      code: stop.station_code || "",
      name: stop.station_name || "",
      arrival: stop.sta_min ? Math.floor(stop.sta_min / 60) + ":" + (stop.sta_min % 60).toString().padStart(2, '0') : "--:--",
      departure: stop.std_min ? Math.floor(stop.std_min / 60) + ":" + (stop.std_min % 60).toString().padStart(2, '0') : "--:--",
      day: stop.day || 1
    }));

    return {
      train_number: train,
      train_name: tt.train_name || "Unknown",
      route_description: `${tt.source_station || 'Unknown'} to ${tt.destination_station || 'Unknown'}`,
      running_days: tt.run_days ? Object.keys(tt.run_days).filter(d => tt.run_days[d] === "1").map(d => d.charAt(0).toUpperCase() + d.slice(1, 3)) : [],
      stops: stops,
      source: "Railyatri",
      cache_ttl: 300,
      notice: "Live data extracted from Railyatri."
    };
    
  }
}
