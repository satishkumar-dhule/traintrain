import { DataSource } from '../../core/AgentAggregator';


export class WhereIsMyTrainPnrSource implements DataSource<any> {
  name = 'WhereIsMyTrain';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('WhereIsMyTrain source not implemented yet')), 50);
    });
    
  }
}
