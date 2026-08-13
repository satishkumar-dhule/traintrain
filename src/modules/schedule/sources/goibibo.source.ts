import { DataSource } from '../../core/AgentAggregator';


export class GoibiboScheduleSource implements DataSource<any> {
  name = 'Goibibo';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Goibibo source not implemented yet')), 50);
    });
    
  }
}
