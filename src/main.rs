#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;

use rocket::http::Method;
use rocket::State;
use rocket_contrib::json::{Json, JsonValue};
use rocket_cors::{AllowedHeaders, AllowedOrigins};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

type CounterMap = Mutex<HashMap<Uuid, Counter>>;

#[derive(Serialize, Deserialize, Clone, Copy)]
struct Counter {
    id: Uuid,
    value: u32,
}

// General routes

#[get("/")]
fn index() -> JsonValue {
    json!({
        "status": "ok",
        "message": "Welcome to Counter a Service"
    })
}

#[catch(404)]
fn not_found() -> JsonValue {
    json!({
        "status": "error",
        "reason": "Resource was not found."
    })
}

// Counter routes

#[get("/", format = "json")]
fn get_all_counters(map: State<CounterMap>) -> Json<Vec<Counter>> {
    let hashmap = map.lock().unwrap();

    Json(hashmap.iter().map(|v| *v.1).collect())
}

#[post("/", format = "json")]
fn create_counter(map: State<CounterMap>) -> Json<Counter> {
    let mut hashmap = map.lock().expect("map lock.");
    let id = Uuid::new_v4();
    let counter = Counter { id, value: 0 };

    hashmap.insert(id, counter);
    Json(counter)
}

#[get("/<id>", format = "json")]
fn get_counter(id: String, map: State<CounterMap>) -> Option<Json<Counter>> {
    let hashmap = map.lock().unwrap();
    let parsed_uuid = Uuid::parse_str(&id).expect("Invalid id");

    hashmap.get(&parsed_uuid).map(|contents| Json(*contents))
}

#[put("/<id>/increment", format = "json")]
fn increment_counter(id: String, map: State<CounterMap>) -> Option<Json<Counter>> {
    let mut hashmap = map.lock().unwrap();
    let parsed_uuid = Uuid::parse_str(&id).expect("Invalid id");

    let counter = hashmap
        .entry(parsed_uuid)
        .and_modify(|contents| contents.value += 1)
        .or_insert(Counter {
            id: parsed_uuid,
            value: 1,
        });

    Some(Json(*counter))
}

#[put("/<id>/decrement", format = "json")]
fn decrement_counter(id: String, map: State<CounterMap>) -> Option<Json<Counter>> {
    let mut hashmap = map.lock().unwrap();
    let parsed_uuid = Uuid::parse_str(&id).expect("Invalid id");

    let counter = hashmap
        .entry(parsed_uuid)
        .and_modify(|contents| {
            if contents.value > 0 {
                contents.value -= 1
            } else {
                ()
            }
        })
        .or_insert(Counter {
            id: parsed_uuid,
            value: 0,
        });

    Some(Json(*counter))
}

// Setup

fn rocket() -> rocket::Rocket {
    let cors = rocket_cors::CorsOptions {
        allowed_origins: AllowedOrigins::All,
        allowed_methods: vec![Method::Options, Method::Get, Method::Post, Method::Put]
            .into_iter()
            .map(From::from)
            .collect(),
        allowed_headers: AllowedHeaders::some(&["Accept", "Content-Type"]),
        allow_credentials: true,
        ..Default::default()
    }
    .to_cors()
    .unwrap();

    rocket::ignite()
        .mount("/", routes![index])
        .mount(
            "/counter",
            routes![
                get_all_counters,
                create_counter,
                get_counter,
                increment_counter,
                decrement_counter
            ],
        )
        .attach(cors)
        .register(catchers![not_found])
        .manage(Mutex::new(HashMap::<Uuid, Counter>::new()))
}

fn main() {
    rocket().launch();
}

// Tests

#[cfg(test)]
mod test {
    use super::rocket;
    use rocket::http::ContentType;
    use rocket::http::Status;
    use rocket::local::Client;

    use super::Counter;

    #[test]
    fn list_counters() {
        let client = Client::new(rocket()).expect("Init failed");

        client.post("/counter").header(ContentType::JSON).dispatch();
        client.post("/counter").header(ContentType::JSON).dispatch();

        let mut response = client.get(format!("/counter")).dispatch();

        assert_eq!(response.status(), Status::Ok);

        let body_string = response.body_string().unwrap();
        let counters: Vec<Counter> = serde_json::from_str(&body_string).unwrap();

        assert_eq!(counters.len(), 2)
    }

    #[test]
    fn create_counter() {
        let client = Client::new(rocket()).expect("Init failed");
        let mut response = client.post("/counter").header(ContentType::JSON).dispatch();

        assert_eq!(response.status(), Status::Ok);

        let body_as_string = response.body_string().unwrap();
        let counter: Counter = serde_json::from_str(&body_as_string).unwrap();

        assert_eq!(counter.value, 0);
    }

    #[test]
    fn create_and_get_counter() {
        let client = Client::new(rocket()).expect("Init failed");
        let mut post_response = client.post("/counter").header(ContentType::JSON).dispatch();

        assert_eq!(post_response.status(), Status::Ok);

        match post_response.body_string() {
            Some(content) => {
                let counter: Counter = serde_json::from_str(&content).unwrap();
                let get_response = client.get(format!("/counter/{}", counter.id)).dispatch();

                assert_eq!(get_response.status(), Status::Ok);
            }
            None => panic!("Invalid body"),
        };
    }

    #[test]
    fn create_and_increment_counter() {
        let client = Client::new(rocket()).expect("Init failed");
        let mut create_response = client.post("/counter").header(ContentType::JSON).dispatch();

        assert_eq!(create_response.status(), Status::Ok);

        match create_response.body_string() {
            Some(create_response_content) => {
                let counter: Counter = serde_json::from_str(&create_response_content).unwrap();
                let mut increment_response = client
                    .put(format!("/counter/{}/increment", counter.id))
                    .header(ContentType::JSON)
                    .dispatch();

                assert_eq!(increment_response.status(), Status::Ok);

                match increment_response.body_string() {
                    Some(increment_response_content) => {
                        let counter_with_increment: Counter =
                            serde_json::from_str(&increment_response_content).unwrap();

                        assert_eq!(counter_with_increment.value, 1);
                    }
                    None => panic!("Invalid body"),
                };
            }
            None => panic!("Invalid body"),
        };
    }

    #[test]
    fn get_nonexistign_counter() {
        let client = Client::new(rocket()).expect("Init failed");
        let response = client.get("/counters/xyz123").dispatch();

        assert_eq!(response.status(), Status::NotFound);
    }
}
