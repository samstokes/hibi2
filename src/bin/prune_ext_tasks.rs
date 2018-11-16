extern crate clap;
extern crate hibi2;

use self::hibi2::*;
use self::models::*;

use chrono::FixedOffset;
use clap::{App, Arg};
use diesel::dsl::{any, delete};
use diesel::prelude::*;
use diesel::{debug_query, pg::Pg};
#[cfg(debug_assertions)]
use dotenv::dotenv;

use std::process;

fn main() {
    use hibi2::schema::ext_tasks::dsl::*;
    use hibi2::schema::tasks::dsl::*;
    use hibi2::schema::users::dsl::*;

    #[cfg(debug_assertions)]
    dotenv().ok();

    let _guard = init_sentry();

    let matches = App::new("prune_ext_tasks")
        .arg(
            Arg::with_name("USER")
                .help("ident of the user whose tasks to prune")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("SOURCE")
                .help("ext_source of the tasks to prune")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::with_name("actually")
                .help("whether to actually delete things")
                .long("actually"),
        )
        .get_matches();

    let user_ident = matches.value_of("USER").unwrap();
    let ext_source = matches.value_of("SOURCE").unwrap();
    let ext_source_hask = to_hask_newtype("ExternalSourceName", &ext_source);
    let actually = matches.is_present("actually");

    let connection = establish_connection();

    let user_query = users.filter(ident.eq(user_ident));

    println!("{}", debug_query::<Pg, _>(&user_query));

    let user: User = user_query.first(&connection).expect("error loading user");
    let offset = user.time_zone_offset();
    println!("{} with time zone {}", user, offset);

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
            println!("stale: {}, {}", task, ext_task);
            stale_task_ids.push(task.id);
            stale_ext_task_ids.push(ext_task.id);
        } else {
            println!("not stale: {}, {}", task, ext_task);
        }
    }

    if stale_task_ids.is_empty() {
        println!("No stale tasks, nothing to do.");
        process::exit(0);
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

    if actually {
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
    } else {
        println!("Not deleting tasks because --actually was not specified.");
    }
}
