// Adapter for Hindsight4 Compiler/packages/Database/IndexedDB/code_langs/js.js.
// The native host remains responsible for signatures, builds and executing games.
export class HindsightLibraryStorage {
    constructor(scope) {
        if (typeof scope !== 'string' || !/^[a-zA-Z0-9_-]{1,128}$/.test(scope)) throw new Error('Invalid library storage scope');
        this.scope = scope;
        this.connection = new indexeddb();
        this.queue = Promise.resolve();
        this.ready = false;
    }
    operation(start, ...args) {
        const job = this.queue.then(async () => {
            const accepted = {data: 0};
            start(this.connection, ...args, accepted);
            if (accepted.data !== 1) {
                const error = {data: ''}; indexeddb_last_error_bytes(this.connection, error);
                throw new Error(error.data || 'Hindsight operation was not started');
            }
            const status = {data: 1};
            while (status.data === 1) {
                indexeddb_status_int(this.connection, status);
                if (status.data === 1) await new Promise(resolve => setTimeout(resolve, 10));
            }
            if (status.data === -1) {
                const error = {data: ''}; indexeddb_last_error_bytes(this.connection, error);
                throw new Error(error.data || 'Hindsight storage failed');
            }
            if (status.data === 3) return null;
            if (status.data !== 2) throw new Error('Unexpected Hindsight completion status');
            const result = {data: ''}; indexeddb_result_bytes(this.connection, result);
            return result.data;
        });
        // Wait for the actual completion; a timeout must never retry a pending write.
        this.queue = job.catch(() => {});
        return job;
    }
    async open() {
        if (this.ready) return;
        const available = {data: 0}; indexeddb_isavailable_int(this.connection, available);
        if (available.data !== 1) throw new Error('Hindsight IndexedDB requires a browser with cross-origin isolation');
        await this.operation(indexeddb_open_constcharptr_constint_constcharptr_int,
            'gamepkg-library-v1', 1, JSON.stringify([{name: 'libraries'}]));
        this.ready = true;
    }
    async saveLibrary(snapshot) {
        if (!this.ready) throw new Error('Open library storage first');
        if (!snapshot || snapshot.format_version !== 1 || !Array.isArray(snapshot.games) || !Array.isArray(snapshot.catalog)) throw new Error('Invalid library snapshot');
        const encoded = JSON.stringify(snapshot);
        if (new TextEncoder().encode(encoded).length > 8 * 1024 * 1024) throw new Error('Library snapshot exceeds 8 MiB');
        await this.operation(indexeddb_put_constcharptr_constcharptr_constcharptr_int,
            'libraries', JSON.stringify(this.scope), encoded);
    }
    async loadLibrary() {
        if (!this.ready) throw new Error('Open library storage first');
        const encoded = await this.operation(indexeddb_get_constcharptr_constcharptr_int,
            'libraries', JSON.stringify(this.scope));
        if (encoded === null) return null;
        const snapshot = JSON.parse(encoded);
        if (snapshot.format_version !== 1 || !Array.isArray(snapshot.games) || !Array.isArray(snapshot.catalog)) throw new Error('Unsupported stored library snapshot');
        return snapshot;
    }
    iconKey(identity) {
        if(typeof identity!=='string'|| !/^[a-f0-9]{64}$/.test(identity))throw new Error('Invalid icon identity');
        return `icon:${this.scope}:${identity}`;
    }
    async saveIcon(icon) {
        if(!this.ready)throw new Error('Open library storage first');
        if(icon.width!==40||icon.height!==40||typeof icon.rgba_hex!=='string'|| !/^[a-f0-9]{12800}$/.test(icon.rgba_hex))throw new Error('Invalid bounded icon pixels');
        await this.operation(indexeddb_put_constcharptr_constcharptr_constcharptr_int,'libraries',JSON.stringify(this.iconKey(icon.identity)),JSON.stringify(icon));
    }
    async loadIcon(identity) {
        if(!this.ready)throw new Error('Open library storage first');
        const encoded=await this.operation(indexeddb_get_constcharptr_constcharptr_int,'libraries',JSON.stringify(this.iconKey(identity)));
        return encoded===null?null:JSON.parse(encoded);
    }
    async close() {
        if (!this.ready) return;
        await this.operation(indexeddb_close_int);
        this.ready = false;
    }
}
