use ntex::{
    util::Bytes,
    web::{
        self,
        types::{self, Query, State},
        App, HttpResponse, HttpServer,
    },
};
use serde::Deserialize;
use sqlx::{
    query, query_as, query_scalar, sqlite::SqliteConnectOptions, Executor, FromRow, Pool, Sqlite,
};
use tracing::warn;

trait Merge<O> {
    fn merge(self) -> O;
}

impl<O> Merge<O> for Result<O, O> {
    fn merge(self) -> O {
        match self {
            Ok(o) => o,
            Err(o) => o,
        }
    }
}

macro_rules! user_err {
    ($w:expr, $e:expr) => {
        warn!("{}:{}:{} => {}", file!(), line!(), $w, $e)
    };
}

#[derive(Debug, FromRow, serde::Serialize)]
struct Index {
    id: u32,
    name: String,
}

#[web::get("/")]
async fn get_root(db: State<Pool<Sqlite>>) -> impl web::Responder {
    // db
    query_as("select id, name from schedule")
        .fetch_all(&*db)
        .await
        .map(|o: Vec<Index>| HttpResponse::Ok().json(&o))
        .map_err(|e| {
            user_err!("读取数据库错误", e);
            "数据库错误"
        })
        .map_err(|e| HttpResponse::InternalServerError().body(e))
        .merge()
}

#[derive(Debug, Deserialize)]
struct ChangeQuery {
    name: String,
}

#[web::post("/")]
async fn push(
    state: State<Pool<Sqlite>>,
    body: Bytes,
    query: Query<ChangeQuery>,
) -> impl web::Responder {
    let data = match String::from_utf8(body.to_vec()) {
        Ok(o) => o,
        Err(e) => {
            user_err!("字符串转换错误", e);
            return HttpResponse::BadRequest().body("不是一个 utf8 字符串");
        }
    };

    query_scalar("insert into schedule(name, data) values ($1, $2) returning id")
        .bind(&query.name)
        .bind(data)
        .fetch_one(&*state)
        .await
        .map(|o: u32| HttpResponse::Ok().body(o.to_string()))
        .map_err(|e| {
            user_err!("数据库错误", e);
            "数据库错误"
        })
        .map_err(|e| HttpResponse::Ok().body(e))
        .merge()
}

#[web::post("/{id}")]
async fn change(
    db: State<Pool<Sqlite>>,
    id: types::Path<(u32,)>,
    query: Query<ChangeQuery>,
    body: Bytes,
) -> impl web::Responder {
    let data = match String::from_utf8(body.to_vec()) {
        Ok(o) => o,
        Err(e) => {
            user_err!("字符串转换错误", e);
            return HttpResponse::BadRequest().body("不是一个 utf8 字符串");
        }
    };

    query_scalar("update schedule set name=$1, data=$2 where id = $3 returning id")
        .bind(&query.name)
        .bind(data)
        .bind(id.0)
        .fetch_one(&*db)
        .await
        .map_err(|e| {
            user_err!("数据库错误", e);
            "数据库错误"
        })
        .map_err(|e| HttpResponse::InternalServerError().body(e))
        .map(|o: u32| HttpResponse::Ok().body(o.to_string()))
        .merge()
}

#[derive(Debug, FromRow)]
struct Data {
    data: String,
}

#[web::get("/{id}")]
async fn get(db: State<Pool<Sqlite>>, id: types::Path<(u32,)>) -> impl web::Responder {
    query_as::<_, Data>("select data from schedule where id = $1")
        .bind(id.0)
        .fetch_optional(&*db)
        .await
        .map(|opt| match opt {
            Some(data) => HttpResponse::Ok().body(data.data),
            None => HttpResponse::NotFound().body("未找到数据"),
        })
        .map_err(|e| {
            user_err!("数据库错误", e);
            HttpResponse::InternalServerError().body("数据库错误")
        })
        .merge()
}

#[web::delete("/{id}")]
async fn del(db: State<Pool<Sqlite>>, id: types::Path<(u32,)>) -> impl web::Responder {
    let Err(e) = query("delete from schedule where id = $1")
        .bind(id.0)
        .execute(&*db)
        .await
    else {
        return HttpResponse::Ok().body("ok");
    };
    user_err!("数据库错误", e);
    HttpResponse::InternalServerError().body("数据库错误")
}

async fn init_db() -> Pool<Sqlite> {
    let options = SqliteConnectOptions::new()
        .create_if_missing(true)
        .filename("db.sqlite3");
    let pool = Pool::connect_with(options).await.unwrap();
    pool.execute(
        r"
        create table if not exists schedule(
            id integer primary key autoincrement not null,
            name text not null,
            data text not null
        )
    ",
    )
    .await
    .unwrap();
    pool
}

#[ntex::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let db = init_db().await;
    let app = move || {
        App::new()
            .state(db.clone())
            .service(get_root)
            .service(push)
            .service(change)
            .service(get)
            .service(del)
    };
    HttpServer::new(app)
        .bind("0.0.0.0:5800")
        .unwrap()
        .run()
        .await
        .unwrap()
}
