import React, { useState, useEffect } from 'react';
import { StationResponse } from '../types';
import { Search, MapPin, Building2, AlertCircle } from 'lucide-react';

export default function StationsTab() {
  const [query, setQuery] = useState('');
  const [loading, setLoading] = useState(false);
  const [stations, setStations] = useState<StationResponse[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Debounce search
  useEffect(() => {
    const fetchStations = async () => {
      setLoading(true);
      setError(null);
      try {
        const res = await fetch(`/rail-api/stations?q=${encodeURIComponent(query)}`);
        const json = await res.json();
        if (!res.ok) throw new Error(json.error || 'Failed to fetch stations.');
        setStations(json);
      } catch (err: any) {
        setError(err.message);
      } finally {
        setLoading(false);
      }
    };

    const timer = setTimeout(() => {
      fetchStations();
    }, 300);

    return () => clearTimeout(timer);
  }, [query]);

  return (
    <div className="space-y-6">
      <div className="relative">
        <Search className="w-5 h-5 absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search by station name, code, or city..."
          className="w-full pl-12 pr-4 py-3 bg-white border border-slate-200 rounded-lg text-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-shadow text-lg"
        />
        {loading && (
          <div className="absolute right-4 top-1/2 -translate-y-1/2">
            <div className="w-5 h-5 border-2 border-slate-300 border-t-blue-600 rounded-full animate-spin"></div>
          </div>
        )}
      </div>

      {error && (
        <div className="p-4 bg-red-50 text-red-700 rounded-lg flex items-start gap-3">
          <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5" />
          <p>{error}</p>
        </div>
      )}

      {!error && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {stations.length > 0 ? (
            stations.map((station) => (
              <div
                key={station.code}
                className="bg-white p-5 rounded-xl border border-slate-200 shadow-sm hover:shadow-md transition-shadow flex items-start gap-4"
              >
                <div className="bg-blue-50 w-12 h-12 rounded-lg flex items-center justify-center flex-shrink-0 text-blue-600 font-bold text-lg">
                  {station.code}
                </div>
                <div>
                  <h4 className="text-lg font-bold text-slate-900">{station.name}</h4>
                  <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-4 mt-1 text-sm text-slate-500">
                    <span className="flex items-center gap-1.5">
                      <MapPin className="w-4 h-4" />
                      {station.city}
                    </span>
                    <span className="hidden sm:inline text-slate-300">•</span>
                    <span className="flex items-center gap-1.5">
                      <Building2 className="w-4 h-4" />
                      {station.zone} Zone
                    </span>
                  </div>
                </div>
              </div>
            ))
          ) : (
            !loading && (
              <div className="col-span-full py-12 text-center text-slate-500 bg-white rounded-xl border border-slate-200 border-dashed">
                <p>No stations found matching "{query}"</p>
                <p className="text-sm mt-1">Try searching for major cities like "Delhi" or "Mumbai"</p>
              </div>
            )
          )}
        </div>
      )}
    </div>
  );
}
