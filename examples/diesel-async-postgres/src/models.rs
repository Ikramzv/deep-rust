use diesel::{
    prelude::{Insertable, Queryable},
    Selectable,
};
use serde::{Deserialize, Serialize};

use crate::schema;

#[derive(Serialize, Queryable, Selectable)]
#[diesel(table_name = schema::posts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = schema::posts)]
pub struct NewPost {
    pub title: String,
    pub body: String,
}
