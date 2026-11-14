/* Page-only adversarial fixture. The Rust driver reads __report; no bridge
 * or permission is installed for the fixture itself. */
(async () => {
  const report = window.__report = {
    done: false, navTried: false, storageBefore: null, attacks: []
  };
  try {
    report.storageBefore = localStorage.getItem("hostile");
    localStorage.setItem("hostile", String(Math.random()));
  } catch (error) {
    // Storage can itself be disabled. Keep the report usable in that case.
    report.storageBefore = null;
  }

  const listener = `http://127.0.0.1:${window.fixtureListener}`;
  const targets = {
    tauri: "tauri://localhost/index.html",
    api: `http://localhost:${location.port}/api/`,
    other: `http://other.localhost:${location.port}/`,
    listener: `${listener}/fetch`
  };
  const pending = [];
  function attack(name, action) {
    pending.push((async () => {
      let timer;
      try {
        const result = await Promise.race([
          Promise.resolve().then(action),
          new Promise((_, reject) => {
            timer = setTimeout(() => reject(new Error("no observable success before timeout")), 1000);
          })
        ]);
        report.attacks.push({ name, outcome: "allowed", detail: String(result) });
      } catch (error) {
        report.attacks.push({ name, outcome: "refused", detail: String(error) });
      } finally {
        clearTimeout(timer);
      }
    })());
  }

  const commands = {
    "release_install": { release: "bad" },
    "release_withdraw": { release: "bad", reason: "hostile" },
    "status": {},
    "mods_list": {},
    "mod_open": { local: "hostile" },
    "mod_enable": { local: "hostile" },
    "mod_disable": { local: "hostile" },
    "mod_remove": { local: "hostile", deleteData: true },
    "grant_withdraw": { local: "hostile", key: "storage" },
    "grant_give": { local: "hostile", key: "storage" },
    "install_folders": {},
    "tools_list": {},
    "toolsets_list": {},
    "toolset_remove": { name: "reader" },
    "toolset_save": { name: "hostile", entries: [] },
    "tokens_list": {},
    "token_issue": { toolset: "hostile", handle: "hostile" },
    "token_end": { id: "0000000000000000" },
    "install_link": { link: "watane:yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy@http://localhost/" },
    "identity_set_name": { name: "Hostile" },
    "site_set_url": { url: "http://localhost/" },
    "identity_backup": { passphrase: "hostile-fixture-passphrase" },
    "identity_restore": { passphrase: "hostile-fixture-passphrase" },
    "identity_confirm_own": { link: null },
    "channels_list": {},
    "channel_follow": { link: "watane:yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy@http://localhost/" },
    "channel_unfollow": { channel: "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy" },
    "channels_refresh": {},
    "indexes_list": {},
    "index_choose": { link: "watane:yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy@http://localhost/" },
    "index_drop": { channel: "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy" },
    "search": { text: "reading", publisher: null, interface: null },
    "install_release": { release: "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy/reading-list@0.1.0", link: null },
    "index_list": { link: "watane:yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy@http://localhost/#reading-list", note: "" },
    "index_include": { link: "watane:yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy@http://localhost/", note: "" },
    "index_unlist": { entry: "0000000000000000000000000000000000000000000000000000000000000000" },
    "consent_get": {},
    "consent_answer": { id: "00000000000000000000000000000000", approve: true, withheld: [] },
    "plugin:window|create": { options: { label: "hostile-popup", url: listener } },
    "plugin:event|listen": { event: "consent", target: { kind: "Any" }, handler: 0 },
    "plugin:dialog|open": { options: {} }
  };
  function invoke(command, args) {
    if (!window.__TAURI_INTERNALS__?.invoke) throw new Error("Tauri invoke API missing");
    return window.__TAURI_INTERNALS__.invoke(command, args);
  }
  for (const [command, args] of Object.entries(commands)) {
    attack(`invoke:${command}`, () => invoke(command, args));
  }
  for (let id = 0; id < 64; id++) {
    attack(`channel:${id}`, () => invoke("plugin:__TAURI_CHANNEL__|fetch", { id }));
  }
  for (const [name, url] of Object.entries(targets)) {
    attack(`fetch:${name}`, async () => (await fetch(url)).text());
    attack(`xhr:${name}`, () => {
      const xhr = new XMLHttpRequest();
      xhr.open("GET", url, false);
      xhr.send();
      return xhr.responseText;
    });
  }

  attack("websocket", () => new Promise((resolve, reject) => {
    const socket = new WebSocket(listener.replace("http:", "ws:") + "/ws");
    const timer = setTimeout(() => {
      socket.close();
      reject(new Error("WebSocket never opened"));
    }, 900);
    socket.onopen = () => { clearTimeout(timer); socket.close(); resolve("opened"); };
    socket.onerror = () => { clearTimeout(timer); reject(new Error("WebSocket blocked or failed")); };
  }));

  for (const [name, tag] of [["frame", "iframe"], ["object", "object"], ["script", "script"]]) {
    attack(name, () => new Promise((resolve, reject) => {
      const element = document.createElement(tag);
      const url = `${listener}/${name}`;
      const timer = setTimeout(() => { element.remove(); reject(new Error(`${name} did not load`)); }, 900);
      element.onload = () => {
        // CSP-blocked frames can fire load for about:blank. That is no success.
        if (name === "frame") {
          try {
            if (element.contentWindow.location.href === "about:blank") return;
          } catch (error) {
            // A cross-origin document loaded; same-origin policy hides its URL.
            if (error.name !== "SecurityError") {
              clearTimeout(timer); reject(error); return;
            }
          }
        }
        clearTimeout(timer); resolve("loaded");
      };
      element.onerror = () => { clearTimeout(timer); element.remove(); reject(new Error(`${name} blocked or failed`)); };
      if (name === "object") element.data = url;
      else element.src = url;
      (name === "script" ? document.head : document.body).appendChild(element);
    }));
  }
  attack("window.open", () => {
    const opened = window.open(`${listener}/popup`, "_blank");
    if (!opened) throw new Error("window.open returned no window");
    return "window returned";
  });
  for (const kind of ["blob", "data"]) {
    attack(`download:${kind}`, () => {
      const url = kind === "blob" ? URL.createObjectURL(new Blob(["hostile"])) : "data:text/plain,hostile";
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = "hostile.txt";
      document.body.appendChild(anchor);
      try { anchor.click(); } finally {
        anchor.remove();
        if (kind === "blob") URL.revokeObjectURL(url);
      }
      // A click is no proof of a download; S10-4 uses the native download log.
      throw new Error("download attempted; page cannot observe completion");
    });
  }

  await Promise.all(pending);
  report.done = true;
  // A second synchronous assignment would cancel the first before the native
  // guard sees it, so give each navigation its own turn.
  for (const url of [targets.tauri, `${listener}/navigate`]) {
    try { location.href = url; } catch (error) { /* Native navigation guard may throw. */ }
    await new Promise(resolve => setTimeout(resolve, 300));
  }
  report.navTried = true;
})();
