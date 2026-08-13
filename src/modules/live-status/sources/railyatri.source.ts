import { DataSource } from '../../core/AgentAggregator';

export class RailyatriLiveSource implements DataSource<any> {
  name = 'Railyatri';

  async fetch(train: string): Promise<any> {
    const axios = (await import('axios')).default;
    const cheerio = await import('cheerio');
    
    const response = await axios.get(`https://www.railyatri.in/live-train-status/${train}`, {
      headers: { 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' }
    });
    
    const $ = cheerio.load(response.data);
    const nextData = $('#__NEXT_DATA__').html();
    if (!nextData) throw new Error(`Live status not found on Railyatri for ${train}.`);
    
    const data = JSON.parse(nextData);
    const lts = data.props?.pageProps?.ltsData;
    
    if (!lts) throw new Error("Invalid live train data received.");

    return {
      train_number: lts.train_number || train,
      train_name: lts.train_name || "Unknown",
      start_date: lts.train_start_date,
      title: lts.title,
      new_message: lts.new_message,
      is_run_day: lts.is_run_day,
      at_src_dstn: lts.at_src_dstn,
      at_src: lts.at_src,
      at_dstn: lts.at_dstn,
      source_name: lts.source_stn_name,
      dest_name: lts.dest_stn_name,
      platform_number: lts.platform_number,
      next_station_name: lts.next_station_name,
      source: "Railyatri",
      notice: "Live data extracted from Railyatri."
    };
  }
}
