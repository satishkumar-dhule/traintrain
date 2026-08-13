import { DataSource } from '../../core/AgentAggregator';


export class IndiaRailInfoPnrSource implements DataSource<any> {
  name = 'IndiaRailInfo';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('IndiaRailInfo source not implemented yet')), 50);
    });
    
  }
}
