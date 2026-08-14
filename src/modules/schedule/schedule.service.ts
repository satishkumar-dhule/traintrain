import { AgentAggregator } from '../core/AgentAggregator';
import { RailyatriScheduleSource } from './sources/railyatri.source';

export class ScheduleService {
  private aggregator: AgentAggregator<any>;

  constructor() {
    this.aggregator = new AgentAggregator([
      new RailyatriScheduleSource()
    ]);
  }

  async getSchedule(train: string) {
    if (!train) {
      throw new Error("Invalid train number.");
    }
    const result = await this.aggregator.execute(train);
    return result.data;
  }
}
