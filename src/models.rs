#![allow(proc_macro_derive_resolution_fallback)]

use super::schema::*;

extern crate chrono;
use chrono::NaiveDateTime;

use diesel::{
    deserialize::{FromSql, Result as DResult},
    pg::Pg,
    sql_types,
};

#[derive(Debug, FromSqlRow)]
pub enum Schedule {
    Once,
    Daily,
    Weekly,
    Fortnightly,
}

impl std::str::FromStr for Schedule {
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

#[derive(Identifiable, Queryable, Debug)]
pub struct User {
    pub id: i64,
    pub ident: String,
    pub password: Option<String>,
    pub time_zone: Option<String>,
    pub features: String, // TODO
}
