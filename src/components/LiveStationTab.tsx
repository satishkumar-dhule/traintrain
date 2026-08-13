import AutocompleteInput from './AutocompleteInput';
import React, { useState } from 'react';
import { Clock, AlertCircle , Server} from 'lucide-react';

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
      const res = await fetch(`/rail-api/ntes/live-station?station=${station}&hours=${hours}`);
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
          <AutocompleteInput type="station" value={station} onChange={setStation} placeholder="Station Code" className="w-1/2" />
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
                              <div>
                  <h2 className="text-2xl sm:text-3xl font-black uppercase tracking-widest">{station} DEPARTURES</h2>
                  <div className="flex items-center gap-1.5 mt-1">
                     <Server className="w-3 h-3 text-zinc-500" />
                     <span className="text-[10px] font-bold text-zinc-500 tracking-widest uppercase">Origin: <span className="text-blue-400">{data.data_source || 'Cache'}</span></span>
                  </div>
               </div>
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
                    <div className={`w-16 text-right font-bold ${t.delay_arr ? 'text-red-500' : 'text-green-500'}`}>
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
