import { AgentAggregator } from '../core/AgentAggregator';
import { RailyatriPnrSource } from './sources/railyatri.source';

export class PnrService {
  private aggregator: AgentAggregator<any>;

  constructor() {
    this.aggregator = new AgentAggregator([
      new RailyatriPnrSource()
    ]);
  }

  async getPnrStatus(pnr: string) {
    if (!/^\d{10}$/.test(pnr)) {
      throw new Error("Invalid PNR format. Must be 10 digits.");
    }
    const result = await this.aggregator.execute(pnr);
    return result.data;
  }
}
