import { AgentAggregator } from '../core/AgentAggregator';
import { RailyatriPnrSource } from './sources/railyatri.source';
import { ConfirmTktPnrSource } from './sources/confirmtkt.source';
import { EtrainPnrSource } from './sources/etrain.source';
import { TrainmanPnrSource } from './sources/trainman.source';
import { IxigoPnrSource } from './sources/ixigo.source';
import { MakeMyTripPnrSource } from './sources/makemytrip.source';
import { GoibiboPnrSource } from './sources/goibibo.source';
import { YatraPnrSource } from './sources/yatra.source';
import { CleartripPnrSource } from './sources/cleartrip.source';
import { NDTVPnrSource } from './sources/ndtv.source';
import { NTESPnrSource } from './sources/ntes.source';
import { IRCTCPnrSource } from './sources/irctc.source';
import { IndiaRailInfoPnrSource } from './sources/indiarailinfo.source';
import { WhereIsMyTrainPnrSource } from './sources/whereismytrain.source';
import { SpotUrTrainPnrSource } from './sources/spoturtrain.source';
import { TrainStatusPnrSource } from './sources/trainstatus.source';
import { ProKeralaPnrSource } from './sources/prokerala.source';
import { RailEnquiryPnrSource } from './sources/railenquiry.source';
import { RunningStatusPnrSource } from './sources/runningstatus.source';
import { LiveTrainStatusPnrSource } from './sources/livetrainstatus.source';

export class PnrService {
  private aggregator: AgentAggregator<any>;

  constructor() {
    this.aggregator = new AgentAggregator([
      new RailyatriPnrSource(),
      new ConfirmTktPnrSource(),
      new EtrainPnrSource(),
      new TrainmanPnrSource(),
      new IxigoPnrSource(),
      new MakeMyTripPnrSource(),
      new GoibiboPnrSource(),
      new YatraPnrSource(),
      new CleartripPnrSource(),
      new NDTVPnrSource(),
      new NTESPnrSource(),
      new IRCTCPnrSource(),
      new IndiaRailInfoPnrSource(),
      new WhereIsMyTrainPnrSource(),
      new SpotUrTrainPnrSource(),
      new TrainStatusPnrSource(),
      new ProKeralaPnrSource(),
      new RailEnquiryPnrSource(),
      new RunningStatusPnrSource(),
      new LiveTrainStatusPnrSource()
    ]);
  }

  async getPnrStatus(pnr: string, targetSource?: string, captcha?: {text: string, sessionId: string}) {
    if (!/^\d{10}$/.test(pnr)) {
      throw new Error("Invalid PNR format. Must be 10 digits.");
    }
    const result = await this.aggregator.execute(pnr, targetSource, captcha);
    return result.data;
  }
}
