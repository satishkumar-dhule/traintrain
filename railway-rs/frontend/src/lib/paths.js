export function normalizePath(p){ if(!p||p.length<=1) return p||'/'; return p.replace(/\/+$/,'') }
