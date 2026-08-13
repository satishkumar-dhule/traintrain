import { AgentAggregator } from '../core/AgentAggregator';
import { RailyatriLiveSource } from './sources/railyatri.source';

export class LiveStatusService {
  private aggregator: AgentAggregator<any>;

  constructor() {
    this.aggregator = new AgentAggregator([
      new RailyatriLiveSource()
    ]);
  }

  async getLiveStatus(train: string) {
    if (!train) {
      throw new Error("Invalid train number.");
    }
    const result = await this.aggregator.execute(train);
    return result.data;
  }
}
