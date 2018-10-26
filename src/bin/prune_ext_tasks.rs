extern crate diesel;
extern crate hibi2;

use std::env;

use self::diesel::dsl::{any, delete};
use self::diesel::prelude::*;

use self::hibi2::*;
use self::models::*;

use chrono::FixedOffset;
use diesel::{debug_query, pg::Pg};

static USAGE: &'static str = "Usage: prune_ext_tasks <email> <ext_source>";

fn main() {
    use hibi2::schema::ext_tasks::dsl::*;
    use hibi2::schema::tasks::dsl::*;
    use hibi2::schema::users::dsl::*;

    let user_ident = env::args()
        .nth(1)
        .expect(&format!("please specify user email\n{}", USAGE));
    let ext_source = env::args()
        .nth(2)
        .expect(&format!("please specify ext_source.\n{}", USAGE));
    let ext_source_hask = format!(
        "ExternalSourceName {{unExternalSourceName = \"{}\"}}",
        ext_source
    );

    let connection = establish_connection();

    let user_query = users.filter(ident.eq(user_ident));

    println!("{}", debug_query::<Pg, _>(&user_query));

    let user: User = user_query.first(&connection).expect("error loading user");
    let offset = user.time_zone_offset();
    println!("{:?} with time zone {:?}", user, offset);

    let tasks_with_ext_query = Task::belonging_to(&user)
        .inner_join(ext_tasks)
        .filter(ext_source_name.eq(ext_source_hask))
        .filter(done_at.is_null())
        .order(order);

    println!("{}", debug_query::<Pg, _>(&tasks_with_ext_query));

    let tasks_with_ext = tasks_with_ext_query
        .load::<(Task, ExtTask)>(&connection)
        .expect("Error loading tasks");

    let mut stale_task_ids = Vec::<i32>::new();
    let mut stale_ext_task_ids = Vec::<i64>::new();

    for (task, ext_task) in tasks_with_ext {
        let zoned_task = task.in_time_zone::<FixedOffset>(&offset);
        let stale = zoned_task.is_overdue_now();
        if stale {
            println!("stale: {:?}, {:?}", task, ext_task);
            stale_task_ids.push(task.id);
            stale_ext_task_ids.push(ext_task.id);
        } else {
            println!("not stale: {:?}, {:?}", task, ext_task);
        }
    }

    let delete_task_query = delete(
        Task::belonging_to(&user).filter(hibi2::schema::tasks::dsl::id.eq(any(stale_task_ids))),
    );
    println!("{}", debug_query::<Pg, _>(&delete_task_query));

    let delete_ext_task_query = delete(
        ExtTask::belonging_to(&user)
            .filter(hibi2::schema::ext_tasks::dsl::id.eq(any(stale_ext_task_ids))),
    );
    println!("{}", debug_query::<Pg, _>(&delete_ext_task_query));

    connection
        .transaction::<(), diesel::result::Error, _>(|| {
            println!("{} tasks deleted", delete_task_query.execute(&connection)?);
            println!(
                "{} ext_tasks deleted",
                delete_ext_task_query.execute(&connection)?
            );

            Ok(())
        })
        .expect("failed to delete tasks");
}
