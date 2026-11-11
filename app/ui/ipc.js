export const invoke = (cmd, args = {}) => window.__TAURI_INTERNALS__.invoke(cmd, args);
