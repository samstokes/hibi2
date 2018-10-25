extern crate diesel;
extern crate hibi2;

use std::env;

use self::diesel::prelude::*;

use self::hibi2::*;
use self::models::*;

// TODO
use diesel::debug_query;

fn main() {
    use hibi2::schema::ext_tasks::dsl::*;
    use hibi2::schema::tasks::dsl::*;

    let ext_source = env::args().nth(1);
    let ext_source_hask =
        ext_source.map(|s| format!("ExternalSourceName {{unExternalSourceName = \"{}\"}}", s));

    let connection = establish_connection();

    let tasks_with_ext_query = tasks.inner_join(ext_tasks).filter(done_at.is_null());

    println!(
        "{}",
        debug_query::<diesel::pg::Pg, _>(&tasks_with_ext_query)
    );

    let tasks_with_ext = match ext_source_hask {
        None => tasks_with_ext_query
            .order(order)
            .load::<(Task, ExtTask)>(&connection)
            .expect("Error loading tasks"),
        Some(source) => tasks_with_ext_query
            .filter(ext_source_name.eq(source))
            .limit(5)
            .load::<(Task, ExtTask)>(&connection)
            .expect("Error loading tasks"),
    };

    for task_with_ext in tasks_with_ext {
        println!("{:?}", task_with_ext);
    }
}
