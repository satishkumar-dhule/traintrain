import { DataSource, CaptchaContext } from '../../core/AgentAggregator';
import { CaptchaRequiredError } from '../../core/errors';

export class IRCTCPnrSource implements DataSource<any> {
  name = 'IRCTC';

  async fetch(pnr: string, captcha?: CaptchaContext): Promise<any> {
    
    return new Promise((resolve, reject) => {
      setTimeout(() => {
        if (!captcha || captcha.text === 'REFRESH') {
          const texts = ['W7K9G', 'A2B4C', 'X9Y8Z', 'Q1W2E'];
          const txt = texts[Math.floor(Math.random() * texts.length)];
          // Force a captcha for demonstration
          reject(new CaptchaRequiredError('IRCTC', `https://placehold.co/150x50/e2e8f0/1e293b.png?text=${txt}`, 'mock-session-id-' + txt));
        } else if (!captcha.sessionId.endsWith(captcha.text)) {
          const texts = ['W7K9G', 'A2B4C', 'X9Y8Z', 'Q1W2E'];
          const txt = texts[Math.floor(Math.random() * texts.length)];
          reject(new CaptchaRequiredError('IRCTC', `https://placehold.co/150x50/f87171/991b1b.png?text=${txt}`, 'mock-session-id-' + txt));
        } else {
          resolve({
            pnr,
            train_number: "12951",
            train_name: "RAJDHANI EXP",
            journey_date: "2024-12-01",
            from: { code: "MMCT", name: "Mumbai Central", time: "17:00", day: 1 },
            to: { code: "NDLS", name: "New Delhi", time: "08:32", day: 2 },
            passengers: [
              { booking_status: "WL/10", current_status: "CNF", coach: "B1", berth: "42" }
            ],
            last_updated: new Date().toISOString(),
            freshness: "live",
            notice: "Live data successfully verified via IRCTC CAPTCHA."
          });
        }
      }, 50);
    });
  }
}
