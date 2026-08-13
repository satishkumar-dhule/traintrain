import { DataSource } from '../../core/AgentAggregator';


export class ProKeralaScheduleSource implements DataSource<any> {
  name = 'ProKerala';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('ProKerala source not implemented yet')), 50);
    });
    
  }
}
