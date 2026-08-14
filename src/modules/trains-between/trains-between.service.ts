import { AgentAggregator } from '../core/AgentAggregator';
import { RailyatriTrainsBetweenSource } from './sources/railyatri.source';

export class TrainsBetweenService {
  private aggregator: AgentAggregator<any>;

  constructor() {
    this.aggregator = new AgentAggregator([
      new RailyatriTrainsBetweenSource()
    ]);
  }

  async getTrainsBetween(from: string, to: string) {
    if (!from || !to) {
      throw new Error("Both source and destination stations are required.");
    }
    if (from === to) {
      throw new Error("Source and destination stations must be different.");
    }
    const result = await this.aggregator.execute(`${from}|${to}`);
    return result.data;
  }
}
