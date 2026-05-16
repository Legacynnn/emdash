// Permissive RpcRouter type. Replaces the formerly-precise type that
// was inferred from `src/main/rpc.ts` (the Electron RPC server). With
// Electron removed, the renderer no longer has a static source of
// truth for controller signatures, so this type intentionally accepts
// any namespace/method call: types validate at the call site only
// loosely, but every call still reaches a real Tauri command via the
// `window.electronAPI` shim in `src-tauri/ui/src/shim/`.
//
// Tightening: once the renderer migrates off the shim and onto the
// Specta-generated bindings (`src-tauri/ui/src/bindings.ts`), the
// `rpc.*` accessor in `@renderer/lib/ipc` goes away and this type
// can be deleted.

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyFn = (...args: any[]) => Promise<any>;

export type RpcRouter = Record<string, Record<string, AnyFn>>;
