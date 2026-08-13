import { DataSource } from '../../core/AgentAggregator';


export class TrainmanPnrSource implements DataSource<any> {
  name = 'Trainman';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Trainman source not implemented yet')), 50);
    });
    
  }
}
