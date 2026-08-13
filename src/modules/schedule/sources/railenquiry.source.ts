import { DataSource } from '../../core/AgentAggregator';


export class RailEnquiryScheduleSource implements DataSource<any> {
  name = 'RailEnquiry';

  async fetch(train: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('RailEnquiry source not implemented yet')), 50);
    });
    
  }
}
