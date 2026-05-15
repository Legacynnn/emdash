/**
 * UUID v4 generator. Prefers the Web Crypto API (available in modern Tauri
 * webviews); falls back to `Math.random()` only for environments without
 * crypto, which is mainly Vitest's default jsdom config — production paths
 * always hit `crypto.randomUUID`.
 */
export function uuidV4(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  // RFC 4122 v4 fallback. Sufficient for sub_id; not cryptographically strong.
  const bytes = new Uint8Array(16);
  for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
  // `noUncheckedIndexedAccess` makes Uint8Array reads `number | undefined`
  // even though that's untrue at runtime — assert non-null to keep the math
  // strict-friendly.
  bytes[6] = (bytes[6]! & 0x0f) | 0x40;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0'));
  return `${hex.slice(0, 4).join('')}-${hex.slice(4, 6).join('')}-${hex.slice(6, 8).join('')}-${hex.slice(8, 10).join('')}-${hex.slice(10, 16).join('')}`;
}
