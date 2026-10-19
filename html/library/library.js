import {GamePKGLibraryStorage} from './storage.js';
const status=document.getElementById('status');
const [token,scope]=location.hash.slice(1).split(':');
let store;
let generation=0;
function render(snapshot,cached) {
    const current=++generation;
    const body=document.getElementById('games');body.replaceChildren();
    const row=values=>{const tr=document.createElement('tr');for(const value of values){const td=document.createElement('td');td.textContent=String(value);tr.append(td);}body.append(tr);return tr;};
    const installed=new Set();
    for(const game of snapshot.games){
        const m=game.manifest;installed.add(m.id);
        const definitions=m.achievements||[];
        const count=game.history?`${definitions.filter(a=>Object.hasOwn(game.history.unlocked,a.id)).length}/${definitions.length} unlocked`:'History unavailable';
        const used=game.used_bytes===null?'Unavailable':`${game.used_bytes} bytes used`;
        const expected=game.expected_bytes==null?'Expected size unknown':`${game.expected_bytes} bytes expected`;
        const state=cached?'Cached installed metadata':game.built===true?'Built':game.built===false?'Not built':'Build status unavailable';
        const tr=row(['🎮',m.title||m.id,m.version,state,game.history?`${Math.floor(game.history.played_ms/1000)} seconds`:'Unavailable',count,`${used}; ${expected}`]);
        for(const text of [m.title?m.id:null,m.description,m.author?`By ${m.author}`:null,m.genres?.join(' · '),m.homepage]){if(text){const detail=document.createElement('p');detail.textContent=text;tr.children[1].append(detail);}}
        tr.lastElementChild.title=game.storage_breakdown||'Storage unavailable';
        if(definitions.length){
            const detail=document.createElement('details'),summary=document.createElement('summary');summary.textContent=count;detail.append(summary);
            for(const achievement of definitions){
                const item=document.createElement('p');
                const state=game.history?(Object.hasOwn(game.history.unlocked,achievement.id)?'Unlocked':'Locked'):'Unavailable';
                item.textContent=`${state} — ${achievement.title}: ${achievement.description}`;detail.append(item);
            }
            tr.children[5].replaceChildren(detail);
        }
        const requirements=document.createElement('details'),label=document.createElement('summary');label.textContent='Game requirements';requirements.append(label);
        const memory=document.createElement('p');memory.textContent=`Memory: ${(m.limits.recommended_memory_bytes||m.limits.hard_memory_bytes)/1048576} MiB recommended; ${m.limits.hard_memory_bytes/1048576} MiB hard limit`;requirements.append(memory);
        const rom=document.createElement('p');rom.textContent=m.roms?.length?`Your ROM files, in order: ${m.roms.map(r=>r.title).join(', ')}`:m.requires_rom?'One ROM supplied by you':'No user ROM required';requirements.append(rom);tr.children[1].append(requirements);
        if(game.icon_identity)loadIcon(game,tr.firstElementChild,cached,current);
    }
    document.getElementById('library-space').textContent=snapshot.library_used_bytes==null?'Library storage unavailable':`Library storage used: ${snapshot.library_used_bytes} bytes`;

    for(const game of snapshot.catalog){if(!installed.has(game.id)){const tr=row(['🎮',game.title,game.version||'Unknown',cached?'Cached catalog metadata':'Not downloaded','Not played','—','Unknown']);if(game.description){const detail=document.createElement('p');detail.textContent=game.description;tr.children[1].append(detail);}}}
    for(const [name,error] of snapshot.rejected)row(['🎮',name,'Unknown',`Unavailable: ${error}`,'Unavailable','Unavailable','Unavailable']);
    const settings=document.getElementById('settings');settings.replaceChildren();
    for(const [label,value] of [['Dark appearance',snapshot.settings.dark_mode?'On':'Off'],['Shader backend',snapshot.settings.shader_backend],['Memory ceiling',`${snapshot.settings.memory_ceiling_mib} MiB`],['Audio',snapshot.settings.mute_audio?'Muted':`${snapshot.settings.audio_volume}% volume`],['Automatic update checks',snapshot.settings.automatic_update_checks?`Every ${snapshot.settings.update_interval_hours} hours`:'Off'],['Optimize builds',snapshot.settings.optimize_builds?'On':'Off']]){const name=document.createElement('dt'),detail=document.createElement('dd');name.textContent=label;detail.textContent=value;settings.append(name,detail);}
}
async function loadIcon(game,cell,cached,current) {
    try {
        let icon=null;try{if(store)icon=await store.loadIcon(game.icon_identity);}catch(_){}
        if(!icon&&!cached){const response=await fetch(`/api/icon/${encodeURIComponent(game.manifest.id)}`,{headers:{Authorization:`Bearer ${token}`}});if(!response.ok)return;icon=await response.json();if(icon.identity!==game.icon_identity)return;try{if(store)await store.saveIcon(icon);}catch(_){}}
        if(!icon||current!==generation||icon.width!==40||icon.height!==40||typeof icon.rgba_hex!=='string'|| !/^[a-f0-9]{12800}$/.test(icon.rgba_hex))return;
        const pixels=new Uint8ClampedArray(icon.rgba_hex.match(/../g).map(value=>parseInt(value,16)));
        const canvas=document.createElement('canvas');canvas.width=40;canvas.height=40;canvas.setAttribute('aria-label',`${game.manifest.id} icon`);canvas.getContext('2d').putImageData(new ImageData(pixels,40,40),0,0);cell.replaceChildren(canvas);
    }catch(_){/* Keep a generic icon when storage or the verified thumbnail is unavailable. */}
}
let refreshing=false;
async function refresh(){
    if(refreshing)return; refreshing=true;document.getElementById('refresh').disabled=true;
    try {
        const response=await fetch('/api/library',{headers:{Authorization:`Bearer ${token}`}});
        if(!response.ok)throw new Error(await response.text());
        const snapshot=await response.json();
        if(snapshot.scope!==scope)throw new Error('Library scope differs from this session link');
        render(snapshot,false);
        try{
            if(!store)store=new GamePKGLibraryStorage(snapshot.scope);
            await store.open();await store.saveLibrary(snapshot);
            status.textContent='Library information saved in the GamePKG browser cache.';
        }catch(error){status.textContent=`Library loaded from host. Browser cache unavailable: ${error.message}`;}
    }catch(error){
        let cached=null;if(store)try{cached=await store.loadLibrary();}catch(_){}
        if(cached)render(cached,true);
        status.textContent=`Unable to refresh: ${error.message}${cached?' — showing cached metadata.':''}`;
    }finally{refreshing=false;document.getElementById('refresh').disabled=false;}
}
try{store=new GamePKGLibraryStorage(scope);await store.open();const cached=await store.loadLibrary();if(cached)render(cached,true);}catch(error){status.textContent=error.message;}
document.getElementById('refresh').addEventListener('click',refresh);
await refresh();
