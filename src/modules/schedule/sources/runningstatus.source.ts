import { DataSource } from '../../core/AgentAggregator';


export class RunningStatusScheduleSource implements DataSource<any> {
  name = 'RunningStatus';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('RunningStatus source not implemented yet')), 50);
    });
    
  }
}
