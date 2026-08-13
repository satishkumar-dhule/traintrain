import { DataSource } from '../../core/AgentAggregator';


export class IxigoScheduleSource implements DataSource<any> {
  name = 'Ixigo';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Ixigo source not implemented yet')), 50);
    });
    
  }
}
