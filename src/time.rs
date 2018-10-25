use chrono::{DateTime, FixedOffset, Local, TimeZone};
use regex::Regex;

use super::models::*;

pub struct TaskInTimeZone<'a, Tz: TimeZone> {
    pub task: &'a Task,
    pub offset: &'a Tz::Offset,
}

impl<'a, Tz: TimeZone> TaskInTimeZone<'a, Tz> {
    pub fn timezone(&self) -> Tz {
        Tz::from_offset(self.offset)
    }

    pub fn scheduled_for(&self) -> DateTime<Tz> {
        DateTime::<Tz>::from_utc(self.task.scheduled_for, self.offset.clone())
    }

    pub fn done_at(&self) -> Option<DateTime<Tz>> {
        self.task
            .done_at
            .map(|done_at| DateTime::<Tz>::from_utc(done_at, self.offset.clone()))
    }

    pub fn is_overdue(&self, now: DateTime<Tz>) -> bool {
        self.scheduled_for().date() < now.date()
    }

    pub fn is_overdue_now(&self) -> bool {
        let now = Local::now().with_timezone(&self.timezone());
        self.is_overdue(now)
    }
}

impl<'a> Task {
    pub fn in_time_zone<Tz: TimeZone>(&'a self, offset: &'a Tz::Offset) -> TaskInTimeZone<'a, Tz> {
        TaskInTimeZone {
            task: self,
            offset: offset,
        }
    }
}

static MINUTE: i32 = 60;
static HOUR: i32 = 60 * MINUTE;

impl User {
    pub fn time_zone_offset(&self) -> FixedOffset {
        self.try_time_zone_offset().expect("invalid time zone!")
    }

    pub fn try_time_zone_offset(&self) -> Result<FixedOffset, String> {
        lazy_static! {
            static ref TZ_PAT: Regex = Regex::new(r"^([-+])(\d{2})(\d{2})").unwrap();
        }

        let tz_str = self.time_zone.as_ref().ok_or("no time zone")?;
        let captures = TZ_PAT
            .captures(tz_str)
            .ok_or(format!("time zone does not match: {:?}", tz_str))?;
        let plusminus = captures.get(1).unwrap().as_str();
        let hours: i32 = captures
            .get(2)
            .unwrap()
            .as_str()
            .parse()
            .map_err(|e| format!("invalid hours: {:?}", e))?;
        let minutes: i32 = captures
            .get(3)
            .unwrap()
            .as_str()
            .parse()
            .map_err(|e| format!("invalid minutes: {:?}", e))?;

        let secs = hours * HOUR + minutes * MINUTE;

        match plusminus {
            "-" => Ok(FixedOffset::west(secs)),
            "+" => Ok(FixedOffset::east(secs)),
            _ => Err(format!("invalid plusminus: {:?}", plusminus)),
        }
    }
}
