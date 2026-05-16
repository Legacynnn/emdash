/**
 * Unit tests for the dependencies.getAll transform in the renderer
 * shim. The Rust DependencyEntry shape (`{ id, installed, resolvedPath }`)
 * is narrower than the renderer's DependencyState; this test pins the
 * reshape so a regression (e.g., dropping `category` or `status`) is
 * caught before reaching the renderer.
 */
import { describe, expect, it } from 'vitest';
import { mapDependencyEntries } from '../route-table';

describe('mapDependencyEntries', () => {
  it('maps installed agent CLI to DependencyState with status=available', () => {
    const result = mapDependencyEntries([
      { id: 'claude', installed: true, resolvedPath: '/usr/local/bin/claude' },
    ]);
    expect(result.claude).toMatchObject({
      id: 'claude',
      category: 'agent',
      status: 'available',
      version: null,
      path: '/usr/local/bin/claude',
    });
    expect((result.claude as { checkedAt: number }).checkedAt).toBeTypeOf('number');
  });

  it('maps missing CLI to status=missing with null path', () => {
    const result = mapDependencyEntries([{ id: 'gemini', installed: false, resolvedPath: null }]);
    expect(result.gemini).toMatchObject({
      id: 'gemini',
      category: 'agent',
      status: 'missing',
      version: null,
      path: null,
    });
  });

  it('derives category=core for known core dependency ids', () => {
    const result = mapDependencyEntries([
      { id: 'git', installed: true, resolvedPath: '/usr/bin/git' },
      { id: 'node', installed: true, resolvedPath: '/usr/local/bin/node' },
    ]);
    expect((result.git as { category: string }).category).toBe('core');
    expect((result.node as { category: string }).category).toBe('core');
  });

  it('returns empty record for non-array input', () => {
    expect(mapDependencyEntries(undefined)).toEqual({});
    expect(mapDependencyEntries(null)).toEqual({});
  });

  it('skips entries with no id', () => {
    const result = mapDependencyEntries([
      { installed: true, resolvedPath: '/bin/foo' },
      { id: 'claude', installed: true, resolvedPath: null },
    ]);
    expect(Object.keys(result)).toEqual(['claude']);
  });
});
