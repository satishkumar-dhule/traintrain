import { DataSource } from '../../core/AgentAggregator';


export class MakeMyTripPnrSource implements DataSource<any> {
  name = 'MakeMyTrip';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('MakeMyTrip source not implemented yet')), 50);
    });
    
  }
}
