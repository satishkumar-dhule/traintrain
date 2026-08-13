import { DataSource } from '../../core/AgentAggregator';


export class YatraScheduleSource implements DataSource<any> {
  name = 'Yatra';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Yatra source not implemented yet')), 50);
    });
    
  }
}
