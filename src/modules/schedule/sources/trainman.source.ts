import { DataSource } from '../../core/AgentAggregator';


export class TrainmanScheduleSource implements DataSource<any> {
  name = 'Trainman';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Trainman source not implemented yet')), 50);
    });
    
  }
}
