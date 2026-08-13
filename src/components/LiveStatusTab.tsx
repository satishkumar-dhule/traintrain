import AutocompleteInput from './AutocompleteInput';
import React, { useState } from 'react';
import { Search, AlertCircle, MapPin, Navigation, Calendar, Server } from 'lucide-react';

export default function LiveStatusTab() {
  const [train, setTrain] = useState('');
  
  const getDates = () => {
    const dates = [];
    for (let i = -2; i <= 1; i++) {
      const d = new Date();
      d.setDate(d.getDate() + i);
      const val = d.toISOString().split('T')[0];
      const label = i === 0 ? 'Today' : i === 1 ? 'Tomorrow' : i === -1 ? 'Yesterday' : '2 Days Ago';
      dates.push({ value: val, label: `${label} (${val})` });
    }
    return dates;
  };
  const dateOptions = getDates();
  
  const [date, setDate] = useState(dateOptions[2].value); // default today
  
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    setData(null);
    try {
      const trainNo = train.split(' - ')[0].trim();
      const res = await fetch(`/rail-api/live-status?train=${trainNo}&date=${date}`);
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
        <form onSubmit={handleSearch} className="flex flex-col md:flex-row gap-3">
          <AutocompleteInput type="train" value={train} onChange={setTrain} placeholder="Search Train Name or No." className="flex-1" />
          
          <div className="relative md:w-64 flex-shrink-0">
            <select
              value={date}
              onChange={e => setDate(e.target.value)}
              className="w-full bg-slate-100 text-slate-900 px-4 py-3 rounded-xl font-bold text-lg focus:outline-none focus:ring-2 focus:ring-blue-500 appearance-none pl-12"
            >
              {dateOptions.map(d => (
                <option key={d.value} value={d.value}>{d.label}</option>
              ))}
            </select>
            <Calendar className="w-5 h-5 absolute left-4 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
          </div>

          <button type="submit" disabled={loading || train.length === 0} className="bg-blue-600 text-white px-6 py-3 rounded-xl font-bold disabled:opacity-50 flex-shrink-0">
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
               <div className="flex justify-between items-start">
                 <div>
                   <h2 className="text-3xl font-black tracking-tight">{data.train_number}</h2>
                   <p className="text-slate-300 font-medium">{data.train_name}</p>
                 </div>
                 <div className="text-right">
                   <p className="text-sm font-bold text-slate-400">Journey Date</p>
                   <p className="text-lg font-bold text-emerald-400">{date}</p>
                 </div>
               </div>
               
               <div className="mt-4 bg-blue-600/20 border border-blue-500/30 p-4 rounded-xl flex items-start gap-3">
                  <Navigation className="w-5 h-5 text-blue-400 flex-shrink-0 mt-0.5" />
                  <p className="font-bold text-blue-100 text-sm">{data.current_location_info}</p>
               </div>
               
               <div className="mt-3 bg-slate-800 border border-slate-700 p-2.5 rounded-lg flex justify-between items-center">
                  <p className="text-xs font-bold text-slate-400">Data provided by Google Underground Engine</p>
                  <div className="flex items-center gap-1.5">
                     <Server className="w-3.5 h-3.5 text-emerald-400" />
                     <span className="text-xs font-bold text-slate-300">Origin: <span className="text-emerald-400">{data.data_source || 'Cache'}</span></span>
                  </div>
               </div>
            </div>
            <div className="p-0">
               {data.stations?.map((st: any, i: number) => {
                 const isPassed = st.status === 'Departed';
                 const isNext = !isPassed && (i === 0 || data.stations[i-1]?.status === 'Departed');
                 
                 return (
                   <div key={i} className={`flex gap-4 p-4 border-b border-slate-100 ${isNext ? 'bg-blue-50/50' : ''}`}>
                      <div className="w-16 flex flex-col items-center relative">
                         <div className={`w-0.5 h-full absolute top-0 ${isPassed ? 'bg-blue-600' : 'bg-slate-200'}`}></div>
                         <div className={`w-4 h-4 rounded-full z-10 my-2 ${isPassed ? 'bg-blue-600' : isNext ? 'bg-amber-500 ring-4 ring-amber-500/20' : 'bg-slate-200 border-2 border-white'}`}></div>
                      </div>
                      <div className="flex-1 py-1">
                         <div className="flex justify-between items-start">
                            <div>
                               <h4 className={`font-bold ${isPassed ? 'text-slate-400' : 'text-slate-900'}`}>{st.name}</h4>
                               <p className="text-xs font-bold text-slate-400">{st.code}</p>
                            </div>
                            <div className="text-right">
                               <p className={`font-bold ${isPassed ? 'text-slate-400' : 'text-slate-900'}`}>{st.actual_arrival || st.scheduled_arrival}</p>
                               <p className={`text-xs font-bold ${st.delay_minutes > 0 ? 'text-red-500' : 'text-emerald-500'}`}>
                                 {st.delay_minutes > 0 ? `${st.delay_minutes}m late` : 'On time'}
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
