// DOM-only companion to the native window tests; executes the shipped page script.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
const root = new URL('../../ui/', import.meta.url);
const words = await import(`data:text/javascript,${encodeURIComponent(await readFile(new URL('words.js', root), 'utf8'))}`);
const source = (await readFile(new URL('consent.js', root), 'utf8')).replace(/^import .*;$/gm, '');
class Node {
  children = []; dataset = {}; disabled = false; checked = false; value = ''; content = '';
  constructor(tag) { this.tagName = tag; }
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
  nodes = Object.fromEntries(['#approve', '#deny', 'main', '.mark', 'h1', '#error'].map(k => [k, new Node(k)]));
  main = nodes.main; events = {}; timers = new Map(); calls = [];
  globalThis.document = { querySelector: key => nodes[key], createElement: tag => new Node(tag),
    body: {dataset:{}}, hasFocus: () => true };
  globalThis.window = {addEventListener: (event, fn) => events[event] = fn};
  globalThis.setTimeout = fn => { const id = Symbol(); timers.set(id, fn); return id; };
  globalThis.clearTimeout = id => timers.delete(id);
  globalThis.__call = async (command, args) => command === 'consent_get' ? {id:'request-id', mark:'cedar otter', request} : calls.push({command, args});
  globalThis.__words = words;
  await import(`data:text/javascript,${encodeURIComponent('const call = globalThis.__call; const {signTitle, checkedLabel, partyLabel} = globalThis.__words;\n' + source)}#${Math.random()}`);
  assert.equal(document.body.dataset.ready, 'true');
}
function expire() { for (const [id, fn] of [...timers]) { timers.delete(id); fn(); } }
const candidate = (choice, name) => ({choice, name, summary:'Book scores', version:'1.0.0',
  publisher:{name:'Frank',fingerprint:'frank-key'}, origin:'alice/books@1.0.0', installed:true, checked_at:0});
if (process.argv[2] === 'pick') {
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
