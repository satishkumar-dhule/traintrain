import fs from 'fs';
import path from 'path';

export interface Station {
  code: string;
  name: string;
  state: string;
  zone: string;
  city: string; // for UI compat
}

export class StationsService {
  private stations: Station[] = [];
  
  constructor() {
    this.loadStations();
  }

  private loadStations() {
    try {
      const dataPath = path.join(process.cwd(), 'data', 'stations.json');
      const raw = fs.readFileSync(dataPath, 'utf-8');
      const geojson = JSON.parse(raw);
      
      this.stations = geojson.features.map((f: any) => ({
        code: f.properties.code || 'UNK',
        name: f.properties.name || 'Unknown',
        state: f.properties.state || 'Unknown',
        city: f.properties.state || 'Unknown',
        zone: f.properties.zone || 'IR'
      }));
    } catch (err) {
      console.error("Failed to load stations DB:", err);
    }
  }

  search(query: string): Station[] {
    if (!query) return [];
    
    const q = query.toLowerCase().trim();
    
    // Fast exact match or starts with
    const startsWith = this.stations.filter(
      (s) => s.code.toLowerCase().startsWith(q) || s.name.toLowerCase().startsWith(q)
    );
    
    if (startsWith.length > 20) return startsWith.slice(0, 20);
    
    // Add includes if we need more results
    const includes = this.stations.filter(
      (s) => 
        (s.code.toLowerCase().includes(q) || s.name.toLowerCase().includes(q) || s.state.toLowerCase().includes(q)) &&
        !startsWith.includes(s)
    );
    
    return [...startsWith, ...includes].slice(0, 20);
  }
}
