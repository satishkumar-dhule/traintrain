import AutocompleteInput from './AutocompleteInput';
import React, { useState } from 'react';
import { ArrowRight, Search, Server, AlertCircle, MapPin } from 'lucide-react';

export default function TrainsBetweenTab() {
  const [src, setSrc] = useState('');
  const [dst, setDst] = useState('');
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const getCode = (val: string) => val.split(' - ')[0].trim().toUpperCase();

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setData(null);
    const fromCode = getCode(src);
    const toCode = getCode(dst);

    if (!/^[A-Z]{2,4}$/.test(fromCode)) {
      setError('Please enter a valid source station code (2-4 letters).');
      return;
    }
    if (!/^[A-Z]{2,4}$/.test(toCode)) {
      setError('Please enter a valid destination station code (2-4 letters).');
      return;
    }
    if (fromCode === toCode) {
      setError('Source and destination stations must be different.');
      return;
    }

    setLoading(true);
    try {
      const res = await fetch(`/rail-api/ntes/trains-between?src=${fromCode}&dst=${toCode}`);
      const json = await res.json();
      if (!res.ok) throw new Error(json.error || 'Failed to fetch trains.');
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
        <form onSubmit={handleSearch} className="flex flex-col md:flex-row gap-2">
          <AutocompleteInput type="station" value={src} onChange={setSrc} placeholder="FROM (e.g. NDLS)" className="md:w-2/5" />
          <div className="hidden md:flex items-center justify-center text-slate-400"><ArrowRight className="w-4 h-4" /></div>
          <AutocompleteInput type="station" value={dst} onChange={setDst} placeholder="TO (e.g. MMCT)" className="md:w-2/5" />
          <button type="submit" disabled={loading} className="md:flex-1 bg-blue-600 text-white rounded-xl font-bold py-3 disabled:opacity-50">{loading ? '...' : 'Go'}</button>
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
          <div className="bg-white border-y md:border md:rounded-2xl overflow-hidden shadow-sm">
             <div className="bg-slate-50 p-4 border-b border-slate-200 flex justify-between items-center">
               <h3 className="font-bold text-slate-700 uppercase tracking-widest text-xs">
                 {data.train_count || data.trains?.length || 0} Trains · {data.src} → {data.dst}
               </h3>
               <div className="flex items-center gap-1.5">
                  <Server className="w-3.5 h-3.5 text-blue-600" />
                  <span className="text-xs font-bold text-slate-500">Origin: <span className="text-blue-600">{data.data_source || 'Cache'}</span></span>
               </div>
             </div>
             {data.trains?.map((t:any, i:number) => (
                <div key={i} className="p-4 border-b border-slate-100">
                   <div className="flex justify-between items-start mb-2">
                      <div className="font-bold text-lg text-slate-900"><span className="text-slate-400 text-sm mr-2">{t.number}</span>{t.name}</div>
                      <div className="hidden sm:block text-right text-xs font-bold text-slate-500">
                        <div className="flex items-center gap-1"><MapPin className="w-3 h-3 text-slate-400" />{t.from_station} → {t.to_station}</div>
                        <div className="mt-0.5">{t.duration || ''}{t.distance ? ` · ${t.distance} km` : ''}</div>
                      </div>
                   </div>
                   <div className="flex items-center justify-between mt-4 bg-slate-50 p-3 rounded-xl">
                      <div className="text-center w-1/3"><div className="text-xs font-bold text-slate-400">DEP</div><div className="font-black text-xl text-slate-900">{t.departure_time}</div></div>
                      <div className="w-px h-8 bg-slate-200"></div>
                      <div className="text-center w-1/3"><div className="text-xs font-bold text-slate-400">ARR</div><div className="font-black text-xl text-slate-900">{t.arrival_time}</div></div>
                   </div>
                   <div className="flex gap-1 mt-3 justify-center">
                     {['M','T','W','T','F','S','S'].map((d,idx) => (
                        <div key={idx} className={`w-6 h-6 flex items-center justify-center rounded text-[10px] font-bold ${t.runs_on[idx] ? 'bg-blue-600 text-white' : 'bg-slate-100 text-slate-300'}`}>{d}</div>
                     ))}
                   </div>
                </div>
             ))}
          </div>
        )}
      </div>
    </div>
  );
}
