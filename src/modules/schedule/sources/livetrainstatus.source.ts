import { DataSource } from '../../core/AgentAggregator';


export class LiveTrainStatusScheduleSource implements DataSource<any> {
  name = 'LiveTrainStatus';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('LiveTrainStatus source not implemented yet')), 50);
    });
    
  }
}
