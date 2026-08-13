const fs = require('fs');
let code = fs.readFileSync('src/modules/core/AgentAggregator.ts', 'utf-8');
code = code.replace(
  'const captchaError = err.errors.find(e => e instanceof CaptchaRequiredError);',
  'const captchaError = err.errors.find(e => e instanceof CaptchaRequiredError || e.name === "CaptchaRequiredError");'
);
fs.writeFileSync('src/modules/core/AgentAggregator.ts', code);
