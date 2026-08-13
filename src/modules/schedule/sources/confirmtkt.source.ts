import { DataSource } from '../../core/AgentAggregator';


export class ConfirmTktScheduleSource implements DataSource<any> {
  name = 'ConfirmTkt';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('ConfirmTkt source not implemented yet')), 50);
    });
    
  }
}
