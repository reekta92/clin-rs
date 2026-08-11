use super::*;
use std::borrow::Cow;
use std::time::{Duration, Instant};

impl App {
    pub fn default_status_text(&self) -> Cow<'static, str> {
        Cow::Borrowed("")
    }

    pub fn set_default_status(&mut self) {
        self.status = self.default_status_text();
        self.status_until = None;
    }

    pub fn set_temporary_status(&mut self, message: &str) {
        self.status = Cow::Owned(message.to_string());
        self.status_until = Some(Instant::now() + Duration::from_secs(2));
    }

    pub fn set_temporary_status_static(&mut self, message: &'static str) {
        self.status = Cow::Borrowed(message);
        self.status_until = Some(Instant::now() + Duration::from_secs(2));
    }

    pub fn tick_status(&mut self) -> bool {
        if let Some(until) = self.status_until
            && Instant::now() >= until
        {
            self.set_default_status();
            true
        } else {
            false
        }
    }
}
