import { DataSource } from '../../core/AgentAggregator';


export class RailEnquiryPnrSource implements DataSource<any> {
  name = 'RailEnquiry';

  async fetch(pnr: string): Promise<any> {
    
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('RailEnquiry source not implemented yet')), 50);
    });
    
  }
}
