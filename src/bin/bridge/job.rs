use serde_json::{Value, json};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

enum Event {
    Callback(Value),
    Done(Value),
}
struct Job {
    callback: String,
    send: mpsc::Sender<Value>,
    receive: mpsc::Receiver<Event>,
}
static JOBS: LazyLock<Mutex<HashMap<u64, Job>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT: AtomicU64 = AtomicU64::new(1);

struct Endpoint {
    send: mpsc::Sender<Event>,
    receive: mpsc::Receiver<Value>,
}
#[derive(Clone)]
pub(super) struct Channel(Rc<Endpoint>);
impl Channel {
    pub(super) fn call(&self, method: &str, args: Value) -> Result<Value, String> {
        self.0
            .send
            .send(Event::Callback(json!({"method":method,"args":args})))
            .map_err(|e| e.to_string())?;
        self.0.receive.recv().map_err(|e| e.to_string())
    }
}

pub(super) fn advance(id: u64, reply: Option<Value>) -> Result<Value, String> {
    let job = JOBS
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or("unknown bridge job")?;
    if let Some(reply) = reply {
        job.send.send(reply).map_err(|e| e.to_string())?;
    }
    match job.receive.recv().map_err(|e| e.to_string())? {
        Event::Done(value) => Ok(value),
        Event::Callback(mut request) => {
            request["job"] = json!(id);
            request["callback"] = json!(job.callback);
            JOBS.lock().unwrap().insert(id, job);
            Ok(request)
        }
    }
}

pub(super) fn start(
    callback: &str,
    run: impl FnOnce(Channel) -> Value + Send + 'static,
) -> Result<Value, String> {
    let (event_send, event_receive) = mpsc::channel();
    let (reply_send, reply_receive) = mpsc::channel();
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    JOBS.lock().unwrap().insert(
        id,
        Job {
            callback: callback.into(),
            send: reply_send,
            receive: event_receive,
        },
    );
    thread::spawn(move || {
        let channel = Channel(Rc::new(Endpoint {
            send: event_send.clone(),
            receive: reply_receive,
        }));
        let result = run(channel);
        let _ = event_send.send(Event::Done(result));
    });
    advance(id, None)
}
