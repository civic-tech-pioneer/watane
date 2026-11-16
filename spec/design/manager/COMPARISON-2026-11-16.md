# Mod manager: design vs the real app (main, a78824e)

Method. The real app (a `driver` debug build, windows hidden) ran on demo homes. Each view's live
DOM was read with the driver's `eval` op and re-rendered with app/ui/style.css in headless Chrome
(screen capture is not permitted on this Mac). So layout and words are the real ones; fonts are
Chrome's system-ui, not WebKit's. The PNGs and `.txt` (the page's innerText) are next to this file.

Overall: the app has nearly every **function** the design shows (tabs, filter, Add mod, details,
Make/Unset main, connections, dependency warning, remove-with-data choice, Find, the three
Updates lists, the menu, launch into the main mod). It has almost none of the **visual design**:
style.css is 13 lines of unstyled-HTML basics. TECHNICAL 5.3 names elements and ids, never a
layout, so the look was not specified, rather than left out on purpose.

## Library (design Main, left) — 05-library.png
- Matches: tabs Library/Find/Updates with an Updates count; a filter on name and publisher;
  All/Published/Local; "Add mod"; the mark in the header; "No mods match. Find a mod" link.
- Differs:
  - Layout: entries are bordered boxes with buttons, not 196 px tiles with a coloured monogram, name and "publisher · version".
  - Each entry carries Open/Disable/Remove buttons in the list. In the design they are only in the details.
  - The filter and Add mod sit in the page body, not in the header.
  - Source is a `<select>`, not pills.
  - Add mod is laid out inline: "Install folders" plus an "Install from link" field. The design has one primary "Add mod" button.
  - No count ("6 mods"), no badges (Main, Update, Problem, Off). A disabled mod reads "Disabled" in its box; nothing is dimmed.
  - The mark is two words ("jade pine"), not the three coloured shapes.
  - Extra: a "Mods | Settings" link row above the tabs (the design reaches Settings only from the menu).
- Left out by spec: badges and the count are not in 5.3. The mark is words by APP design (consent mark, 6.4).

## Details (design Main, right) — 06-details-reading-list.png, 07-details-desk.png, 08-disable-warning.png, 12-details-published-withdrawn.png
- Matches:
  - Name, version, summary and the notice ("Withdrawn by the publisher: counts wrong").
  - Open, Disable/Enable, Remove, and Make main / Unset main (only with a page).
  - The grants in words.
  - The four connection groups with exactly the design's words ("Uses", "Adds to", "Used by", "Extended by", "Works on its own: …"), each a link to the other mod.
  - The dependency warning before disable/remove, with Cancel focused.
  - The remove question with Keep data / Delete data.
  - Publisher plus "signature verified".
- Differs:
  - Layout: there is no side panel. Details is a `<details>` disclosure inside each entry, so several can be open at once. The design selects one tile and shows a fixed right pane.
  - Connections name the **local name** ("recommendations"), not the mod name with its monogram chip. There is no "it calls"/"it answers" hint under each label.
  - Grants show "— Granted [Withdraw]" / "Not granted [Give back]" buttons, not switches, and no glyphs. The design's note "A change here takes effect now…" is missing.
  - Grant words are technical: "Call mods:reading-list/books@0.1.0 of reading-list (local)". The design has "Call books of Reading list by Alice".
  - Technical details are always shown as unlabeled lines (version, "local", page URL, the full package hash, the release id). The design hides them behind "Technical details" with labels. Publisher key and "Made from" appear only as the derived-from label.
  - The disable/remove warnings are modal `<dialog>`s: "Disable Reading list? These mods use it: recommendations." with Cancel/Continue. The design uses an inline panel ("Turn Reading list off?", radio buttons for the data, a "stops working until it is back" warning).
  - Remove asks "Delete the data of X?" with two buttons. It has no Cancel, and Keep data is the default.
  - The notice has no action button ("Review", "Install 1.1.0"); an offer button appears in the entry when there is one.
  - Extra: "Made from this: as far as your indexes list them" (step 5 derived mods).
- Left out by spec: 5.3 puts origin, page origin and package id "behind a disclosure"; the app shows them unlabeled inside the one Details disclosure. "Publisher key" is not in 5.3.

## Find — 02-empty-find.png, 10-find-results.png, 13-find-after.png
- Matches:
  - Search by words, publisher and interface.
  - The chosen indexes, each with its mod count and status ("Alice (mg1q9jzin1) · ok · 1 mod").
  - Hits show name, version, summary, publisher, "signature verified" and via which index.
  - Install opens the consent view.
  - A compromised hit cannot be installed.
  - An installed hit says so.
- Differs:
  - Three plain fields and a Search button, not one big search box ("Search by words, publisher or interface").
  - Indexes are bordered blocks with "Stop using as index", not chips under "Searching". "Add an index" is an "Index link" field plus "Add index", not a button.
  - No result count ("4 mods found"), no monogram, no "read 4 min ago" (status words like "ok" or "failed: …" instead), and no "You have 0.3.1" tag.
  - The design's "Adds commands to Desk, which you use as your main mod" line is missing.
  - The button reads "Install", not "Install…". The design's explaining footnote is missing.
  - Extra:
    - A "Refresh" button and a "Your index · you" row.
    - The whole Channels section (follow link, Refresh channels, per-channel Unfollow, Use as index, Include in my index, List in my index with notes). The design has none of it.
    - "Install from link" sits in Library; the design puts it in the Find header.
- Left out by spec: APP-F-65 puts followed channels and indexes in Find on purpose. The "works with your main mod" line, "read N ago" and the result count are not specified.

## Updates — 03-empty-updates.png, 11-updates.png
- Matches:
  - The three lists in the design's order: needs you, channels that could not be read, installed without a question (APP-F-69).
  - The tab count (Updates 1).
  - A withdrawal shows its reason.
  - An offer gets a button.
- Differs:
  - No section label for the first list (the design's "Needs you"). The headings read "Channels that could not be read" and "Installed without a question", not "Channels" and "Updated by itself".
  - Rows are one plain line: "Tally: Withdrawn by the publisher: counts wrong". The design has cards with a monogram, "0.3.1 → 0.4.0", the new grant as a chip, "Not now", and "Review…".
  - The withdrawn row offers no action and does not say who uses it. The design has "Install 1.1.0" and "Notes uses it…".
  - A channel error is raw: "network error at http://127.0.0.1:8791/head: io: Connection refused (os error 61)". It has no "last read 2 days ago · Weather comes from it" and no "Try again".
  - The header has no "Checked 3 min ago" and no "Check now"; refresh is in Find.
  - No explanatory line under the auto-updated list.
- Left out by spec: "Not now", "Check now", "last read" and "Try again" are not in APP-F-69 or 5.3.

## Empty (first start) — 01-empty-library.png
- Matches: the three ways exist (link, folder, and an index via Find); "Find a mod" appears when a filter matches nothing.
- Differs:
  - No dedicated empty state. The normal Library shows "No mods yet" under the same filter and install controls.
  - No "Add your first mod" with the three explained cards.
  - The Updates tab is not hidden when empty (the design shows only Library and Find).
- Left out by spec: 5.3 and APP-F-62 define no empty view.

## Shell / launch with a main mod — 09-launch-main-mod.png (+ driver `windows`)
- Matches:
  - With `<home>/app/main` set, launch opens only the mod window, titled "Desk — local" (design: "Desk — Alice"), 900 × 680, and no manager.
  - Mods… opens the manager on `#mods`, and Settings… opens it on `#settings`.
  - Closing the manager leaves the mod open.
  - A mod with `main = true` becomes main when the app installs it and none is set; it reads "Unset main".
- Differs:
  - A `main = true` mod installed through `watane-dev install` (`POST /api/install`) does **not** become main. Only the app's own installs adopt it (TECHNICAL 5.4 lists install_folders, install_paths, install_link and release_install). The design note says "when installed", with no exception.
  - Neither the consent view nor the details say that installing it makes it main.
  - The demo's Desk is a test page ("Your desk"), so the design's commands and panels are not shown. Collecting commands is step 9, not the manager.
- Left out by spec: the API-install case is not mentioned in 5.4. That reads as unintended rather than deliberate; worth a question.

## Menu
- Matches the design item for item: app submenu "watane" with About watane, Mods… (⇧⌘M), Settings… (⌘,), a separator, Quit watane (`views::install_menu`).
- Differs:
  - No separator between About and Mods…. The design has one.
  - The native macOS menu also has no File/Edit/View/Window menus, which the design's bar shows.

## Settings (no design board)
- Holds Identity, Site, backup (a disclosure), Agents and Own machines as plain stacked sections (04-settings.png), as APP-F-65 asks. No design exists to compare it with.

## Light and dark
- The design defines a full dark palette per board. The app has **no dark mode**: style.css has fixed light colours (`#faf9f5`, `#232c30`) and no `prefers-color-scheme`. In a dark macOS session the main window stays light.
- Not addressed in TECHNICAL 5.3/5.4.
