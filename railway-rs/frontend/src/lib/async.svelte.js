import { api } from './api.js'
export function createResource(keyFn, fetcher, opts={}){
  const {remember}=opts
  let phase=$state('idle')
  let data=$state(null)
  let error=$state(null)
  let _key=''
  async function load({force=false}={}){
    const k=typeof keyFn==='function'?keyFn():String(keyFn??'')
    if(!k && !force){ phase='idle'; return }
    _key=k
    const hadData=data!=null
    phase=hadData?'refreshing':'loading'
    error=null
    try{
      const res=await fetcher(k)
      if(_key!==k) return
      if(res?.ok){ data=res.data; phase='ok'; if(remember) remember(k,res.data) } else { phase='error'; error=res?.error||`HTTP ${res?.status??0}` }
    }catch(e){ if(_key!==k) return; phase='error'; error=e?.message??String(e) }
  }
  return { get phase(){return phase}, get data(){return data}, get error(){return error}, get key(){return _key}, load, reload:()=>load({force:true}), refresh:()=>load({force:false}), setData(v){data=v;phase='ok'}, setError(m){error=m;phase='error'} }
}
export function createApiResource(keyFn, pathFn, opts={}){ return createResource(keyFn, (k)=> api(typeof pathFn==='function'?pathFn(k):String(pathFn)), opts) }
