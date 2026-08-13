import { DataSource } from '../../core/AgentAggregator';


export class GoibiboPnrSource implements DataSource<any> {
  name = 'Goibibo';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Goibibo source not implemented yet')), 50);
    });
    
  }
}
