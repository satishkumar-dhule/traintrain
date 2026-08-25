export const DATE_RE = /^\d{4}-\d{2}-\d{2}$/
export function todayISO() { const d=new Date(); return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}` }
export function isoShift(iso, days){ const d=new Date(`${iso}T00:00:00`); d.setDate(d.getDate()+days); return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}` }
export function diffDays(a,b){ return Math.round((new Date(`${b}T00:00:00`).getTime()-new Date(`${a}T00:00:00`).getTime())/86400000) }
export function clampDate(iso,min,max){ if(min && iso<min) return min; if(max && iso>max) return max; return iso }
export function parseIso(s){ const m=/^(\d{4})-(\d{2})-(\d{2})$/.exec(String(s??'').trim()); if(!m) return null; const d=new Date(`${m[1]}-${m[2]}-${m[3]}T00:00:00`); return Number.isNaN(d.getTime())?null:d }
