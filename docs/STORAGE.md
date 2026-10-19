# GamePKG storage

GamePKG owns its storage implementation. No separately installed storage package or database server is required.

## Current records

The Rust host manages settings and catalog TOML, signed package archives, remembered ROM paths, per-creator progress and saves, and verified build caches. Existing persistence code uses bounded reads, record-specific locking where required, and staged atomic replacement. Browser metadata is rebuilt into `database/library.json` from these native records. This preserves current installations without a format migration or moving player files.

The browser view uses GamePKG-owned JavaScript over the browser's standard IndexedDB API as an optional cache. Its new cache namespace is separate from older implementations. Metadata, icons, achievements and play history can be displayed after a browser restart, but cached values cannot authorize installation, affect signatures or execute games. Authenticated host metadata still displays if browser storage fails or is disabled.

## Exported browser game checkpoints

Exported browser games use a separate GamePKG-owned IndexedDB database,
`gamepkg-progress-v1`. A checkpoint writes its opaque save bytes and play
history in one transaction, then acknowledges the frame. A failed transaction
retains both previous records. Save keys include game ID, creator and save
schema; history keys include game ID and creator. Concurrent history updates
merge inside the transaction. Opaque saves use the last committed writer.

Valid legacy localStorage values migrate at the first successful checkpoint.
Their original keys remain available as recovery copies. Existing IndexedDB
records take precedence. Invalid legacy JSON, invalid stored record schemas,
oversized data and malformed achievement IDs are preserved and rejected rather
than silently replaced. The save limit remains 1 MiB; history is bounded to
64 KiB and 1,024 portable achievement IDs. The game receives no database handle.
Restart reloads committed save bytes; Stop cancels pending persistence and
stale startup work. This does not provide a browser progress repair interface
or rewind an already running game's guest memory after a rejected checkpoint.

`tests/browser_progress_store.py` exercises actual IndexedDB migration,
transaction abort after a successful save request, quota failure, cancellation,
complete maximum-size save data, concurrent writers and corruption preservation.
The exported-player tests inspect these records through standard IndexedDB APIs.
The native app's files and the optional library metadata cache are unchanged.

## Does the native app need a database?

Not currently. Its operations are bounded record loads and updates, rather than SQL queries or cross-record database transactions. Adding a database now would add a dependency and data migration without replacing the signed-package verification that remains necessary. Existing native files remain authoritative; the JSON snapshot is rebuildable.

Two candidates were reviewed for future needs:

| Option | Appropriate future use | Tradeoff |
| --- | --- | --- |
| [rusqlite](https://github.com/rusqlite/rusqlite) | SQLite queries, indexes and transactional metadata updates | Rust bindings over SQLite; the `bundled` feature builds the C library into the host, avoiding a separately installed database server/library. It is not a pure-Rust database engine. |
| [redb](https://github.com/cberner/redb) | Typed embedded key/value tables and ACID transactions | A pure-Rust database with a different data model from SQL. |

No database crate was added. If query or transaction requirements arise, select a version only after verifying it on the project's required 32/64-bit Windows, Linux and Android targets and designing migration/recovery around the existing signed records. Games still receive no direct database or filesystem access; any future database belongs to the host implementation.

## Verification

`tests/home_database.py` verifies home-folder persistence across native host restarts. `tests/browser_storage.py` checks browser restart persistence, library isolation, committed writes and failure behavior without third-party scripts. `tests/web_library.py` checks authenticated host metadata, literal descriptions, cached icons, browser restart and blocked-cache fallback. `tests/progress_gui.py --browser-cache` covers actual keyboard gameplay flowing into native progress and the browser cache.
