const fs = require('fs');

const liveStatusTsx = `import React, { useState } from 'react';
import { Search, AlertCircle, MapPin, Navigation } from 'lucide-react';

export default function LiveStatusTab() {
  const [train, setTrain] = useState('');
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    setData(null);
    try {
      const res = await fetch(\`/rail-api/live-status?train=\${train}\`);
      const json = await res.json();
      if (!res.ok) throw new Error(json.error || 'Failed to fetch.');
      setData(json);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="w-full">
      <div className="bg-white border-b border-slate-200 md:border md:rounded-2xl shadow-sm p-4 sticky top-0 md:relative z-40">
        <form onSubmit={handleSearch} className="flex gap-2">
          <input
            type="text"
            value={train}
            onChange={(e) => setTrain(e.target.value.replace(/\\D/g, '').slice(0, 5))}
            placeholder="5-digit Train No."
            className="flex-1 bg-slate-100 text-slate-900 px-4 py-3 rounded-xl font-bold text-lg tracking-widest placeholder:font-normal placeholder:tracking-normal focus:outline-none focus:ring-2 focus:ring-blue-500"
            required
            pattern="\\d{5}"
          />
          <button type="submit" disabled={loading || train.length !== 5} className="bg-blue-600 text-white px-6 py-3 rounded-xl font-bold disabled:opacity-50">
            {loading ? '...' : 'Spot'}
          </button>
        </form>
      </div>

      <div className="mt-4 md:mt-6">
        {error && (
          <div className="mx-4 md:mx-0 p-4 bg-red-50 border border-red-100 rounded-xl flex items-start gap-3 text-red-700 mb-4">
            <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5" />
            <p className="font-medium">{error}</p>
          </div>
        )}

        {data && !error && (
          <div className="bg-white border-y md:border border-slate-200 md:rounded-2xl overflow-hidden shadow-sm">
            <div className="p-4 sm:p-6 bg-slate-900 text-white">
               <h2 className="text-3xl font-black tracking-tight">{data.train_number}</h2>
               <p className="text-slate-300 font-medium">{data.train_name}</p>
               <div className="mt-4 bg-blue-600/20 border border-blue-500/30 p-4 rounded-xl flex items-start gap-3">
                  <Navigation className="w-5 h-5 text-blue-400 flex-shrink-0 mt-0.5" />
                  <p className="font-bold text-blue-100 text-sm">{data.current_location_info}</p>
               </div>
            </div>

            <div className="p-0">
               {data.stations?.map((st: any, i: number) => {
                 const isPassed = st.status === 'Departed';
                 const isNext = !isPassed && data.stations[i-1]?.status === 'Departed';
                 
                 return (
                   <div key={i} className={\`flex gap-4 p-4 border-b border-slate-100 \${isNext ? 'bg-blue-50/50' : ''}\`}>
                      <div className="w-16 flex flex-col items-center relative">
                         <div className={\`w-0.5 h-full absolute top-0 \${isPassed ? 'bg-blue-600' : 'bg-slate-200'}\`}></div>
                         <div className={\`w-4 h-4 rounded-full z-10 my-2 \${isPassed ? 'bg-blue-600' : isNext ? 'bg-amber-500 ring-4 ring-amber-500/20' : 'bg-slate-200 border-2 border-white'}\`}></div>
                      </div>
                      <div className="flex-1 py-1">
                         <div className="flex justify-between items-start">
                            <div>
                               <h4 className={\`font-bold \${isPassed ? 'text-slate-400' : 'text-slate-900'}\`}>{st.name}</h4>
                               <p className="text-xs font-bold text-slate-400">{st.code}</p>
                            </div>
                            <div className="text-right">
                               <p className={\`font-bold \${isPassed ? 'text-slate-400' : 'text-slate-900'}\`}>{st.actual_arrival || st.scheduled_arrival}</p>
                               <p className={\`text-xs font-bold \${st.delay_minutes > 0 ? 'text-red-500' : 'text-emerald-500'}\`}>
                                 {st.delay_minutes > 0 ? \`\${st.delay_minutes}m late\` : 'On time'}
                               </p>
                            </div>
                         </div>
                      </div>
                   </div>
                 );
               })}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
`;

fs.writeFileSync('src/components/LiveStatusTab.tsx', liveStatusTsx);

const liveStationTsx = `import React, { useState } from 'react';
import { Clock, AlertCircle } from 'lucide-react';

export default function LiveStationTab() {
  const [station, setStation] = useState('');
  const [hours, setHours] = useState('2');
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!station) return;
    setLoading(true);
    setError(null);
    setData(null);
    try {
      const res = await fetch(\`/rail-api/ntes/live-station?station=\${station}&hours=\${hours}\`);
      const json = await res.json();
      if (!res.ok) throw new Error(json.error || 'Failed.');
      setData(json);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="w-full">
      <div className="bg-white border-b border-slate-200 md:border md:rounded-2xl shadow-sm p-4 sticky top-0 md:relative z-40">
        <form onSubmit={handleSearch} className="flex gap-2">
          <input
            type="text"
            value={station}
            onChange={(e) => setStation(e.target.value.toUpperCase())}
            placeholder="Station Code"
            className="w-1/2 bg-slate-100 text-slate-900 px-4 py-3 rounded-xl font-bold text-lg uppercase focus:outline-none focus:ring-2 focus:ring-blue-500"
            required
          />
          <select value={hours} onChange={(e) => setHours(e.target.value)} className="w-1/4 bg-slate-100 font-bold px-2 rounded-xl focus:outline-none">
            <option value="2">2H</option><option value="4">4H</option><option value="8">8H</option>
          </select>
          <button type="submit" disabled={loading} className="w-1/4 bg-slate-900 text-white rounded-xl font-bold">{loading ? '...' : 'Go'}</button>
        </form>
      </div>

      <div className="mt-4 md:mt-6">
        {error && <div className="mx-4 p-4 bg-red-50 text-red-700 rounded-xl font-medium">{error}</div>}
        
        {data && !error && (
          <div className="bg-black border-y md:border md:rounded-2xl overflow-hidden shadow-xl text-yellow-400 font-mono">
            <div className="p-4 bg-zinc-900 border-b border-zinc-800 flex justify-between items-end">
               <h2 className="text-2xl sm:text-3xl font-black uppercase tracking-widest">{station} DEPARTURES</h2>
               <p className="text-sm font-bold text-zinc-400">NEXT {hours}H</p>
            </div>
            
            <div className="flex flex-col text-sm sm:text-base">
               <div className="flex px-4 py-2 bg-zinc-900 border-b border-zinc-800 text-zinc-500 font-bold tracking-widest text-[10px] sm:text-xs">
                  <div className="w-16">TIME</div>
                  <div className="flex-1">TRAIN</div>
                  <div className="w-12 text-center">PF</div>
                  <div className="w-16 text-right">STATUS</div>
               </div>
               
               {data.trains?.map((t: any, i: number) => (
                 <div key={i} className="flex px-4 py-3 border-b border-zinc-800 items-center">
                    <div className="w-16 font-bold">{t.eta}</div>
                    <div className="flex-1 truncate pr-2">
                       <span className="text-zinc-500 mr-2">{t.number}</span>
                       <span className="font-bold">{t.name}</span>
                    </div>
                    <div className="w-12 text-center font-bold text-white">{t.platform || '-'}</div>
                    <div className={\`w-16 text-right font-bold \${t.delay_arr ? 'text-red-500' : 'text-green-500'}\`}>
                       {t.delay_arr ? 'DELAY' : 'ON TIME'}
                    </div>
                 </div>
               ))}
               {!data.trains?.length && <div className="p-8 text-center text-zinc-600">NO TRAINS SCHEDULED</div>}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
`;

fs.writeFileSync('src/components/LiveStationTab.tsx', liveStationTsx);
