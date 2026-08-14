import React, { useState } from 'react';
import { AlertCircle, CheckCircle2 } from 'lucide-react';

export default function PnrTab() {
  const [pnr, setPnr] = useState('');
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    setData(null);

    try {
      const res = await fetch(`/rail-api/pnr?pnr=${pnr}`);
      const json = await res.json();
      if (!res.ok) throw new Error(json.error || 'Failed to fetch PNR status.');
      setData(json);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="w-full">
      {/* Search Bar - Fixed at top on mobile, relative on desktop */}
      <div className="bg-white border-b border-slate-200 md:border md:rounded-2xl shadow-sm p-4 sticky top-0 md:relative z-40">
        <form onSubmit={(e) => handleSearch(e)} className="flex gap-2">
          <input
            type="tel"
            value={pnr}
            onChange={(e) => setPnr(e.target.value.replace(/\D/g, '').slice(0, 10))}
            placeholder="10-digit PNR"
            className="flex-1 bg-slate-100 text-slate-900 px-4 py-3 rounded-xl font-bold text-lg tracking-widest placeholder:font-normal placeholder:tracking-normal focus:outline-none focus:ring-2 focus:ring-blue-500"
            required
            pattern="\d{10}"
          />
          <button
            type="submit"
            disabled={loading || pnr.length !== 10}
            className="bg-blue-600 text-white px-6 py-3 rounded-xl font-bold disabled:opacity-50 transition-opacity whitespace-nowrap"
          >
            {loading ? '...' : 'Check'}
          </button>
        </form>
      </div>

      <div className="mt-4 md:mt-6 space-y-4">
        {error && (
          <div className="mx-4 md:mx-0 p-4 bg-red-50 border border-red-100 rounded-xl flex items-start gap-3 text-red-700">
            <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5" />
            <p className="font-medium">{error}</p>
          </div>
        )}

        {data && !error && (
          <div className="bg-white border-y md:border border-slate-200 md:rounded-2xl overflow-hidden shadow-sm">
            {/* Header / Train Info */}
            <div className="p-4 sm:p-6 bg-slate-900 text-white">
               <div className="flex justify-between items-start mb-2">
                  <div>
                    <h2 className="text-3xl font-black tracking-tight">{data.train_number}</h2>
                    <p className="text-slate-300 font-medium truncate max-w-[200px] sm:max-w-md">{data.train_name}</p>
                  </div>
                  <div className="text-right">
                    <p className="text-xs font-bold uppercase tracking-widest text-slate-400">Date</p>
                    <p className="text-lg font-bold">{data.journey_date}</p>
                  </div>
               </div>
               
               {/* Route */}
               <div className="mt-6 flex items-center justify-between bg-slate-800/50 p-4 rounded-xl">
                  <div className="text-left w-1/3">
                     <p className="text-3xl font-black">{data.from?.code}</p>
                     <p className="text-slate-400 text-xs font-medium truncate">{data.from?.name}</p>
                     <p className="text-emerald-400 font-bold mt-1">{data.from?.time}</p>
                  </div>
                  <div className="flex-1 px-2 flex items-center">
                     <div className="w-full h-0.5 bg-slate-700 relative">
                        <div className="absolute right-0 top-1/2 -translate-y-1/2 border-t-4 border-b-4 border-l-6 border-transparent border-l-slate-500 w-0 h-0"></div>
                     </div>
                  </div>
                  <div className="text-right w-1/3">
                     <p className="text-3xl font-black">{data.to?.code}</p>
                     <p className="text-slate-400 text-xs font-medium truncate">{data.to?.name}</p>
                     <p className="text-emerald-400 font-bold mt-1">{data.to?.time}</p>
                  </div>
               </div>
            </div>

            {/* Passenger List */}
            <div className="p-4 sm:p-6 bg-slate-50">
               <p className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-3">Passenger Status</p>
               <div className="flex flex-col gap-2">
                 {data.passengers?.map((p: any, i: number) => {
                   const isCnf = p.current_status === 'CNF';
                   return (
                     <div key={i} className="bg-white border border-slate-200 p-4 rounded-xl flex items-center justify-between shadow-sm">
                        <div className="flex items-center gap-4">
                           <span className="w-6 h-6 flex items-center justify-center bg-slate-100 rounded-full text-slate-500 font-bold text-xs">
                             {i + 1}
                           </span>
                           <div>
                             <p className="text-xs text-slate-400 font-medium uppercase">Booking: {p.booking_status}</p>
                             <div className="flex items-center gap-2 mt-0.5">
                               <p className={`font-black text-xl ${isCnf ? 'text-emerald-600' : 'text-amber-600'}`}>
                                 {p.current_status}
                               </p>
                               {isCnf && <CheckCircle2 className="w-5 h-5 text-emerald-500" />}
                             </div>
                           </div>
                        </div>
                        <div className="text-right">
                           <p className="text-xs text-slate-400 font-medium uppercase mb-0.5">Coach/Berth</p>
                           <p className="font-bold text-slate-900 text-xl">{p.coach || '-'}{p.berth ? ` / ${p.berth}` : ''}</p>
                        </div>
                     </div>
                   );
                 })}
               </div>
            </div>
            
            {/* Metadata Footer */}
            <div className="bg-slate-100 px-4 py-3 text-[10px] sm:text-xs font-bold text-slate-400 uppercase tracking-widest flex justify-between">
              <span>{data.freshness} Data</span>
              <span>Updated: {new Date(data.last_updated).toLocaleTimeString()}</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
