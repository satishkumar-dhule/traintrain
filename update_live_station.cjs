const fs = require('fs');
let content = fs.readFileSync('src/components/LiveStationTab.tsx', 'utf-8');

const newRender = `  return (
    <div className="space-y-8 animate-in fade-in duration-500">
      <form onSubmit={handleSearch} className="flex flex-col md:flex-row gap-4 p-2 bg-white rounded-3xl shadow-sm border border-slate-100">
        <div className="flex-1 relative group">
          <div className="absolute inset-y-0 left-0 pl-6 flex items-center pointer-events-none">
            <MapPin className="w-6 h-6 text-slate-400 group-focus-within:text-blue-600 transition-colors" />
          </div>
          <input
            type="text"
            value={station}
            onChange={(e) => setStation(e.target.value.toUpperCase())}
            placeholder="Station Code (e.g. NDLS)"
            className="w-full pl-16 pr-6 py-5 bg-transparent text-slate-900 focus:outline-none text-xl font-bold tracking-widest placeholder:font-medium placeholder:tracking-normal uppercase"
            required
          />
        </div>
        <div className="w-px bg-slate-100 hidden md:block my-2"></div>
        <div className="md:w-64 relative">
          <select
            value={hours}
            onChange={(e) => setHours(e.target.value)}
            className="w-full px-6 py-5 bg-transparent text-slate-900 focus:outline-none text-lg font-bold appearance-none cursor-pointer"
          >
            <option value="2">Next 2 Hours</option>
            <option value="4">Next 4 Hours</option>
            <option value="8">Next 8 Hours</option>
          </select>
          <div className="absolute inset-y-0 right-6 flex items-center pointer-events-none text-slate-400">
            <Clock className="w-5 h-5" />
          </div>
        </div>
        <button
          type="submit"
          disabled={loading || !station}
          className="m-2 md:m-0 px-10 py-5 bg-slate-900 text-white font-bold rounded-2xl hover:bg-slate-800 disabled:opacity-50 transition-all active:scale-95"
        >
          {loading ? 'Searching...' : 'Go'}
        </button>
      </form>

      {error && (
        <div className="p-5 bg-red-50 text-red-700 rounded-2xl flex items-start gap-4 border border-red-100">
          <AlertCircle className="w-6 h-6 flex-shrink-0 mt-0.5 text-red-500" />
          <p className="font-medium text-lg leading-tight">{error}</p>
        </div>
      )}

      {data && !error && (
        <div className="bg-white border-2 border-slate-200 rounded-[2rem] overflow-hidden shadow-sm">
          <div className="bg-slate-900 p-8 sm:p-10 text-white flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
            <div>
              <p className="text-sm font-bold tracking-widest text-slate-400 uppercase mb-2">Departures & Arrivals</p>
              <h3 className="text-3xl sm:text-4xl font-extrabold tracking-tight">
                {station}
              </h3>
            </div>
            <div className="px-4 py-2 bg-slate-800 rounded-xl text-sm font-bold uppercase tracking-widest text-slate-300">
              {hours} Hour Window
            </div>
          </div>
          
          <div className="flex flex-col">
            {data.trains && data.trains.length > 0 ? data.trains.map((t: any, i: number) => (
              <div key={i} className="p-6 sm:p-8 border-b border-slate-100 hover:bg-slate-50 transition-colors flex flex-col xl:flex-row xl:items-center justify-between gap-6 group">
                <div className="flex-1">
                  <div className="flex items-center gap-3 mb-2">
                    <span className="px-3 py-1 bg-slate-100 text-slate-600 rounded-lg text-sm font-bold tracking-widest">{t.number}</span>
                    <h4 className="font-bold text-slate-900 text-xl tracking-tight">{t.name}</h4>
                  </div>
                  <p className="text-slate-500 flex items-center gap-2 font-medium">
                    {t.source} <ArrowRight className="w-4 h-4 text-slate-300" /> {t.dest}
                  </p>
                </div>
                
                <div className="flex gap-4 sm:gap-8 bg-white xl:bg-transparent p-4 xl:p-0 rounded-2xl xl:rounded-none border xl:border-none border-slate-100 shadow-sm xl:shadow-none">
                  <div className="w-24">
                    <p className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-1">STA</p>
                    <p className="font-bold text-slate-900 text-xl">{t.sta}</p>
                  </div>
                  <div className="w-24">
                    <p className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-1">ETA</p>
                    <p className={\`font-black text-xl \${t.delay_arr ? 'text-red-500' : 'text-emerald-500'}\`}>{t.eta}</p>
                  </div>
                  <div className="w-20 pl-4 sm:pl-8 border-l-2 border-slate-100">
                    <p className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-1">PF</p>
                    <p className="font-black text-slate-900 text-2xl">{t.platform || '-'}</p>
                  </div>
                </div>
              </div>
            )) : (
              <div className="p-12 text-center text-slate-400 font-medium">No trains scheduled in this window.</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
`;

const startIndex = content.indexOf('return (');
if (startIndex !== -1) {
  content = content.substring(0, startIndex) + newRender;
  fs.writeFileSync('src/components/LiveStationTab.tsx', content);
}
