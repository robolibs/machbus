# VT upload problems

Common causes:

- object pool too large
- duplicate object ID
- missing child reference
- malformed child-list tail
- unknown object type
- EndOfObjectPool validation error
- TP transfer not finished before finalization

Remember: successful protocol upload does not mean a GUI window was painted.
`VTServer` records activated pool state and accepted render effects; hosted Rust
code can replay that through `VtRenderRuntime`, GTUI commands, or the software
framebuffer. If a screen stays blank, check both the upload/server state and the
separate hosted render/runtime path.
