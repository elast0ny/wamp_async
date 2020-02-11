
use log::*;
use crate::core::*;

pub async fn subscribed(core: &mut Core, request: WampId, sub_id: WampId) -> Status {
    
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
pub async fn unsubscribed(core: &mut Core, request: WampId) -> Status {
    
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
pub async fn published(core: &mut Core, request: WampId, pub_id: WampId) -> Status {
    
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
pub async fn event(core: &mut Core,subscription: WampId,
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
pub async fn registered(core: &mut Core, request: WampId, rpc_id: WampId) -> Status {
    
    let (rpc_func, res) = match core.pending_register.remove(&request) {
        Some(v) => v, 
        None => {
            warn!("Server sent subscribed event for ID we never asked for : {}", request);
            return Status::Ok;
        }
    };

    // Check for ID collision
    if core.rpc_endpoints.contains_key(&rpc_id) {
        warn!("Server sent registered ID we already had registered");
        return Status::Ok;
    }

    // Add the registered ID to our registered rpc map
    let _ = core.rpc_endpoints.insert(rpc_id, rpc_func);

    // Send the rpc info back to the requestor
    let _ = res.send(Ok(rpc_id));

    Status::Ok
}
pub async fn unregisterd(core: &mut Core, request: WampId) -> Status {
    
    let res = match core.pending_transactions.remove(&request) {
        Some(v) => v, 
        None => {
            warn!("Server sent unsolicited unregistered ID : {}", request);
            return Status::Ok;
        }
    };

    // Send the event queue back to the requestor
    let _ = res.send(Ok(None));

    Status::Ok
}
pub async fn invocation(core: &mut Core,
    request: WampId,
    registration: WampId,
    _details: WampDict,
    arguments: Option<WampList>,
    arguments_kw: Option<WampDict>
) -> Status {
    let rpc_func = match core.rpc_endpoints.get(&registration) {
        Some(e) => e,
        None => {
            warn!("Server sent invocation for rpc ID but we do not have this endpoint : {}", registration);
            return Status::Ok;
        }
    };

    let ctl_channel = core.ctl_sender.clone();
    let func_future = rpc_func(arguments, arguments_kw);

    // Forward the event to the client
    if let Err(_) = core.rpc_event_queue_w.send(Box::pin(async move {
        match ctl_channel.send(Request::InvocationResult {
            request: request,
            res: func_future.await,
        }) {
            Ok(_) => Ok(()),
            Err(_) => Err(From::from("Event loop has died !".to_string())), 
        }
    })) {
        warn!("Client not listenning to rpc events but got invocation for rpc ID {}", registration);
        // TODO : Should we be nice and send an UNSUBSCRIBE to the server ?
    }

    Status::Ok
}
pub async fn call_result(core: &mut Core, request: WampId,
    _details: WampDict,
    arguments: Option<WampList>,
    arguments_kw: Option<WampDict>
) -> Status {
    let res = match core.pending_call.remove(&request) {
        Some(r) => r,
        None => {
            warn!("Server sent result for CALL we never sent : request id {}", request);
            return Status::Ok;
        }
    };

    // Forward the event to the client
    if let Err(_) = res.send(Ok((arguments, arguments_kw))) {
        warn!("Client not waiting for call result id {}", request);
        // TODO : Should we be nice and send an UNSUBSCRIBE to the server ?
    }

    Status::Ok
}

pub async fn goodbye(core: &mut Core, details: WampDict, reason: WampString) -> Status {
    debug!("Server sent goodbye : {:?} {:?}", details, reason);

    if !core.valid_session && reason == "wamp.close.goodbye_and_out" {
        Status::Ok
    } else {
        debug!("Peer is closing on us !");
        let _ = core.send(&Msg::Goodbye {
            details: WampDict::new(),
            reason: "wamp.close.goodbye_and_out".to_string()
        }).await;
        Status::Shutdown
    }
}

pub async fn abort(_core: &mut Core, details: WampDict, reason: WampString) -> Status {
    error!("Server sent abort : {:?} {:?}", details, reason);
    Status::Shutdown
}
// Handles an error sent by the peer
pub async fn error(core: &mut Core, typ: WampInteger,
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
        REGISTER_ID => {
            let (_, res) = match core.pending_register.remove(&request) {
                Some(r) => r,
                None => {
                    warn!("Received error for RPC register message we never sent");
                    return Status::Ok;
                }
            };
            let _ = res.send(Err(error));   
        },
        CALL_ID => {
            let res = match core.pending_call.remove(&request) {
                Some(r) => r,
                None => {
                    warn!("Received error for CALL message we never sent");
                    return Status::Ok;
                }
            };
            let _ = res.send(Err(error));   
        },
        PUBLISH_ID | UNSUBSCRIBE_ID | UNREGISTER_ID => {
            let res = match core.pending_transactions.remove(&request) {
                Some(r) => r,
                None => {
                    warn!("Received error for message we never sent");
                    return Status::Ok;
                }
            };
            let _ = res.send(Err(error));
        },
        _ => {},
    };
    Status::Ok
}