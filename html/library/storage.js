// GamePKG-owned optional browser cache. Native Rust records remain authoritative.
export class GamePKGLibraryStorage {
    constructor(scope) {
        if(typeof scope!=='string'|| !/^[a-zA-Z0-9_-]{1,128}$/.test(scope))throw new Error('Invalid library storage scope');
        this.scope=scope;this.connection=null;this.opening=null;this.queue=Promise.resolve();
    }
    async open() {
        if(this.connection)return;
        if(this.opening)return this.opening;
        if(!globalThis.indexedDB)throw new Error('Browser storage is unavailable');
        this.opening=new Promise((resolve,reject)=>{
            const request=indexedDB.open('gamepkg-browser-cache-v1',1);
            let failed=false;
            request.onupgradeneeded=()=>{if(!request.result.objectStoreNames.contains('libraries'))request.result.createObjectStore('libraries');};
            request.onerror=()=>{failed=true;reject(request.error||new Error('Cannot open browser cache'));};
            request.onblocked=()=>{failed=true;reject(new Error('Close other library tabs before opening browser cache'));};
            request.onsuccess=()=>{
                if(failed){request.result.close();return;}
                this.connection=request.result;
                this.connection.onversionchange=()=>{this.connection?.close();this.connection=null;};
                resolve();
            };
        });
        try{await this.opening;}finally{this.opening=null;}
    }
    operation(mode,action) {
        const result=this.queue.then(()=>new Promise((resolve,reject)=>{
            if(!this.connection){reject(new Error('Open library storage first'));return;}
            let transaction,request;
            try{transaction=this.connection.transaction('libraries',mode);request=action(transaction.objectStore('libraries'));}
            catch(error){if(transaction)transaction.abort();reject(error);return;}
            // Resolve writes on commit, not when their individual requests succeed.
            transaction.oncomplete=()=>resolve(request.result);
            transaction.onabort=()=>reject(transaction.error||request.error||new Error('Browser cache transaction aborted'));
            transaction.onerror=()=>{}; // The abort event reports the final outcome.
        }));
        this.queue=result.catch(()=>{});
        return result;
    }
    validate(snapshot) {
        if(!snapshot||snapshot.format_version!==1||!Array.isArray(snapshot.games)||!Array.isArray(snapshot.catalog))throw new Error('Invalid library snapshot');
        if(snapshot.scope!==undefined&&snapshot.scope!==this.scope)throw new Error('Library scope differs from stored metadata');
    }
    async saveLibrary(snapshot) {
        this.validate(snapshot);
        const encoded=JSON.stringify(snapshot);
        if(new TextEncoder().encode(encoded).length>8*1024*1024)throw new Error('Library snapshot exceeds 8 MiB');
        await this.operation('readwrite',store=>store.put(encoded,this.scope));
    }
    async loadLibrary() {
        const encoded=await this.operation('readonly',store=>store.get(this.scope));
        if(encoded===undefined)return null;
        if(typeof encoded!=='string'||new TextEncoder().encode(encoded).length>8*1024*1024)throw new Error('Invalid stored library snapshot');
        const snapshot=JSON.parse(encoded);this.validate(snapshot);return snapshot;
    }
    iconKey(identity) {
        if(typeof identity!=='string'|| !/^[a-f0-9]{64}$/.test(identity))throw new Error('Invalid icon identity');
        return `icon:${this.scope}:${identity}`;
    }
    validateIcon(icon) {
        if(!icon||icon.width!==40||icon.height!==40||typeof icon.rgba_hex!=='string'|| !/^[a-f0-9]{12800}$/.test(icon.rgba_hex))throw new Error('Invalid bounded icon pixels');
        return this.iconKey(icon.identity);
    }
    async saveIcon(icon) {const key=this.validateIcon(icon);await this.operation('readwrite',store=>store.put(JSON.stringify(icon),key));}
    async loadIcon(identity) {
        const encoded=await this.operation('readonly',store=>store.get(this.iconKey(identity)));
        if(encoded===undefined)return null;
        if(typeof encoded!=='string'||encoded.length>16384)throw new Error('Invalid stored icon');
        const icon=JSON.parse(encoded);this.validateIcon(icon);if(icon.identity!==identity)throw new Error('Stored icon identity differs');return icon;
    }
    async close() {if(this.opening)await this.opening.catch(()=>{});await this.queue;if(this.connection)this.connection.close();this.connection=null;}
}
