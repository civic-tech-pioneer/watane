import { call } from './ipc.js';

const byId = id => document.getElementById(id);
const busy = new Set();
let claudeChosen = false;

function updateButtons() {
  byId('chatgpt-sign-in').disabled = busy.has('chatgpt');
  byId('chatgpt-sign-out').disabled = busy.has('chatgpt');
  byId('claude-choose').disabled = busy.has('claude');
  byId('claude-clear').disabled = busy.has('claude') || !claudeChosen;
}

export function agentSettingsView({ chatgpt, claude }) {
  if (!busy.has('chatgpt')) {
    byId('chatgpt-status').textContent = chatgpt.signedIn
      ? `Signed in with ChatGPT · ${chatgpt.plan}`
      : 'Not signed in';
    byId('chatgpt-sign-in').hidden = chatgpt.signedIn;
    byId('chatgpt-sign-out').hidden = !chatgpt.signedIn;
  }
  if (chatgpt.error) byId('chatgpt-error').textContent = chatgpt.error;
  claudeChosen = claude !== null;
  byId('claude-path').textContent = claude?.path ?? '';
  byId('claude-status').textContent = claude?.error ?? (claude ? `Claude Code ${claude.version}` : 'No program chosen');
  updateButtons();
}

export function setupAgentSettings(refresh) {
  function action(group, id, command) {
    byId(id).onclick = async () => {
      const error = byId(`${group}-error`);
      error.textContent = '';
      busy.add(group);
      updateButtons();
      if (id === 'chatgpt-sign-in') byId('chatgpt-status').textContent = 'Finish signing in in your browser.';
      try {
        await call(command);
      } catch (cause) {
        error.textContent = cause.message || String(cause);
      } finally {
        busy.delete(group);
        updateButtons();
        try {
          await refresh();
        } catch (cause) {
          if (!error.textContent) error.textContent = cause.message || String(cause);
        }
      }
    };
  }

  action('chatgpt', 'chatgpt-sign-in', 'chatgpt_sign_in');
  action('chatgpt', 'chatgpt-sign-out', 'chatgpt_sign_out');
  action('claude', 'claude-choose', 'claude_binary_pick');
  action('claude', 'claude-clear', 'claude_binary_clear');
}
