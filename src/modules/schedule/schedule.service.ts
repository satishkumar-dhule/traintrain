import { AgentAggregator } from '../core/AgentAggregator';
import { RailyatriScheduleSource } from './sources/railyatri.source';
import { ConfirmTktScheduleSource } from './sources/confirmtkt.source';
import { EtrainScheduleSource } from './sources/etrain.source';
import { TrainmanScheduleSource } from './sources/trainman.source';
import { IxigoScheduleSource } from './sources/ixigo.source';
import { MakeMyTripScheduleSource } from './sources/makemytrip.source';
import { GoibiboScheduleSource } from './sources/goibibo.source';
import { YatraScheduleSource } from './sources/yatra.source';
import { CleartripScheduleSource } from './sources/cleartrip.source';
import { NDTVScheduleSource } from './sources/ndtv.source';
import { NTESScheduleSource } from './sources/ntes.source';
import { IRCTCScheduleSource } from './sources/irctc.source';
import { IndiaRailInfoScheduleSource } from './sources/indiarailinfo.source';
import { WhereIsMyTrainScheduleSource } from './sources/whereismytrain.source';
import { SpotUrTrainScheduleSource } from './sources/spoturtrain.source';
import { TrainStatusScheduleSource } from './sources/trainstatus.source';
import { ProKeralaScheduleSource } from './sources/prokerala.source';
import { RailEnquiryScheduleSource } from './sources/railenquiry.source';
import { RunningStatusScheduleSource } from './sources/runningstatus.source';
import { LiveTrainStatusScheduleSource } from './sources/livetrainstatus.source';

export class ScheduleService {
  private aggregator: AgentAggregator<any>;

  constructor() {
    this.aggregator = new AgentAggregator([
      new RailyatriScheduleSource(),
      new ConfirmTktScheduleSource(),
      new EtrainScheduleSource(),
      new TrainmanScheduleSource(),
      new IxigoScheduleSource(),
      new MakeMyTripScheduleSource(),
      new GoibiboScheduleSource(),
      new YatraScheduleSource(),
      new CleartripScheduleSource(),
      new NDTVScheduleSource(),
      new NTESScheduleSource(),
      new IRCTCScheduleSource(),
      new IndiaRailInfoScheduleSource(),
      new WhereIsMyTrainScheduleSource(),
      new SpotUrTrainScheduleSource(),
      new TrainStatusScheduleSource(),
      new ProKeralaScheduleSource(),
      new RailEnquiryScheduleSource(),
      new RunningStatusScheduleSource(),
      new LiveTrainStatusScheduleSource()
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
