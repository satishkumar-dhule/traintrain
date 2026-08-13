import { DataSource } from '../../core/AgentAggregator';


export class TrainStatusScheduleSource implements DataSource<any> {
  name = 'TrainStatus';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('TrainStatus source not implemented yet')), 50);
    });
    
  }
}
