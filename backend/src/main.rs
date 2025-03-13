#[macro_use] extern crate rocket; // exportar dependencias del archivo de cargo.toml

use rocket::{State, Shutdown};
use rocket::http::Method;
use rocket_cors::{AllowedOrigins, CorsOptions};
use rocket_cors::AllowedHeaders;
use rocket::form::Form;
use rocket::response::stream::{EventStream, Event};
use rocket::serde::{Serialize, Deserialize};
use rocket::tokio::sync::broadcast::{channel, Sender, error::RecvError};
use rocket::tokio::select;

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, UriDisplayQuery))]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message: String,
}

#[get("/events")]
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();
    EventStream! {
        loop {
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };
            yield Event::json(&msg);
        }
    }    
}

#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    let _res = queue.send(form.into_inner());
}

#[launch]
fn rocket() -> _ {
    #[cfg(not(debug_assertions))]
    dotenv::dotenv().ok();

    let cors = CorsOptions {
        allowed_origins: AllowedOrigins::default(),
        ..Default::default()
    }
    .to_cors()
    .unwrap();

    #[cfg(not(debug_assertions))]
    let cors = {
        let frontend_url = std::env::var("FRONTEND_URL")
            .expect("FRONTEND_URL must be set in .env");
        
            CorsOptions {
                allowed_origins: AllowedOrigins::some_exact(&[frontend_url]),
                allowed_methods: vec![Method::Get, Method::Post].into_iter().map(From::from).collect(),
                allowed_headers: AllowedHeaders::all(),
                allow_credentials: true,
                ..Default::default()
            }
        .to_cors()
        .unwrap()
    };

    let config = rocket::Config {
        address: std::net::Ipv4Addr::new(0, 0, 0, 0).into(),
        port: 8070,
        ..rocket::Config::default()
    };

    rocket::custom(config)
        .manage(channel::<Message>(1024).0)
        .mount("/", routes![post, events])
        // .mount("/", FileServer::from(relative!("static")))
        .attach(cors)
}