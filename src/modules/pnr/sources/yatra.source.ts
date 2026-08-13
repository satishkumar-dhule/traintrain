import { DataSource } from '../../core/AgentAggregator';


export class YatraPnrSource implements DataSource<any> {
  name = 'Yatra';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Yatra source not implemented yet')), 50);
    });
    
  }
}
