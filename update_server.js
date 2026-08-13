const fs = require('fs');
let content = fs.readFileSync('server.ts', 'utf-8');

// Import CaptchaRequiredError
content = content.replace(
  'import { ScheduleService } from "./src/modules/schedule/schedule.service";',
  'import { ScheduleService } from "./src/modules/schedule/schedule.service";\nimport { CaptchaRequiredError } from "./src/modules/core/errors";'
);

// PNR logic
content = content.replace(
  'const data = await pnrService.getPnrStatus(pnr);',
  `const targetSource = req.query.captcha_source as string;
      const captcha = req.query.captcha_text ? {
        text: req.query.captcha_text as string,
        sessionId: req.query.captcha_session as string
      } : undefined;
      
      const data = await pnrService.getPnrStatus(pnr, targetSource, captcha);`
);

content = content.replace(
  'res.status(500).json({ error: err.message || "Live PNR check failed." });',
  `if (err instanceof CaptchaRequiredError) {
        res.status(428).json({
          error: 'captcha_required',
          source: err.source,
          image: err.imageBase64,
          sessionId: err.sessionId,
          message: err.message
        });
      } else {
        res.status(500).json({ error: err.message || "Live PNR check failed." });
      }`
);

fs.writeFileSync('server.ts', content);
