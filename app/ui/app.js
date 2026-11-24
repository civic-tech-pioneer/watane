import { call } from './ipc.js';
import { setupAgentSettings, agentSettingsView } from './settings-agents.js';
import {setupMachines, machinesView} from './machines.js';
import {connections, dependents, matchesLibrary, updateOffers, libraryOrder, badge, problem} from './manager.js';
import { ownWithdrawal, offerLabel, offerNotice, partyLabel, modCount, viaLabels, derivedAction, actionErrors, visibleIndexes, monogram, searchWords, ago, when, foundCount } from './words.js';
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
      location.hash = '#mods'; route(); filterLibrary(); installedNote(added, result?.followed || []);
    }
    closeAdd();
  }
  return result;
}
// Decorative manager icons share geometry and take their size and colour from CSS.
function icon(kind, size, className = '') {
  const shapes = {
    plus: [['path', {d:'M12 5v14M5 12h14'}]],
    link: [['path', {d:'M10 14a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1'}], ['path', {d:'M14 10a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1'}]],
    check: [['path', {d:'M20 6 9 17l-5-5'}]],
    warning: [['path', {d:'M12 9v4M12 17h.01'}], ['path', {d:'M10.3 3.9 2 18a2 2 0 0 0 1.7 3h16.6A2 2 0 0 0 22 18L13.7 3.9a2 2 0 0 0-3.4 0z'}]],
    globe: [['circle', {cx:12,cy:12,r:9}], ['path', {d:'M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18'}]]
  };
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [key, value] of Object.entries({viewBox:'0 0 24 24',fill:'none',stroke:'currentColor','stroke-width':kind === 'check' ? '2.4' : kind === 'plus' || kind === 'globe' && size === 13 ? '2.2' : '2','stroke-linecap':'round','stroke-linejoin':'round','aria-hidden':'true',class:`icon icon-${size} ${className}`})) svg.setAttribute(key, value);
  for (const [tag, attrs] of shapes[kind]) {
    const shape = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [key, value] of Object.entries(attrs)) shape.setAttribute(key, value);
    svg.append(shape);
  }
  return svg;
}
function grantIcon(grant, parent, size) {
  if (['network','http'].includes(grant.kind)) parent.append(icon('globe', size));
  else element('span', {storage:'▤',call:'→',subscribe:'⇄',manage:'⚙'}[grant.kind] || '→', parent).setAttribute('aria-hidden','true');
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
async function busyAction(selector, text, cmd, args) {
  const button = $(selector), old = button.textContent;
  if (button.disabled) return;
  button.disabled = true; button.setAttribute('aria-busy', 'true'); button.textContent = text;
  try { await action(cmd, args); }
  catch (e) { showError(e); }
  finally { button.disabled = false; button.removeAttribute('aria-busy'); button.textContent = old; }
}
function button(parent, text, className, callback) {
  const b = element('button', text, parent, `btn ${className || ''}`);
  b.onclick = () => Promise.resolve().then(callback).catch(showError); return b;
}
let lastMods = '', currentMods = [], startupFailed = false;
let managerTab = 'library', selected = null, sourceFilter = '', noteSelection = null;
function dismissInstalled() { $('#installed-note').hidden = true; noteSelection = null; }
function installedNote(mods, followed = []) {
  const note = $('#installed-note'); note.replaceChildren();
  note.append(icon('check', 18, 'verified'));
  const text = element('div', '', note);
  const names = mods.map(m => m.name);
  const label = names.length < 3 ? names.join(' and ') : `${names.slice(0, -1).join(', ')} and ${names.at(-1)}`;
  element('strong', `${label} ${mods.length === 1 ? 'is' : 'are'} installed.`, text);
  const targets = [...new Set(mods.flatMap(m => connections(m, currentMods).addsTo))].map(local => currentMods.find(m => m.local === local)?.name).filter(Boolean);
  if (mods.length === 1 && targets.length) { text.append(document.createTextNode(' ')); element('span', `It adds to ${targets.join(' and ')}.`, text, 'hint'); }
  for (const label of followed) { text.append(document.createTextNode(' ')); element('span', `watane now follows ${label}, so its updates reach you.`, text, 'hint'); }
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
    element('div', `Version ${mod.version}`, heading, 'version');
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
      element('span', mod.error || mod.notice || offerNotice(mod.offer, mod.offerGrants), notice);
      if (mod.offer) button(notice, `Install ${mod.offer.slice(mod.offer.lastIndexOf('@') + 1)}${mod.offerGrants?.length ? '…' : ''}`, 'offer', () => action('release_install', {release:mod.offer}));
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
      grantIcon(grant, element('span', '', row, 'grant-icon'), 14);
      const text = grant.short || grant.text; element('span', text, row, 'grant-text').title = grant.text;
      makeSwitch(row, text, grant.granted, grant.granted ? 'withdraw' : 'give', () => action(grant.granted ? 'grant_withdraw' : 'grant_give', {local:mod.local,key:grant.key}));
    }
    if (mod.consents?.length) {
      const agents = element('div', '', entry, 'grants agents'); element('div', 'Agents', agents, 'label');
      for (const consent of mod.consents) {
        const row = element('p', '', agents, 'grant-row consent-row'); row.dataset.key = consent.key;
        element('span', '', row, 'grant-icon').append(document.createTextNode('→'));
        element('span', consent.text, row, 'grant-text').title = consent.text;
        makeSwitch(row, consent.text, true, 'withdraw', () => action('consent_withdraw', {local:mod.local,key:consent.key}));
      }
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
  if (confirming) confirming.entry.querySelector('.actions').hidden = false;
  confirming = null;
}
function warn(mod, verb) {
  selectMod(mod.local);
  const {usedBy, extendedBy} = dependents(mod, currentMods);
  if (verb === 'Disable' && !usedBy.length && !extendedBy.length && !mod.main) return action('mod_disable', {local:mod.local});
  cancelConfirm();
  const entry = Array.from($('#mod-details').children).find(n => n.dataset.local === mod.local);
  const panel = $('#confirm-panel'), actions = entry.querySelector('.actions');
  actions.hidden = true;
  actions.after(panel);
  confirming = {mod,verb,entry};
  $('#confirm-title').textContent = `${verb} ${mod.name}?`;
  const warning = $('#dependency-warning'); warning.replaceChildren();
  warning.append(icon('warning', 18));
  const lines = element('div', '', warning, 'warning-lines');
  for (const [locals, label] of [[usedBy,'use'],[extendedBy,'extend']]) {
    if (locals.length) { const line = element('div', `These mods ${label} it: ${locals.map(local => currentMods.find(m => m.local === local).name).join(', ')}.`, lines); line.dataset.locals = locals.join(' '); }
  }
  if (mod.main) element('div', verb === 'Disable' ? 'Until you enable it, the manager opens at start.' : 'No mod will open at start.', lines);
  warning.hidden = !lines.children.length;
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
$('#first-index').onclick = () => { location.hash = '#find'; route(); $('#index-entry').hidden = false; $('#show-index').setAttribute('aria-expanded','true'); $('#index-link').focus(); };
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
    checkedAt = status.checkedAt ?? checkedAt; renderCheck();
    if (status.error) {
      startupFailed = true;
      launcher.textContent = status.error;
      $('#manager-header nav').hidden = true;
      route();
      for (const id of ['agents', 'chatgpt', 'claude', 'distribution', 'find']) $(`#${id}`).hidden = true;
      $('#install').disabled = true;
      document.body.dataset.ready = 'true';
      return;
    }
    for (const mark of document.querySelectorAll('.mark')) mark.textContent = status.mark;
    agentSettingsView(status);
    const machines = await machinesView(status);
    identityView(status, machines);
    await channelsView();
    const mods = await call('mods_list');
    modsView(mods);
    await findView();
    await updatesView(mods);
    await agentsView();
    document.body.dataset.ready = 'true';
  })().finally(() => { refreshing = null; });
  return refreshing;
}
setupAgentSettings(readRefresh);
window.addEventListener('focus', () => readRefresh().catch(showError));
document.addEventListener('visibilitychange', () => readRefresh().catch(showError));
setInterval(() => { if (!document.hidden) readRefresh().catch(showError); }, 2000);
setInterval(() => { if (document.hidden) readRefresh().catch(showError); }, 10000);
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
let lastChannels = '', knownChannels = [], knownIndexes = [];

async function channelsView() {
  const channels = await call('channels_list');
  knownChannels = channels;
  const mods = await Promise.all(channels.map(channel => call('search', {text: '', publisher: channel.channel, interface: null})));
  const signature = JSON.stringify([channels, mods, canSign, channels.map(c => c.readAt == null ? null : ago(c.readAt))]); if (signature === lastChannels) return;
  lastChannels = signature; $('#channels').replaceChildren();
  for (const [i, channel] of channels.entries()) {
    const state = channel.status === 'ok' ? (channel.readAt == null ? '' : `Read ${ago(channel.readAt)}`) : channel.status === 'failed' ? `Couldn't read: ${channel.cause}` : channel.cause ? `Compromised: ${channel.cause}` : 'Compromised';
    const row = element('div', `${[partyLabel(channel), state].filter(Boolean).join(' · ')} `, $('#channels'), 'card channel-card');
    row.title = channel.base || '';
    row.dataset.channel = channel.channel;
    button(row, 'Unfollow', 'unfollow', () => action('channel_unfollow', {channel: channel.channel}));
    if (!channel.base) continue;
    const link = `watane:${channel.channel}@${channel.base}`;
    button(row, 'Use as index', 'use-index', () => action('index_choose', {link, derived: true}));
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
export function renderIndexes(indexes) {
  knownIndexes = indexes;
  const signature = JSON.stringify(indexes);
  if (signature === lastIndexes) return;
  lastIndexes = signature;
  const shown = visibleIndexes(indexes);
  $('#no-index').hidden = !!shown.length;
  $('#indexes').replaceChildren(); $('#index-notices').replaceChildren();
  for (const index of shown) {
    const name = index.own ? 'You' : index.name || index.fingerprint;
    const chip = element('span', '', $('#indexes'), `ix${index.status === 'ok' ? '' : ' failed'}`);
    chip.dataset.channel = index.channel; chip.dataset.index = index.channel;
    if (index.own) chip.dataset.own = 'true';
    chip.title = `${partyLabel(index)} · ${index.status}`;
    element('span', '', chip, 'index-dot').setAttribute('aria-hidden','true');
    element('span', `${name} · ${index.status === 'compromised' ? 'compromised' : index.status === 'failed' ? 'could not be read' : modCount(index.mods)}`, chip);
    element('span', ` (${index.fingerprint}) · ${index.status}`, chip, 'sr-only');
    if (!index.own) {
      const drop = button(chip, '×', 'quiet drop-index', () => action('index_drop', {channel:index.channel}));
      drop.setAttribute('aria-label', `Stop using ${name} as index`);
    }
    const notes = element('div', '', $('#index-notices')); notes.dataset.index = index.channel;
    if (index.own) notes.dataset.own = 'true';
    if (index.cause) element('p', index.cause.startsWith('Could not read ') ? index.cause : `Could not read ${name}: ${index.cause}`, notes);
    for (const included of index.included) element('p', `Includes ${partyLabel(included)}`, notes);
    for (const skipped of index.skipped) element('p', `Skipped ${partyLabel(skipped.party)}: ${skipped.cause}`, notes, 'skipped');
    for (const listing of index.listings) {
      const entry = element('div', `${listing.kind} ${listing.target} · ${listing.note}`, notes); entry.dataset.entry = listing.entry;
      button(entry, 'End a listing', 'unlist', () => action('index_unlist', {entry:listing.entry}));
    }
  }
}
export function renderResults(hits) {
  const signature = JSON.stringify([hits, currentMods.map(m=>[m.local,m.version]), hits.map(h=>[h.checkedAt == null ? null : ago(h.checkedAt), h.uncheckedSince == null ? null : ago(h.uncheckedSince)])]);
  if (signature === lastHits) return;
  lastHits = signature;
  $('#result-count').textContent = foundCount(hits.length);
  $('#results').replaceChildren();
  for (const hit of hits) {
    const compromised = hit.status === 'compromised';
    const row = element('article', '', $('#results'), `result${compromised ? ' compromised' : ''}`); row.dataset.release = hit.release;
    row.append(monogram(hit, 48));
    const body = element('div', '', row, 'result-body');
    const head = element('div', '', body, 'result-head');
    const name = element('h2', hit.name, head); name.title = hit.slug;
    element('span', hit.version, head, 'version');
    const installed = currentMods.find(m => m.local === hit.installed);
    const exact = installed && installed.version === hit.version;
    if (installed && !exact) element('span', `You have ${installed.version}`, head, 'badge b-update');
    if (compromised) element('span', 'Compromised', head, 'badge b-problem');
    element('p', hit.summary, body, 'result-summary');
    const publisher = hit.publisher, label = publisher?.name || publisher?.fingerprint || '';
    const metadata = element('div', '', body, 'result-meta');
    const pub = element('p', '', metadata, 'publisher');
    const shield = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    shield.setAttribute('viewBox','0 0 24 24'); shield.setAttribute('aria-hidden','true');
    shield.setAttribute('class',`shield ${compromised ? 'bad' : 'verified'}`);
    const path = document.createElementNS('http://www.w3.org/2000/svg','path');
    path.setAttribute('d','M12 3 4 6v6c0 4 4 7 8 9 4-2 8-5 8-9V6z'); shield.append(path); pub.append(shield);
    element('strong', publisher?.you ? 'you' : label, pub);
    element('span', ` (${publisher?.fingerprint || ''}) · ${compromised ? 'channel compromised' : hit.verified ? 'signature verified' : hit.status}`, pub);
    for (const via of viaLabels(hit.via)) element('p', via, metadata, 'hint');
    if (hit.uncheckedSince != null) element('p', `not read since ${ago(hit.uncheckedSince)}`, metadata, 'checked hint');
    else if (hit.checkedAt != null) element('p', `read ${ago(hit.checkedAt)}`, metadata, 'checked hint');
    if (hit.from?.length) element('p', `From your machines: ${hit.from.join(', ')}`, body, 'hint');
    if (compromised) element('div', `Can't install: ${label}'s channel no longer matches what it said before, so watane can't trust it. Ask ${label} for the link to a new channel.`, body, 'blocked');
    else if (installed) {
      const link = element('a', 'Installed', row, 'btn installed-link'); link.href = '#mods';
      link.onclick = () => { $('#library-filter').value = ''; sourceFilter = ''; location.hash = '#mods'; route(); filterLibrary(); selectMod(installed.local, true); };
    } else if (hit.verified) {
      if (hit.link) element('p', 'Installing may follow this channel for updates.', body, 'hint');
      button(row, 'Install…', 'primary install-release', () => action('install_release', {release:hit.release,link:hit.link}));
    }
  }
}
async function findView() {
  renderIndexes(await call('indexes_list'));
  if (query !== null) renderResults(await call('search', query));
}
$('#show-index').onclick = () => {
  const open = $('#index-entry').hidden; $('#index-entry').hidden = !open;
  $('#show-index').setAttribute('aria-expanded',String(open)); if (open) $('#index-link').focus();
};
$('#add-index').onclick = () => busyAction('#add-index', 'Adding…', 'index_choose', {link: $('#index-link').value});
$('#search').onclick = () => {
  errors.begin();
  const parties = [...knownIndexes, ...knownChannels];
  query = searchWords($('#search-text').value, parties);
  const publisher = parties.find(p => p.channel === query.publisher);
  const reading = [publisher ? `By ${publisher.name || publisher.fingerprint}` : '', query.interface ? `Offering ${query.interface}` : ''].filter(Boolean);
  $('#search-reading').textContent = reading.join(' · '); $('#search-reading').hidden = !reading.length;
  readRefresh().catch(showError);
};
$('#search-text').onkeydown = event => { if (event.key === 'Enter') { event.preventDefault(); $('#search').click(); } };
$('#install-from-link').onclick = () => action('install_link', {link: $('#install-link').value}).catch(showError);
$('#set-name').onclick = () => action('identity_set_name', {name: $('#profile-name').value}).catch(showError);
$('#set-site').onclick = () => action('site_set_url', {url: $('#site-url').value}).catch(showError);
$('#copy-link').onclick = () => runAction(() => navigator.clipboard.writeText($('#own-link').textContent)).catch(showError);
$('#follow-channel').onclick = () => busyAction('#follow-channel', 'Following…', 'channel_follow', {link: $('#follow-link').value});
$('#refresh-channels').onclick = () => refreshChannels().catch(showError);
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
  const settings = !startupFailed && location.hash === '#settings';
  document.body.classList.toggle('settings', settings);
  $('#manager-header').hidden = settings; $('#settings-header').hidden = !settings;
  $('#refresh-channels').hidden = managerTab !== 'updates';
  $('#checked-at').hidden = managerTab !== 'updates';
  $('#filter-control').hidden = managerTab !== 'library' || !currentMods.length;
  $('#add-mod').hidden = startupFailed || managerTab === 'updates' || managerTab === 'library' && !currentMods.length;
  const add = $('#add-mod'); add.replaceChildren(); add.append(icon(managerTab === 'find' ? 'link' : 'plus', 16));
  element('span', managerTab === 'find' ? 'Install from a link' : 'Install a mod', add);
  $('#add-mod').classList.toggle('primary', managerTab === 'library');
  $('[data-tab=updates]').hidden = !currentMods.length && !$('#updates-count').textContent && !$('#update-channels .card') && !$('#update-done .done-row');
}
function route() {
  const settings = !startupFailed && location.hash === '#settings';
  managerTab = startupFailed ? 'library' : location.hash === '#find' ? 'find' : location.hash === '#updates' ? 'updates' : 'library';
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
const dismissedOffers = new Map();
let updateData = null, lastUpdates = '';
const offerKey = mod => JSON.stringify([mod.offer, mod.offerGrants, mod.notice?.startsWith('Withdrawn by the publisher: ') ? mod.notice : null]);
export function renderUpdates(mods, done, channels, indexes) {
  updateData = [mods, done, channels, indexes];
  const allOffers = updateOffers(mods);
  for (const [local, key] of dismissedOffers) if (!allOffers.some(m => m.local === local && offerKey(m) === key)) dismissedOffers.delete(local);
  const offers = allOffers.filter(m => dismissedOffers.get(m.local) !== offerKey(m));
  const signature = JSON.stringify([mods, offers, done, channels, indexes, [...channels,...indexes].map(c=>c.readAt == null ? null : ago(c.readAt)), done.map(u=>when(u.at))]);
  if (signature === lastUpdates) { renderCheck(); return; }
  lastUpdates = signature;
  $('#updates-count').textContent = offers.length ? String(offers.length) : ''; $('#updates-count').hidden = !offers.length;
  for (const selector of ['#update-offers','#update-channels','#update-done']) $(selector).replaceChildren();
  if (!offers.length) element('p','Nothing needs you.',$('#update-offers'),'hint');
  for (const mod of offers) {
    const compromised = mod.notice === "The publisher's channel is compromised";
    const withdrawn = mod.notice?.startsWith('Withdrawn by the publisher: ');
    const row = element('article','',$('#update-offers'),`card update-card${withdrawn || compromised ? ' problem' : ''}`); row.dataset.local = mod.local;
    row.append(monogram(mod,44));
    const body = element('div','',row,'update-body');
    const head = element('div','',body,'update-head'); element('strong',mod.name,head);
    const version = mod.offer?.slice(mod.offer.lastIndexOf('@')+1);
    element('span', withdrawn || compromised ? mod.version : `${mod.version} → ${version}`,head,'version');
    if (withdrawn || compromised) element('p',mod.notice,body,'bad');
    if (withdrawn) {
      const users = [...new Set([...dependents(mod,mods).usedBy,...dependents(mod,mods).extendedBy])].map(local=>mods.find(m=>m.local===local).name);
      const details = [];
      if (users.length) details.push(`${users.join(' and ')} ${users.length === 1 ? 'uses' : 'use'} it.`);
      if (mod.offer) details.push(`${version} is the newest version that is not withdrawn${mod.offerGrants.length ? '.' : ' and asks for nothing new.'}`);
      if (details.length) element('p',details.join(' '),body,'hint');
    } else if (!compromised) {
      const grants = element('div','Asks for something new:',body,'grant-chips hint');
      for (const grant of mod.offerGrants) {
        const chip = element('span','',grants,'newgrant'); chip.title=grant.text;
        grantIcon(grant, chip, 13); element('span',grant.short || grant.text,chip);
      }
      element('p',`Not now keeps ${mod.version} and hides this until the offer changes or watane restarts.`,body,'later-hint');
    }
    if (!compromised) {
      const actions = element('div','',row,'update-actions');
      if (!withdrawn) button(actions,'Not now','not-now',()=>{dismissedOffers.set(mod.local,offerKey(mod)); renderUpdates(...updateData);});
      if (mod.offer) button(actions,withdrawn ? offerLabel(mod.offer,mod.offerGrants) : 'Review…','primary offer',()=>action('release_install',{release:mod.offer}));
    }
  }
  const seen = new Set();
  for (const channel of [...channels,...indexes]) {
    if (!channel.cause || seen.has(channel.channel)) continue; seen.add(channel.channel);
    const row = element('article','',$('#update-channels'),'card update-card'); row.dataset.channel=channel.channel;
    element('span','',row,'channel-warning').append(icon('warning',20));
    const body = element('div','',row,'update-body');
    element('strong',channel.name ? `Couldn't read ${channel.name}'s channel` : `Couldn't read channel ${channel.fingerprint}`,body);
    const names = mods.filter(m=>m.release?.split('/')[0] === channel.channel).map(m=>m.name);
    const comes = names.length ? `${names.join(' and ')} ${names.length === 1 ? 'comes from it, so its' : 'come from it, so their'} updates wait. ` : '';
    element('p',`${comes}watane tries again at the next check.${channel.readAt == null ? '' : ` Last read ${ago(channel.readAt)}.`}`,body,'hint');
    element('p',`Cause: ${channel.cause}`,body,'channel-cause');
    button(row,'Try again','retry-channel',()=>refreshChannels());
  }
  $('#failed-channels').hidden = !seen.size;
  if (!done.length) element('p','None since the app started.',$('#update-done'),'hint');
  for (const update of [...done].sort((a,b)=>b.at-a.at)) {
    const row=element('div','',$('#update-done'),'done-row'); row.dataset.local=update.local;
    row.append(monogram(update,28)); element('strong',update.name,row); element('span',`${update.from} → ${update.to}`,row,'version'); element('span',when(update.at),row,'done-when hint');
  }
  headerView(); renderCheck();
}
async function updatesView(mods) {
  const [done, channels, indexes] = await Promise.all([call('updates_done'),call('channels_list'),call('indexes_list')]);
  renderUpdates(mods,done,channels,indexes);
}
let checking = false, checkedAt = null;
function renderCheck() {
  $('#refresh-channels').disabled = checking;
  $('#refresh-channels .refresh-label').textContent = checking ? 'Checking…' : 'Check now';
  $('#refresh-channels .refresh-glyph').classList.toggle('spin',checking);
  const count = new Set([...knownChannels,...knownIndexes.filter(i=>!i.own)].map(c=>c.channel)).size;
  $('#checked-at').textContent = checking ? count ? `Reading ${count} ${count === 1 ? 'channel' : 'channels'}…` : 'Checking…' : checkedAt == null ? 'Not checked yet' : `Checked ${ago(checkedAt)}`;
  for (const retry of document.querySelectorAll('.retry-channel')) retry.disabled = checking;
}
async function refreshChannels() {
  if (checking) return;
  checking = true; renderCheck();
  await runAction(async () => {
    try {
      const result = await call('channels_refresh'); checkedAt=Date.now(); return result;
    } finally { checking=false; renderCheck(); }
  });
}

window.wataneDropped = paths => action('install_paths', {paths}).catch(showError);

setupMachines({action, showError});
