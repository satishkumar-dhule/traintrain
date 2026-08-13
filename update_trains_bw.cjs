const fs = require('fs');
let content = fs.readFileSync('src/components/TrainsBetweenTab.tsx', 'utf-8');

const newRender = `  return (
    <div className="space-y-8 animate-in fade-in duration-500">
      <form onSubmit={handleSearch} className="flex flex-col md:flex-row gap-4 p-2 bg-white rounded-3xl shadow-sm border border-slate-100 relative">
        <div className="flex-1 relative group">
          <input
            type="text"
            value={src}
            onChange={(e) => setSrc(e.target.value.toUpperCase())}
            placeholder="From (e.g. NDLS)"
            className="w-full px-6 py-5 bg-transparent text-slate-900 focus:outline-none text-xl font-bold tracking-widest placeholder:font-medium placeholder:tracking-normal uppercase"
            required
          />
        </div>
        
        <div className="md:absolute md:left-1/2 md:-translate-x-1/2 md:top-1/2 md:-translate-y-1/2 flex items-center justify-center py-2 md:py-0 z-10">
          <div className="bg-slate-900 text-white p-3 rounded-xl shadow-md border-4 border-white">
            <ArrowRight className="w-5 h-5" />
          </div>
        </div>

        <div className="flex-1 relative group text-right">
          <input
            type="text"
            value={dst}
            onChange={(e) => setDst(e.target.value.toUpperCase())}
            placeholder="To (e.g. MMCT)"
            className="w-full px-6 py-5 bg-transparent text-slate-900 focus:outline-none text-xl font-bold tracking-widest placeholder:font-medium placeholder:tracking-normal uppercase text-left md:text-right"
            required
          />
        </div>

        <button
          type="submit"
          disabled={loading || !src || !dst}
          className="m-2 md:m-0 px-10 py-5 bg-blue-600 text-white font-bold rounded-2xl hover:bg-blue-700 disabled:opacity-50 transition-all active:scale-95"
        >
          {loading ? 'Searching...' : 'Search'}
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
          <div className="p-8 sm:p-10 border-b border-slate-100 flex items-center gap-4">
             <div className="p-3 bg-blue-50 text-blue-600 rounded-2xl">
                <Train className="w-6 h-6" />
             </div>
             <div>
               <h3 className="text-2xl font-extrabold text-slate-900 tracking-tight">Available Trains</h3>
               <p className="text-slate-500 font-medium">{src} to {dst}</p>
             </div>
          </div>
          
          <div className="flex flex-col">
            {data.trains && data.trains.length > 0 ? data.trains.map((t: any, i: number) => (
              <div key={i} className="p-6 sm:p-8 border-b border-slate-100 hover:bg-slate-50 transition-colors flex flex-col xl:flex-row xl:items-center justify-between gap-6 group">
                <div className="flex-1">
                  <div className="flex items-center gap-3 mb-3">
                    <span className="px-3 py-1 bg-slate-100 text-slate-600 rounded-lg text-sm font-bold tracking-widest">{t.number}</span>
                    <h4 className="font-bold text-slate-900 text-xl tracking-tight">{t.name}</h4>
                  </div>
                  <div className="flex gap-1.5">
                    {['S','M','T','W','T','F','S'].map((day, idx) => (
                      <span key={idx} className={\`w-7 h-7 flex items-center justify-center rounded-lg text-[10px] font-bold \${t.runs_on[idx === 0 ? 6 : idx - 1] ? 'bg-blue-600 text-white shadow-sm' : 'bg-slate-100 text-slate-400'}\`}>
                        {day}
                      </span>
                    ))}
                  </div>
                </div>
                
                <div className="flex gap-4 sm:gap-8 items-center bg-white xl:bg-transparent p-4 xl:p-0 rounded-2xl xl:rounded-none border xl:border-none border-slate-100 shadow-sm xl:shadow-none">
                  <div className="text-center xl:text-right w-20">
                    <p className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-1">Dep</p>
                    <p className="font-bold text-slate-900 text-xl">{t.departure_time}</p>
                  </div>
                  <div className="flex-1 xl:w-32 flex items-center justify-center relative">
                    <div className="h-0.5 bg-slate-200 w-full absolute top-1/2 -translate-y-1/2"></div>
                    <div className="w-2 h-2 rounded-full bg-slate-300 absolute left-0"></div>
                    <div className="w-2 h-2 rounded-full bg-blue-600 absolute right-0"></div>
                  </div>
                  <div className="text-center xl:text-left w-20">
                    <p className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-1">Arr</p>
                    <p className="font-bold text-slate-900 text-xl">{t.arrival_time}</p>
                  </div>
                </div>
              </div>
            )) : (
              <div className="p-12 text-center text-slate-400 font-medium">No trains found.</div>
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
  fs.writeFileSync('src/components/TrainsBetweenTab.tsx', content);
}
