use std::{cell::RefCell, future::ready, io, rc::Rc, sync::Arc, time::Instant};

use futures::future::{select, Either};
use ntex::{
    // service::fn_factory_with_config,
    chain,
    channel::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    fn_service,
    rt,
    service::{fn_factory_with_config, fn_shutdown},
    util::Bytes,
    web::{self, ws, HttpRequest},
    ws::{Frame, Message, WsSink},
    Service,
};

use crate::state::{Config, GLOBLE_STORE};

pub struct WsState {
    pub cfg: Arc<Config>,
    pub timeout_count: u8,
    pub heartbeat: Instant,
}

impl WsState {
    pub fn new(cfg: Arc<Config>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            heartbeat: Instant::now(),
            timeout_count: 0,
            cfg,
        }))
    }
}

async fn service(
    sink: WsSink,
) -> Result<impl Service<Frame, Response = Option<Message>, Error = io::Error>, web::Error> {
    let config = GLOBLE_STORE.read().unwrap();
    let cfg = config.as_ref().unwrap();
    let state = WsState::new(cfg.config.clone());
    let (hb_tx, hb_rx) = oneshot::channel();
    let (tx, rx) = mpsc::channel();
    rt::spawn(heart_beat(state, tx.clone(), hb_rx));
    rt::spawn(sender(sink, rx));
    let service = fn_service(move |frame| {
        let item = match frame {
            _ => None,
        };
        ready(Ok(item))
    });
    let on_shutdown = fn_shutdown(move || {
        let _ = hb_tx.send(());
    });
    Ok(chain(service).and_then(on_shutdown))
    // todo!()
}

async fn sender(sink: WsSink, rx: Receiver<Message>) {
    loop {
        let Some(msg) = rx.recv().await else {
            return;
        };
        let Err(e) = sink.send(msg).await else {
            continue;
        };
        tracing::error!("ws recv err: {e:?}");
        break;
    }
}

async fn heart_beat(
    state: Rc<RefCell<WsState>>,
    tx: Sender<Message>,
    mut rx: oneshot::Receiver<()>,
) {
    use Either::*;

    let state = state.borrow();
    loop {
        let Left(_) = select(Box::pin(ntex::time::sleep(state.cfg.heartbeat)), &mut rx).await
        else {
            tracing::info!("close connect");
            return;
        };
        let is_timeout = Instant::now().duration_since(state.heartbeat) > state.cfg.timeout;
        let is_max_retry =
            state.cfg.max_timeout != 0 && state.timeout_count > state.cfg.max_timeout;
        if is_timeout && !is_max_retry {
            let Err(_) = tx.send(Message::Ping(Bytes::default())) else {
                continue;
            };
            return;
        } else if is_max_retry {
            return;
        }
    }
}

pub async fn index(req: HttpRequest) -> Result<web::HttpResponse, web::Error> {
    // todo!()
    ws::start(req, fn_factory_with_config(service)).await
}
