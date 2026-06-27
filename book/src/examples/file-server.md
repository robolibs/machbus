# File Server example

Use the File Server example for connect/properties/status/read/write directory
workflows.

Look for:

- `examples/file_server_demo.rs`
- the `FsServer` plugin on the session facade (see
  [The session facade](../guide/session-facade.md))

The file server rejects traversal and invalid volume/path values before they
mutate state.
