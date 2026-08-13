import { DataSource } from '../../core/AgentAggregator';


export class NDTVScheduleSource implements DataSource<any> {
  name = 'NDTV';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('NDTV source not implemented yet')), 50);
    });
    
  }
}
