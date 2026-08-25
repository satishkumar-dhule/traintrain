export function parseDelay(v){ if(v==null) return null; if(typeof v==='number') return Number.isFinite(v)?Math.round(v):null; const s=String(v).trim(); if(!s||s==='-'||s==='--') return null; if(/on time/i.test(s)) return 0; let m=/^(\d{1,3}):(\d{2})$/.exec(s); if(m) return Number(m[1])*60+Number(m[2]); m=/^[+-]?\d+$/.exec(s); if(m) return Number(s); m=/^[-+]?\d+(\.\d+)?\s*(m|min|mins|minutes?)?$/i.exec(s); if(m){ const n=Math.round(parseFloat(s)); return Number.isFinite(n)?n:null } m=/(\d+)\s*min/i.exec(s); if(m) return Number(m[1]); return null }
export function delayKind(minutes){ if(minutes==null) return 'none'; if(minutes<=0) return 'ontime'; return minutes<=15?'minor':'major' }
export const parseDelayMinutes=parseDelay
export const parseAvgDelayMinutes=parseDelay
