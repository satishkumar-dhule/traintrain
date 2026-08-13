import { DataSource } from '../../core/AgentAggregator';


export class EtrainPnrSource implements DataSource<any> {
  name = 'Etrain';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Etrain source not implemented yet')), 50);
    });
    
  }
}
