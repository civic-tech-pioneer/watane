// Runs the Library renderer with a small DOM; native tests also check WebKit focus.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import * as manager from '../../ui/manager.js';
import * as words from '../../ui/words.js';
class Node {
  children = []; dataset = {}; content = ''; className = ''; open = false;
  constructor(tag) { this.tag = tag; }
  append(...nodes) { for (const n of nodes) { n.parent = this; this.children.push(n); } }
  replaceChildren() { this.children = []; }
  set textContent(value) { this.content = String(value); this.children = []; }
  get textContent() { return this.content + this.children.map(n => n.textContent).join(''); }
  querySelectorAll(selector) {
    const matches = n => selector.startsWith('.') ? n.className === selector.slice(1) : selector === 'details[open]' ? n.tag === 'details' && n.open : n.tag === selector;
    return this.children.flatMap(n => [...(matches(n) ? [n] : []), ...n.querySelectorAll(selector)]);
  }
  querySelector(selector) { return this.querySelectorAll(selector)[0]; }
  closest(tag) { return this.tag === tag ? this : this.parent?.closest(tag); }
  focus() { document.activeElement = this; }
  scrollIntoView() { this.scrolled = true; }
  addEventListener() {}
  showModal() { this.open = true; }
}
const nodes = new Map();
const $ = selector => { if (!nodes.has(selector)) nodes.set(selector, new Node(selector)); return nodes.get(selector); };
const document = {createElement: tag => new Node(tag), createTextNode: text => Object.assign(new Node('text'), {textContent:text}), activeElement:null};
const launcher = $('#launcher');
function element(tag, text, parent, className) {
  const node = document.createElement(tag); node.textContent = text;
  if (className) node.className = className;
  parent?.append(node); return node;
}
const context = vm.createContext({document, $, launcher, element, ...manager, ...words,
  button(parent, text, className, callback) { const b = element('button', text, parent, className); b.onclick = callback; return b; },
  filterLibrary() {}, route() {}, action() {}, remove() {}, showError(error) { throw error; }});
const source = readFileSync(new URL('../../ui/app.js', import.meta.url), 'utf8');
vm.runInContext(source.slice(source.indexOf("let lastMods ="), source.indexOf("$('#dependency-cancel').onclick")), context);
const mod = (local, extra = {}) => ({local, name:local.toUpperCase(), version:'1.0.0', enabled:true, page:'page', grants:[], links:[], ...extra});
const render = mods => { context.mods = mods; vm.runInContext('modsView(mods)', context); };
if (process.argv[2] === 'main') {
  render([mod('a', {main:true}), mod('b')]);
  assert.equal(launcher.querySelectorAll('.main-mark').length, 1);
  assert.equal(launcher.children[0].querySelector('.main-mark').textContent, 'Opens at start');
  assert.equal(launcher.children[1].querySelector('.main-mark'), undefined);
  render([mod('a'), mod('b')]);
  assert.equal(launcher.querySelectorAll('.main-mark').length, 0);
}
if (process.argv[2] === 'focus') {
  render([mod('a'), mod('b', {links:[{kind:'call',provider:'a',granted:true}]})]);
  const [a, b] = launcher.children;
  const link = a.querySelector('a');
  assert.equal(link.textContent, 'b');
  document.activeElement = link;
  let prevented = false;
  link.onclick({preventDefault() { prevented = true; }});
  assert.equal(b.querySelector('details').open, true);
  assert.equal(document.activeElement.closest('section'), b);
  assert.equal(prevented, true, 'the anchor must not move focus back to #mods');
}
if (process.argv[2] === 'warning') {
  for (const [kinds, sentences] of [
    [['subscribe'], 'These mods extend it: y.'],
    [['call'], 'These mods use it: y.'],
    [['subscribe','call','call'], 'These mods use it: y. These mods extend it: y.'],
  ]) {
    for (const verb of ['Disable', 'Remove']) {
      render([mod('x'), mod('y', {links:kinds.map(kind => ({kind,provider:'x',granted:true}))}),
        mod('off', {enabled:false,links:[{kind:'call',provider:'x',granted:true}]}),
        mod('withdrawn', {links:[{kind:'subscribe',provider:'x',granted:false}]})]);
      launcher.children[0].querySelector(verb === 'Disable' ? '.disable' : '.remove').onclick();
      assert.equal($('#dependency-dialog p').textContent, `${verb} X? ${sentences}`);
    }
  }
}
