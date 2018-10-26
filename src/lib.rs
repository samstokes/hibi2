#[macro_use]
extern crate diesel;
#[cfg(debug_assertions)]
extern crate dotenv;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate simple_error;

pub mod models;
pub mod schema;
pub mod time;

use diesel::pg::PgConnection;
use diesel::prelude::*;
#[cfg(debug_assertions)]
use dotenv::dotenv;
use std::env;

pub fn establish_connection() -> PgConnection {
    #[cfg(debug_assertions)]
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}
