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
if (which === 'ac5') {
  const a=mod('a',{name:'Alpha'}), b=mod('b',{name:'Beta',links:[{kind:'subscribe',provider:'a',granted:true}]}), c=mod('c',{name:'Charlie'});
  render([a,b,c]);
  for (const [locals, text] of [[['b'],'Beta is installed.'],[['a','b'],'Alpha and Beta are installed.'],[['a','b','c'],'Alpha, Beta and Charlie are installed.']]) {
    context.locals=locals; vm.runInContext('installedNote(currentMods.filter(m=>locals.includes(m.local)))',context);
    assert.equal($('#installed-note strong').textContent,text);
    if (locals.length === 1) { assert.equal($('#installed-note .hint').tag,'span'); assert.equal($('#installed-note .hint').textContent,'It adds to Alpha.'); }
    else assert.equal($('#installed-note .hint'),null);
  }
  assert.equal($('.library-list').children[0],$('#installed-note'));
}
if (which === 'ac6') {
  render([mod('a',{main:true})]); entry('a').querySelector('.remove').click(); await tick();
  assert.equal($('#confirm-panel').hidden,false);
  assert.equal(entry('a').querySelector('.actions').hidden,true);
  assert.equal(entry('a').querySelector('.start-row').hidden,false);
  $('#confirm-cancel').click(); assert.equal(entry('a').querySelector('.actions').hidden,false);
}
if (which === 'update-band' || which === 'look') {
  const offer='key/book@0.4.0';
  const grants=[{short:'Connect to openlibrary.org',text:'Full network words'}, {short:'',text:'Keep its own data'}, {short:'Call Notes',text:'Full call words'}];
  for (const [offerGrants, expected] of [
    [[], 'Version 0.4.0 is available.'],
    [grants.slice(0,1), 'Version 0.4.0 asks to connect to openlibrary.org.'],
    [grants.slice(0,2), 'Version 0.4.0 asks to connect to openlibrary.org and keep its own data.'],
    [grants, 'Version 0.4.0 asks to connect to openlibrary.org, keep its own data and call Notes.'],
  ]) {
    render([mod('book',{offer,offerGrants})]);
    assert.equal(entry('book').querySelector('.notice span').textContent,expected);
    assert.equal(entry('book').querySelector('.notice .offer').textContent,words.offerLabel(offer,offerGrants));
  }
  render([mod('book',{offer})]);
  assert.equal(entry('book').querySelector('.notice span').textContent,'Version 0.4.0 is available.');
  for (const extra of [{notice:'Publisher notice'}, {notice:'Publisher notice',error:'Exact error'}]) {
    render([mod('book',{offer,offerGrants:grants,...extra})]);
    assert.equal(entry('book').querySelector('.notice span').textContent,extra.error || extra.notice);
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
  assert.equal(entry('b').querySelector('.b-off'),null);
  assert.equal(entry('b').querySelector('.notice span').textContent,'Withdrawn by the publisher: broken');
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

if (['find-updates','ac11','ac12'].includes(which)) {
  vm.runInContext(source.slice(source.indexOf("let lastChannels ="), source.indexOf("$('#identity-backup').onclick")).replaceAll('export function ', 'function '),context);
  vm.runInContext(source.slice(source.indexOf("const dismissedOffers ="), source.indexOf('window.wataneDropped')).replaceAll('export function ', 'function '),context);
  if (which === 'ac11') {
    render([mod('book',{version:'1.0.0'})]);
    const alice={name:'Alice',fingerprint:'alicefp'}, ivy={name:'Ivy',fingerprint:'ivyfp'};
    context.hits=[{slug:'book',name:'Book',version:'2.0.0',release:'key/book@2',publisher:alice,status:'ok',verified:true,installed:'book',via:[{kind:'index',index:alice,through:ivy}],checkedAt:null,uncheckedSince:null}];
    vm.runInContext('renderResults(hits)',context);
    const row=$('#results article');
    assert.equal(row.querySelector('.installed-link').textContent,'Installed');
    assert.equal(row.querySelector('.install-release'),null);
    assert.equal(row.querySelector('.badge').textContent,'You have 1.0.0');
    assert.ok(row.textContent.includes('listed by Alice through Ivy'));
    assert.equal(words.viaLabel({kind:'index',index:{...alice,you:true}}),'listed by you');
    assert.equal(words.viaLabel({kind:'index',index:{fingerprint:'fallback'}}),'listed by fallback');
    process.exit(0);
  }
  if (which === 'ac12') {
    context.offered=[mod('tags',{notice:'Withdrawn by the publisher: broken',offer:'key/tags@2',offerGrants:[]})];
    vm.runInContext('renderUpdates(offered, [], [], [])',context);
    assert.equal($('#update-offers .not-now'),null);
    assert.ok($('#update-offers .offer'));
    let finish; context.call=()=>new Promise(resolve=>finish=resolve);
    vm.runInContext('knownChannels=[]; knownIndexes=[]',context);
    const pending=vm.runInContext('refreshChannels()',context);
    assert.equal($('#checked-at').textContent,'Checking…');
    finish([]); await pending;
    process.exit(0);
  }
  const ivy={channel:'ivy-key',name:'Ivy',fingerprint:'ivyfp',status:'ok',base:'https://ivy.test/'};
  const index={...ivy,mods:6,own:false,included:[],listings:[],skipped:[]};
  context.indexes=[index]; context.hits=[]; context.channels=[ivy];
  vm.runInContext('renderIndexes(indexes); renderResults(hits);',context);
  assert.equal($('#search-text').getAttribute('placeholder'),'Search by words, publisher or interface');
  for (const id of ['search-publisher','search-interface','refresh-find']) assert.equal($('#'+id),null);
  const chip=$('.ix[data-channel=ivy-key]'); assert.ok(chip); assert.ok(chip.textContent.includes('Ivy · 6 mods'));
  assert.equal(chip.querySelector('.drop-index').getAttribute('aria-label'),'Stop using Ivy as index');
  assert.equal($('#result-count').textContent,'No mods found');
  $('#show-index').click(); assert.equal($('#index-entry').hidden,false); assert.equal(document.activeElement,$('#index-link'));
  render([mod('book',{name:'Book',version:'1.0.0'})]);
  const hit={slug:'book',name:'Book',version:'2.0.0',release:'ivy-key/book@2.0.0',summary:'Read together <img>',publisher:ivy,status:'ok',verified:true,installed:'book',checkedAt:Date.now()-180000,uncheckedSince:null,via:[],offers:[],events:[],uses:[]};
  context.hits=[{...hit},{...hit,version:'1.0.0'},{...hit,slug:'bad',installed:null,status:'compromised',verified:false}];
  vm.runInContext('renderResults(hits)',context);
  assert.equal($('#result-count').textContent,'3 mods found');
  $('#results .installed-link').click();
  assert.equal(context.location.hash,'#mods'); assert.equal(tile('book').getAttribute('aria-pressed'),'true');
  assert.equal($('#results article.result').querySelector('.installed-link').textContent,'Installed');
  assert.ok($('#results').textContent.includes('You have 1.0.0')); assert.ok($('#results').textContent.includes('read 3 min ago'));
  assert.ok($('#results .installed-link')); const blocked=$('#results .compromised');
  assert.equal(blocked.querySelector('button'),null);
  assert.equal(blocked.querySelector('.blocked').textContent,"Can't install: Ivy's channel no longer matches what it said before, so watane can't trust it. Ask Ivy for the link to a new channel.");
  assert.equal($('#find img'),null); assert.ok($('#channels')); assert.ok($('#follow-link')); assert.ok($('#follow-channel'));
  context.indexes=[{...index,status:'failed',cause:'Could not read Ivy: full: nested cause',skipped:[{party:ivy,cause:'whole skipped cause'}]}];
  vm.runInContext('renderIndexes(indexes)',context);
  assert.ok($('#indexes').textContent.includes('could not be read'));
  assert.ok($('#index-notices').textContent.includes('Could not read Ivy: full: nested cause'));
  assert.ok($('#index-notices').textContent.includes('Skipped Ivy (ivyfp): whole skipped cause'));
  context.indexes=[{...index,status:'compromised'}, {...index,channel:'own-key',own:true,mods:1}];
  vm.runInContext('renderIndexes(indexes)',context);
  assert.ok($('#indexes').textContent.includes('compromised'));
  assert.ok($('#indexes').textContent.includes('You · 1 mod'));
  assert.equal($('#no-index').hidden,true);
  context.indexes=[]; vm.runInContext('renderIndexes(indexes)',context); assert.equal($('#no-index').hidden,false);

  context.offered=[mod('book',{name:'Book',offer:'ivy-key/book@2.0.0',offerGrants:[{kind:'network',short:'Connect to books.test',text:'Full grant words'}]}),mod('tags',{name:'Tags',notice:'Withdrawn by the publisher: loses tags',offer:'ivy-key/tags@2.0.0',offerGrants:[]}),mod('bad',{notice:"The publisher's channel is compromised",offer:null,offerGrants:[]})];
  context.offered[1].links=[]; context.offered[1].release='ivy-key/tags@1.0.0'; context.offered[1].publisher={name:'Ivy',fingerprint:'ivyfp'};
  context.offered.push(mod('notes',{name:'Notes',links:[{kind:'call',provider:'tags',granted:true}]}));
  context.failed=[{...ivy,status:'failed',cause:'whole: nested cause',readAt:Date.now()-172800000}];
  context.done=[{local:'older',name:'Older',from:'1',to:'2',at:1},{local:'newer',name:'Newer',from:'1',to:'2',at:2}];
  vm.runInContext('renderUpdates(offered, done, failed, [])',context);
  assert.equal($('#updates h1').textContent,'Updates');
  assert.deepEqual($('#updates').querySelectorAll('h2').map(n=>n.textContent),['Needs you','Channels','Updated by itself']);
  assert.equal($('#updates-count').textContent,'3');
  assert.equal($('#update-offers .newgrant').textContent,'Connect to books.test'); assert.ok($('#update-offers .newgrant svg'));
  assert.ok($('#update-offers').textContent.includes('Not now keeps 1.0.0 and hides this until the offer changes or watane restarts.'));
  assert.equal($('#update-offers .offer').textContent,'Review…');
  const review=$('#update-offers .offer'); vm.runInContext('renderUpdates(offered, done, failed, [])',context); assert.equal($('#update-offers .offer'),review,'a poll must preserve the focused control');
  assert.ok($('#update-offers').textContent.includes('Notes uses it.'));
  assert.ok($('#update-offers').textContent.includes('2.0.0 is the newest version that is not withdrawn and asks for nothing new.'));
  assert.equal($('#update-offers [data-local=bad]').querySelector('button'),null);
  $('#update-offers .not-now').click(); await tick();
  assert.equal($('#updates-count').textContent,'2'); assert.equal($('#update-offers [data-local=book]'),null);
  vm.runInContext('renderUpdates(offered, done, failed, [])',context); assert.equal($('#updates-count').textContent,'2');
  context.offered[0].notice='Update to 2.0.0 failed: offline'; vm.runInContext('renderUpdates(offered, done, failed, [])',context); assert.equal($('#updates-count').textContent,'2','unrelated failures do not change the offer');
  context.offered[0].offer='ivy-key/book@3.0.0';
  vm.runInContext('renderUpdates(offered, done, failed, [])',context); assert.equal($('#updates-count').textContent,'3');
  for(const text of ["Couldn't read Ivy's channel",'Tags comes from it, so its updates wait.','watane tries again at the next check.','Last read 2 days ago.','Cause: whole: nested cause','Try again']) assert.ok($('#update-channels').textContent.includes(text),text);
  assert.equal($('#update-done').children[0].dataset.local,'newer');
  context.call=(cmd,args={})=>{calls.push({cmd,args}); return Promise.resolve(cmd==='indexes_list' ? [index] : []);};
  context.readRefresh=()=>vm.runInContext('findView()',context);
  context.indexes=[index]; vm.runInContext('renderIndexes(indexes)',context);
  $('#search-text').value='book ivy'; $('#search').click(); await tick();
  assert.equal($('#search-reading').textContent,'By Ivy');
  let queryCalls=calls.filter(c=>c.cmd==='search');
  assert.deepEqual(JSON.parse(JSON.stringify(queryCalls.at(-1).args)),{text:'book',publisher:'ivy-key',interface:null});
  await vm.runInContext('findView()',context);
  assert.equal(calls.filter(c=>c.cmd==='search').length,queryCalls.length+1,'refresh reruns the last search');
  $('#search-text').value='book mods:books/list'; $('#search-text').onkeydown({key:'Enter',preventDefault(){}}); await tick();
  assert.equal($('#search-reading').textContent,'Offering mods:books/list');
  assert.deepEqual(JSON.parse(JSON.stringify(calls.filter(c=>c.cmd==='search').at(-1).args)),{text:'book',publisher:null,interface:'mods:books/list'});
  context.location.hash='#updates'; vm.runInContext('route()',context);
  let finishCheck;
  context.call=(cmd,args={})=>{calls.push({cmd,args});return cmd==='channels_refresh' ? new Promise(resolve=>finishCheck=resolve) : Promise.resolve([]);};
  vm.runInContext('knownChannels = channels; knownIndexes = []; checkedAt = Date.now()-180000; renderCheck()',context);
  assert.equal($('#checked-at').getAttribute('role'),'status');
  assert.equal($('#checked-at').textContent,'Checked 3 min ago');
  const pending=vm.runInContext('refreshChannels()',context);
  assert.equal($('#refresh-channels').disabled,true);
  assert.equal($('#refresh-channels .refresh-label').textContent,'Checking…');
  assert.equal($('#refresh-channels .refresh-glyph').classList.contains('spin'),true);
  assert.equal($('#checked-at').textContent,'Reading 1 channel…');
  vm.runInContext("knownChannels = [...channels, {channel:'other'}]; renderCheck()",context);
  assert.equal($('#checked-at').textContent,'Reading 2 channels…');
  finishCheck([]); await pending;
  assert.equal($('#refresh-channels').disabled,false);
  assert.equal($('#refresh-channels .refresh-label').textContent,'Check now');
  assert.equal($('#refresh-channels .refresh-glyph').classList.contains('spin'),false);
  assert.equal($('#checked-at').textContent,'Checked just now');
  let finishRead;
  context.refresh=()=>new Promise(resolve=>finishRead=resolve);
  const secondCheck=vm.runInContext('refreshChannels()',context);
  finishCheck([]); await tick();
  assert.equal($('#refresh-channels').disabled,false,'Checking ends when channels_refresh returns');
  finishRead(); await secondCheck;
  context.location.hash='#find'; vm.runInContext('route()',context); $('#add-mod').click();
  assert.equal(document.activeElement,$('#install-link'));
  vm.runInContext('renderUpdates([], [], [], [])',context);
  assert.equal($('#update-offers').textContent,'Nothing needs you.');
  assert.equal($('#update-channels').parent.hidden,true);
  assert.equal($('#update-done').textContent,'None since the app started.');
}
