import { DataSource } from '../../core/AgentAggregator';


export class NTESPnrSource implements DataSource<any> {
  name = 'NTES';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('NTES source not implemented yet')), 50);
    });
    
  }
}
