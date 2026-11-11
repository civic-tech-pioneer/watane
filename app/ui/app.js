import { call } from './ipc.js';
const launcher = document.querySelector('#launcher');
const error = document.querySelector('#error');
async function render() {
  const status = await call('status');
  if (status.error) { document.body.textContent = status.error; document.body.dataset.ready = 'true'; return; }
  document.querySelector('.mark').textContent = status.mark;
  launcher.replaceChildren();
  for (const mod of await call('mods_list')) {
    const entry = document.createElement('section'); entry.dataset.local = mod.local;
    const name = document.createElement('h2'); name.textContent = mod.name; entry.append(name);
    if (mod.page && mod.enabled) {
      const open = document.createElement('button'); open.className = 'open'; open.textContent = 'Open';
      open.onclick = () => call('mod_open', {local: mod.local}).catch(showError); entry.append(open);
    }
    const disable = document.createElement('button'); disable.className = 'disable'; disable.textContent = 'Disable'; disable.disabled = !mod.enabled;
    disable.onclick = () => call('mod_disable', {local: mod.local}).then(render).catch(showError); entry.append(disable);
    launcher.append(entry);
  }
  document.body.dataset.ready = 'true';
}
function showError(e) { error.textContent = e.message || String(e); }
render().catch(showError);
