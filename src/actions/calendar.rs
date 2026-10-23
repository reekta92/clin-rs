use super::*;

struct CalendarAction(&'static str, &'static str, &'static str);

impl Action for CalendarAction {
    fn id(&self) -> Cow<'static, str> {
        self.0.into()
    }
    fn name(&self) -> Cow<'static, str> {
        self.1.into()
    }
    fn description(&self) -> Cow<'static, str> {
        self.2.into()
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Views
    }
    fn glyph(&self) -> (&'static str, &'static str) {
        crate::ui::icons::CALENDAR.pair()
    }
    fn execute(&self, app: &mut App, note: Option<&str>) -> Result<()> {
        app.calendar_command(self.0, note)
    }
}

pub(super) fn commands() -> Vec<Box<dyn Action>> {
    [
        (
            "calendar.new",
            "Calendar: New Event",
            "Schedule an all-day or timed event",
        ),
        (
            "calendar.schedule_note",
            "Schedule Note",
            "Assign the selected note to a date, time, or recurring event",
        ),
        (
            "calendar.import",
            "Calendar: Import iCalendar",
            "Import event series from an .ics file",
        ),
        (
            "calendar.export",
            "Calendar: Export iCalendar",
            "Export events to a new .ics file",
        ),
        (
            "calendar.sync",
            "Calendar: Sync CalDAV",
            "Discover calendars and synchronize local and remote edits",
        ),
    ]
    .into_iter()
    .map(|(id, name, description)| {
        Box::new(CalendarAction(id, name, description)) as Box<dyn Action>
    })
    .collect()
}
