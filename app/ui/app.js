import { call } from './ipc.js';
import {setupMachines, machinesView} from './machines.js';
import {connections, dependents, matchesLibrary, updateOffers, libraryOrder, badge, problem} from './manager.js';
import { ownWithdrawal, offerLabel, partyLabel, modCount, checkedLabel, viaLabels, emptySearch, derivedAction, actionErrors, visibleIndexes, monogram } from './words.js';
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
async function action(cmd, args = {}) {
  const installing = ['install_link', 'install_folders', 'install_paths', 'install_release'].includes(cmd);
  const before = new Set(currentMods.map(mod => mod.local));
  if (installing) dismissInstalled();
  const result = await runAction(() => call(cmd, args));
  if (installing) {
    const added = (result?.installed || []).filter(local => !before.has(local)).map(local => currentMods.find(mod => mod.local === local)).filter(Boolean);
    if (added.length) {
      $('#library-filter').value = ''; sourceFilter = ''; selected = added[0].local;
      location.hash = '#mods'; route(); filterLibrary(); installedNote(added);
    }
    closeAdd();
  }
  return result;
}
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
  const b = element('button', text, parent, `btn ${className || ''}`);
  b.onclick = () => Promise.resolve().then(callback).catch(showError); return b;
}
let lastMods = '', currentMods = [];
let managerTab = 'library', selected = null, sourceFilter = '', noteSelection = null;
function dismissInstalled() { $('#installed-note').hidden = true; noteSelection = null; }
function installedNote(mods) {
  const note = $('#installed-note'); note.replaceChildren();
  element('span', '✓', note, 'verified').setAttribute('aria-hidden', 'true');
  const text = element('div', '', note);
  element('strong', `${mods.map(m => m.name).join(' and ')} ${mods.length === 1 ? 'is' : 'are'} installed.`, text);
  const targets = [...new Set(mods.flatMap(m => connections(m, currentMods).addsTo))].map(local => currentMods.find(m => m.local === local)?.name).filter(Boolean);
  if (targets.length) element('div', `It adds to ${targets.join(' and ')}.`, text, 'hint');
  const dismiss = button(note, '✕', 'quiet dismiss', dismissInstalled); dismiss.setAttribute('aria-label', 'Dismiss');
  noteSelection = selected; note.hidden = false;
}
function selectMod(local, focus = false) {
  if (noteSelection && noteSelection !== local) dismissInstalled();
  if (confirming && selected !== local) cancelConfirm();
  selected = local;
  for (const tile of launcher.children) tile.setAttribute('aria-pressed', String(tile.dataset.tile === local));
  for (const section of $('#mod-details').children) section.hidden = section.dataset.local !== local;
  if (focus) Array.from($('#mod-details').children).find(n => n.dataset.local === local)?.querySelector('h2').focus();
}
function drawBadge(mod, parent) {
  const text = badge(mod);
  if (text) element('span', text, parent, `badge b-${{Off:'off',Problem:'problem',Update:'update'}[text]}`);
}
function makeSwitch(parent, label, checked, className, callback) {
  const sw = button(parent, '', `sw ${className}`, callback);
  sw.setAttribute('role', 'switch'); sw.setAttribute('aria-checked', String(checked)); sw.setAttribute('aria-label', label);
  return sw;
}
export function modsView(mods) {
  currentMods = mods;
  const signature = JSON.stringify(mods);
  if (signature === lastMods) return;
  lastMods = signature;
  // Keep the shared confirm node alive when a poll replaces the panel.
  cancelConfirm(); document.body.append($('#confirm-panel'));
  launcher.replaceChildren(); $('#mod-details').replaceChildren();
  $('#library-count').textContent = modCount(mods.length);
  $('#first-start').hidden = !!mods.length; $('#library-content').hidden = !mods.length;
  for (const mod of libraryOrder(mods)) {
    const tile = button(launcher, '', `tile${mod.enabled ? '' : ' off'}`, () => selectMod(mod.local));
    tile.dataset.tile = mod.local; tile.append(monogram(mod, 44));
    drawBadge(mod, tile);
    element('span', mod.name, tile, 'tile-name');
    element('span', `${mod.publisher?.name || mod.publisher?.fingerprint || 'local'} · ${mod.version}`, tile, 'tile-by');
    if (mod.main) element('span', 'Opens at start', tile, 'main-mark');
    const entry = element('section', '', $('#mod-details'), 'mod-details'); entry.dataset.local = mod.local;
    const head = element('div', '', entry, 'detail-head'); head.append(monogram(mod, 56));
    const heading = element('div', '', head);
    const h2 = element('h2', mod.name, heading); h2.tabIndex = -1;
    element('div', `Version ${mod.version}`, heading, 'version'); drawBadge(mod, heading);
    const publisher = element('div', '', heading, 'publisher');
    if (mod.publisher) {
      const shield = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      shield.setAttribute('viewBox', '0 0 24 24'); shield.setAttribute('aria-hidden', 'true'); shield.setAttribute('class', 'verified shield');
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.setAttribute('d', 'M12 3 4 6v6c0 4 4 7 8 9 4-2 8-5 8-9V6z M8 12l3 3 5-6'); shield.append(path); publisher.append(shield);
      element('strong', mod.publisher.name || mod.publisher.fingerprint, publisher);
      element('span', ` (${mod.publisher.fingerprint}) · signature verified`, publisher);
    } else { element('span', 'local', publisher, 'badge b-local'); publisher.append(document.createTextNode(' made on this device')); }
    element('p', mod.summary || '', entry, 'summary');
    if (mod.notice || mod.error || mod.offer) {
      const notice = element('div', '', entry, `notice${problem(mod) ? ' problem' : ''}`);
      element('span', mod.error || mod.notice || 'An update is available.', notice);
      if (mod.offer) button(notice, offerLabel(mod.offer, mod.offerGrants || []), 'offer', () => action('release_install', {release:mod.offer}));
    }
    const actions = element('div', '', entry, 'actions');
    if (mod.enabled && mod.page) button(actions, 'Open', 'open primary', () => action('mod_open', {local:mod.local}));
    if (mod.enabled) button(actions, 'Disable', 'disable', () => warn(mod, 'Disable'));
    else button(actions, 'Enable', 'enable', () => action('mod_enable', {local:mod.local}));
    button(actions, 'Remove', 'remove danger quiet', () => warn(mod, 'Remove'));
    if (mod.page) {
      const start = element('div', '', entry, 'start-row'), text = element('div', '', start);
      const label = element('div', 'Opens at start', text, 'start-label'); label.id = `start-label-${mod.local}`;
      const main = mods.find(m => m.main);
      element('div', mod.main ? 'watane opens it when it starts, in place of the manager.' : `Turn on to open it at start, in place of ${main ? main.name : 'the manager'}.`, text, 'hint');
      const sw = makeSwitch(start, 'Opens at start', !!mod.main, 'main-set', () => action('main_set', {local:mod.main ? null : mod.local})); sw.setAttribute('aria-labelledby', label.id);
    }
    const grants = element('div', '', entry, 'grants'); element('div', 'What it may do', grants, 'label');
    for (const grant of mod.grants) {
      const row = element('p', '', grants, `grant-row${grant.granted ? '' : ' denied'}`); row.dataset.key = grant.key;
      element('span', {storage:'▤', network:'◎', http:'◎', call:'→', subscribe:'⇄', manage:'⚙'}[grant.kind] || '→', row, 'grant-icon').setAttribute('aria-hidden', 'true');
      const text = grant.short || grant.text; element('span', text, row, 'grant-text').title = grant.text;
      makeSwitch(row, text, grant.granted, grant.granted ? 'withdraw' : 'give', () => action(grant.granted ? 'grant_withdraw' : 'grant_give', {local:mod.local,key:grant.key}));
    }
    element('p', 'A change here takes effect now. A mod that asks for more goes through the consent view.', grants, 'hint');
    const links = element('div', '', entry, 'connections'); element('div', 'Connections', links, 'label');
    const connected = connections(mod, mods);
    if (Object.values(connected).every(names => !names.length)) element('p', 'Works on its own: it uses no mod and no mod uses it.', links);
    for (const [kind, rel, label, hint] of [['uses','uses','Uses','it needs them'],['addsTo','adds-to','Adds to','it puts things in them'],['usedBy','used-by','Used by','they need it'],['extendedBy','extended-by','Extended by','they put things in it']]) {
      if (!connected[kind].length) continue;
      const row = element('div', '', links, 'connection-row'); row.dataset.rel = rel;
      const caption = element('div', '', row, 'connection-label');
      element('span', label, caption); element('span', hint, caption, 'hint');
      const targets = element('div', '', row, 'connection-targets');
      for (const local of connected[kind]) {
        const target = mods.find(m => m.local === local);
        const chip = button(targets, '', 'chip', () => {
          $('#library-filter').value = ''; sourceFilter = ''; location.hash = '#mods'; route(); filterLibrary(); selectMod(local, true);
        });
        chip.dataset.target = local; chip.append(monogram(target, 24)); element('span', target.name, chip);
      }
    }
    if (mod.release) {
      const derived = element('div', '', entry, 'derived'); derived.hidden = true;
      const origin = mod.release.slice(0, mod.release.lastIndexOf('@'));
      const actionId = errors.current();
      call('search', {text:'',publisher:null,interface:null,origin}).then(hits => {
        if (!hits.length) return;
        derived.hidden = false; element('p', 'Made from this: as far as your indexes list them', derived);
        for (const hit of hits) {
          const row = element('p', `${hit.name} ${hit.version} by ${partyLabel(hit.publisher)} `, derived); row.dataset.release = hit.release;
          if (derivedAction(hit) === 'Installed') element('span', 'Installed', row);
          else if (derivedAction(hit) === 'Install') button(row, 'Install', 'install-release', () => action('install_release', {release:hit.release,link:hit.link}));
        }
      }).catch(e => showError(taggedError(e, actionId)));
    }
    const technical = element('details', '', entry, 'technical'); element('summary', 'Technical details', technical);
    const dl = element('dl', '', technical);
    const values = [['Made from', mod.derivedFromLabel || 'its own source'], ['Page origin', mod.page || 'none'], ['Package', mod.package]];
    if (mod.release) values.push(['Release',mod.release]);
    values.push(['Publisher key',mod.publisher?.fingerprint || 'none (local)']);
    for (const [label,value] of values) { element('dt', label, dl); element('dd', value || '', dl); }
  }
  filterLibrary(); headerView();
}
let confirming = null;
function cancelConfirm() {
  const panel = $('#confirm-panel'); panel.hidden = true;
  if (confirming) { confirming.entry.querySelector('.actions').hidden = false; const start = confirming.entry.querySelector('.start-row'); if (start) start.hidden = false; }
  confirming = null;
}
function warn(mod, verb) {
  selectMod(mod.local);
  const {usedBy, extendedBy} = dependents(mod, currentMods);
  if (verb === 'Disable' && !usedBy.length && !extendedBy.length && !mod.main) return action('mod_disable', {local:mod.local});
  cancelConfirm();
  const entry = Array.from($('#mod-details').children).find(n => n.dataset.local === mod.local);
  const panel = $('#confirm-panel'), actions = entry.querySelector('.actions');
  actions.hidden = true; const start = entry.querySelector('.start-row'); if (start) start.hidden = true;
  (start || actions).after(panel);
  confirming = {mod,verb,entry};
  $('#confirm-title').textContent = `${verb} ${mod.name}?`;
  const warning = $('#dependency-warning'); warning.replaceChildren();
  for (const [locals, label] of [[usedBy,'use'],[extendedBy,'extend']]) {
    if (locals.length) { const line = element('div', `These mods ${label} it: ${locals.map(local => currentMods.find(m => m.local === local).name).join(', ')}.`, warning); line.dataset.locals = locals.join(' '); }
  }
  if (mod.main) element('div', verb === 'Disable' ? 'Until you enable it, the manager opens at start.' : 'No mod will open at start.', warning);
  warning.hidden = !warning.children.length;
  $('#data-choice').hidden = verb !== 'Remove'; $('#keep-data').checked = true; $('#delete-data').checked = false;
  $('#confirm-continue').textContent = verb; panel.hidden = false; $('#confirm-cancel').focus();
}
$('#confirm-cancel').onclick = cancelConfirm;
$('#delete-data').onchange = $('#keep-data').onchange = () => { $('#confirm-continue').textContent = $('#delete-data').checked ? 'Remove and delete data' : 'Remove'; };
$('#confirm-continue').onclick = () => {
  const {mod,verb} = confirming, deleteData = $('#delete-data').checked; cancelConfirm();
  action(verb === 'Disable' ? 'mod_disable' : 'mod_remove', verb === 'Disable' ? {local:mod.local} : {local:mod.local,deleteData}).catch(showError);
};
function closeAdd() { $('#add-menu').hidden = true; $('#add-mod').setAttribute('aria-expanded', 'false'); }
$('#add-mod').onclick = () => {
  const open = $('#add-menu').hidden; $('#add-menu').hidden = !open; $('#add-mod').setAttribute('aria-expanded', String(open));
  if (open) $('#install-link').focus();
};
document.addEventListener('keydown', event => { if (event.key === 'Escape') { cancelConfirm(); closeAdd(); } });
document.addEventListener('click', event => { if (!event.target.closest('#add-menu, #add-mod')) closeAdd(); });
$('#install').onclick = $('#first-folder').onclick = () => action('install_folders').catch(showError);
$('#first-link-go').onclick = () => action('install_link', {link:$('#first-link').value}).catch(showError);
$('#first-index').onclick = () => { location.hash = '#find'; route(); $('#index-link').focus(); };
launcher.addEventListener('keydown', event => {
  if (!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key)) return;
  const tiles = Array.from(launcher.children).filter(tile => !tile.hidden);
  const index = tiles.findIndex(tile => tile.dataset.tile === selected);
  const columns = getComputedStyle(launcher).gridTemplateColumns.split(' ').length;
  const step = {ArrowLeft:-1,ArrowRight:1,ArrowUp:-columns,ArrowDown:columns}[event.key];
  const next = tiles[Math.max(0, Math.min(tiles.length - 1, index + step))];
  if (next) { event.preventDefault(); selectMod(next.dataset.tile); next.focus(); }
});
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
      const row = element('p', `${set.name}: ${set.tools.join(', ')} `, $('#toolsets'));
      button(row, 'Edit', 'edit', () => {
        $('#toolset-name').value = set.name;
        for (const input of $('#tools').querySelectorAll('input')) input.checked = set.entries.includes(input.value);
      });
      button(row, 'Remove', 'remove-toolset', () => action('toolset_remove', {name: set.name}));
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
    for (const mark of document.querySelectorAll('.mark')) mark.textContent = status.mark;
    const machines = await machinesView(status);
    identityView(status, machines);
    await channelsView();
    await findView();
    const mods = await call('mods_list');
    modsView(mods);
    await updatesView(mods);
    await agentsView();
    document.body.dataset.ready = 'true';
  })().finally(() => { refreshing = null; });
  return refreshing;
}
window.addEventListener('focus', () => readRefresh().catch(showError));
document.addEventListener('visibilitychange', () => { if (!document.hidden) readRefresh().catch(showError); });
setInterval(() => { if (!document.hidden) readRefresh().catch(showError); }, 2000);
readRefresh().catch(showError);

let lastReleases = '', canSign = false;
function identityView(status, machines) {
  const id = status.identity, site = status.site;
  canSign = !!id;
  $('#identity-info').textContent = id ? `${id.name || 'No profile name'} (${id.fingerprint})` : machines.anchor ? `This machine holds no identity. It is a ${machines.machine} machine of ${machines.anchor}.` : 'This machine holds no identity. It has not joined your machines.';
  $('#identity-controls').hidden = !id;
  $('#site').hidden = !id;
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
  const signature = JSON.stringify([channels, mods, canSign]); if (signature === lastChannels) return;
  lastChannels = signature; $('#channels').replaceChildren();
  for (const [i, channel] of channels.entries()) {
    const row = element('div', `${partyLabel(channel)} · ${channel.base || ''} · ${channel.status}${channel.cause ? ': ' + channel.cause : ''} `, $('#channels'));
    row.dataset.channel = channel.channel;
    button(row, 'Unfollow', 'unfollow', () => action('channel_unfollow', {channel: channel.channel}));
    if (!channel.base) continue;
    const link = `watane:${channel.channel}@${channel.base}`;
    button(row, 'Use as index', 'use-index', () => action('index_choose', {link}));
    if (!canSign) continue;
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
    for (const index of visibleIndexes(indexes)) {
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
    if (hit.from?.length) element('p', `From your machines: ${hit.from.join(', ')}`, row);
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
  query = {text: $('#search-text').value, publisher: null, interface: null};
  readRefresh().catch(showError);
};
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

function filterLibrary() {
  const visible = [];
  for (const tile of launcher.children) {
    const mod = currentMods.find(m => m.local === tile.dataset.tile);
    tile.hidden = !matchesLibrary(mod, $('#library-filter').value, sourceFilter);
    if (!tile.hidden) visible.push(mod.local);
  }
  for (const pill of document.querySelectorAll('[data-source]')) pill.setAttribute('aria-pressed', String(pill.dataset.source === sourceFilter));
  $('#no-matches').hidden = !currentMods.length || visible.length > 0;
  selectMod(visible.includes(selected) ? selected : visible[0] || null);
}
$('#library-filter').oninput = filterLibrary;
for (const pill of document.querySelectorAll('[data-source]')) pill.onclick = () => { sourceFilter = pill.dataset.source; filterLibrary(); };
function headerView() {
  const settings = location.hash === '#settings';
  document.body.classList.toggle('settings', settings);
  $('#manager-header').hidden = settings; $('#settings-header').hidden = !settings;
  $('#refresh-channels').hidden = managerTab !== 'updates';
  $('#filter-control').hidden = managerTab !== 'library' || !currentMods.length;
  $('#add-mod').hidden = managerTab === 'updates' || managerTab === 'library' && !currentMods.length;
  $('#add-mod').textContent = managerTab === 'find' ? '↗ Install from a link' : '＋ Install a mod';
  $('#add-mod').classList.toggle('primary', managerTab === 'library');
  $('[data-tab=updates]').hidden = !currentMods.length && !$('#updates-count').textContent && !$('#update-channels').children.length && !$('#update-done').children.length;
}
function route() {
  const settings = location.hash === '#settings';
  managerTab = location.hash === '#find' ? 'find' : location.hash === '#updates' ? 'updates' : 'library';
  $('#settings').hidden = !settings; $('#mods').hidden = settings;
  for (const tab of ['library', 'find', 'updates']) {
    $(`#${tab}`).hidden = tab !== managerTab;
    const link = $(`[data-tab=${tab}]`);
    if (tab === managerTab) link.setAttribute('aria-current', 'page'); else link.removeAttribute('aria-current');
  }
  closeAdd(); headerView();
  if (settings) readRefresh().catch(showError);
}
window.addEventListener('hashchange', route);
if (!location.hash) location.hash = '#mods';
route();
let lastUpdates = '';
async function updatesView(mods) {
  const [done, channels, indexes] = await Promise.all([call('updates_done'), call('channels_list'), call('indexes_list')]);
  const offers = updateOffers(mods);
  $('#updates-count').textContent = offers.length ? String(offers.length) : ''; $('#updates-count').hidden = !offers.length;
  const signature = JSON.stringify([offers, channels, indexes, done]);
  if (signature === lastUpdates) return;
  lastUpdates = signature;
  for (const selector of ['#update-offers', '#update-channels', '#update-done']) $(selector).replaceChildren();
  for (const mod of offers) {
    const row = element('p', `${mod.name}: ${mod.notice || ''}`, $('#update-offers'));
    if (mod.offer) button(row, offerLabel(mod.offer, mod.offerGrants), 'offer', () => action('release_install', {release:mod.offer}));
  }
  const seen = new Set();
  for (const channel of [...channels, ...indexes]) {
    if (channel.cause && !seen.has(channel.channel)) { seen.add(channel.channel); element('p', `${partyLabel(channel)}: ${channel.cause}`, $('#update-channels')); }
  }
  for (const update of done) element('p', `${update.name}: ${update.from} → ${update.to} · ${new Date(update.at).toLocaleString()}`, $('#update-done'));
  headerView();
}


window.wataneDropped = paths => action('install_paths', {paths}).catch(showError);

setupMachines({action, showError});
