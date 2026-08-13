import { DataSource } from '../../core/AgentAggregator';


export class IndiaRailInfoScheduleSource implements DataSource<any> {
  name = 'IndiaRailInfo';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('IndiaRailInfo source not implemented yet')), 50);
    });
    
  }
}
