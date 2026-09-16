use super::{Contact, Control, Error, Event, Result, control, session};
use std::{
    collections::HashMap,
    net::IpAddr,
    time::{Duration, Instant},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, watch},
    task::{JoinHandle, JoinSet},
};

pub struct Endpoint {
    pub address: String,
    pub contact: watch::Sender<Contact>,
    commands: mpsc::Sender<(Vec<String>, session::Request)>,
    task: JoinHandle<()>,
}
impl Endpoint {
    pub async fn bind(
        address: &str,
        local: Contact,
        events: mpsc::Sender<Event>,
    ) -> crate::Result<Self> {
        let listener = TcpListener::bind(address).await?;
        let address = listener.local_addr()?.to_string();
        let (contact, identity) = watch::channel(local);
        let (commands, mut jobs) = mpsc::channel::<(Vec<String>, session::Request)>(4);
        let task = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            let mut recent = HashMap::<IpAddr, Instant>::new();
            loop {
                tokio::select! {
                    next = listener.accept() => {
                        let Ok((stream, address)) = next else { tokio::time::sleep(Duration::from_secs(1)).await; continue; };
                        recent.retain(|_,at| at.elapsed()<Duration::from_secs(3));
                        if tasks.len()>=8 || recent.len()>=256 || recent.contains_key(&address.ip()) { continue; }
                        recent.insert(address.ip(),Instant::now());
                        let Ok(id) = crate::new_session_id() else { continue; };
                        let (handle,actions,cancelled) = control();
                        tasks.spawn(session::run(session::Request {id,control:handle,actions,cancelled},stream,identity.borrow().clone(),events.clone(),false));
                    }
                    Some((addresses,mut request)) = jobs.recv() => {
                        let events=events.clone(); let local=identity.borrow().clone();
                        if tasks.len()>=8 { let _ = events.try_send(Event::Failed{id:request.id,error:Error::Busy}); continue; }
                        tasks.spawn(async move {
                            let connect = async {
                                for address in addresses {
                                    if let Ok(Ok(stream)) = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(address)).await { return Some(stream); }
                                }
                                None
                            };
                            let stream = tokio::select! {
                                result = tokio::time::timeout(Duration::from_secs(15), connect) => result.ok().flatten(),
                                _ = request.cancelled.changed() => None,
                            };
                            if let Some(stream) = stream { session::run(request,stream,local,events,true).await; }
                            else { let _ = events.send(Event::Failed{id:request.id,error:Error::Connect}).await; }
                        });
                    }
                    _ = tasks.join_next(), if !tasks.is_empty() => {}
                }
            }
        });
        Ok(Self {
            address,
            contact,
            commands,
            task,
        })
    }
    pub fn connect(&self, id: String, addresses: Vec<String>) -> Result<Control> {
        let (handle, actions, cancelled) = control();
        self.commands
            .try_send((
                addresses,
                session::Request {
                    id,
                    control: handle.clone(),
                    actions,
                    cancelled,
                },
            ))
            .map_err(|_| Error::Busy)?;
        Ok(handle)
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.task.abort();
    }
}
