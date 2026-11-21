// watane.js: the page side of a watane mod. It reaches only its own mod.
(() => {
  const handlers = new Set();
  let source = null;
  let connection = null;
  class WataneError extends Error {
    constructor(code, message, limit) {
      super(message);
      this.name = 'WataneError';
      this.code = code;
      this.limit = limit;
    }
  }
  const dispatch = (message) => {
    if (message.type === 'connection') { connection = message.id; return; }
    if (message.type === 'reload') { location.reload(); return; }
    for (const handler of handlers) {
      try { handler(message); } catch (error) { console.error(error); }
    }
  };
  window.watane = Object.freeze({
    WataneError,
    async call(fn, args) {
      const response = await fetch('/call', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(connection ? { fn, args: args ?? null, connection } : { fn, args: args ?? null }),
      });
      const body = await response.json().catch(() => null);
      if (response.ok && body && 'value' in body) return body.value;
      const error = (body && body.error) || { code: 'internal', message: `HTTP ${response.status}` };
      throw new WataneError(error.code, error.message, error.limit);
    },
    on(handler) {
      handlers.add(handler);
      if (!source) {
        source = new EventSource('/events');
        source.onmessage = (event) => dispatch(JSON.parse(event.data));
        source.onopen = () => dispatch({ type: 'changed' });
      }
      return () => handlers.delete(handler);
    },
  });
})();
