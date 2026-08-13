import { DataSource } from '../../core/AgentAggregator';


export class NTESScheduleSource implements DataSource<any> {
  name = 'NTES';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('NTES source not implemented yet')), 50);
    });
    
  }
}
