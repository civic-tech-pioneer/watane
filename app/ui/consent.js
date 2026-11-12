import { call } from './ipc.js';
import { signTitle } from './words.js';

const approve = document.querySelector('#approve');
const deny = document.querySelector('#deny');
const main = document.querySelector('main');
const grants = [];
let timer;
let view;
let submitting = false;

function disarm() {
  clearTimeout(timer);
  approve.disabled = true;
}
function arm() {
  disarm();
  deny.focus();
  timer = setTimeout(() => {
    if (document.hasFocus() && !submitting) approve.disabled = false;
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
  return p.name ? `${p.name} (${p.fingerprint})` : p.fingerprint;
}
async function answer(allow) {
  if (!view || submitting || (allow && approve.disabled)) return;
  submitting = true;
  disarm();
  deny.disabled = true;
  try {
    const withheld = allow ? grants.filter(g => !g.input.checked).map(g => ({local: g.local, key: g.key})) : [];
    await call('consent_answer', {id: view.id, approve: allow, withheld});
  } catch (e) {
    text(document.querySelector('#error'), 'span', e.message || String(e));
    submitting = false;
    deny.disabled = false;
    arm();
  }
}
deny.onclick = () => answer(false);
approve.onclick = () => answer(true);

try {
  view = await call('consent_get');
  if (view) {
    document.querySelector('.mark').textContent = view.mark;
    const request = view.request;
    document.querySelector('h1').textContent = request.kind === 'install' ? (request.mods.length === 1 ? (request.mods[0].replaces ? `Update ${request.mods[0].name} to ${request.mods[0].version}` : `Install ${request.mods[0].name}`) : `Install ${request.mods.length} mods`) : signTitle(request.statement);
    if (request.kind === 'install') {
      if (request.by_program) text(main, 'p', 'Asked by a program on this device');
      for (const mod of request.mods) {
        const section = document.createElement('section');
        main.append(section);
        text(section, 'h2', mod.name);
        text(section, 'p', mod.version);
        text(section, 'p', mod.publisher ? `${mod.release} by ${party(mod.publisher)} · signature verified` : 'local');
        if (mod.replaces) text(section, 'p', `replaces ${mod.replaces}`);
        for (const grant of mod.grants) {
          const label = document.createElement('label');
          const input = document.createElement('input');
          input.type = 'checkbox';
          input.checked = true;
          label.append(input);
          text(label, 'span', `Allow: ${grant.text}${grant.new ? ' · new' : ''}`);
          section.append(label);
          grants.push({input, local: mod.local, key: grant.key});
        }
        for (const provider of mod.uses) text(section, 'p', `uses ${provider}`);
      }
    } else {
      text(main, 'p', party(request.signer));
      for (const detail of request.details) text(main, 'p', `${detail.label}: ${detail.value}`);
    }
    arm();
    document.body.dataset.ready = 'true';
  }
} catch (e) {
  text(document.querySelector('#error'), 'span', e.message || String(e));
}
