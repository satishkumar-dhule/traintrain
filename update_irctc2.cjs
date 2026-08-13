const fs = require('fs');
let content = fs.readFileSync('src/modules/pnr/sources/irctc.source.ts', 'utf-8');

content = content.replace(
  "if (!captcha) {",
  `if (!captcha || captcha.text === 'REFRESH') {
          const texts = ['W7K9G', 'A2B4C', 'X9Y8Z', 'Q1W2E'];
          const txt = texts[Math.floor(Math.random() * texts.length)];
          // Force a captcha for demonstration
          reject(new CaptchaRequiredError('IRCTC', \`https://placehold.co/150x50/e2e8f0/1e293b.png?text=\${txt}\`, 'mock-session-id-' + txt));
        }`
).replace(
  "reject(new CaptchaRequiredError('IRCTC', 'https://placehold.co/150x50/e2e8f0/1e293b.png?text=W7K9G', 'mock-session-id'));",
  ""
).replace(
  "else if (captcha.text !== 'W7K9G') {",
  `else if (!captcha.sessionId.endsWith(captcha.text)) {
          const texts = ['W7K9G', 'A2B4C', 'X9Y8Z', 'Q1W2E'];
          const txt = texts[Math.floor(Math.random() * texts.length)];
          reject(new CaptchaRequiredError('IRCTC', \`https://placehold.co/150x50/f87171/991b1b.png?text=\${txt}\`, 'mock-session-id-' + txt));
        }`
).replace(
  "reject(new CaptchaRequiredError('IRCTC', 'https://placehold.co/150x50/f87171/991b1b.png?text=X2M4P', 'mock-session-id-2'));",
  ""
);

fs.writeFileSync('src/modules/pnr/sources/irctc.source.ts', content);
