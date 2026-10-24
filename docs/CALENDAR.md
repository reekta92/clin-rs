# Calendar

Open **Open Calendar View** from the command palette (`F6` or `Ctrl+Space`), or click the Notes `calendar` widget. Clicking a day selects that date. The view offers Month, Week, Day, and a 30-day Agenda, with a selected-date event list and event details. Its shared header and footer follow `ui.hint_bar_style` and the global statusline templates. Month/Week/Day/Agenda tabs share the header with the selected date; sync progress, conflicts and errors appear in the footer. Calendar panes are borderless, with a darker event sidebar derived from the active theme. A muted divider matching preview panes separates the main calendar from the right sidebar on wide terminals. Month days use a checkerboard background; adjacent-month days have a muted background and text. Week days, day time slots, agenda rows and sidebar sections alternate subtle theme shades. Event titles use the theme accent; entries linked to notes, linked-note names and note markers use the theme subnote color across all displays. Narrow headers keep the active tab visible. Times display in your system timezone; `week_start` follows the Notes setting.

The widget highlights dates with events. The old note-activity heatmap remains available as `activity`:

```toml
[list]
sections = ["calendar", "activity"] # At most two widgets
week_start = "monday"

[features]
calendar = true # Controls the Notes widget strip; the Calendar view remains available
```

## Schedule and link notes

Choose **Schedule Note** with a Markdown note selected in Notes, then fill out its event. Alternatively, press `n` in Calendar. Title, start, end, location, description, repeat rule, and calendar are editable. `Tab` changes fields; `Ctrl+S` saves. Validation errors retain your input. `Esc` cancels.

- `2026-10-23` specifies an all-day event. Its end date is exclusive: October 23 through October 25 uses end `2026-10-26`.
- `2026-10-23T10:30` specifies system-local time. An RFC3339 value such as `2026-10-23T10:30:00+03:00` supplies an explicit offset. Ambiguous or nonexistent local times require an offset.
- Repeat accepts an RFC 5545 RRULE, for example `FREQ=WEEKLY;BYDAY=MO,WE`. Newly entered date-times are stored as UTC, so their recurrence follows UTC across daylight-saving changes. Imported IANA timezone recurrences retain their timezone when their schedule is unchanged.
- Choose Local, or a discovered writable CalDAV collection. Sync once to discover remote choices. Existing events keep their calendar.

Select a date and event, then press `a` to link or unlink a Markdown note. On an empty date, `a` chooses a note and opens a new event form. Several notes may link to one event. Press `o` or `Enter`, choose a linked note, and open it in the editor. Leaving that editor returns to Calendar.

Links are stored in the note's frontmatter and survive native rename/move:

```yaml
calendar-events:
  - 1878c31c-e10a-41cc-a7e5-229e7c9515a4
```

`y` copies an inline reference to the selected event, or the date when no event is selected:

```markdown
[[calendar:2026-10-23]]
[[calendar:1878c31c-e10a-41cc-a7e5-229e7c9515a4@2026-10-23]]
```

Use the editor's **Open Link Preview** binding (`Alt+l` by default) with the cursor on the reference to open Calendar. Departure saves the note first. Event IDs belong to the vault; copying references to a different vault does not copy events.

## Keys and palette

| Key | Action |
| --- | --- |
| `m`, `w`, `d`, `A` / `Tab` | Month, Week, Day, Agenda / next view |
| `h`, `l` / Left/Right | Previous/next date |
| `k`, `j` / Up/Down | Previous/next week in Month/Week; select event in Day/Agenda |
| `PgUp`, `PgDn` | Previous/next displayed period |
| `t` | Today |
| `J`, `K` | Next/previous event in any view |
| `n`, `e` | New event / edit selected series |
| `Delete` or `x` | Confirm deletion of the entire series |
| `a`, `o` or `Enter` | Link/unlink note / open linked note |
| `y` | Copy date/event reference |
| `i`, `E` | Import / export iCalendar |
| `s`, `r` | Sync CalDAV / reload from disk |
| `c` | Resolve a sync conflict |
| `?`, `F1` | Calendar help |
| `Esc`, `q` | Return to Notes |

View tabs and dates also accept clicks. Scroll selects events; in Day it scrolls hours. The command palette offers **Calendar: New Event**, **Calendar: Import iCalendar**, **Calendar: Export iCalendar**, and **Calendar: Sync CalDAV** while Calendar is active. Forms and confirmations retain keyboard ownership.

## iCalendar files

`i` imports a local `.ics` file into Local. Import validates every series before committing, skips identical local UIDs, and refuses a differing duplicate UID. `E` exports all nondeleted event series to a **new** `.ics` path. Existing files are never overwritten. Conflicting timezone definitions or duplicate UIDs across calendars stop an ambiguous export.

Supported data includes VEVENT, all-day and timed events, RRULE/RDATE/EXDATE, moved/cancelled recurrence overrides, and IANA timezone names. Unknown event properties, attendees, alarms, and VTIMEZONE components are retained during editing and export. Clin does not send invitations or execute alarms.

Edits and deletes apply to the whole series. Changing time/repeat on a series with exceptions is refused; descriptive fields remain editable. VTODO/VJOURNAL import, custom non-IANA timezone evaluation, and `RANGE=THISANDFUTURE` are unsupported. Unsupported remote sources stay stored with a visible warning and remain exportable. Expansion is bounded to 4096 instances per query; very dense or long-running recurrence may hit the limit. Import/HTTP/export documents are limited to 16 MiB and the local store to 64 MiB.

## CalDAV

Add one or more accounts to `config.toml`:

```toml
[[calendar.caldav]]
name = "Work"
url = "https://example.org/dav/"
username = "user"
password_env = "CLIN_CALDAV_PASSWORD"
```

Set that environment variable before starting clin, using your provider's password or app password. The password value is never written to config or calendar storage. Authentication uses HTTP Basic over HTTPS. A public server can omit both `username` and `password_env`. OAuth and provider-specific sign-in are not implemented.

The URL can identify an account endpoint, calendar home, or direct calendar collection. Discovery follows `current-user-principal` and `calendar-home-set`, then lists VEVENT calendars. Configure the direct HTTPS collection URL if the provider redirects its endpoint or uses another origin: redirects and cross-origin discovery are deliberately rejected so credentials remain on the configured origin. Plain HTTP is allowed only on localhost for development. URL credentials, queries, and fragments are rejected.

Press `s` to synchronize in the background. Sync pulls remote events, then publishes local changes with strong ETag preconditions. Read-only calendars remain visible. Removing an account from config retains its cached events but disables edits to that calendar. Local events remain local; remote event creation requires choosing a discovered calendar in the form.

Concurrent local/remote changes retain both versions. Press `c`, inspect the conflict, then `l` to keep local or `r` to use remote. Keeping local queues a conditional write for the next sync. A deleted event with a conflict is reachable through `c` even though it is absent from the grid. Connection errors retain pending edits; rerun sync after recovery. Each successful mutation is checkpointed, so a later failure may leave earlier calendar changes already synchronized.

Per-vault data lives in `.clin/calendar.json`, including cached remote content, ETags, pending deletes, and conflicts. Writes are atomic, detect stale disk state, and use a separate calendar lock. Back up this file with your notes. Note paths, note bodies, `calendar-events`, and inline references are never added to remote calendar payloads. Event title/description/location are synchronized as entered. Sync is manual; no polling schedule runs in the background.

## Local protocol verification

The committed [Postman simulation](../postman/simulations/calendar.sim.yaml) models RFC 4791 discovery, calendar-query, conditional writes, conflicts, and outages. It uses local state and requires no provider credentials or Postman workspace publication.

```sh
postman simulation run postman/simulations/calendar.sim.yaml --port 49147 --no-history
```

In another terminal:

```sh
CLIN_CALDAV_TEST_URL=http://127.0.0.1:49147/caldav/ cargo test --locked calendar::tests::caldav_simulation_round_trip_and_conflicts --lib -- --ignored --exact
```

This proves behavior against the fixture, not compatibility with every CalDAV provider. The normal unit/interaction suite runs without a network service.
