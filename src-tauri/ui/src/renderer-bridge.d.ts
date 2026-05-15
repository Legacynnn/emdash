// The renderer entry is dynamically imported by `src/main.tsx` via the
// Vite alias `@renderer`. Declaring it as an untyped module keeps the
// Tauri UI's typecheck from following into the entire renderer source
// tree (which depends on Electron-specific aliases and the Drizzle
// schema layer that this package shouldn't see).
declare module '@renderer/main';
