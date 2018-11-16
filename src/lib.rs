#[macro_use]
extern crate diesel;
#[cfg(debug_assertions)]
extern crate dotenv;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate simple_error;
extern crate sentry;

pub mod models;
pub mod schema;
pub mod time;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use std::env;

pub fn establish_connection() -> PgConnection {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

pub fn init_sentry() -> Option<sentry::internals::ClientInitGuard> {
    env::var("SENTRY_DSN").ok().map(|dsn| {
        let guard = sentry::init(dsn);
        sentry::integrations::panic::register_panic_handler();
        guard
    })
}
