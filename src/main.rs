use legion::*;
use legion::systems::SyncResources;
use futures::{FutureExt, StreamExt};
use warp::{
    ws::{Message, WebSocket},
    reply::{Reply},
    Filter,
    Rejection
};
use std::convert::Infallible;
use std::sync::Arc;
use std::marker::Send;
use tokio::sync::{mpsc, RwLock};

pub struct WorldWrap(World);

impl WorldWrap {
    pub fn modify_state(
        &mut self,
    ) -> &mut World {
        // Do something with world
        &mut self.0
    }
}

type WarpResult<T> = std::result::Result<T, Rejection>;

type Sender = mpsc::UnboundedSender<std::result::Result<warp::filters::ws::Message, warp::Error>>;

type WorldAsync = Arc<RwLock<WorldWrap>>;
type ResourcesAsync<'a> = Arc<RwLock<SyncResources<'a>>>;

pub async fn client_connection(
    ws: WebSocket,
    worldA: WorldAsync,
    resourcesA: ResourcesAsync<'_>,
) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();

    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            eprintln!("error sending websocket msg: {}", e);
        }
    }));

    while let Some(result) = client_ws_rcv.next().await {
        match result {
            Ok(_) => {
                worldA.write().await.modify_state();
                client_sender.send(
                    Ok(Message::text(
                        "pong".to_string()
                    ))
                );
            },
            Err(e) => {
                eprintln!("error receiving ws message: {}", e);
                break;
            }
        };
    }
    println!("disconnected");
}

pub async fn ws_handler(
    ws: warp::ws::Ws,
    worldA: WorldAsync,
    resourcesA: ResourcesAsync<'static>,
) -> WarpResult<impl Reply> {
    Ok (ws.on_upgrade(
        move |socket| client_connection(
            socket,
            worldA,
            resourcesA,
        )
    )
    )
}

fn with_state<T: Clone+Send>(state: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

#[tokio::main]
async fn main() {
    let world = World::default();
    let worldA: WorldAsync = Arc::new(RwLock::new(WorldWrap(world)));

    let mut resources: Resources = Resources::default();
    // resources.insert(Location::new(256, 256));
    let resourcesA: ResourcesAsync = Arc::new(RwLock::new(resources.sync()));

    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(with_state(worldA))
        .and(with_state(resourcesA))
        .and_then(ws_handler);

    warp::serve(ws_route).run(([127, 0, 0, 1], 8000)).await
}
