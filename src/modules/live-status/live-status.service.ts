import { AgentAggregator } from '../core/AgentAggregator';
import { RailyatriLiveSource } from './sources/railyatri.source';
import { ConfirmTktLiveSource } from './sources/confirmtkt.source';
import { EtrainLiveSource } from './sources/etrain.source';
import { TrainmanLiveSource } from './sources/trainman.source';
import { IxigoLiveSource } from './sources/ixigo.source';
import { MakeMyTripLiveSource } from './sources/makemytrip.source';
import { GoibiboLiveSource } from './sources/goibibo.source';
import { YatraLiveSource } from './sources/yatra.source';
import { CleartripLiveSource } from './sources/cleartrip.source';
import { NDTVLiveSource } from './sources/ndtv.source';
import { NTESLiveSource } from './sources/ntes.source';
import { IRCTCLiveSource } from './sources/irctc.source';
import { IndiaRailInfoLiveSource } from './sources/indiarailinfo.source';
import { WhereIsMyTrainLiveSource } from './sources/whereismytrain.source';
import { SpotUrTrainLiveSource } from './sources/spoturtrain.source';
import { TrainStatusLiveSource } from './sources/trainstatus.source';
import { ProKeralaLiveSource } from './sources/prokerala.source';
import { RailEnquiryLiveSource } from './sources/railenquiry.source';
import { RunningStatusLiveSource } from './sources/runningstatus.source';
import { LiveTrainStatusLiveSource } from './sources/livetrainstatus.source';

export class LiveStatusService {
  private aggregator: AgentAggregator<any>;

  constructor() {
    this.aggregator = new AgentAggregator([
      new RailyatriLiveSource(),
      new ConfirmTktLiveSource(),
      new EtrainLiveSource(),
      new TrainmanLiveSource(),
      new IxigoLiveSource(),
      new MakeMyTripLiveSource(),
      new GoibiboLiveSource(),
      new YatraLiveSource(),
      new CleartripLiveSource(),
      new NDTVLiveSource(),
      new NTESLiveSource(),
      new IRCTCLiveSource(),
      new IndiaRailInfoLiveSource(),
      new WhereIsMyTrainLiveSource(),
      new SpotUrTrainLiveSource(),
      new TrainStatusLiveSource(),
      new ProKeralaLiveSource(),
      new RailEnquiryLiveSource(),
      new RunningStatusLiveSource(),
      new LiveTrainStatusLiveSource()
    ]);
  }

  async getLiveStatus(train: string, dateStr?: string) {
    if (!train) {
      throw new Error("Invalid train number.");
    }
    const result = await this.aggregator.execute({ train, dateStr });
    return result; // returning { source, data }
  }
}
