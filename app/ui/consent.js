import { call } from './ipc.js';
import { signTitle, checkedLabel, partyLabel, askerLabel, manageTitle } from './words.js';

const approve = document.querySelector('#approve');
const deny = document.querySelector('#deny');
const stop = document.querySelector('#stop');
const main = document.querySelector('main');
const grants = [];
let timer;
let view;
let submitting = false;
let gateReady = false;
let choice = null;
function updateApprove() {
  approve.disabled = !gateReady || submitting || (view?.request.kind === 'pick' && choice === null);
}

function disarm() {
  clearTimeout(timer);
  gateReady = false;
  updateApprove();
}
function arm() {
  disarm();
  deny.focus();
  timer = setTimeout(() => {
    gateReady = document.hasFocus();
    updateApprove();
  }, 800);
}
window.addEventListener('blur', disarm);
window.addEventListener('focus', arm);

function text(parent, tag, value) {
  const node = document.createElement(tag);
  node.textContent = value;
  parent.append(node);
  return node;
}
function party(p) {
  return partyLabel(p);
}
async function answer(allow, stopAsks = false) {
  if (!view || submitting || (allow && approve.disabled)) return;
  submitting = true;
  disarm();
  deny.disabled = true;
  stop.disabled = true;
  try {
    const withheld = allow ? grants.filter(g => !g.input.checked).map(g => ({local: g.local, key: g.key})) : [];
    await call('consent_answer', {id: view.id, approve: allow, withheld, choice: allow ? choice : null, ...(stopAsks ? {stop: true} : {})});
  } catch (e) {
    text(document.querySelector('#error'), 'span', e.message || String(e));
    submitting = false;
    deny.disabled = false;
    stop.disabled = false;
    arm();
  }
}
deny.onclick = () => answer(false);
stop.onclick = () => answer(false, true);
approve.onclick = () => answer(true);

try {
  view = await call('consent_get');
  if (view) {
    document.querySelector('.mark').textContent = view.mark;
    const request = view.request;
    document.querySelector('h1').textContent = request.kind === 'agent' ? view.title : request.kind === 'read' ? `${request.asked_by.name} asks to read ${request.link}` : request.kind === 'manage' ? manageTitle(request) : request.kind === 'install' ? (request.mods.length === 1 ? (request.mods[0].replaces ? `Update ${request.mods[0].name} to ${request.mods[0].version}` : `Install ${request.mods[0].name}`) : `Install ${request.mods.length} mods`) : request.kind === 'pick' ? `Choose a mod for ${request.consumer}` : request.kind === 'join' ? 'Pair a machine' : request.kind === 'reach' ? (request.what === 'relay' ? 'Use a relay' : 'Turn on the DHT') : signTitle(request.statement);
    const heading = document.querySelector('h1').textContent;
    deny.title = `Deny: ${heading}`;
    deny.setAttribute('aria-label', deny.title);
    stop.hidden = !request.asked_by;
    if (request.asked_by) {
      stop.textContent = `Deny and stop asks from ${request.asked_by.name}`;
      text(main, 'p', `Asked by ${askerLabel(request.asked_by)}, which manages your mods`);
    }
    if (request.kind === 'read') {
      text(main, 'p', `To install ${request.release}, this home reads the channel of this link. It has not read it before.`);
    } else if (request.kind === 'manage') {
      const target = request.target;
      text(main, 'h2', target.name);
      text(main, 'p', target.version);
      text(main, 'p', target.publisher ? `by ${party(target.publisher)}` : 'local');
      if (request.action === 'grant' && request.grant) {
        text(main, 'p', request.grant.short || request.grant.text).title = request.grant.text;
      }
      if (request.action === 'remove' || request.action === 'remove_data') {
        text(main, 'p', request.action === 'remove' ? 'Its data is kept. Installing it again brings it back.' : 'Its data is deleted. This cannot be undone.');
        if (request.users.length) text(main, 'p', `Used by: ${request.users.join(', ')}`);
      }
    } else if (request.kind === 'install') {
      if (request.by_program) text(main, 'p', 'Asked by a program on this device');
      for (const mod of request.mods) {
        const section = document.createElement('section');
        main.append(section);
        text(section, 'h2', mod.name);
        text(section, 'p', mod.version);
        text(section, 'p', mod.publisher ? `${mod.slug} ${mod.version} by ${party(mod.publisher)} · signature verified — ${mod.release}` : 'local');
        if (mod.origin) text(section, 'p', `made from ${mod.origin} (the publisher says so)`);
        for (const bind of mod.binds || []) text(section, 'p', bind);
        if (mod.replaces) text(section, 'p', `replaces ${mod.replaces}`);
        if (mod.becomes_main) text(section, 'p', `${mod.name} opens when watane starts, in place of the manager.`);
        for (const grant of mod.grants) {
          const label = document.createElement('label');
          const input = document.createElement('input');
          input.type = 'checkbox';
          input.checked = true;
          label.append(input);
          text(label, 'span', `Allow: ${grant.short || grant.text}${grant.new ? ' · new' : ''}`).title = grant.text;
          section.append(label);
          grants.push({input, local: mod.local, key: grant.key});
        }
        for (const provider of mod.uses) text(section, 'p', `uses ${provider}`);
        for (const provider of mod.agents_via || []) text(section, 'p', `Uses Claude Code through ${provider}.`);
      }
    } else if (request.kind === 'pick') {
      approve.textContent = 'Use this mod';
      text(main, 'p', `${request.provider} is not available: ${request.cause}`);
      for (const candidate of request.candidates) {
        const label = document.createElement('label');
        const input = document.createElement('input');
        input.type = 'radio';
        input.name = 'candidate';
        input.value = candidate.choice;
        input.onchange = () => {
          choice = candidate.choice;
          updateApprove();
        };
        label.append(input);
        text(label, 'strong', `${candidate.name} ${candidate.version}`);
        text(label, 'p', candidate.summary);
        text(label, 'p', `by ${party(candidate.publisher)} · signature verified`);
        if (candidate.origin) text(label, 'p', `made from ${candidate.origin} (the publisher says so)`);
        if (candidate.installed) text(label, 'p', 'installed');
        text(label, 'p', checkedLabel({checkedAt: candidate.checked_at, uncheckedSince: null}));
        main.append(label);
      }
    } else if (request.kind === 'agent') {
      text(main, 'p', view.question);
    } else if (request.kind === 'join' || request.kind === 'reach') {
      text(main, 'p', request.text);
      if (request.kind === 'join') text(main, 'h2', request.node);
    } else {
      text(main, 'p', party(request.signer));
      for (const detail of request.details) text(main, 'p', detail.label ? `${detail.label}: ${detail.value}` : detail.value);
    }
    arm();
    document.body.dataset.ready = 'true';
  }
} catch (e) {
  text(document.querySelector('#error'), 'span', e.message || String(e));
}
