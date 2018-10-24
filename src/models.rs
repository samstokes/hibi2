#![allow(proc_macro_derive_resolution_fallback)]

#[derive(Queryable, Debug)]
pub struct Task {
    pub id: i32,
    pub title: String,
    pub ext_task_id: Option<i64>,
}

#[derive(Queryable, Debug)]
pub struct ExtTask {
    pub id: i64,
    pub ext_id: String,
    pub ext_source_name: String,
    pub ext_url: Option<String>,
    pub ext_status: Option<String>,
}
