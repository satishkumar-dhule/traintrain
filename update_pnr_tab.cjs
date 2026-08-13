const fs = require('fs');
let content = fs.readFileSync('src/components/PnrTab.tsx', 'utf-8');

// Add captcha states
content = content.replace(
  "const [error, setError] = useState<string | null>(null);",
  `const [error, setError] = useState<string | null>(null);
  const [captchaRequest, setCaptchaRequest] = useState<{ image: string; sessionId: string; source: string } | null>(null);
  const [captchaInput, setCaptchaInput] = useState('');`
);

// Update handleSearch signature and behavior
content = content.replace(
  "const handleSearch = async (e: React.FormEvent) => {",
  `const handleSearch = async (e?: React.FormEvent, isCaptchaSubmit = false) => {
    if (e) e.preventDefault();`
);

content = content.replace(
  "setError(null);\n    setData(null);",
  `setError(null);
    if (!isCaptchaSubmit) {
       setData(null);
       setCaptchaRequest(null);
       setCaptchaInput('');
    }`
);

// Modify fetch URL
content = content.replace(
  "const res = await fetch(`/rail-api/pnr?pnr=${pnr}`);",
  `let url = \`/rail-api/pnr?pnr=\${pnr}\`;
      if (isCaptchaSubmit && captchaRequest && captchaInput) {
        url += \`&captcha_text=\${encodeURIComponent(captchaInput)}&captcha_session=\${encodeURIComponent(captchaRequest.sessionId)}&captcha_source=\${encodeURIComponent(captchaRequest.source)}\`;
      }
      const res = await fetch(url);`
);

// Catch 428
content = content.replace(
  "if (!res.ok) {",
  `if (res.status === 428) {
        const json = await res.json();
        setCaptchaRequest({
          image: json.image,
          sessionId: json.sessionId,
          source: json.source
        });
        setLoading(false);
        return;
      }
      const json = await res.json();
      if (!res.ok) {`
);

// Add clear for success
content = content.replace(
  "setData(json);",
  `setData(json);\n      setCaptchaRequest(null);\n      setCaptchaInput('');`
);

// Add CAPTCHA UI block
const captchaUI = `{captchaRequest && (
        <div className="p-6 bg-white border border-blue-200 rounded-xl shadow-sm space-y-4">
          <div className="flex items-center gap-2 text-blue-800">
            <Info className="w-5 h-5" />
            <h4 className="font-semibold text-lg">Verification Required by {captchaRequest.source}</h4>
          </div>
          <p className="text-slate-600 text-sm">Please solve the CAPTCHA below to proceed with checking your PNR status.</p>
          <div className="bg-slate-50 p-4 rounded-lg flex justify-center border border-slate-200">
            {captchaRequest.image.startsWith('data:image') || captchaRequest.image.startsWith('http') ? (
              <img src={captchaRequest.image} alt="CAPTCHA" className="max-h-24 rounded shadow-sm" />
            ) : (
               <div className="text-lg font-mono font-bold tracking-widest text-slate-800 bg-white px-6 py-3 border border-slate-300 shadow-inner rounded">{captchaRequest.image}</div>
            )}
          </div>
          <form onSubmit={(e) => handleSearch(e, true)} className="flex gap-4">
            <input
              type="text"
              value={captchaInput}
              onChange={(e) => setCaptchaInput(e.target.value)}
              placeholder="Enter text from image"
              className="flex-1 px-4 py-3 bg-white border border-slate-200 rounded-lg text-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-shadow text-lg"
              required
              autoFocus
            />
            <button
              type="submit"
              disabled={loading || !captchaInput}
              className="px-6 py-3 bg-blue-600 text-white font-medium rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {loading ? 'Verifying...' : 'Submit CAPTCHA'}
            </button>
          </form>
        </div>
      )}`;

content = content.replace(
  "const json = await res.json();",
  ""
);

content = content.replace(
  "{error && (",
  captchaUI + "\n      {error && ("
);

fs.writeFileSync('src/components/PnrTab.tsx', content);
