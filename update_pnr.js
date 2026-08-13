const fs = require('fs');
let content = fs.readFileSync('src/modules/pnr/pnr.service.ts', 'utf-8');
content = content.replace(
  'async getPnrStatus(pnr: string) {',
  'async getPnrStatus(pnr: string, targetSource?: string, captcha?: {text: string, sessionId: string}) {'
);
content = content.replace(
  'const result = await this.aggregator.execute(pnr);',
  'const result = await this.aggregator.execute(pnr, targetSource, captcha);'
);
fs.writeFileSync('src/modules/pnr/pnr.service.ts', content);
