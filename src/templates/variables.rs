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
        let mut result = String::with_capacity(template.len() + 50);
        let mut chars = template.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' {
                let mut var_name = String::new();
                let mut closed = false;
                while let Some(&next) = chars.peek() {
                    if next == '}' {
                        chars.next();
                        closed = true;
                        break;
                    }
                    var_name.push(chars.next().unwrap());
                }

                if closed {
                    match var_name.as_str() {
                        "date" => result.push_str(&self.date),
                        "datetime" => result.push_str(&self.datetime),
                        "time" => result.push_str(&self.time),
                        "weekday" => result.push_str(&self.weekday),
                        "year" => result.push_str(&self.year),
                        "month" => result.push_str(&self.month),
                        "day" => result.push_str(&self.day),
                        _ => {
                            result.push('{');
                            result.push_str(&var_name);
                            result.push('}');
                        }
                    }
                } else {
                    result.push('{');
                    result.push_str(&var_name);
                }
            } else {
                result.push(c);
            }
        }
        result
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
