import { DATE_RE } from './dates.js'
import { norm, asText } from './format.js'
export const isTrainNumber = (v)=> /^\d{1,8}$/.test(String(v??'').trim())
export const isStationCode = (v)=> /^[A-Z0-9]{2,4}$/.test(norm(v))
export const isPnr = (v)=> /^\d{10}$/.test(asText(v))
export const isIsoDate = (v)=> DATE_RE.test(String(v??'').trim())
export const isDifferentStation = (a,b)=> norm(a)!==norm(b)
export const isTrain5 = (v)=>{ const s=String(v??'').trim(); return s.length===5 && /^\d{5}$/.test(s) && s!=='00000' }
