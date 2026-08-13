import { DataSource } from '../../core/AgentAggregator';


export class ConfirmTktPnrSource implements DataSource<any> {
  name = 'ConfirmTkt';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('ConfirmTkt source not implemented yet')), 50);
    });
    
  }
}
