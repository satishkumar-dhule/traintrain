import { DataSource } from '../../core/AgentAggregator';


export class EtrainScheduleSource implements DataSource<any> {
  name = 'Etrain';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Etrain source not implemented yet')), 50);
    });
    
  }
}
