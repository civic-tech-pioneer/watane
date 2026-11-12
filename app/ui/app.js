import { call } from './ipc.js';
import { ownWithdrawal, offerLabel } from './words.js';
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
    element('span', mod.version, entry, 'version');
    if (mod.notice) element('p', mod.notice, entry, 'notice');
    if (mod.offer) button(entry, offerLabel(mod.offer, mod.offerGrants), 'offer', () => action('release_install', {release: mod.offer}));
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
// A refresh asked for during another may follow a change that one already
// read past, so it runs once more after it; callers share that one rerun.
let refreshing = null, rerun = null;
function refresh() {
  if (rerun) return rerun;
  if (refreshing) return rerun = refreshing.catch(() => {}).then(() => { rerun = null; return refresh(); });
  refreshing = (async () => {
    const status = await call('status');
    if (status.error) { document.querySelector('main').textContent = status.error; $('#agents').hidden = true; $('#distribution').hidden = true; $('#install').disabled = true; document.body.dataset.ready = 'true'; return; }
    $('.mark').textContent = status.mark;
    identityView(status);
    await channelsView();
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

let lastReleases = '';
function identityView(status) {
  const id = status.identity, site = status.site;
  $('#identity-info').textContent = id ? `${id.name || 'No profile name'} (${id.fingerprint})` : 'Identity unavailable';
  $('#confirm-own').hidden = !id || id.ownConfirmed;
  $('#site-dir').textContent = site ? `Site folder: ${site.dir}${site.url ? ' · ' + site.url : ''}` : '';
  $('#own-link').textContent = site?.link || '';
  $('#copy-link').disabled = !site?.link;
  const releases = id?.releases || [];
  const signature = JSON.stringify(releases);
  if (signature !== lastReleases) {
    lastReleases = signature; $('#releases').replaceChildren();
    for (const release of releases) {
      const row = element('div', `${release.slug} ${release.version}`, $('#releases'));
      row.dataset.release = release.release;
      if (release.withdrawn !== null) element('p', ownWithdrawal(release.withdrawn), row);
      else {
        const reason = document.createElement('input'); reason.placeholder = 'Reason for withdrawal'; reason.setAttribute('aria-label', `Reason for withdrawing ${release.slug} ${release.version}`); row.append(reason);
        button(row, 'Withdraw', 'withdraw-release', () => action('release_withdraw', {release: release.release, reason: reason.value}));
      }
    }
  }
}
let lastChannels = '';
async function channelsView() {
  const channels = await call('channels_list');
  const signature = JSON.stringify(channels); if (signature === lastChannels) return;
  lastChannels = signature; $('#channels').replaceChildren();
  for (const channel of channels) {
    const row = element('p', `${channel.name || ''} (${channel.fingerprint}) · ${channel.base || ''} · ${channel.status}${channel.cause ? ': ' + channel.cause : ''} `, $('#channels'));
    button(row, 'Unfollow', 'unfollow', () => action('channel_unfollow', {channel: channel.channel}));
  }
}
$('#install-from-link').onclick = () => action('install_link', {link: $('#install-link').value}).catch(showError);
$('#set-name').onclick = () => action('identity_set_name', {name: $('#profile-name').value}).catch(showError);
$('#set-site').onclick = () => action('site_set_url', {url: $('#site-url').value}).catch(showError);
$('#copy-link').onclick = () => navigator.clipboard.writeText($('#own-link').textContent).catch(showError);
$('#follow-channel').onclick = () => action('channel_follow', {link: $('#follow-link').value}).catch(showError);
$('#refresh-channels').onclick = () => action('channels_refresh').catch(showError);
$('#identity-backup').onclick = async () => {
  const passphrase = $('#passphrase').value;
  if (Array.from(passphrase).length < 12 || passphrase !== $('#passphrase-repeat').value) { showError({message:'Use matching passphrases of at least 12 characters'}); return; }
  try { await action('identity_backup', {passphrase}); } catch (e) { showError(e); }
  finally { $('#passphrase').value = ''; $('#passphrase-repeat').value = ''; }
};
$('#identity-restore').onclick = async () => {
  try { await action('identity_restore', {passphrase: $('#passphrase').value}); } catch (e) { showError(e); }
  finally { $('#passphrase').value = ''; $('#passphrase-repeat').value = ''; }
};
$('#confirm-identity').onclick = () => {
  if (!$('#never-published').checked && !$('#own-channel').value) {showError({message:'Enter your channel link or state that this identity never published'});return;}
  action('identity_confirm_own', {link: $('#never-published').checked ? null : $('#own-channel').value}).catch(showError);
};
