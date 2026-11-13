import { call } from './ipc.js';
import { ownWithdrawal, offerLabel, partyLabel, modCount, checkedLabel, viaLabels, emptySearch, derivedAction, actionErrors } from './words.js';
const launcher = document.querySelector('#launcher');
const error = document.querySelector('#error');
const $ = selector => document.querySelector(selector);
function element(tag, text, parent, className) {
  const node = document.createElement(tag); node.textContent = text;
  if (className) node.className = className;
  if (parent) parent.append(node); return node;
}
const errors = actionErrors(text => error.textContent = text);
function showError(e) { errors.show(e, e.actionId); }
function taggedError(e, actionId) { return {message: e.message || String(e), actionId}; }
function readRefresh() {
  const id = errors.current();
  return refresh().catch(e => { throw taggedError(e, id); });
}
async function action(cmd, args = {}) { return runAction(() => call(cmd, args)); }
async function runAction(work) {
  const id = errors.begin();
  try {
    const result = await work();
    await refresh();
    return result;
  } catch (e) {
    const error = taggedError(e, id);
    showError(error);
    try { await refresh(); } catch (_) { /* Keep the action's cause. */ }
    throw error;
  }
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
    const publisher = mod.publisher ? `${partyLabel(mod.publisher)} · signature verified` : 'local';
    for (const text of [mod.version, publisher, mod.page, mod.package, mod.release, mod.derivedFromLabel]) if (text) element('p', text, details);
    if (mod.release) {
      const derived = element('div', '', details, 'derived');
      let loading = false;
      const showDerived = async () => {
        if (!details.open || loading) return;
        loading = true;
        const actionId = errors.current();
        try {
          const origin = mod.release.slice(0, mod.release.lastIndexOf('@'));
          const hits = await call('search', {text: '', publisher: null, interface: null, origin});
          derived.replaceChildren();
          element('p', 'Made from this: as far as your indexes list them', derived);
          for (const hit of hits) {
            const row = element('p', `${hit.name} ${hit.version} by ${partyLabel(hit.publisher)} `, derived);
            row.dataset.release = hit.release;
            if (derivedAction(hit) === 'Installed') element('span', 'Installed', row);
            else if (derivedAction(hit) === 'Install') button(row, 'Install', 'install-release', () => action('install_release', {release: hit.release, link: hit.link}));
          }
        } catch (error) { showError(taggedError(error, actionId)); }
        finally { loading = false; }
      };
      details.addEventListener('toggle', showDerived);
      if (details.open) showDerived();
    }
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
$('#copy-token').onclick = () => runAction(() => navigator.clipboard.writeText($('#token-secret').textContent)).catch(showError);
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
    if (status.error) { document.querySelector('main').textContent = status.error; $('#agents').hidden = true; $('#distribution').hidden = true; $('#find').hidden = true; $('#install').disabled = true; document.body.dataset.ready = 'true'; return; }
    $('.mark').textContent = status.mark;
    identityView(status);
    await channelsView();
    await findView();
    modsView(await call('mods_list'));
    await agentsView();
    document.body.dataset.ready = 'true';
  })().finally(() => { refreshing = null; });
  return refreshing;
}
window.addEventListener('focus', () => readRefresh().catch(showError));
document.addEventListener('visibilitychange', () => { if (!document.hidden) readRefresh().catch(showError); });
setInterval(() => { if (!document.hidden) readRefresh().catch(showError); }, 2000);
readRefresh().catch(showError);

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
  const mods = await Promise.all(channels.map(channel => call('search', {text: '', publisher: channel.channel, interface: null})));
  const signature = JSON.stringify([channels, mods]); if (signature === lastChannels) return;
  lastChannels = signature; $('#channels').replaceChildren();
  for (const [i, channel] of channels.entries()) {
    const row = element('div', `${partyLabel(channel)} · ${channel.base || ''} · ${channel.status}${channel.cause ? ': ' + channel.cause : ''} `, $('#channels'));
    row.dataset.channel = channel.channel;
    button(row, 'Unfollow', 'unfollow', () => action('channel_unfollow', {channel: channel.channel}));
    if (!channel.base) continue;
    const link = `watane:${channel.channel}@${channel.base}`;
    button(row, 'Use as index', 'use-index', () => action('index_choose', {link}));
    const note = noteInput(row, 'Note for including this channel', 'include-note');
    button(row, 'Include in my index', 'include-index', () => action('index_include', {link, note: note.value}));
    for (const mod of mods[i]) {
      const entry = element('div', `${mod.name} (${mod.slug}) ${mod.version}`, row);
      const note = noteInput(entry, `Note for ${mod.slug}`, 'mod-note');
      button(entry, 'List in my index', 'list-mod', () => action('index_list', {link: `${link}#${mod.slug}`, note: note.value}));
    }
  }
}

function noteInput(parent, label, className) {
  const wrapper = element('label', label, parent);
  const input = document.createElement('input'); input.className = className; wrapper.append(input);
  return input;
}

let query = null, lastIndexes = '', lastHits = '';
async function findView() {
  const indexes = await call('indexes_list');
  $('#no-index').hidden = indexes.some(index => !index.own);
  const signature = JSON.stringify(indexes);
  if (signature !== lastIndexes) {
    lastIndexes = signature; $('#indexes').replaceChildren();
    for (const index of indexes) {
      const row = element('div', '', $('#indexes'), 'index-row'); row.dataset.index = index.channel;
      if (index.own) row.dataset.own = 'true';
      element('h3', index.own ? `Your index · ${partyLabel(index)}` : partyLabel(index), row);
      element('p', `${index.status}${index.cause ? ': ' + index.cause : ''} · ${modCount(index.mods)}`, row);
      for (const included of index.included) element('p', `Includes ${partyLabel(included)}`, row);
      for (const skipped of index.skipped) element('p', `Skipped ${partyLabel(skipped.party)}: ${skipped.cause}`, row, 'skipped');
      if (!index.own) button(row, 'Stop using as index', 'drop-index', () => action('index_drop', {channel: index.channel}));
      for (const listing of index.listings) {
        const entry = element('div', `${listing.kind} ${listing.target} · ${listing.note}`, row); entry.dataset.entry = listing.entry;
        button(entry, 'End a listing', 'unlist', () => action('index_unlist', {entry: listing.entry}));
      }
    }
  }
  if (query === null) return;
  const hits = await call('search', query);
  const rendered = hits.map(hit => ({...hit, checked: checkedLabel(hit)}));
  const hitSignature = JSON.stringify(rendered);
  if (hitSignature === lastHits) return;
  lastHits = hitSignature; $('#results').replaceChildren();
  if (!hits.length) element('p', emptySearch(), $('#results'));
  for (const hit of rendered) {
    const row = element('article', '', $('#results')); row.dataset.release = hit.release;
    element('h3', `${hit.name} (${hit.slug}) ${hit.version}`, row);
    element('p', hit.summary, row);
    if (hit.publisher) element('p', partyLabel(hit.publisher), row);
    element('p', hit.status === 'compromised' ? 'compromised' : (hit.verified ? 'signature verified' : hit.status), row);
    for (const via of viaLabels(hit.via)) element('p', via, row);
    if (hit.checked) element('p', hit.checked, row, 'checked');
    for (const [label, values] of [['Offers', hit.offers], ['Events', hit.events], ['Uses', hit.uses]]) {
      if (values.length) element('p', `${label}: ${values.join(', ')}`, row);
    }
    if (hit.installed) element('p', `Installed as ${hit.installed}`, row);
    if (hit.status !== 'compromised' && hit.verified && !hit.installed) button(row, 'Install', 'install-release', () => action('install_release', {release: hit.release, link: hit.link}));
  }
}
$('#add-index').onclick = () => action('index_choose', {link: $('#index-link').value}).catch(showError);
$('#search').onclick = () => {
  errors.begin();
  query = {text: $('#search-text').value, publisher: $('#search-publisher').value.trim() || null, interface: $('#search-interface').value.trim() || null};
  readRefresh().catch(showError);
};
$('#refresh-find').onclick = () => action('channels_refresh').catch(showError);
$('#install-from-link').onclick = () => action('install_link', {link: $('#install-link').value}).catch(showError);
$('#set-name').onclick = () => action('identity_set_name', {name: $('#profile-name').value}).catch(showError);
$('#set-site').onclick = () => action('site_set_url', {url: $('#site-url').value}).catch(showError);
$('#copy-link').onclick = () => runAction(() => navigator.clipboard.writeText($('#own-link').textContent)).catch(showError);
$('#follow-channel').onclick = () => action('channel_follow', {link: $('#follow-link').value}).catch(showError);
$('#refresh-channels').onclick = () => action('channels_refresh').catch(showError);
$('#identity-backup').onclick = async () => {
  errors.begin();
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
  errors.begin();
  if (!$('#never-published').checked && !$('#own-channel').value) {showError({message:'Enter your channel link or state that this identity never published'});return;}
  action('identity_confirm_own', {link: $('#never-published').checked ? null : $('#own-channel').value}).catch(showError);
};
