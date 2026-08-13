import { DataSource } from '../../core/AgentAggregator';


export class IxigoPnrSource implements DataSource<any> {
  name = 'Ixigo';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Ixigo source not implemented yet')), 50);
    });
    
  }
}
