//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.3

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "message_mark")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    pub msg_id: i64,
    pub uid: i64,
    pub r#type: i32,
    pub status: i32,
    pub create_time: TimeDateTime,
    pub update_time: TimeDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
