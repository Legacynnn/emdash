export const MAX_TASK_NAME_LENGTH = 64;

// Allowed characters: lowercase alphanumerics plus `-`, `_`, `/`, `.`
// — the subset of git ref-name characters most users actually type.
// `/` lets people write their own prefixes (feat/x, dan/foo).
//
// We intentionally do NOT auto-derive a prefix or apply any AI heuristic;
// what the user types is what they get as both task name and branch.
export const liveTransformTaskName = (input: string): string =>
  input
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^a-z0-9\-_./]/g, '')
    .replace(/-+/g, '-')
    .replace(/\/+/g, '/')
    .slice(0, MAX_TASK_NAME_LENGTH);

export const normalizeTaskName = (input: string): string =>
  input
    .trim()
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^a-z0-9\-_./]/g, '')
    .replace(/-+/g, '-')
    .replace(/\/+/g, '/')
    // Git rejects refs that start or end with `.` or `/` (and `-` is
    // also a poor leading char). Trim those off so what the user sees
    // is also a valid ref name.
    .replace(/^[-./]+|[-./]+$/g, '')
    .slice(0, MAX_TASK_NAME_LENGTH);

export const ensureUniqueTaskName = (
  baseName: string,
  existingNames: Iterable<string>,
  maxAttempts = 6
): string => {
  const normalizedExisting = new Set(
    Array.from(existingNames, (name) => normalizeTaskName(name)).filter(Boolean)
  );
  const base = normalizeTaskName(baseName);
  if (base && !normalizedExisting.has(base)) return base;

  for (let i = 2; i < 2 + maxAttempts; i++) {
    const candidate = normalizeTaskName(`${baseName}-${i}`);
    if (candidate && !normalizedExisting.has(candidate)) {
      return candidate;
    }
  }

  const fallback = normalizeTaskName(`${baseName}-${Date.now().toString(36)}`);
  return fallback || base;
};
