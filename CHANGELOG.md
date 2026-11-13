# Changelog

## 0.0.7 (in progress)

### English

- **The window has three faces**, chosen in the menu **Face**: *Classic* (the first window), *Board* (columns by status, cards, a panel with "Move to") and *Day list* (Today, Upcoming, Anytime, Someday, Logbook, labels). The choice is remembered in `settings.json` in the data folder (`$XDG_DATA_HOME/freesynergy/tasks/`), written atomically; a missing or broken file means the default, **Board**. A failed write is a warning, not a crash.
- `--plan classic|board|next` starts with a face this time only and does not change the memory (`board2` and `next2` still work; the earlier workbench layouts are `board1` and `next1`). `--version` / `-V` is unchanged.
- Built on `fs-components` v0.0.13 (the plan widgets live there), `fs-interfaces` v0.0.19. The typing test starts the classic face (`--plan classic`).
- Core: boards, actors as assignees, execution members, claims (see DESIGN.md). **Comments are not in this program**; they live in the comment topic.
- Not done: moving a card with the mouse; Windows was not run; the faces are tested by events, the router and headless pictures, not by a person's mouse.

### Deutsch

- **Das Fenster hat drei Gesichter**, gewählt im Menü **Face** („Gesicht“): *Classic* (das erste Fenster), *Board* (Spalten nach Status, Karten, Seitenfeld mit „Move to“) und *Day list* (Tagesliste: Today, Upcoming, Anytime, Someday, Logbook, Labels). Die Wahl wird in `settings.json` im Datenordner (`$XDG_DATA_HOME/freesynergy/tasks/`) gemerkt, atomar geschrieben; fehlt die Datei oder ist sie kaputt, gilt der Standard **Board**. Ein fehlgeschlagenes Schreiben ist eine Warnung, kein Absturz.
- `--plan classic|board|next` startet nur dieses eine Mal mit einem Gesicht und ändert das Gemerkte nicht (`board2` und `next2` gehen weiter; die früheren Workbench-Layouts heißen `board1` und `next1`). `--version` / `-V` bleibt.
- Baut auf `fs-components` v0.0.13 (dort liegen die Plan-Widgets) und `fs-interfaces` v0.0.19. Der Tipptest startet das klassische Gesicht (`--plan classic`).
- Kern: Boards, Akteure als Zuständige, Ausführungsfelder, Claims (siehe DESIGN.md). **Kommentare gehören nicht in dieses Programm**, sie liegen im Kommentar-Thema.
- Nicht erledigt: Karten per Maus ziehen; Windows wurde nicht ausgeführt; die Gesichter sind mit Ereignissen, Router und Bildern ohne Fenster getestet, nicht mit der Maus einer Person.
