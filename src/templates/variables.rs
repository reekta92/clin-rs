use chrono::Local;

#[derive(Debug, Clone)]
pub struct TemplateVariables {
    pub date: String,
    pub datetime: String,
    pub time: String,
    pub weekday: String,
    pub year: String,
    pub month: String,
    pub day: String,
}

impl TemplateVariables {
    pub fn now() -> Self {
        let now = Local::now();
        Self {
            date: now.format("%Y-%m-%d").to_string(),
            datetime: now.format("%Y-%m-%d %H:%M").to_string(),
            time: now.format("%H:%M").to_string(),
            weekday: now.format("%A").to_string(),
            year: now.format("%Y").to_string(),
            month: now.format("%m").to_string(),
            day: now.format("%d").to_string(),
        }
    }

    pub fn substitute(&self, template: &str) -> String {
        template
            .replace("{date}", &self.date)
            .replace("{datetime}", &self.datetime)
            .replace("{time}", &self.time)
            .replace("{weekday}", &self.weekday)
            .replace("{year}", &self.year)
            .replace("{month}", &self.month)
            .replace("{day}", &self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_variables_substitution() {
        let vars = TemplateVariables {
            date: "2026-03-28".to_string(),
            datetime: "2026-03-28 14:30".to_string(),
            time: "14:30".to_string(),
            weekday: "Saturday".to_string(),
            year: "2026".to_string(),
            month: "03".to_string(),
            day: "28".to_string(),
        };

        let template = "Meeting on {date} at {time}";
        let result = vars.substitute(template);
        assert_eq!(result, "Meeting on 2026-03-28 at 14:30");
    }
}
