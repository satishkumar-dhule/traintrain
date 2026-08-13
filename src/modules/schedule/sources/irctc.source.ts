import { DataSource } from '../../core/AgentAggregator';


export class IRCTCScheduleSource implements DataSource<any> {
  name = 'IRCTC';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('IRCTC source not implemented yet')), 50);
    });
    
  }
}
