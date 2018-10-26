#![allow(proc_macro_derive_resolution_fallback)]

use super::schema::*;

use std::{fmt, str};

extern crate chrono;
use chrono::NaiveDateTime;

use diesel::{
    deserialize::{FromSql, Result as DResult},
    pg::Pg,
    sql_types,
};

use simple_error::SimpleResult;

#[derive(Debug, FromSqlRow)]
pub enum Schedule {
    Once,
    Daily,
    Weekly,
    Fortnightly,
}

impl fmt::Display for Schedule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Schedule::*;
        match self {
            Once => write!(f, "Once"),
            Daily => write!(f, "Daily"),
            Weekly => write!(f, "Weekly"),
            Fortnightly => write!(f, "Fortnightly"),
        }
    }
}

impl str::FromStr for Schedule {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        use self::Schedule::*;

        match &text as &str {
            "Once" => Ok(Once),
            "Daily" => Ok(Daily),
            "Weekly" => Ok(Weekly),
            "Fortnightly" => Ok(Fortnightly),
            _ => Err(format!("invalid schedule \"{}\"", text)),
        }
    }
}

impl FromSql<sql_types::Text, Pg> for Schedule {
    fn from_sql(bytes: Option<&[u8]>) -> DResult<Self> {
        let text: DResult<String> = FromSql::<sql_types::Text, Pg>::from_sql(bytes);
        Ok(text?.parse()?)
    }
}

#[derive(Identifiable, Queryable, Associations, Debug)]
#[belongs_to(User)]
pub struct Task {
    pub id: i32,
    pub user_id: i64,
    pub title: String,
    pub pomos: i64,
    pub scheduled_for: NaiveDateTime,
    pub done_at: Option<NaiveDateTime>,
    pub active: bool,
    pub order: i64,
    pub schedule: Schedule,
    pub ext_task_id: Option<i64>,
}

static APPROX_DATETIME_FORMAT: &'static str = "%Y-%m-%d %-I%P UTC";

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let scheduled = self.scheduled_for.format(APPROX_DATETIME_FORMAT);
        let done = self.done_at.map_or("".into(), |done_at| {
            format!(", done at {}", done_at.format(APPROX_DATETIME_FORMAT))
        });
        let schedule = match self.schedule {
            Schedule::Once => "".into(),
            _ => format!(" ({})", self.schedule),
        };
        let is_ext = if self.ext_task_id.is_none() {
            ""
        } else {
            " (ext)"
        };
        write!(
            f,
            "Task{}{} \"{}\" scheduled for {}{}",
            schedule, is_ext, self.title, scheduled, done
        )
    }
}

#[derive(Identifiable, Queryable, Associations, Debug)]
#[belongs_to(User)]
pub struct ExtTask {
    pub id: i64,
    pub user_id: i64,
    pub ext_id: String,
    pub ext_source_name: String,
    pub ext_url: Option<String>,
    pub ext_status: Option<String>,
}

pub fn to_hask_newtype(name: &str, s: &str) -> String {
    format!("{} {{un{} = \"{}\"}}", name, name, s)
}
static HASK_NEWTYPE_SUFFIX: &'static str = "\"}";
static HASK_NEWTYPE_SUFFIX_LEN: usize = 2;
pub fn from_hask_newtype(name: &str, s: &str) -> SimpleResult<String> {
    let prefix = format!("{} {{un{} = \"", name, name);
    if !s.starts_with(&prefix) || !s.ends_with(HASK_NEWTYPE_SUFFIX) {
        bail!("not formatted like a Haskell newtype: {:?}", s)
    }
    let real_start = prefix.len();
    let real_end = s.len() - HASK_NEWTYPE_SUFFIX_LEN;
    Ok(s[real_start..real_end].into())
}

impl fmt::Display for ExtTask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Ext task {} {}",
            from_hask_newtype("ExternalSourceName", &self.ext_source_name)
                .map_err(|_| fmt::Error)?,
            from_hask_newtype("ExternalIdent", &self.ext_id).map_err(|_| fmt::Error)?
        )
    }
}

#[derive(Identifiable, Queryable, Debug)]
pub struct User {
    pub id: i64,
    pub ident: String,
    pub password: Option<String>,
    pub time_zone: Option<String>,
    pub features: String, // TODO
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "User {}", self.ident)
    }
}
