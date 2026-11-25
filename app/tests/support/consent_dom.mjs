// DOM-only companion to the native window tests; executes the shipped page script.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
const root = new URL('../../ui/', import.meta.url);
const words = await import(`data:text/javascript,${encodeURIComponent(await readFile(new URL('words.js', root), 'utf8'))}`);
const source = (await readFile(new URL('consent.js', root), 'utf8')).replace(/^import .*;$/gm, '');
class Node {
  children = []; dataset = {}; disabled = false; checked = false; value = ''; content = '';
  attributes = new Map();
  constructor(tag) { this.tagName = tag; }
  setAttribute(name, value) { this.attributes.set(name, String(value)); }
  getAttribute(name) { return this.attributes.get(name) ?? null; }
  append(...nodes) { this.children.push(...nodes); }
  set textContent(value) { this.content = String(value); this.children = []; }
  get textContent() { return this.content + this.children.map(n => n.textContent).join(' '); }
  focus() { document.activeElement = this; }
  click() {
    if (this.disabled) return;
    if (this.type === 'radio') {
      for (const n of all(main)) if (n.type === 'radio') n.checked = false;
      this.checked = true;
      this.onchange?.();
    }
    return this.onclick?.();
  }
}
function all(node) { return [node, ...node.children.flatMap(all)]; }
let main, nodes, events, timers, calls;
async function load(request) {
  nodes = Object.fromEntries(['#approve', '#deny', '#stop', 'main', '.mark', 'h1', '#error'].map(k => [k, new Node(k)]));
  main = nodes.main; events = {}; timers = new Map(); calls = [];
  globalThis.document = { querySelector: key => nodes[key], createElement: tag => new Node(tag),
    body: {dataset:{}}, hasFocus: () => true };
  globalThis.window = {addEventListener: (event, fn) => events[event] = fn};
  globalThis.setTimeout = fn => { const id = Symbol(); timers.set(id, fn); return id; };
  globalThis.clearTimeout = id => timers.delete(id);
  globalThis.__call = async (command, args) => command === 'consent_get' ? {id:'request-id', mark:'cedar otter', request} : calls.push({command, args});
  globalThis.__words = words;
  await import(`data:text/javascript,${encodeURIComponent('const call = globalThis.__call; const {signTitle, checkedLabel, partyLabel, askerLabel, manageTitle} = globalThis.__words;\n' + source)}#${Math.random()}`);
  assert.equal(document.body.dataset.ready, 'true');
  assert.equal(nodes['#deny'].getAttribute('aria-label'), `Deny: ${nodes.h1.textContent}`);
  assert.equal(nodes['#deny'].title, nodes['#deny'].getAttribute('aria-label'));
}
function expire() { for (const [id, fn] of [...timers]) { timers.delete(id); fn(); } }
const candidate = (choice, name) => ({choice, name, summary:'Book scores', version:'1.0.0',
  publisher:{name:'Frank',fingerprint:'frank-key'}, origin:'alice/books@1.0.0', installed:true, checked_at:0});
if (process.argv[2] === 's82') {
  for (const name of ['Desk', '<img src=x onerror=alert(1)>']) {
    const asked_by = {local:'desk',name,publisher:null};
    const link = '<img src=x onerror=alert(1)>';
    for (const kind of ['read','install','pick','manage']) {
      await load({kind,asked_by,link,release:'channel/books@1.0.0',mods:[],consumer:'Notes',provider:'Weather',cause:'offline',candidates:[],action:'enable',target:{name:'Weather',version:'1.0.0',publisher:null},users:[]});
      assert.equal(main.children[0].textContent, `Asked by ${name} (local), which manages your mods`);
      if (kind === 'read') {
        assert.equal(nodes.h1.textContent, `${name} asks to read ${link}`);
        assert.equal(main.children[1].textContent, 'To install channel/books@1.0.0, this home reads the channel of this link. It has not read it before.');
      }
      assert.equal(nodes['#stop'].textContent, `Deny and stop asks from ${name}`);
      assert.equal(nodes['#stop'].hidden, false);
      assert.equal(document.activeElement, nodes['#deny']);
      assert.equal(nodes['#approve'].disabled, true);
      assert.equal(all(main).filter(n => n.tagName === 'img').length, 0);
      await nodes['#stop'].click();
      assert.deepEqual(calls[0], {command:'consent_answer',args:{id:'request-id',approve:false,choice:null,withheld:[],stop:true}});
    }
  }
  await load({kind:'install',mods:[]});
  assert.equal(nodes['#stop'].hidden, true);
} else if (process.argv[2] === 'manage') {
  const asked_by = {local:'desk',name:'Desk',publisher:null};
  const target = {local:'weather',name:'Weather',version:'1.2.3',publisher:null};
  for (const kind of ['install','pick']) {
    await load({kind,asked_by,mods:[],consumer:'Notes',provider:'Weather',cause:'offline',candidates:[]});
    assert.equal(main.children[0].textContent, 'Asked by Desk (local), which manages your mods');
  }
  for (const [action,title,data,users] of [
    ['enable','Desk asks to turn on Weather',null,[]],
    ['grant','Desk asks to give Weather a grant',null,[]],
    ['remove','Desk asks to remove Weather','Its data is kept. Installing it again brings it back.',['Notes']],
    ['remove_data','Desk asks to remove Weather and delete its data','Its data is deleted. This cannot be undone.',[]]
  ]) {
    await load({kind:'manage',asked_by,action,target,users,grant:{text:'Keep its own data on this device'}});
    assert.equal(nodes.h1.textContent,title);
    assert.equal(main.children[0].textContent, 'Asked by Desk (local), which manages your mods');
    for (const text of ['Weather','1.2.3','local',...(data ? [data] : []),...(users.length ? ['Used by: Notes'] : [])]) assert.ok(main.textContent.includes(text),text);
    assert.equal(main.textContent.includes('Used by:'),!!users.length);
    if (action === 'grant') assert.ok(main.textContent.includes('Keep its own data on this device'));
    assert.equal(all(main).filter(n => n.type === 'checkbox').length,0);
    expire(); await nodes['#approve'].click();
    assert.deepEqual(calls[0].args,{id:'request-id',approve:true,choice:null,withheld:[]});
  }
  asked_by.name = '<img src=x onerror=alert(1)>';
  for (const kind of ['install','pick','manage']) {
    await load({kind,asked_by,action:'enable',target,mods:[],consumer:'Notes',provider:'Weather',cause:'offline',candidates:[]});
    assert.ok((main.textContent + nodes.h1.textContent).includes(asked_by.name));
    assert.equal(all(main).filter(n => n.tagName === 'img').length,0);
  }
} else if (process.argv[2] === 's113') {
  const short = 'Call books of Ratings';
  const exact = 'Call mods:a/books@0.1.0 of Ratings';
  const collision = 'Call mods:b/books@0.1.0 of Ratings';
  await load({kind:'install',mods:[{name:'Notes',version:'1.0.0',local:'notes',publisher:null,uses:[],
    grants:[{short,text:exact,key:'one',new:true},{short:'',text:collision,key:'two'}]}]});
  const labels = all(main).filter(node => node.tagName === 'label');
  assert.equal(labels.length, 2);
  assert.ok(labels[0].textContent.includes(short));
  assert.ok(labels[0].textContent.includes(exact));
  assert.ok(labels[0].textContent.includes('new'));
  assert.equal(labels[1].textContent.trim(), `Allow: ${collision}`);
  await load({kind:'manage',asked_by:{name:'Desk',local:'desk',publisher:null},action:'grant',
    target:{name:'Notes',version:'1.0.0',publisher:null},users:[],grant:{short,text:exact}});
  assert.ok(main.textContent.includes(short));
  assert.ok(main.textContent.includes(exact));
} else if (process.argv[2] === 's118') {
  const storage = 'Keep its own data on this device';
  await load({kind:'install',mods:[{name:'Notes',version:'1.0.0',local:'notes',publisher:null,uses:[],
    grants:[{key:'storage',kind:'storage',text:storage,granted:true,new:false,kept:true},
      {key:'http:example.com',kind:'http',text:'Connect to example.com',granted:true,new:true}]}]});
  const labels = all(main).filter(node => node.tagName === 'label');
  assert.equal(labels.length, 2);
  assert.equal(labels[0].textContent.trim(), `Allow: ${storage} · data kept from before`);
  assert.ok(!labels[0].textContent.includes('· new'));
  assert.equal(labels[1].textContent.trim(), 'Allow: Connect to example.com · new');
} else if (process.argv[2] === 'main') {
  for (const becomes_main of [true, false, undefined]) {
    await load({kind:'install',by_program:false,mods:[{name:'Desk',version:'1.0.0',local:'desk',publisher:null,
      grants:[],uses:[],becomes_main}]});
    const line = 'Desk opens when watane starts, in place of the manager.';
    assert.equal(all(main).some(n => n.content === line), becomes_main === true);
  }
} else if (process.argv[2] === 'sign_first') {
  const warning = 'After this, this home cannot join your other machines.';
  for (const first of [true, false]) {
    await load({kind:'sign',statement:'release',signer:{name:'Carol',fingerprint:'carol-key',you:true},
      details:[{label:'Made from',value:'alice/books@1.0.0'}, ...(first ? [{label:'',value:warning}] : [])]});
    assert.equal(all(main).some(n => n.content === warning), first);
    assert.ok(all(main).some(n => n.content === 'Made from: alice/books@1.0.0'));
  }
} else if (process.argv[2] === 'pick') {
  const request = {kind:'pick',consumer:'Dave stats',provider:'Carol ratings',cause:'<img src=x onerror=alert(1)>',
    candidates:[candidate('first','Ratings'), candidate('second','<img src=x onerror=alert(1)>')]};
  await load(request);
  assert.equal(nodes.h1.textContent, 'Choose a mod for Dave stats');
  for (const field of ['Carol ratings is not available:', request.cause, 'Ratings', '1.0.0', 'Frank (frank-key)',
    'Book scores','made from alice/books@1.0.0','installed','checked']) assert.ok(main.textContent.includes(field), field);
  assert.equal(all(main).filter(n => n.tagName === 'img').length, 0);
  const radios = all(main).filter(n => n.type === 'radio');
  assert.equal(radios.length, 2);
  assert.ok(radios.every(r => !r.checked));
  expire(); assert.equal(nodes['#approve'].disabled, true);
  radios[1].click(); assert.equal(nodes['#approve'].disabled, false);
  events.blur(); assert.equal(nodes['#approve'].disabled, true);
  events.focus(); assert.equal(nodes['#approve'].disabled, true);
  expire(); assert.equal(nodes['#approve'].disabled, false);
  await nodes['#approve'].click();
  assert.deepEqual(calls, [{command:'consent_answer',args:{id:'request-id',approve:true,choice:'second',withheld:[]}}]);
  await load(request); await nodes['#deny'].click();
  assert.deepEqual(calls[0].args, {id:'request-id',approve:false,choice:null,withheld:[]});
} else if (process.argv[2] === 'regress_s58') {
  // S58 AC2: a value that starts with a release reference starts with the readable part;
  // the reference follows at the end. Seen live: '<key>/reading-list@0.1.0 by Alice (6gduw4q4p6) · signature verified'.
  const release = 'c1prermeecwogtb8mo8wra1h76noou5q87ffp86o6wqreycrigoy/reading-list@0.1.0';
  await load({kind:'install',by_program:false,mods:[{name:'Reading list',version:'0.1.0',local:'reading-list',slug:'reading-list',
    release,publisher:{name:'Alice',fingerprint:'c1prermeec'},grants:[],uses:[],binds:[]}]});
  const line = all(main).map(n => n.content).find(t => t.includes(release));
  assert.ok(line, 'the release reference stays visible');
  assert.ok(!line.startsWith(release), `starts with the reference: ${line}`);
  assert.ok(line.indexOf('Alice (c1prermeec)') < line.indexOf(release), `name after reference: ${line}`);
  assert.equal(line, `reading-list 0.1.0 by Alice (c1prermeec) · signature verified — ${release}`);
} else {
  await load({kind:'install',by_program:false,mods:[{name:'Stats',version:'1.0.0',local:'stats',publisher:null,
    grants:[],uses:[],origin:'alice/books@1.0.0',binds:['Frank in place of Carol (your pick)']}]});
  assert.ok(main.textContent.includes('made from alice/books@1.0.0 (the publisher says so)'));
  assert.ok(main.textContent.includes('Frank in place of Carol (your pick)'));
  expire(); await nodes['#approve'].click(); assert.equal(calls[0].args.choice, null);
  await load({kind:'sign',statement:'release',signer:{name:'Carol',fingerprint:'carol-key'},
    details:[{label:'Made from',value:'alice/books@1.0.0 by Alice (alice-key)'}]});
  assert.ok(main.textContent.includes('Made from: alice/books@1.0.0 by Alice (alice-key)'));
  expire(); await nodes['#approve'].click(); assert.equal(calls[0].args.choice, null);
  await load({kind:'sign',statement:'profile',signer:{name:'Carol',fingerprint:'carol-key',you:true},details:[]});
  assert.equal(main.textContent, 'you');
}
