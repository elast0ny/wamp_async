use std::collections::{HashSet, HashMap};

use tokio::sync::oneshot::{Sender};
use log::*;

use crate::common::*;
use crate::message::*;
use crate::core::*;

pub enum Request {
    Shutdown,
    Join {
        uri: WampString,
        roles: HashSet<ClientRole>,
        agent_str: Option<WampString>,
        res: Sender<Result<(WampId, HashMap<WampString, Arg>), WampError>>,
    },
    Leave {
        res: Sender<Result<(), WampError>>
    },
    Subscribe {
        uri: WampString,
        res: PendingSubResult,
    },
    Unsubscribe {
        sub_id: WampId,
        res: Sender<Result<Option<WampId>, WampError>>
    },
    Publish {
        uri: WampString,
        options: WampDict,
        arguments: WampArgs,
        arguments_kw: WampKwArgs,
        res: Sender<Result<Option<WampId>, WampError>>,
    },
    Register {
        uri: WampString,
        res: PendingRegisterResult,
        func_ptr: RpcFunc,
    },
    Unregister {
        rpc_id: WampId,
        res: Sender<Result<Option<WampId>, WampError>>
    },
    InvocationResult {
        request: WampId,
        res: Result<(WampArgs, WampKwArgs), WampError>,
    },
    Call {
        uri: WampString,
        options: WampDict,
        arguments: WampArgs,
        arguments_kw: WampKwArgs,
        res: PendingCallResult,
    },
}

/// Handler for any join realm request. This will send a HELLO and wait for the WELCOME response
pub async fn join_realm(core: &mut Core, uri: WampString, roles: HashSet<ClientRole>, mut agent_str: Option<WampString>, res: JoinResult) -> Status {
    let mut details: WampDict = WampDict::new();
    let mut client_roles: WampDict = WampDict::new();
    // Add all of our roles
    for role in &roles {
        client_roles.insert(role.to_string(), Arg::Dict(WampDict::new()));    
    }
    details.insert("roles".to_owned(), Arg::Dict(client_roles));

    if let Some(agent) = agent_str.take() {
        details.insert("agent".to_owned(), Arg::String(agent));
    }

    // Send hello with our info
    if let Err(e) = core.send(&Msg::Hello {
        realm: uri,
        details: details,
    }).await {
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    // Receive the WELCOME message
    let resp = match core.recv().await {
        Ok(r) => r,
        Err(e) => {
            let _ = res.send(Err(e));
            return Status::Shutdown;
        },
    };

    // Make sure the server responded with the proper message
    let (session_id, server_roles) = match resp {
        Msg::Welcome{session, details} => (session, details),
        m => {
            let _ = res.send(Err(From::from(format!("Server did not respond with WELCOME : {:?}", m))));
            return Status::Shutdown;
        },
    };

    // Return the pertinent info to the caller
    core.valid_session = true;
    let _ = res.send(Ok((session_id, server_roles)));
    return Status::Ok;
}

/// Handler for any leave realm request. This function will send a GOODBYE and wait for a GOODBYE response
pub async fn leave_realm(core: &mut Core, res: Sender<Result<(), WampError>>) -> Status {    

    core.valid_session = false;

    if let Err(e) = core.send(
        &Msg::Goodbye {
            reason: "wamp.close.close_realm".to_string(),
            details: WampDict::new(),
        }
    ).await {
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    let _ = res.send(Ok(()));
    return Status::Ok;
}

pub async fn subscribe(core: &mut Core, topic: WampString, res: PendingSubResult) -> Status {
    let request = core.create_request();

    if let Err(e) = core.send(
        &Msg::Subscribe {
            request,
            topic,            
            options: WampDict::new(),
        }
    ).await {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }
    
    core.pending_sub.insert(request, res);    
    return Status::Ok;
}

pub async fn unsubscribe(core: &mut Core, sub_id: WampId, res: Sender<Result<Option<WampId>, WampError>>) -> Status {
    
    match core.subscriptions.remove(&sub_id) {
        Some(_v) => {/*drop*/},
        None => {
            warn!("Tried to unsubscribe using invalid sub_id : {}", sub_id);
            let _ = res.send(Err(From::from("Tried to unsubscribe from unknown sub_id".to_string())));
            return Status::Ok;
        }
    };

    let request = core.create_request();

    if let Err(e) = core.send(
        &Msg::Unsubscribe {
            request,
            subscription: sub_id,
        }
    ).await {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_transactions.insert(request, res);
    
    return Status::Ok;
}

pub async fn publish(core: &mut Core, uri: WampString,
    options: WampDict,
    arguments: WampArgs,
    arguments_kw: WampKwArgs,
    res: Sender<Result<Option<WampId>, WampError>>) -> Status {
    
    let request = core.create_request();

    if let Err(e) = core.send(
        &Msg::Publish {
            request,
            topic: uri,
            options,
            arguments,
            arguments_kw,
        }
    ).await {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_transactions.insert(request, res);
    
    return Status::Ok;
}

pub async fn register(core: &mut Core, uri: WampString, res: PendingRegisterResult, func_ptr: RpcFunc) -> Status {
    let request = core.create_request();

    if let Err(e) = core.send(
        &Msg::Register {
            request,
            procedure: uri,            
            options: WampDict::new(),
        }
    ).await {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }
    
    core.pending_register.insert(request, (func_ptr, res));    
    return Status::Ok;
}

pub async fn unregister(core: &mut Core, rpc_id: WampId, res: Sender<Result<Option<WampId>, WampError>>) -> Status {
    
    match core.rpc_endpoints.remove(&rpc_id) {
        Some(_v) => {/*drop*/},
        None => {
            warn!("Tried to unregister RPC using invalid ID : {}", rpc_id);
            let _ = res.send(Err(From::from("Tried to unregister RPC using invalid ID".to_string())));
            return Status::Ok;
        }
    };

    let request = core.create_request();

    if let Err(e) = core.send(
        &Msg::Unregister {
            request,
            registration: rpc_id,
        }
    ).await {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_transactions.insert(request, res);
    
    return Status::Ok;
}

pub async fn invoke_yield(core: &mut Core, request: WampId,
    res: Result<(WampArgs, WampKwArgs), WampError>
) -> Status {
    
    let msg: Msg = match res {
        Ok((arguments,arguments_kw)) => {
            Msg::Yield {
                request,
                options: WampDict::new(),
                arguments,
                arguments_kw,
            }
        },
        Err(e) => {
            Msg::Error {
                typ: INVOCATION_ID as WampInteger,
                request,
                details: WampDict::new(),
                error: "wamp.async.rs.rpc.failed".to_string(),
                arguments: Some(vec![Arg::String(format!("{:?}", e))]),
                arguments_kw: None,
            }
        }
    };
    if let Err(_) = core.send(&msg).await {
        return Status::Shutdown;
    }
    
    return Status::Ok;
}

pub async fn call(core: &mut Core, uri: WampString,
    options: WampDict,
    arguments: WampArgs,
    arguments_kw: WampKwArgs,
    res: PendingCallResult) -> Status {
    
    let request = core.create_request();

    if let Err(e) = core.send(
        &Msg::Call {
            request,
            procedure: uri,
            options,
            arguments,
            arguments_kw,
        }
    ).await {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_call.insert(request, res);
    
    return Status::Ok;
}