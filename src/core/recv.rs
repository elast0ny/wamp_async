
use log::*;
use crate::core::*;

pub async fn subscribed(core: &mut Connection, request: WampId, sub_id: WampId) -> Status {
    
    let res = match core.pending_sub.remove(&request) {
        Some(v) => v, 
        None => {
            warn!("Server sent subscribed event for ID we never asked for : {}", request);
            return Status::Ok;
        }
    };

    if core.subscriptions.contains_key(&sub_id) {
        warn!("Server sent subcribed event for ID we already we subscribed to...");
        return Status::Ok;
    }

    // Add the subscription ID to our subscription map
    let (evt_queue_w, evt_queue_r) = mpsc::unbounded_channel();
    let _ = core.subscriptions.insert(sub_id, evt_queue_w);

    // Send the event queue back to the requestor
    let _ = res.send(Ok((sub_id, evt_queue_r)));

    Status::Ok
}
pub async fn unsubscribed(core: &mut Connection, request: WampId) -> Status {
    
    let res = match core.pending_transactions.remove(&request) {
        Some(v) => v, 
        None => {
            warn!("Server sent unsubscribed event for ID we never asked for : {}", request);
            return Status::Ok;
        }
    };

    // Send the event queue back to the requestor
    let _ = res.send(Ok(None));

    Status::Ok
}
pub async fn published(core: &mut Connection, request: WampId, pub_id: WampId) -> Status {
    
    let res = match core.pending_transactions.remove(&request) {
        Some(v) => v, 
        None => {
            warn!("Server sent published event for ID we never asked for : {}", request);
            return Status::Ok;
        }
    };
    let _ = res.send(Ok(Some(pub_id)));

    Status::Ok
}
pub async fn event(core: &mut Connection,subscription: WampId,
    publication: WampId,
    _details: WampDict,
    arguments: Option<WampList>,
    arguments_kw: Option<WampDict>
) -> Status {
    let evt_queue = match core.subscriptions.get(&subscription) {
        Some(e) => e,
        None => {
            warn!("Server sent event for sub ID we are not subscribed to : {}", subscription);
            return Status::Ok;
        }
    };

    // Forward the event to the client
    if let Err(_) = evt_queue.send((publication, arguments, arguments_kw)) {
        warn!("Client not listenning to subscription {} but did not unsubscribe...", subscription);
        // TODO : Should we be nice and send an UNSUBSCRIBE to the server ?
    }

    Status::Ok
}
// Handles an error sent by the peer
pub async fn error(core: &mut Connection, typ: WampInteger,
    request: WampId,
    details: WampDict,
    error: WampUri,
    _arguments: Option<WampList>,
    _arguments_kw: Option<WampDict>
) -> Status {

    let error = WampError::ServerError(error, details);
    match typ as WampId {
        SUBSCRIBE_ID => {
            let res = match core.pending_sub.remove(&request) {
                Some(r) => r,
                None => {
                    warn!("Received error for subscribe message we never sent");
                    return Status::Ok;
                }
            };
            let _ = res.send(Err(error));      
        },
        UNSUBSCRIBE_ID => {
            let res = match core.pending_transactions.remove(&request) {
                Some(r) => r,
                None => {
                    warn!("Received error for unsubscribe message we never sent");
                    return Status::Ok;
                }
            };
            let _ = res.send(Err(error));
        },
        PUBLISH_ID => {
            let res = match core.pending_transactions.remove(&request) {
                Some(r) => r,
                None => {
                    warn!("Received error for publish message we never sent");
                    return Status::Ok;
                }
            };
            let _ = res.send(Err(error));
        }
        _ => {},
    };
    Status::Ok
}