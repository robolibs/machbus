# TC/DDOP problems

Common causes:

- duplicate DDOP object IDs
- invalid element references
- non-ASCII or overlong designators
- activation before upload
- malformed transfer
- server topology does not match the implement

machbus rejects malformed new transfers without corrupting an already active
accepted pool.
