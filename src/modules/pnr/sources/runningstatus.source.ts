import { DataSource } from '../../core/AgentAggregator';


export class RunningStatusPnrSource implements DataSource<any> {
  name = 'RunningStatus';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('RunningStatus source not implemented yet')), 50);
    });
    
  }
}
