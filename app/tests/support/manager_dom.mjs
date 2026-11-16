// Executes the page's own renderer; native look tests additionally prove WebKit focus and theme.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import * as manager from '../../ui/manager.js';
import * as words from '../../ui/words.js';
class Node {
  children = []; dataset = {}; attrs = {}; content = ''; className = ''; hidden = false; value = ''; checked = false;
  constructor(tag) { this.tag = tag; }
  append(...nodes) {
    for (let node of nodes) {
      if (typeof node === 'string') node = Object.assign(new Node('text'), {content:node});
      if (node.parent) node.parent.children = node.parent.children.filter(n => n !== node);
      node.parent = this; this.children.push(node);
    }
  }
  after(node) { const p = this.parent; p.append(node); p.children.splice(p.children.indexOf(this)+1,0,p.children.pop()); }
  replaceChildren() { for (const n of this.children) n.parent = null; this.children = []; this.content = ''; }
  set textContent(value) { this.content = String(value); this.children = []; }
  get textContent() { return this.content + this.children.map(n => n.textContent).join(''); }
  setAttribute(key, value) { this.attrs[key] = String(value); if (key === 'class') this.className = value; if (key === 'id') this.id = value; if (key.startsWith('data-')) this.dataset[key.slice(5).replace(/-([a-z])/g,(_,c)=>c.toUpperCase())] = value; }
  getAttribute(key) { return key === 'class' ? this.className : this.attrs[key] ?? null; }
  removeAttribute(key) { delete this.attrs[key]; }
  get classList() { return {toggle:(name,on)=>{ const parts = new Set(this.className.split(' ').filter(Boolean)); if (on ?? !parts.has(name)) parts.add(name); else parts.delete(name); this.className=[...parts].join(' '); }, contains:name=>this.className.split(' ').includes(name)}; }
  matches(selector) {
    return selector.split(',').some(sel => {
      sel = sel.trim();
      const bits = sel.split(/\s+(?![^[]*\])/); const last = bits.pop();
      if (bits.length) { if (!this.matches(last)) return false; let p = this.parent; while(p) { if(p.matches(bits.join(' '))) return true; p=p.parent; } return false; }
      const tag = sel.match(/^[\w-]+/); if (tag && this.tag !== tag[0]) return false;
      for (const [,id] of sel.matchAll(/#([\w-]+)/g)) if(this.id !== id) return false;
      for (const [,cls] of sel.matchAll(/\.([\w-]+)/g)) if(!this.className.split(' ').includes(cls)) return false;
      for (const [,key,,value] of sel.matchAll(/\[([\w-]+)(?:=(['"]?)([^\]'"]+)\2)?\]/g)) {
        const actual = key.startsWith('data-') ? this.dataset[key.slice(5).replace(/-([a-z])/g,(_,c)=>c.toUpperCase())] : key === 'hidden' ? (this.hidden ? '' : undefined) : this.getAttribute(key);
        if (value === undefined ? actual == null : String(actual) !== value) return false;
      }
      return true;
    });
  }
  querySelectorAll(selector) { return this.children.flatMap(n => [...(n.matches(selector) ? [n] : []), ...n.querySelectorAll(selector)]); }
  querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
  closest(selector) { return this.matches(selector) ? this : this.parent?.closest(selector); }
  focus() { document.activeElement = this; }
  scrollIntoView() {}
  listeners = {};
  addEventListener(type, listener) { (this.listeners[type] ||= []).push(listener); }
  dispatchEvent(event) { for (const listener of this.listeners[event.type] || []) listener(event); }
  click() { return this.onclick?.(); }
}
const document = {
  body:new Node('body'), activeElement:null,
  createElement:tag=>new Node(tag), createElementNS:(_,tag)=>new Node(tag), createTextNode:text=>Object.assign(new Node('text'),{content:text}),
  querySelector(selector) { return this.body.querySelector(selector); }, querySelectorAll(selector) { return this.body.querySelectorAll(selector); }, listeners:{}, addEventListener(type,listener) { (this.listeners[type] ||= []).push(listener); }, dispatchEvent(event) { for(const listener of this.listeners[event.type] || []) listener(event); }
};
// Read only the HTML, never a specification, to preserve the actual static ids.
const html = readFileSync(new URL('../../ui/index.html', import.meta.url),'utf8').split('<body>')[1].split('</body>')[0];
const stack = [document.body];
for (const token of html.matchAll(/<\/?([\w-]+)([^>]*)>|([^<]+)/g)) {
  if (token[3]) { stack.at(-1).append(token[3]); continue; }
  const [raw,tag,attrs] = token;
  if (raw.startsWith('</')) { if (stack.length>1) stack.pop(); continue; }
  const node = new Node(tag);
  for (const [,key,,value] of attrs.matchAll(/([\w-]+)(?:=(['"])(.*?)\2)?/g)) { node.setAttribute(key,value ?? ''); if (key === 'hidden') node.hidden = true; if(key === 'checked') node.checked = true; }
  stack.at(-1).append(node);
  if (!['input','meta','link','hr','path','rect'].includes(tag) && !raw.endsWith('/>')) stack.push(node);
}
const $ = selector => document.querySelector(selector);
const launcher = $('#launcher');
function element(tag, text, parent, className) { const n = document.createElement(tag); n.textContent = text; n.className = className || ''; parent?.append(n); return n; }
const calls=[]; let commandResult={}, nextMods=null;
const context = vm.createContext({document, $, launcher, element, ...manager, ...words,
  // monogram closes over its module's global document.
  monogram(mod,size) { globalThis.document=document; return words.monogram(mod,size); },
  location:{hash:'#mods'}, console, getComputedStyle:()=>({gridTemplateColumns:'200px 200px'}),
  call(cmd,args={}) { calls.push({cmd,args}); return Promise.resolve(cmd === 'search' ? [] : commandResult); },
  errors:{begin:()=>1,current:()=>1}, readRefresh:()=>Promise.resolve(),
  taggedError:e=>e, showError(error) { throw error; },
  refresh() { if (nextMods) { context.mods=nextMods; vm.runInContext('modsView(mods)',context); nextMods=null; } return Promise.resolve(); },
});
const source = readFileSync(new URL('../../ui/app.js',import.meta.url),'utf8');
vm.runInContext(source.slice(source.indexOf('async function action('),source.indexOf('let lastTools =')).replace('export function modsView','function modsView'),context);
vm.runInContext(source.slice(source.indexOf('function filterLibrary()'),source.indexOf("window.addEventListener('hashchange'")),context);
const mod = (local, extra={}) => ({local,slug:local,name:local.toUpperCase(),version:'1.0.0',enabled:true,page:'page',grants:[],links:[],...extra});
const render = mods => {context.mods=mods;vm.runInContext('modsView(mods)',context);};
const entry = local => $(`[data-local=${local}]`);
const tile = local => $(`[data-tile=${local}]`);
const tick = async()=>{await new Promise(resolve=>setImmediate(resolve));};
const which=process.argv[2];
if (which === 'main') {
  render([mod('a',{main:true}),mod('b')]);
  assert.equal(launcher.querySelectorAll('.main-mark').length,1);
  assert.equal(tile('a').querySelector('.main-mark').textContent,'Opens at start');
  assert.equal(tile('b').querySelector('.main-mark'),null);
  render([mod('a'),mod('b')]); assert.equal(launcher.querySelectorAll('.main-mark').length,0);
}
if (which === 'focus') {
  render([mod('a'),mod('b',{links:[{kind:'call',provider:'a',granted:true}]})]);
  const chip=entry('a').querySelector('.chip'); assert.equal(chip.children.at(-1).textContent,'B');
  chip.click(); await tick();
  assert.equal(entry('b').hidden,false); assert.equal(document.activeElement,entry('b').querySelector('h2'));
}
if (which === 'warning') {
  for (const [kinds,sentences] of [[['subscribe'],'These mods extend it: Y.'],[['call'],'These mods use it: Y.'],[['subscribe','call','call'],'These mods use it: Y.These mods extend it: Y.']]) {
    for (const verb of ['Disable','Remove']) {
      render([mod('x'),mod('y',{links:kinds.map(kind=>({kind,provider:'x',granted:true}))}),mod('off',{enabled:false,links:[{kind:'call',provider:'x',granted:true}]}),mod('withdrawn',{links:[{kind:'subscribe',provider:'x',granted:false}]})]);
      entry('x').querySelector(verb === 'Disable' ? '.disable' : '.remove').click(); await tick();
      assert.equal($('#confirm-title').textContent,`${verb} X?`);
      assert.equal($('#dependency-warning').textContent,sentences);
      assert.equal(document.activeElement,$('#confirm-cancel'));
      $('#confirm-cancel').click();
    }
  }
}
if (which === 'look') {
  assert.equal($('#add-menu').hidden,true);
  $('#add-mod').click(); assert.equal($('#add-menu').hidden,false); assert.equal(document.activeElement,$('#install-link'));
  document.dispatchEvent({type:'keydown',key:'Escape'}); assert.equal($('#add-menu').hidden,true);
  $('#add-mod').click(); document.dispatchEvent({type:'click',target:document.body}); assert.equal($('#add-menu').hidden,true);
  assert.ok($('.mark').getAttribute('title').includes('Your consent mark.'));
  assert.equal($('.mark').getAttribute('aria-label'),'Consent mark of this home');
  context.location.hash='#settings'; vm.runInContext('route()',context);
  assert.equal($('#settings-header').hidden,false); assert.equal($('#manager-header').hidden,true);
  assert.ok($('#settings-header').textContent.includes('Back to mods'));
  for (const id of ['settings', 'distribution', 'identity', 'identity-info', 'identity-controls', 'releases', 'profile-name', 'set-name', 'backup', 'passphrase', 'passphrase-repeat', 'identity-backup', 'identity-restore', 'confirm-own', 'own-channel', 'never-published', 'confirm-identity', 'own-hints', 'site', 'site-dir', 'site-url', 'set-site', 'own-link', 'copy-link', 'agents', 'toolset-name', 'tools', 'save-toolset', 'toolsets', 'token-toolset', 'handle', 'issue-token', 'token-dialog', 'token-secret', 'mcp-url', 'copy-token', 'close-token', 'tokens', 'machines', 'net-on-text', 'machines-on', 'machine-info', 'pair-full', 'pair-light', 'pair-code', 'pair-copy', 'pair-expiry', 'join-code', 'join-replace-label', 'join-replace', 'join', 'joining', 'join-question', 'join-cancel', 'join-confirm', 'machine-list', 'machines-expiry', 'save-expiry', 'relay-url', 'relay-add', 'relay-list', 'dht-on', 'machine-problems', 'machine-remove-question', 'machine-remove-cancel', 'machine-remove-confirm']) assert.ok($('#'+id),id);
  context.location.hash='#mods'; vm.runInContext('route()',context);
  render([]);
  $('#first-link').value='watane:literal'; $('#first-link-go').click(); await tick(); assert.deepEqual(JSON.parse(JSON.stringify(calls.at(-1))),{cmd:'install_link',args:{link:'watane:literal'}});
  $('#first-folder').click(); await tick(); assert.equal(calls.at(-1).cmd,'install_folders');
  assert.equal($('#first-start').hidden,false); assert.equal($('#add-mod').hidden,true); assert.equal($('#filter-control').hidden,true);
  render([mod('logic',{page:null,grants:[{key:'storage',kind:'storage',text:'Full fallback words',short:'',granted:false}]})]);
  assert.equal(entry('logic').querySelector('.start-row'),null);
  assert.equal(entry('logic').querySelector('.grant-text').textContent,'Full fallback words');
  assert.equal(entry('logic').querySelector('.give').getAttribute('aria-label'),'Full fallback words');
  entry('logic').querySelector('.give').click(); await tick(); assert.deepEqual(JSON.parse(JSON.stringify(calls.at(-1))),{cmd:'grant_give',args:{local:'logic',key:'storage'}});
  const a=mod('a',{name:'Alpha',release:'abc/a@1.0.0',publisher:{name:'Alice',fingerprint:'abc'},grants:[{key:'storage',kind:'storage',text:'Exact full words',short:'Short words',granted:true}]});
  const b=mod('b',{name:'Beta',offer:'key/b@2',notice:'Withdrawn by the publisher: broken',main:true,enabled:false});
  render([a,b]); assert.equal($('#first-start').hidden,true); assert.equal(launcher.children[0],tile('b'));
  assert.equal(tile('a').querySelector('.tile-name').textContent,'Alpha'); assert.equal(tile('a').querySelector('.tile-by').textContent,'Alice · 1.0.0'); assert.equal(tile('b').querySelector('.tile-by').textContent,'local · 1.0.0'); assert.ok(tile('a').querySelector('.mono'));
  assert.equal(tile('b').querySelectorAll('.badge').length,1); assert.equal(tile('b').querySelector('.badge').textContent,'Off');
  assert.equal(entry('a').hidden,true); tile('a').click(); await tick(); assert.equal(entry('a').hidden,false);
  const grant=entry('a').querySelector('[data-key=storage]'); assert.equal(grant.querySelector('.grant-text').title,'Exact full words'); assert.equal(grant.querySelector('.grant-text').textContent,'Short words');
  assert.equal(grant.querySelector('.sw').getAttribute('aria-checked'),'true'); grant.querySelector('.withdraw').click(); await tick(); assert.deepEqual(JSON.parse(JSON.stringify(calls.at(-1))),{cmd:'grant_withdraw',args:{local:'a',key:'storage'}});
  assert.deepEqual(entry('a').children.map(n=>n.className),['detail-head','summary','actions','start-row','grants','connections','derived','technical']);
  assert.deepEqual(entry('a').querySelectorAll('.technical dt').map(n=>n.textContent),['Made from','Page origin','Package','Release','Publisher key']);
  assert.equal(entry('a').querySelectorAll('.actions .primary').length,1);
  assert.equal(entry('a').querySelector('.actions').children.at(-1).classList.contains('quiet'),true);
  assert.equal(entry('a').querySelector('.start-row .hint').textContent,'Turn on to open it at start, in place of Beta.');
  entry('a').querySelector('.main-set').click(); await tick(); assert.equal(calls.at(-1).cmd,'main_set'); assert.equal(calls.at(-1).args.local,'a');
  entry('b').querySelector('.main-set').click(); await tick(); assert.equal(calls.at(-1).args.local,null);
  entry('a').querySelector('.disable').click(); await tick(); assert.equal($('#confirm-panel').hidden,true); assert.equal(calls.at(-1).cmd,'mod_disable');
  entry('b').querySelector('.remove').click(); await tick(); assert.equal(entry('b').hidden,false); assert.equal($('#keep-data').checked,true); assert.equal(document.activeElement,$('#confirm-cancel'));
  $('#delete-data').checked=true; $('#delete-data').onchange(); assert.equal($('#confirm-continue').textContent,'Remove and delete data'); document.dispatchEvent({type:'keydown',key:'Escape'}); assert.equal($('#confirm-panel').hidden,true);
  let prevented=false; launcher.dispatchEvent({type:'keydown',key:'ArrowRight',preventDefault(){prevented=true;}}); assert.equal(prevented,true); assert.equal(document.activeElement,tile('a'));
  $('#library-filter').value='alpha alice'; $('#library-filter').oninput(); assert.equal(tile('a').hidden,false); assert.equal(tile('b').hidden,true);
  $('[data-source=local]').click(); assert.equal(tile('a').hidden,true); assert.equal($('#no-matches').hidden,false); assert.ok([...$('#mod-details').children].every(n=>n.hidden)); assert.equal($('#no-matches').textContent,'No mod matches. Look in Find'); assert.equal($('#no-matches a').getAttribute('href'),'#find');
  $('#library-filter').value=''; $('[data-source=""]').click();
  for (const cmd of ['install_link','install_folders','install_paths','install_release']) {
    render([a,b]); const c=mod('c',{name:'Charlie',links:[{kind:'subscribe',provider:'a',granted:true}]});
    commandResult={installed:['c']}; nextMods=[a,b,c];
    context.cmd=cmd; await vm.runInContext('action(cmd)',context);
    assert.equal($('#installed-note').hidden,false); assert.ok($('#installed-note').textContent.includes('Charlie is installed.')); assert.ok($('#installed-note').textContent.includes('It adds to Alpha.'));
    assert.equal(tile('c').getAttribute('aria-pressed'),'true'); assert.equal($('#library-filter').value,'');
    $('#installed-note .dismiss').click(); await tick(); assert.equal($('#installed-note').hidden,true);
    vm.runInContext('installedNote([currentMods.find(m=>m.local===selected)])',context); assert.equal($('#installed-note').hidden,false);
    commandResult={installed:[]}; await vm.runInContext('action(cmd)',context); assert.equal($('#installed-note').hidden,true);
    vm.runInContext('installedNote([currentMods.find(m=>m.local===selected)])',context);
    tile('a').click(); await tick(); assert.equal($('#installed-note').hidden,true);
  }
  render([a,b]); assert.equal(tile('a').getAttribute('aria-pressed'),'true'); assert.equal($('#installed-note').hidden,true,'external refresh produces no note');
  render([b]); assert.equal(tile('b').getAttribute('aria-pressed'),'true','first shown tile selected after removal');
  render([mod('main',{main:true})]); entry('main').querySelector('.disable').click(); await tick(); assert.equal($('#confirm-panel').hidden,false); assert.equal($('#dependency-warning').textContent,'Until you enable it, the manager opens at start.');
  document.dispatchEvent({type:'keydown',key:'Escape'}); assert.equal($('#confirm-panel').hidden,true);
}
