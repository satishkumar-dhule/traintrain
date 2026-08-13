import { DataSource } from '../../core/AgentAggregator';


export class SpotUrTrainPnrSource implements DataSource<any> {
  name = 'SpotUrTrain';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('SpotUrTrain source not implemented yet')), 50);
    });
    
  }
}
