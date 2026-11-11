import { call } from './ipc.js';
const launcher = document.querySelector('#launcher');
const error = document.querySelector('#error');
const $ = selector => document.querySelector(selector);
function element(tag, text, parent, className) {
  const node = document.createElement(tag); node.textContent = text;
  if (className) node.className = className;
  if (parent) parent.append(node); return node;
}
function showError(e) { error.textContent = e.message || String(e); }
async function action(cmd, args = {}) {
  try { error.textContent = ''; return await call(cmd, args); }
  catch (e) { showError(e); throw e; }
  finally { await refresh(); }
}
function button(parent, text, className, callback) {
  const b = element('button', text, parent, className);
  b.onclick = () => Promise.resolve().then(callback).catch(showError); return b;
}
let lastMods = '';
function modsView(mods) {
  // Polls must not replace controls while the participant is using them.
  const signature = JSON.stringify(mods);
  if (signature === lastMods) return;
  lastMods = signature;
  const expanded = new Set(Array.from(launcher.querySelectorAll('details[open]'), d => d.closest('section').dataset.local));
  launcher.replaceChildren();
  if (!mods.length) element('p', 'No mods yet', launcher);
  for (const mod of mods) {
    const entry = element('section', '', launcher); entry.dataset.local = mod.local;
    element('h2', mod.name, entry);
    if (mod.error) element('p', mod.error, entry);
    if (mod.enabled && mod.page) button(entry, 'Open', 'open', () => action('mod_open', {local: mod.local}));
    if (!mod.enabled) { element('p', 'Disabled', entry); button(entry, 'Enable', 'enable', () => action('mod_enable', {local: mod.local})); }
    // Keep the established disable hook, including its disabled state.
    const disable = button(entry, 'Disable', 'disable', () => action('mod_disable', {local: mod.local})); disable.disabled = !mod.enabled;
    button(entry, 'Remove', 'remove', () => remove(mod));
    const details = element('details', '', entry); details.open = expanded.has(mod.local);
    element('summary', 'Details', details);
    const publisher = mod.publisher ? `${mod.publisher.name || ''} (${mod.publisher.fingerprint}) · signature verified` : 'local';
    for (const text of [mod.version, publisher, mod.page, mod.package, mod.release, mod.derivedFrom]) if (text) element('p', text, details);
    for (const grant of mod.grants) {
      const row = element('p', '', details); row.dataset.key = grant.key;
      element('span', `${grant.text} — ${grant.granted ? 'Granted' : 'Not granted'} `, row);
      button(row, grant.granted ? 'Withdraw' : 'Give back', grant.granted ? 'withdraw' : 'give', () => action(grant.granted ? 'grant_withdraw' : 'grant_give', {local:mod.local, key:grant.key}));
    }
  }
}
let removing = null;
function remove(mod) {
  removing = mod.local;
  $('#remove-dialog p').textContent = `Delete the data of ${mod.name}?`;
  $('#remove-dialog').showModal(); $('#keep-data').focus();
}
for (const [id, deleteData] of [['keep-data', false], ['delete-data', true]]) {
  $(`#${id}`).onclick = () => {
    const local = removing; removing = null; $('#remove-dialog').close();
    action('mod_remove', {local, deleteData}).catch(showError);
  };
}
$('#install').onclick = () => action('install_folders').catch(showError);
let lastTools = '', lastSets = '', lastTokens = '';
async function agentsView() {
  const [tools, sets, tokens] = await Promise.all([call('tools_list'), call('toolsets_list'), call('tokens_list')]);
  if (JSON.stringify(tools) !== lastTools) {
    lastTools = JSON.stringify(tools); $('#tools').replaceChildren();
    for (const tool of tools) {
      const label = element('label', '', $('#tools')); const check = document.createElement('input'); check.type = 'checkbox'; check.value = tool.entry; label.append(check); element('span', tool.tool, label);
    }
  }
  if (JSON.stringify(sets) !== lastSets) {
    lastSets = JSON.stringify(sets); $('#toolsets').replaceChildren(); $('#token-toolset').replaceChildren();
    for (const set of sets) {
      const row = element('p', `${set.name}: ${set.entries.map(entry => entry.replace(':', '_').replace('.', '_')).join(', ')} `, $('#toolsets'));
      button(row, 'Edit', 'edit', () => {
        $('#toolset-name').value = set.name;
        for (const input of $('#tools').querySelectorAll('input')) input.checked = set.entries.includes(input.value);
      });
      const option = element('option', set.name, $('#token-toolset')); option.value = set.name;
    }
  }
  if (JSON.stringify(tokens) !== lastTokens) {
    lastTokens = JSON.stringify(tokens); $('#tokens').replaceChildren();
    for (const token of tokens) {
      const row = element('p', `${token.handle} · ${token.toolset} · ${token.id} `, $('#tokens'));
      button(row, 'End token', 'end', () => action('token_end', {id: token.id}));
    }
  }
}
$('#save-toolset').onclick = () => action('toolset_save', {name: $('#toolset-name').value, entries: Array.from($('#tools').querySelectorAll('input:checked'), c => c.value)}).catch(showError);
$('#issue-token').onclick = async () => {
  try {
    const issued = await action('token_issue', {toolset: $('#token-toolset').value, handle: $('#handle').value});
    $('#token-secret').textContent = issued.token; $('#mcp-url').textContent = issued.mcpUrl; $('#token-dialog').showModal();
  } catch (e) { showError(e); }
};
$('#copy-token').onclick = () => navigator.clipboard.writeText($('#token-secret').textContent).catch(showError);
$('#close-token').onclick = () => $('#token-dialog').close();
$('#token-dialog').addEventListener('close', () => { $('#token-secret').textContent = ''; $('#mcp-url').textContent = ''; });
let refreshing = null;
function refresh() {
  if (refreshing) return refreshing;
  refreshing = (async () => {
    const status = await call('status');
    if (status.error) { document.querySelector('main').textContent = status.error; $('#agents').hidden = true; $('#install').disabled = true; document.body.dataset.ready = 'true'; return; }
    $('.mark').textContent = status.mark;
    modsView(await call('mods_list'));
    await agentsView();
    document.body.dataset.ready = 'true';
  })().finally(() => { refreshing = null; });
  return refreshing;
}
window.addEventListener('focus', () => refresh().catch(showError));
document.addEventListener('visibilitychange', () => { if (!document.hidden) refresh().catch(showError); });
setInterval(() => { if (!document.hidden) refresh().catch(showError); }, 2000);
refresh().catch(showError);
