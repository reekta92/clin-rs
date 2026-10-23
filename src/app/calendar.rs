use super::*;

impl App {
    pub(crate) fn open_calendar_reference(&mut self, target: &str) -> anyhow::Result<()> {
        let (resource, date) = if let Ok(date) = target.parse::<chrono::NaiveDate>() {
            (None, date)
        } else {
            let (id, date) = target.split_once('@').ok_or_else(|| {
                anyhow::anyhow!("Use calendar:YYYY-MM-DD or calendar:EVENT-ID@YYYY-MM-DD")
            })?;
            uuid::Uuid::parse_str(id)?;
            let date = date.parse::<chrono::NaiveDate>()?;
            self.calendar.ensure_loaded(&self.storage)?;
            anyhow::ensure!(
                self.calendar
                    .data()
                    .resources
                    .iter()
                    .any(|r| r.id == id && !r.deleted),
                "Referenced calendar event no longer exists"
            );
            (Some(id), date)
        };
        self.open_calendar_at(Some(date))?;
        if let Some(id) = resource
            && let Some(mut view) = self.calendar_view.take()
        {
            view.select_resource(self, id);
            self.calendar_view = Some(view);
        }
        Ok(())
    }

    pub(crate) fn open_calendar_at(
        &mut self,
        date: Option<chrono::NaiveDate>,
    ) -> anyhow::Result<()> {
        self.navigate_from_palette("calendar.open")?;
        if let Some(date) = date
            && let Some(view) = &mut self.calendar_view
        {
            view.date = date;
        }
        Ok(())
    }

    pub(crate) fn calendar_command(&mut self, id: &str, note: Option<&str>) -> anyhow::Result<()> {
        if id == "calendar.schedule_note" {
            self.open_calendar_at(None)?;
        }
        if id == "calendar.sync" {
            return self
                .calendar
                .sync(&self.storage, self.config.calendar.caldav.clone());
        }
        let mut view = self
            .calendar_view
            .take()
            .ok_or_else(|| anyhow::anyhow!("Calendar view unavailable"))?;
        let result = match id {
            "calendar.new" => view.begin_form(self, None, None),
            "calendar.schedule_note" => view.begin_form(self, None, note.map(str::to_owned)),
            "calendar.import" | "calendar.export" => {
                view.begin_path(self, id == "calendar.import");
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unknown calendar command")),
        };
        self.calendar_view = Some(view);
        result
    }

    pub(crate) fn poll_calendar(&mut self) -> bool {
        let Some(result) = self.calendar.poll(&self.storage) else {
            return false;
        };
        match result {
            Ok(message) => {
                self.set_temporary_status(&message);
                if let Some(view) = &mut self.calendar_view {
                    view.error = Some(message);
                }
            }
            Err(error) => {
                let message = format!("Calendar sync failed; local changes retained: {error:#}");
                self.set_temporary_status(&message);
                self.messages
                    .push(message, messages::MessageSeverity::Warning);
            }
        }
        true
    }
}
