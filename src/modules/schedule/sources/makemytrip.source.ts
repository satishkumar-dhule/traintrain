import { DataSource } from '../../core/AgentAggregator';


export class MakeMyTripScheduleSource implements DataSource<any> {
  name = 'MakeMyTrip';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('MakeMyTrip source not implemented yet')), 50);
    });
    
  }
}
