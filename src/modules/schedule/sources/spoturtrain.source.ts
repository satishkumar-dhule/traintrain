import { DataSource } from '../../core/AgentAggregator';


export class SpotUrTrainScheduleSource implements DataSource<any> {
  name = 'SpotUrTrain';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('SpotUrTrain source not implemented yet')), 50);
    });
    
  }
}
