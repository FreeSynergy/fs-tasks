# Worker protocol / Arbeiter-Protokoll

A worker (an agent, an API run) takes a task, shows that it is working, and reports when done. People and the
strategy see it on the board (window, frontend *Board*): columns *Needs action / In progress / Completed / Failed*.
Ein Arbeiter (Agent, API-Lauf) nimmt eine Aufgabe, zeigt, dass er arbeitet, und meldet das Ende. Menschen und
Strategie sehen es im Kanban (Fenster, Ansicht *Board*).

## English

**Steps** (all against the local server, which the first command starts by itself; add `--json` for scripts):

1. Begin: `fs-tasks next --as <agent> [--project P]` takes the first *ready* task and claims it in one step
   (nothing printed, JSON `null`, exit 0 if none is ready). Or `fs-tasks claim <id> --as <agent> [--until 2h]`
   (default 2 h, at most 24 h; claiming again renews).
2. `fs-tasks status <id> in-progress`.
3. Optional: `fs-tasks note <id> "one short line"` (at most 500 bytes) for the board.
4. End: `fs-tasks status <id> done|failed --note '<result line>' --report <file-or-URL> --cost <usd>`.
   The claim is released. An *objective* task (made with `add --objective '<check command>'`) accepts `done` only
   after `fs-tasks check <id>` ran its command with exit 0; a *subjective* one (default) is done when the actor says so.

**Exit codes:** 0 ok, 1 error, 2 wrong words, **3 conflict** (somebody else holds the claim: the holder and the end
of the claim are in the message; with `--json` stderr is `{"error":..,"conflict":true}`). On 3 the worker takes
another task (`next`) or stops; it never works on a task whose claim it lost.
Output has no colour or ANSI codes. Secrets (`sk-ant-`, `ghp_`, `-----BEGIN`, `Bearer ` ...) are refused in titles,
notes, reports and commands: tasks are shared, put nothing secret into them.

**Check commands are code.** `check` runs one only if the task was made on this machine (`origin` local) by a trusted
actor (default: the owner; more in `check-policy.json` in the data folder: `{"trusted_actors":["agent"],
"programs":["make"],"timeout_secs":600}`) and its program is allowed (`cargo`, `fs-release`, `bash scripts/...`
relative without `..`, plus the listed ones; never a path, never a shell). Imported tasks are always foreign:
`check command not run: foreign origin`, exit 1, task unchanged, `done` refused.

**Wrapper for a runner** (bash; `run.sh` already writes `$OUT` with `total_cost_usd`):

```bash
#!/bin/bash
# run-task.sh <agent> <project> <report-dir> -- <the runner command ...>
AGENT=$1; PROJECT=$2; REPORTS=$3; shift 4
J=$(fs-tasks next --as "$AGENT" --project "$PROJECT" --json) || exit $?
[ "$J" = null ] || [ -z "$J" ] && exit 0                 # nothing ready
ID=$(python3 -I -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$J")
fs-tasks status "$ID" in-progress >/dev/null
"$@"; RC=$?                                               # e.g. run.sh <task> <model> <budget> <wt> <prompt>
REPORT="$REPORTS/$ID.md"; COST=$(python3 -I -c 'import json,glob,os; f=max(glob.glob("out-*.json"),key=os.path.getmtime,default=None); print(json.load(open(f)).get("total_cost_usd",0) if f else 0)')
if [ $RC -eq 0 ]; then S=done; else S=failed; fi
fs-tasks status "$ID" "$S" --note "runner exit $RC" --report "$REPORT" --cost "$COST" >/dev/null || \
  echo "status $S refused (objective task: run 'fs-tasks check $ID')" >&2
```

**Limits.** Sync to other devices is not built: the claim is a note for this machine only. A claim is a lock for
cooperating workers, not a security boundary: whoever can open the socket can edit tasks (the window's board moves cards
without the `done` gate). One process holds the store: the window, when open, answers the command line itself;
without a window the autostarted server does, and then a window refuses to open the same file.

## Deutsch

**Schritte** (alles gegen den lokalen Server, den der erste Befehl selbst startet; `--json` fuer Skripte):

1. Anfang: `fs-tasks next --as <agent> [--project P]` holt die erste *bereite* Aufgabe und claimt sie in einem Schritt
   (keine Ausgabe, JSON `null`, Exit 0, wenn keine bereit ist). Oder `fs-tasks claim <id> --as <agent> [--until 2h]`
   (Standard 2 h, hoechstens 24 h; erneut claimen verlaengert).
2. `fs-tasks status <id> in-progress`.
3. Optional: `fs-tasks note <id> "eine kurze Zeile"` (hoechstens 500 Byte) fuer das Board.
4. Ende: `fs-tasks status <id> done|failed --note '<Ergebniszeile>' --report <Datei-oder-URL> --cost <usd>`.
   Der Claim wird freigegeben. Eine *objektive* Aufgabe (`add --objective '<Pruefbefehl>'`) nimmt `done` erst an,
   wenn `fs-tasks check <id>` den Befehl mit Exit 0 ausgefuehrt hat; eine *subjektive* (Standard) ist fertig, wenn der
   Akteur es sagt.

**Exit-Codes:** 0 ok, 1 Fehler, 2 falsche Worte, **3 Konflikt** (ein anderer haelt den Claim: Inhaber und Ende stehen in
der Meldung; mit `--json` ist stderr `{"error":..,"conflict":true}`). Bei 3 nimmt der Arbeiter eine andere Aufgabe
(`next`) oder hoert auf; er arbeitet nie an einer Aufgabe, deren Claim er verloren hat. Keine Farben/ANSI. Geheimnisse
(`sk-ant-`, `ghp_`, `-----BEGIN`, `Bearer ` ...) werden in Titel, Notiz, Bericht und Befehl abgelehnt.

**Pruefbefehle sind Code.** `check` fuehrt einen nur aus, wenn die Aufgabe auf diesem Rechner (`origin` local) von einem
vertrauten Akteur angelegt wurde (Standard: der Eigentuemer; mehr in `check-policy.json` im Datenordner) und das
Programm erlaubt ist (`cargo`, `fs-release`, `bash scripts/...` relativ ohne `..`, dazu die Eingetragenen; nie ein
Pfad, nie eine Shell). Importierte Aufgaben sind immer fremd: `check command not run: foreign origin`, Exit 1, Aufgabe
unveraendert, `done` abgelehnt.

**Wrapper fuer ein Runner-Skript:** siehe das Bash-Beispiel oben (`next` -> `in-progress` -> Lauf -> `done|failed`).

**Grenzen.** Kein Sync zu anderen Geraeten: der Claim gilt nur fuer diesen Rechner. Ein Claim ist eine Sperre fuer
zusammenarbeitende Arbeiter, keine Sicherheitsgrenze: wer den Socket oeffnen kann, kann Aufgaben aendern (das Board im
Fenster verschiebt Karten ohne das `done`-Tor). Einen Speicher haelt nur ein Prozess: das offene Fenster beantwortet
die Kommandozeile selbst; ohne Fenster tut es der automatisch gestartete Server, und ein Fenster oeffnet dann dieselbe
Datei nicht.
