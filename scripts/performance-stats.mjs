import assert from 'node:assert/strict';

// The artifact PERFORMANCE.md is rendered from: the default for measuring,
// profiling and rendering. The runner refuses to overwrite an existing one.
export const DEFAULT_REPORT = 'reports/performance-2026-10-03.json';

export function median(values) {
 assert.ok(values.length > 0 && values.every(Number.isFinite));
 const sorted = [...values].sort((a,b)=>a-b), middle = Math.floor(sorted.length / 2);
 return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

export function cpuList(value) {
 assert.match(value, /^\d+(,\d+)*$/, 'CPU affinity must be a comma-separated list, e.g. 2,3');
 const ids = value.split(',').map(Number);
 assert.ok(ids.every(Number.isSafeInteger));
 return [...new Set(ids)].sort((a,b)=>a-b).join(',');
}
