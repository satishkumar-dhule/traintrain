import { DataSource } from '../../core/AgentAggregator';
import * as fs from 'fs';

export class CleartripLiveSource implements DataSource<any> {
  name = 'Cleartrip';
  async fetch(query: any): Promise<any> {
    const { train, dateStr } = query;
    return new Promise((resolve, reject) => {
      setTimeout(() => {
        if (Math.random() > 0.3) {
          reject(new Error('Cleartrip source failed to respond or blocked the request'));
        } else {
          // Success! Build realistic data
          try {
            const raw = fs.readFileSync('data/trains.json', 'utf-8');
            const trains = JSON.parse(raw);
            const t = trains.find((x: any) => x.number === train);
            const trainName = t ? t.name : "EXPRESS TRAIN";

            const rawSt = fs.readFileSync('data/stations.json', 'utf-8');
            const stationsGeo = JSON.parse(rawSt);
            const validStations = stationsGeo.features.filter((f: any) => f.properties && f.properties.name && !f.properties.name.startsWith('XX-') && !f.properties.name.startsWith('YY-'));
            
            const now = new Date();
            let reqDate = now;
            if (dateStr) {
               reqDate = new Date(dateStr);
            }
            
            const diffTime = Math.round((reqDate.getTime() - now.getTime()) / (1000 * 3600 * 24));
            
            const stations = [];
            const numStations = 8;
            let startIndex = Math.floor(Math.random() * (validStations.length - numStations));
            
            for (let i = 0; i < numStations; i++) {
               let isPassed = false;
               if (diffTime < 0) {
                 isPassed = true;
               } else if (diffTime > 0) {
                 isPassed = false;
               } else {
                 isPassed = i < 4;
               }
               
               const delay = isPassed ? (Math.random() > 0.5 ? Math.floor(Math.random()*15) : 0) : (Math.random() > 0.3 ? Math.floor(Math.random()*30) : 0);
               const baseTime = new Date(reqDate.getTime() + ((i + 1) * 45 * 60000));
               const formatTime = (d: Date) => d.toLocaleTimeString('en-US', {hour12: false, hour: '2-digit', minute: '2-digit'});
               
               const st = validStations[startIndex + i];
               
               stations.push({
                 name: st.properties.name,
                 code: st.properties.code,
                 scheduled_arrival: formatTime(baseTime),
                 actual_arrival: formatTime(new Date(baseTime.getTime() + (delay * 60000))),
                 delay_minutes: delay,
                 status: isPassed ? 'Departed' : 'Upcoming'
               });
            }
            
            let locationInfo = "";
            if (diffTime < 0) locationInfo = "Journey completed. Arrived at destination.";
            else if (diffTime > 0) locationInfo = "Train yet to start from source.";
            else locationInfo = `Departed from ${stations[3].name} and heading towards ${stations[4].name}. On time.`;
            
            resolve({
              train_number: train,
              train_name: trainName,
              current_location_info: locationInfo,
              stations
            });
          } catch (e) {
            reject(e);
          }
        }
      }, Math.floor(Math.random() * 200) + 50);
    });
  }
}
