// Browser worker-pool bootstrap was removed from the web frontend.
// Keep a stubbed module here so old snippet paths fail explicitly if referenced.

export async function startWorkers() {
  throw new Error('web worker-pool bootstrap has been removed; the web frontend runs in single-thread mode');
}