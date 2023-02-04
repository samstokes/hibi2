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
use google_signin;
use rocket::config::{Config, ConfigError, Environment};
use rocket::fairing::{self, Fairing};
use rocket::http::hyper::header::{AccessControlAllowHeaders, AccessControlAllowOrigin};
use rocket::http::{Method, RawStr, Status};
use rocket::request::{self, FromFormValue, FromRequest, Request};
use rocket::Outcome;
use rocket::Response;
use rocket_contrib::databases::diesel;
use rocket_contrib::json::Json;

use std::env;
use std::str::FromStr;

const GOOGLE_CLIENT_ID: &'static str =
    "TODO GET FROM CONFIG"; // TODO get from config

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

struct AuthedUser(User);

fn fake_user_for_cors() -> AuthedUser {
    AuthedUser(User {
        id: -1,
        password: None,
        ident: "".into(),
        features: "".into(),
        time_zone: None,
    })
}

impl<'a, 'r> FromRequest<'a, 'r> for AuthedUser {
    type Error = String;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        println!("{:#?}", request.headers());
        let auth = match (
            request.headers().contains("Access-Control-Request-Method"),
            request.headers().get_one("Authorization"),
        ) {
            (_, Some(auth_header)) => auth_header,
            (true, None) => return Outcome::Success(fake_user_for_cors()), // TODO
            _ => {
                return Outcome::Failure((
                    Status::Unauthorized,
                    "missing authorization header".into(),
                ))
            }
        };

        let id_info = match verify_google_auth_header(auth) {
            Ok(id_info) => id_info,
            Err(e) => {
                println!("Failed to verify auth header: {}", e);
                return Outcome::Failure((
                    Status::Unauthorized,
                    "failed to verify auth header".into(),
                ));
            }
        };

        let db: HibiDbConn = request
            .guard()
            .map_failure(|(s, e)| (s, format!("{:?}", e)))?;

        let user_query = users.filter(ident.eq(id_info.email.unwrap()));
        println!("{}", debug_query::<Pg, _>(&user_query));
        match user_query.first::<User>(&db as &PgConnection).optional() {
            Ok(Some(user)) => Outcome::Success(AuthedUser(user)),
            Ok(None) => Outcome::Failure((Status::Unauthorized, "Not a recognised user".into())),
            Err(e) => Outcome::Failure((
                Status::InternalServerError,
                format!("error loading user: {:?}", e),
            )),
        }
    }
}

fn verify_google_auth_header(header: &str) -> Result<google_signin::IdInfo, String> {
    if !header.starts_with("X-Google ") {
        return Err("unexpected authorization type".into());
    }

    let token = header.replace("X-Google ", "");

    let mut client = google_signin::Client::new();
    client.audiences.push(GOOGLE_CLIENT_ID.into());

    let id_token = client
        .verify(&token)
        .map_err(|e| format!("Couldn't verify token: {}", e))?;

    if id_token.email.is_none() {
        return Err("user has no email address!".into());
    }

    if id_token.email_verified != Some("true".into()) {
        return Err("email is not verified!".into());
    }

    Ok(id_token)
}

#[get("/tasks?<q>&<days>")]
#[allow(unused_variables)] // task_set uses them, can't use underscore or route won't match
fn list_tasks(
    q: TaskSetType,
    days: Option<u8>,
    task_set: TaskSet<Local>,
    db: HibiDbConn,
    user: AuthedUser,
) -> Json<Vec<Task>> {
    println!("task_set={:?}", task_set);

    let requested_tasks: Vec<Task> = task_set
        .query(&db, &user.0)
        // TODO
        .into_iter()
        //.map(|t| t.title)
        .collect();

    // TODO use rocket's built in JSON support?
    // https://rocket.rs/v0.4/guide/responses/#json
    Json(requested_tasks)
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

struct DumbCors;

impl Fairing for DumbCors {
    fn info(&self) -> fairing::Info {
        fairing::Info {
            name: "Dumb CORS",
            kind: fairing::Kind::Request | fairing::Kind::Response,
        }
    }

    fn on_request(&self, request: &mut Request, _data: &rocket::Data) {
        if request.method() == Method::Options {
            if let Some(method) = request.headers().get_one("Access-Control-Request-Method") {
                if let Ok(method) = Method::from_str(method) {
                    request.set_method(method);
                    // TODO tell the handler it doesn't have to do the work!
                }
            }
        }
    }

    fn on_response(&self, _request: &Request, response: &mut Response) {
        response.set_header(AccessControlAllowOrigin::Value(
            "http://localhost:3000".to_string(),
        ));
        response.set_header(AccessControlAllowHeaders(vec!["Authorization".into()]));
    }
}

fn main() {
    let config = config().expect("couldn't configure Rocket");

    rocket::custom(config)
        .attach(HibiDbConn::fairing())
        .attach(DumbCors)
        .mount("/", routes![index, list_tasks])
        .launch();
}
