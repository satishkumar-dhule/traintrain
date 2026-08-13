import { DataSource } from '../../core/AgentAggregator';


export class ProKeralaPnrSource implements DataSource<any> {
  name = 'ProKerala';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('ProKerala source not implemented yet')), 50);
    });
    
  }
}
