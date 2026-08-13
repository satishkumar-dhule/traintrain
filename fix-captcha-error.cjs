const fs = require('fs');
let code = fs.readFileSync('server.ts', 'utf-8');
code = code.replace(
  'if (err instanceof CaptchaRequiredError) {',
  'if (err instanceof CaptchaRequiredError || err.name === "CaptchaRequiredError" || err.error === "captcha_required") {'
);
fs.writeFileSync('server.ts', code);
