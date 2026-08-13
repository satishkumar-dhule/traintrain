import React, { useState, useEffect, useRef } from 'react';

interface Props {
  type: 'train' | 'station';
  value: string;
  onChange: (val: string) => void;
  placeholder: string;
  icon?: React.ReactNode;
  className?: string;
}

export default function AutocompleteInput({ type, value, onChange, placeholder, icon, className }: Props) {
  const [query, setQuery] = useState(value);
  const [results, setResults] = useState<any[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setQuery(value);
  }, [value]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    if (!query || query === value && !isOpen) {
      setResults([]);
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const endpoint = type === 'train' ? '/rail-api/search/trains' : '/rail-api/search/stations';
        const searchTerm = query.split(' - ')[0].trim();
        const res = await fetch(`${endpoint}?q=${encodeURIComponent(searchTerm)}`);
        const json = await res.json();
        setResults(json);
        if (json.length > 0) setIsOpen(true);
      } catch (err) {
        setResults([]);
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [query, type, value, isOpen]);

  const handleSelect = (item: any) => {
    const val = type === 'train' ? `${item.number} - ${item.name}` : `${item.code} - ${item.name}`;
    setQuery(val);
    onChange(val);
    setIsOpen(false);
  };

  return (
    <div className={`relative flex-1 group ${className || ''}`} ref={wrapperRef}>
      {icon && (
        <div className="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none z-10">
          {icon}
        </div>
      )}
      <input
        type="text"
        value={query}
        onChange={(e) => {
          const val = e.target.value.toUpperCase();
          setQuery(val);
          onChange(val);
          setIsOpen(true);
        }}
        onFocus={() => { if (results.length > 0) setIsOpen(true); }}
        placeholder={placeholder}
        className={`w-full bg-slate-100 text-slate-900 px-4 py-3 rounded-xl font-bold text-lg focus:outline-none focus:ring-2 focus:ring-blue-500 ${icon ? 'pl-12' : ''}`}
        required
      />
      
      {isOpen && results.length > 0 && (
        <div className="absolute top-full left-0 right-0 mt-2 bg-white rounded-xl shadow-xl border border-slate-200 overflow-hidden z-50 max-h-60 overflow-y-auto">
          {results.map((item, idx) => (
            <div
              key={idx}
              onClick={() => handleSelect(item)}
              className="p-3 border-b border-slate-100 hover:bg-slate-50 cursor-pointer flex justify-between items-center"
            >
              <div className="flex items-center gap-3">
                <span className="font-bold text-slate-900 bg-slate-100 px-2 py-1 rounded text-sm whitespace-nowrap">
                  {type === 'train' ? item.number : item.code}
                </span>
                <span className="text-slate-600 font-medium text-sm truncate max-w-[200px] md:max-w-xs">
                  {item.name}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
