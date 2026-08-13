const fs = require('fs');
let content = fs.readFileSync('src/components/PnrTab.tsx', 'utf-8');

// Ensure RefreshCw is imported
if (!content.includes('RefreshCw')) {
  content = content.replace('Ticket }', 'Ticket, RefreshCw }');
}

// Add the refresh button next to the info header
const captchaHeaderOriginal = `<h4 className="font-semibold text-lg">Verification Required by {captchaRequest.source}</h4>`;
const captchaHeaderNew = `<div className="flex-1">
              <h4 className="font-semibold text-lg">Verification Required by {captchaRequest.source}</h4>
            </div>
            <button
              type="button"
              onClick={(e) => handleSearch(e, false)}
              className="p-2 text-blue-600 hover:bg-blue-50 rounded-full transition-colors flex items-center gap-1 text-sm font-medium"
              title="Get a new image"
            >
              <RefreshCw className="w-4 h-4" />
              Refresh
            </button>`;
content = content.replace(captchaHeaderOriginal, captchaHeaderNew);

fs.writeFileSync('src/components/PnrTab.tsx', content);
