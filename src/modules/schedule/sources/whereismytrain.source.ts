import { DataSource } from '../../core/AgentAggregator';


export class WhereIsMyTrainScheduleSource implements DataSource<any> {
  name = 'WhereIsMyTrain';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('WhereIsMyTrain source not implemented yet')), 50);
    });
    
  }
}
