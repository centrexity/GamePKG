import {HindsightLibraryStorage} from './storage.js';
const status=document.getElementById('status');
const [token,scope]=location.hash.slice(1).split(':');
let store;
function render(snapshot,cached) {
    const body=document.getElementById('games');body.replaceChildren();
    const row=values=>{const tr=document.createElement('tr');for(const value of values){const td=document.createElement('td');td.textContent=String(value);tr.append(td);}body.append(tr);};
    const installed=new Set();
    for(const game of snapshot.games){const m=game.manifest;installed.add(m.id);row([m.id,m.version,cached?'Cached installed metadata':'Installed',game.history?`${Math.floor(game.history.played_ms/1000)} seconds`:'Unavailable',game.history?`${(m.achievements||[]).filter(a=>Object.hasOwn(game.history.unlocked,a.id)).length}/${(m.achievements||[]).length}`:'Unavailable',game.used_bytes===null?'Unavailable':`${game.used_bytes} bytes`]);}
    for(const game of snapshot.catalog){if(!installed.has(game.id))row([game.title,game.version||'Unknown',cached?'Cached catalog metadata':'Not downloaded','Not played','—','Unknown']);}
    for(const [name,error] of snapshot.rejected)row([name,'Unknown',`Unavailable: ${error}`,'Unavailable','Unavailable','Unavailable']);
    const settings=document.getElementById('settings');settings.replaceChildren();
    for(const [label,value] of [['Dark appearance',snapshot.settings.dark_mode?'On':'Off'],['Shader backend',snapshot.settings.shader_backend],['Memory ceiling',`${snapshot.settings.memory_ceiling_mib} MiB`],['Audio',snapshot.settings.mute_audio?'Muted':`${snapshot.settings.audio_volume}% volume`],['Automatic update checks',snapshot.settings.automatic_update_checks?`Every ${snapshot.settings.update_interval_hours} hours`:'Off'],['Optimize builds',snapshot.settings.optimize_builds?'On':'Off']]){const name=document.createElement('dt'),detail=document.createElement('dd');name.textContent=label;detail.textContent=value;settings.append(name,detail);}
}
let refreshing=false;
async function refresh(){
    if(refreshing)return; refreshing=true;document.getElementById('refresh').disabled=true;
    try {
        const response=await fetch('/api/library',{headers:{Authorization:`Bearer ${token}`}});
        if(!response.ok)throw new Error(await response.text());
        const snapshot=await response.json();
        if(snapshot.scope!==scope)throw new Error('Library scope differs from this session link');
        if(!store){store=new HindsightLibraryStorage(snapshot.scope);await store.open();}
        await store.saveLibrary(snapshot);render(await store.loadLibrary(),false);
        status.textContent='Library information saved using Hindsight IndexedDB.';
    }catch(error){
        let cached=null;if(store)try{cached=await store.loadLibrary();}catch(_){}
        if(cached)render(cached,true);
        status.textContent=`Unable to refresh: ${error.message}${cached?' — showing cached metadata.':''}`;
    }finally{refreshing=false;document.getElementById('refresh').disabled=false;}
}
try{store=new HindsightLibraryStorage(scope);await store.open();const cached=await store.loadLibrary();if(cached)render(cached,true);}catch(error){status.textContent=error.message;}
document.getElementById('refresh').addEventListener('click',refresh);
await refresh();
