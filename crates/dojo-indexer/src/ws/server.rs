use std::time::Duration;

use actix::prelude::*;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use serde_json::{json, Value};

async fn ws_index(r: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(AutobahnWebSocket::default(), &r, stream)
}

#[derive(Debug, Clone, Default)]
struct AutobahnWebSocket;

impl Actor for AutobahnWebSocket {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for AutobahnWebSocket {
    fn started(&mut self, ctx: &mut Self::Context) {
        let interval = Duration::from_secs(1);
        ctx.run_interval(interval, |_, ctx| {
            let response = json!({
                "entity": "1",
                "component": "position",
                "data": ["1", "2"]
            });

            let json_string = serde_json::to_string(&response).unwrap();

            ctx.text(json_string);
        });
    }

    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        if let Ok(msg) = msg {
            match msg {
                ws::Message::Text(text) => {
                    // Create the JSON object
                    let response = json!({
                        "entity": "1",
                        "component": "position",
                        "data": ["1", "2"]
                    });

                    // Serialize the JSON object back to a string
                    let json_string = serde_json::to_string(&response).unwrap();

                    // Send the JSON string as a text message
                    ctx.text(json_string);
                }
                ws::Message::Binary(bin) => ctx.binary(bin),
                ws::Message::Ping(bytes) => ctx.pong(&bytes),
                ws::Message::Close(reason) => {
                    ctx.close(reason);
                    ctx.stop();
                }
                _ => {}
            }
        } else {
            ctx.stop();
        }
    }
}

pub async fn start_ws() -> std::io::Result<()> {
    // env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    log::info!("starting HTTP server at http://localhost:9001");

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(web::resource("/").route(web::get().to(ws_index)))
    })
    .workers(2)
    .bind(("127.0.0.1", 9001))?
    .run()
    .await
}
