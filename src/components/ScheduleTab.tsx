import AutocompleteInput from './AutocompleteInput';
import React, { useState } from 'react';
import { ScheduleResponse } from '../types';
import { Search, Train, ArrowRight, AlertCircle, Calendar , Server} from 'lucide-react';

export default function ScheduleTab() {
  const [train, setTrain] = useState('');
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<ScheduleResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    setError(null);
    setData(null);
    setLoading(true);
    try {
      const trainNo = train.split(' - ')[0].trim();
      const res = await fetch(`/rail-api/schedule?train=${trainNo}`);
      const json = await res.json();
      if (!res.ok) {
        throw new Error(json.error || 'Failed to fetch schedule.');
      }
      setData(json);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <form onSubmit={handleSearch} className="flex gap-4">
        <AutocompleteInput type="train" value={train} onChange={setTrain} placeholder="Search Train Name or No." className="w-full" />
        <button
          type="submit"
          disabled={loading || train.length === 0}
          className="px-6 py-3 bg-blue-600 text-white font-medium rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {loading ? 'Searching...' : 'View Schedule'}
        </button>
      </form>
      
      {error && (
        <div className="p-4 bg-red-50 text-red-700 rounded-lg flex items-start gap-3">
          <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5" />
          <p>{error}</p>
        </div>
      )}
      
      {data && !error && (
        <div className="bg-white border border-slate-200 rounded-xl overflow-hidden shadow-sm">
          {/* Header */}
          <div className="bg-slate-50 p-6 border-b border-slate-200">
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
              <div>
                <h3 className="text-xl font-bold text-slate-900 flex items-center gap-2">
                  <Train className="w-6 h-6 text-blue-600" />
                  {data.train_name} ({data.train_number})
                </h3>
                <p className="text-slate-600 mt-1 flex items-center gap-1.5">
                  {data.route_description?.split(' to ')[0]}
                  <ArrowRight className="w-4 h-4 text-slate-400" />
                  {data.route_description?.split(' to ')[1]}
                </p>
              </div>
              <div className="flex flex-col items-end gap-2">
                <div className="flex gap-1">
                  {['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map(day => {
                    const isRunning = data.running_days?.includes(day);
                    return (
                      <span
                        key={day}
                        className={`text-xs px-2 py-1 rounded font-medium ${
                          isRunning ? 'bg-green-100 text-green-800' : 'bg-slate-100 text-slate-400'
                        }`}
                      >
                        {day[0]}
                      </span>
                    );
                  })}
                </div>
                                <p className="text-xs text-amber-600 bg-amber-50 px-2 py-1 rounded font-medium">
                  {data.notice}
                </p>
                <div className="flex items-center gap-1.5 mt-2 bg-slate-200 px-2.5 py-1 rounded-md">
                   <Server className="w-3.5 h-3.5 text-slate-600" />
                   <span className="text-xs font-bold text-slate-600">Source: <span className="text-blue-600">{data.data_source || 'Cache'}</span></span>
                </div>
              </div>
            </div>
          </div>
          {/* Stops List */}
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="bg-white border-b border-slate-200 text-xs uppercase tracking-wider text-slate-500">
                  <th className="py-4 px-6 font-semibold">Station</th>
                  <th className="py-4 px-6 font-semibold">Arrive</th>
                  <th className="py-4 px-6 font-semibold">Depart</th>
                  <th className="py-4 px-6 font-semibold">Day</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {data.stops?.map((stop, i) => (
                  <tr key={i} className="hover:bg-slate-50 transition-colors">
                    <td className="py-4 px-6">
                      <p className="font-medium text-slate-900">{stop.name}</p>
                      <p className="text-sm text-slate-500">{stop.code}</p>
                    </td>
                    <td className="py-4 px-6">
                      <span className={`font-medium ${stop.arrival === '--:--' ? 'text-slate-400' : 'text-slate-900'}`}>
                        {stop.arrival}
                      </span>
                    </td>
                    <td className="py-4 px-6">
                      <span className={`font-medium ${stop.departure === '--:--' ? 'text-slate-400' : 'text-slate-900'}`}>
                        {stop.departure}
                      </span>
                    </td>
                    <td className="py-4 px-6">
                      <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-sm font-medium bg-slate-100 text-slate-700">
                        <Calendar className="w-4 h-4" />
                        Day {stop.day}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
