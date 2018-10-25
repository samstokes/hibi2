extern crate diesel;
extern crate hibi2;

use std::env;

use self::diesel::prelude::*;

use self::hibi2::*;
use self::models::*;

use diesel::{debug_query, pg::Pg};

fn main() {
    use hibi2::schema::ext_tasks::dsl::*;
    use hibi2::schema::tasks::dsl::*;
    use hibi2::schema::users::dsl::*;

    let user_ident = env::args().nth(1).expect("please specify user email");
    let ext_source = env::args().nth(2);
    let ext_source_hask =
        ext_source.map(|s| format!("ExternalSourceName {{unExternalSourceName = \"{}\"}}", s));

    let connection = establish_connection();

    let user_query = users.filter(ident.eq(user_ident));

    println!("{}", debug_query::<Pg, _>(&user_query));

    let user: User = user_query.first(&connection).expect("error loading user");
    println!("{:?}", user);

    let tasks_with_ext_query = Task::belonging_to(&user)
        .inner_join(ext_tasks)
        .filter(done_at.is_null())
        .order(order);

    println!("{}", debug_query::<Pg, _>(&tasks_with_ext_query));

    let tasks_with_ext = match ext_source_hask {
        None => tasks_with_ext_query
            .load::<(Task, ExtTask)>(&connection)
            .expect("Error loading tasks"),
        Some(source) => tasks_with_ext_query
            .filter(ext_source_name.eq(source))
            .load::<(Task, ExtTask)>(&connection)
            .expect("Error loading tasks"),
    };

    for task_with_ext in tasks_with_ext {
        println!("{:?}", task_with_ext);
    }
}
