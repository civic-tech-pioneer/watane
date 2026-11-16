import {call} from './ipc.js';
const $ = selector => document.querySelector(selector);
let hooks, lastView = '', lastHints = '', status, view, joinPending = false;
function text(parent, tag, value) {const e = document.createElement(tag); e.textContent = value; parent.append(e); return e;}
function button(parent, label, work) {const b = text(parent, 'button', label); b.className = 'btn'; b.onclick = () => Promise.resolve().then(work).catch(hooks.showError); return b;}
export function setupMachines(options) {
  hooks = options;
  const act = (cmd, args) => hooks.action(cmd, args).catch(hooks.showError);
  $('#machines-on').onchange = () => act('machines_on', {on:$('#machines-on').checked});
  for (const role of ['full','light']) $('#pair-'+role).onclick = () => act('pair_code', {role});
  $('#pair-copy').onclick = () => navigator.clipboard.writeText($('#pair-code').value).catch(hooks.showError);
  let pendingJoin;
  const join = () => act('machine_join', pendingJoin).finally(() => {joinPending = false; $('#join').disabled = !!view?.joining;});
  // Read the identity at the click: the page's last status may predate a
  // signature (an agent's publish), and the question must not say "signed nothing".
  $('#join').onclick = () => {
    if (joinPending) return;
    joinPending = true; $('#join').disabled = true;
    return Promise.resolve().then(async () => {
      pendingJoin = {code:$('#join-code').value, replace:$('#join-replace').checked};
      const identity = (await call('status')).identity;
      if (!identity || identity.signed) {await join(); return;}
      const dialog = $('#join-question');
      dialog.querySelector('p').textContent = `This home's identity ${identity.fingerprint} has signed nothing. Joining your machines deletes it, and this machine then signs nothing. Join?`;
      dialog.showModal(); $('#join-cancel').focus();
    }).catch(error => {joinPending = false; $('#join').disabled = !!view?.joining; hooks.showError(error);});
  };
  $('#join-question').onclose = () => {if (!pendingJoin) {joinPending = false; $('#join').disabled = !!view?.joining;}};
  $('#join-question').oncancel = () => {pendingJoin = null;};
  $('#join-cancel').onclick = () => {pendingJoin = null; $('#join-question').close();};
  $('#join-confirm').onclick = () => {$('#join-question').close(); join();};
  $('#save-expiry').onclick = () => act('machines_expiry', {days:Number($('#machines-expiry').value)});
  $('#relay-add').onclick = () => act('relay_add', {url:$('#relay-url').value});
  $('#dht-on').onchange = () => act('dht_set', {on:$('#dht-on').checked});
  $('#machine-remove-cancel').onclick = () => $('#machine-remove-question').close();
}
export async function machinesView(currentStatus) {
  status = currentStatus;
  view = await call('machines');
  $('#machines-on').checked = view.on;
  $('#machine-info').textContent = `This machine: ${view.node} · ${['full','light'].includes(view.machine) ? view.machine+' of '+view.anchor : view.machine}`;
  $('#join-replace-label').hidden = view.machine === 'alone';
  $('#joining').textContent = view.joining ? `This machine: ${view.joining.node}. Confirm it on ${view.joining.anchor}.` : '';
  $('#join').disabled = joinPending || !!view.joining;
  $('#pair-full').disabled = $('#pair-light').disabled = !view.on || !['alone','anchor'].includes(view.machine);
  $('#pair-code').value = view.code?.code || '';
  $('#pair-expiry').textContent = view.code ? `Valid until ${new Date(view.code.expiresAt).toLocaleString()}` : '';
  $('#dht-on').checked = view.dht;
  for (const selector of ['#relay-url','#relay-add','#dht-on']) $(selector).disabled = !view.on;
  if (document.activeElement !== $('#machines-expiry')) $('#machines-expiry').value = view.expiresDays;
  const signature = JSON.stringify([view.machines,view.relays,view.problems]);
  if (signature !== lastView) {
    lastView = signature;
    $('#machine-list').replaceChildren();
    for (const machine of view.machines) {
      const row = text($('#machine-list'), 'div', `${machine.node} · ${machine.role}${machine.you ? ' · this machine' : ''}${machine.anchor ? ' · anchor' : ''}${machine.you ? '' : ` · ${machine.online ? 'online' : 'offline'} · Last seen: ${machine.lastSeen ? new Date(machine.lastSeen).toLocaleString() : 'unknown'}`} · Last pull: ${machine.lastPull ? new Date(machine.lastPull).toLocaleString() : 'unknown'}`);
      row.dataset.node = machine.node;
      if (view.machine === 'anchor' && !machine.anchor) {
        button(row, machine.role === 'full' ? 'Make light' : 'Make full', () => hooks.action('machine_role',{node:machine.node,role:machine.role === 'full' ? 'light' : 'full'}));
        button(row, 'Remove', () => {
          const dialog = $('#machine-remove-question');
          dialog.querySelector('p').textContent = `Remove ${machine.node}? It stops being trusted when your machines learn of it.`;
          $('#machine-remove-confirm').onclick = () => {dialog.close(); hooks.action('machine_remove',{node:machine.node}).catch(hooks.showError);};
          dialog.showModal(); $('#machine-remove-cancel').focus();
        });
      }
    }
    $('#relay-list').replaceChildren();
    for (const url of view.relays) button(text($('#relay-list'),'div',url),'Remove relay',()=>hooks.action('relay_remove',{url}));
    $('#machine-problems').replaceChildren();
    for (const problem of view.problems) text($('#machine-problems'),'p',problem.text);
  }
  for (const b of $('#relay-list').querySelectorAll('button')) b.disabled = !view.on;
  if (status.identity && !status.identity.ownConfirmed) {
    const hints = await call('own_hints');
    const signature = JSON.stringify(hints);
    if (signature !== lastHints) {
      lastHints = signature; $('#own-hints').replaceChildren();
      for (const hint of hints) {
        const row = text($('#own-hints'),'p',`${hint.node}: ${hint.seq === null ? 'no statement' : 'position '+hint.seq}${hint.forked ? ' · forked' : ''}`);
        if (hint.seq !== null && !hint.forked) button(row,'Use this',()=>hooks.action('identity_confirm_hint',{node:hint.node,seq:hint.seq,id:hint.id}));
      }
    }
  } else {lastHints=''; $('#own-hints').replaceChildren();}
  return view;
}
