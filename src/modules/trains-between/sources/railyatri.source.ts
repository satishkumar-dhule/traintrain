import { DataSource } from '../../core/AgentAggregator';

export class RailyatriTrainsBetweenSource implements DataSource<any> {
  name = 'Railyatri';

  async fetch(query: string): Promise<any> {
    const axios = (await import('axios')).default;
    const [from, to, date] = query.split('|');
    if (!from || !to) throw new Error("Both source and destination stations are required.");

    const dateOfJourney = date || new Date().toISOString().split('T')[0];

    const response = await axios.get(
      `https://trainticketapi.railyatri.in/api/trains-between-station-with-sa.json?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}&dateOfJourney=${encodeURIComponent(dateOfJourney)}`,
      {
        headers: {
          'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
          'Content-Type': 'application/json; charset=UTF-8',
          'lang': 'English'
        },
        timeout: 8000
      }
    );

    const body = response.data;
    if (!body || body.success !== true) {
      throw new Error(body?.error_msg || "No trains found between the selected stations.");
    }

    const dayIndex = (day: string): number => {
      const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
      const idx = days.indexOf(day);
      return idx === -1 ? -1 : idx;
    };

    const trains = (body.train_between_stations || []).map((t: any) => {
      const runs_on = Array(7).fill(false);
      (t.run_days || []).forEach((d: string) => {
        const idx = dayIndex(d);
        if (idx >= 0) runs_on[idx] = true;
      });

      return {
        number: t.train_number,
        name: t.extended_train_name || t.train_name || "Unknown",
        departure_time: t.from_std || "--:--",
        arrival_time: t.to_std || "--:--",
        duration: t.duration || "",
        duration_min: t.duration_min || 0,
        distance: t.distance || 0,
        from_station: t.from_station_name || "",
        to_station: t.to_station_name || "",
        runs_on,
        classes: Array.isArray(t.journey_class) ? t.journey_class : []
      };
    }).sort((a: any, b: any) => (a.departure_time.localeCompare(b.departure_time)));

    return {
      src: from,
      dst: to,
      date: dateOfJourney,
      train_count: trains.length,
      trains,
      source: "Railyatri",
      notice: "Live trains-between data from Railyatri."
    };
  }
}
