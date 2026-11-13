# Changelog

## 0.0.7 (unreleased until tagged)

### English

- **Fix: `fs-tasks --plan classic …` opens the window**: `--plan`, `--open`, `--list`, `--sample`, `--today` and `--export-sensitive` were not known as window options, so such a call went to the command line (`unknown command`) and the typing test saw no window.
- **Import of a long `DEPENDS-ON` chain is linear again**: the tasks of an import are judged for "who became ready" in one pass over the store, not once for each task a relation names (that was quadratic: 300 tasks 35 s in a debug build, 1500 did not finish). Actor IRI (2048 bytes), actor name (200 bytes) and the end of a claim (24 hours) are bounded. The choice of the face is synced to disk before it replaces the old file, and a relative `$XDG_DATA_HOME` is ignored (XDG rule).
- **The window has three faces**, chosen in the menu **Face**: *Classic* (the first window), *Board* (columns by status, cards, a panel with "Move to") and *Day list* (Today, Upcoming, Anytime, Someday, Logbook, labels). The choice is remembered in `settings.json` in the data folder (`$XDG_DATA_HOME/freesynergy/tasks/`), written atomically; a missing or broken file means the default, **Board**. A failed write is a warning, not a crash.
- `--plan classic|board|next` starts with a face this time only and does not change the memory (`board2` and `next2` still work; the earlier workbench layouts are `board1` and `next1`). `--version` / `-V` is unchanged.
- Built on `fs-components` v0.0.13 (the plan widgets live there), `fs-interfaces` v0.0.19. The typing test starts the classic face (`--plan classic`).
- Core: boards, actors as assignees, execution members, claims (see DESIGN.md). **Comments are not in this program**; they live in the comment topic.
- Not done: moving a card with the mouse; Windows was not run; the faces are tested by events, the router and headless pictures, not by a person's mouse.

### Deutsch

- **Fehlerbehebung: `fs-tasks --plan classic …` öffnet das Fenster**: `--plan`, `--open`, `--list`, `--sample`, `--today` und `--export-sensitive` galten nicht als Fensteroptionen, der Aufruf ging an die Kommandozeile (`unknown command`) und der Tipptest sah kein Fenster.
- **Das Fenster hat drei Gesichter**, gewählt im Menü **Face** („Gesicht“): *Classic* (das erste Fenster), *Board* (Spalten nach Status, Karten, Seitenfeld mit „Move to“) und *Day list* (Tagesliste: Today, Upcoming, Anytime, Someday, Logbook, Labels). Die Wahl wird in `settings.json` im Datenordner (`$XDG_DATA_HOME/freesynergy/tasks/`) gemerkt, atomar geschrieben; fehlt die Datei oder ist sie kaputt, gilt der Standard **Board**. Ein fehlgeschlagenes Schreiben ist eine Warnung, kein Absturz.
- `--plan classic|board|next` startet nur dieses eine Mal mit einem Gesicht und ändert das Gemerkte nicht (`board2` und `next2` gehen weiter; die früheren Workbench-Layouts heißen `board1` und `next1`). `--version` / `-V` bleibt.
- Baut auf `fs-components` v0.0.13 (dort liegen die Plan-Widgets) und `fs-interfaces` v0.0.19. Der Tipptest startet das klassische Gesicht (`--plan classic`).
- Kern: Boards, Akteure als Zuständige, Ausführungsfelder, Claims (siehe DESIGN.md). **Kommentare gehören nicht in dieses Programm**, sie liegen im Kommentar-Thema.
- Nicht erledigt: Karten per Maus ziehen; Windows wurde nicht ausgeführt; die Gesichter sind mit Ereignissen, Router und Bildern ohne Fenster getestet, nicht mit der Maus einer Person.
