#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate maplit;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

use hibi2::models::*;
use hibi2::schema::tasks::dsl::*;
use hibi2::schema::users::dsl::*;

use chrono::{DateTime, Duration, Local, TimeZone};
use diesel::prelude::*;
use diesel::{debug_query, pg::Pg};
#[cfg(debug_assertions)]
use dotenv::dotenv;
use rocket::config::{Config, ConfigError, Environment};
use rocket::http::{RawStr, Status};
use rocket::request::{self, FromFormValue, FromRequest, Request};
use rocket::response::content;
use rocket::Outcome;
use rocket_contrib::databases::diesel;

use std::env;

#[database("hibi")]
struct HibiDbConn(diesel::PgConnection);

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[derive(Clone, Copy, Debug)]
enum TaskSetType {
    Today,
    Postponed,
    Done,
}

impl<'t> FromFormValue<'t> for TaskSetType {
    type Error = String;

    fn from_form_value(form_value: &'t RawStr) -> Result<Self, Self::Error> {
        match form_value.as_str() {
            "today" => Ok(TaskSetType::Today),
            "postponed" => Ok(TaskSetType::Postponed),
            "done" => Ok(TaskSetType::Done),
            _ => Err(format!("invalid form value: {:?}", form_value)),
        }
    }

    fn default() -> Option<Self> {
        Some(TaskSetType::Today)
    }
}

#[derive(Debug)]
enum TaskSet<Tz: TimeZone> {
    Today,
    Postponed,
    DoneSince(DateTime<Tz>),
}

impl<'a, 'r> FromRequest<'a, 'r> for TaskSet<Local> {
    type Error = String;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let default_horizon = Local::now() - Duration::days(14);

        let typ_ = request
            .get_query_value("q")
            .unwrap_or(Ok(TaskSetType::Today));
        if typ_.is_err() {
            return Outcome::Failure((Status::BadRequest, typ_.unwrap_err()));
        }
        let typ = typ_.unwrap();

        let days__ = request.get_query_value("days");

        match (typ, days__) {
            (TaskSetType::Today, None) => Outcome::Success(TaskSet::Today),
            (TaskSetType::Postponed, None) => Outcome::Success(TaskSet::Postponed),
            (TaskSetType::Done, Some(Ok(days))) => {
                Outcome::Success(TaskSet::DoneSince(Local::now() - Duration::days(days)))
            }
            (TaskSetType::Done, None) => Outcome::Success(TaskSet::DoneSince(default_horizon)),
            (TaskSetType::Done, Some(Err(err))) => Outcome::Failure((
                Status::BadRequest,
                format!("invalid days parameter: {}", err),
            )),
            (_, Some(_)) => Outcome::Failure((
                Status::BadRequest,
                format!("can't specify days with {:?}", typ),
            )),
        }
    }
}

impl<Tz: TimeZone> TaskSet<Tz> {
    fn query(&self, pg: &PgConnection, user: &User) -> Vec<Task> {
        let end_of_today = Local::today().and_hms(23, 59, 59).naive_utc();
        let result = match self {
            TaskSet::Today => {
                let query = Task::belonging_to(user)
                    .filter(
                        done_at
                            .is_null()
                            .and(active)
                            .and(scheduled_for.le(end_of_today)),
                    )
                    .order(order);
                println!("{}", debug_query::<Pg, _>(&query));
                query.load(pg)
            }
            TaskSet::Postponed => {
                let query = Task::belonging_to(user)
                    .filter(
                        done_at
                            .is_null()
                            .and(active)
                            .and(scheduled_for.gt(end_of_today)),
                    )
                    .order(scheduled_for);
                println!("{}", debug_query::<Pg, _>(&query));
                query.load(pg)
            }
            TaskSet::DoneSince(horizon) => {
                let query = Task::belonging_to(user)
                    .filter(done_at.ge(horizon.naive_utc()))
                    .order(done_at.desc());
                println!("{}", debug_query::<Pg, _>(&query));
                query.load(pg)
            }
        };
        result.expect("error loading tasks")
    }
}

struct FakeUser(User);

impl<'a, 'r> FromRequest<'a, 'r> for FakeUser {
    type Error = String;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let db: HibiDbConn = request
            .guard()
            .map_failure(|(s, e)| (s, format!("{:?}", e)))?;

        const HARDCODED_USER_ID: i64 = 4;

        let user_query = users.find(HARDCODED_USER_ID);
        println!("{}", debug_query::<Pg, _>(&user_query));
        match user_query.first::<User>(&db as &PgConnection).optional() {
            Ok(Some(user)) => Outcome::Success(FakeUser(user)),
            Ok(None) => Outcome::Failure((
                Status::Unauthorized,
                "couldn't authenticate even with hardcoded user creds!".into(),
            )),
            Err(e) => Outcome::Failure((
                Status::InternalServerError,
                format!("error loading user: {:?}", e),
            )),
        }
    }
}

#[get("/tasks?<q>&<days>")]
#[allow(unused_variables)] // task_set uses them, can't use underscore or route won't match
fn list_tasks(
    q: TaskSetType,
    days: Option<u8>,
    task_set: TaskSet<Local>,
    db: HibiDbConn,
    user: FakeUser,
) -> content::Json<String> {
    println!("task_set={:?}", task_set);

    let requested_tasks: Vec<String> = task_set
        .query(&db, &user.0)
        // TODO
        .into_iter()
        .map(|t| t.title)
        .collect();

    // TODO use rocket's built in JSON support?
    // https://rocket.rs/v0.4/guide/responses/#json
    content::Json(format!("{:?}", requested_tasks))
}

fn config() -> Result<Config, ConfigError> {
    #[cfg(debug_assertions)]
    dotenv().ok();

    Config::build(Environment::active()?)
        .extra(
            "databases",
            hashmap!{
                "hibi" => hashmap!{
                    "url" => env::var("DATABASE_URL").map_err(|e| ConfigError::BadEnvVal("DATABASE_URL".into(), format!("{}", e).into(), "must be specified".into()))?,
                },
            },
        )
        .finalize()
}

fn main() {
    let config = config().expect("couldn't configure Rocket");

    rocket::custom(config)
        .attach(HibiDbConn::fairing())
        .mount("/", routes![index, list_tasks])
        .launch();
}
