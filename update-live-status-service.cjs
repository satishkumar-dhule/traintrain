const fs = require('fs');

const sources = [
  'Railyatri', 'ConfirmTkt', 'Etrain', 'Trainman', 'Ixigo', 'MakeMyTrip',
  'Goibibo', 'Yatra', 'Cleartrip', 'NDTV', 'NTES', 'IRCTC', 'IndiaRailInfo',
  'WhereIsMyTrain', 'SpotUrTrain', 'TrainStatus', 'ProKerala', 'RailEnquiry',
  'RunningStatus', 'LiveTrainStatus'
];

let imports = sources.map(s => \`import { \${s}LiveSource } from './sources/\${s.toLowerCase()}.source';\`).join('\\n');

let code = \`import { AgentAggregator } from '../core/AgentAggregator';
\${imports}

export class LiveStatusService {
  private aggregator: AgentAggregator<any>;

  constructor() {
    this.aggregator = new AgentAggregator([
      \${sources.map(s => \`new \${s}LiveSource()\`).join(',\\n      ')}
    ]);
  }

  async getLiveStatus(train: string, dateStr?: string) {
    if (!train) {
      throw new Error("Invalid train number.");
    }
    const result = await this.aggregator.execute({ train, dateStr });
    return result; // returning { source, data } instead of just data so we can track the origin!
  }
}
\`;

fs.writeFileSync('src/modules/live-status/live-status.service.ts', code);
