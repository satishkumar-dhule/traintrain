import { DataSource } from '../../core/AgentAggregator';


export class CleartripScheduleSource implements DataSource<any> {
  name = 'Cleartrip';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Cleartrip source not implemented yet')), 50);
    });
    
  }
}
