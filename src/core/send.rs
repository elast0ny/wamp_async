use std::collections::{HashMap, HashSet};

use log::*;
use tokio::sync::oneshot::Sender;

use crate::common::*;
use crate::core::*;
use crate::message::*;

pub type JoinRealmResult = Result<(WampId, HashMap<WampString, Arg>), WampError>;
pub enum Request<'a> {
    Shutdown,
    Join {
        uri: WampString,
        roles: HashSet<ClientRole>,
        agent_str: Option<WampString>,
        authentication_methods: Vec<AuthenticationMethod>,
        authentication_id: Option<WampString>,
        authextra: Option<HashMap<String, String>>,
        on_challenge_handler: Option<AuthenticationChallengeHandler<'a>>,
        res: Sender<JoinRealmResult>,
    },
    Leave {
        res: Sender<Result<(), WampError>>,
    },
    Subscribe {
        uri: WampString,
        res: PendingSubResult,
    },
    Unsubscribe {
        sub_id: WampId,
        res: Sender<Result<Option<WampId>, WampError>>,
    },
    Publish {
        uri: WampString,
        options: WampDict,
        arguments: Option<WampArgs>,
        arguments_kw: Option<WampKwArgs>,
        res: Sender<Result<Option<WampId>, WampError>>,
    },
    Register {
        uri: WampString,
        res: PendingRegisterResult,
        func_ptr: RpcFunc<'a>,
    },
    Unregister {
        rpc_id: WampId,
        res: Sender<Result<Option<WampId>, WampError>>,
    },
    InvocationResult {
        request: WampId,
        res: Result<(Option<WampArgs>, Option<WampKwArgs>), WampError>,
    },
    Call {
        uri: WampString,
        options: WampDict,
        arguments: Option<WampArgs>,
        arguments_kw: Option<WampKwArgs>,
        res: PendingCallResult,
    },
}

/// Handler for any join realm request. This will send a HELLO and wait for the WELCOME response
pub async fn join_realm(
    core: &mut Core<'_>,
    uri: WampString,
    roles: HashSet<ClientRole>,
    agent_str: Option<WampString>,
    authentication_methods: Vec<AuthenticationMethod>,
    authextra: Option<HashMap<String, String>>,
    authid: Option<WampString>,
    on_challenge_handler: Option<AuthenticationChallengeHandler<'_>>,
    res: JoinResult,
) -> Status {
    let mut details: WampDict = WampDict::new();
    let mut client_roles: WampDict = WampDict::new();
    // Add all of our roles
    for role in &roles {
        client_roles.insert(String::from(role.to_str()), Arg::Dict(WampDict::new()));
    }
    details.insert("roles".to_owned(), Arg::Dict(client_roles));

    if let Some(agent) = agent_str {
        details.insert("agent".to_owned(), Arg::String(agent));
    }

    if !authentication_methods.is_empty() {
        details.insert(
            "authmethods".to_owned(),
            Arg::List(
                authentication_methods
                    .iter()
                    .map(|authentication_method| {
                        Arg::String(authentication_method.as_ref().to_owned())
                    })
                    .collect::<Vec<_>>(),
            ),
        );
        if let Some(extra) = authextra {
            let a: WampDict = WampDict::from([
                ("pubkey".to_owned(), Arg::String(String::from(extra.get("pubkey").unwrap().to_owned()))),
            ]);
            details.insert("authextra".to_owned(), Arg::Dict(a));
        }
    }

    if let Some(authid) = authid {
        details.insert("authid".to_owned(), Arg::String(authid));
    }

    // Send hello with our info
    if let Err(e) = core
        .send(&Msg::Hello {
            realm: uri,
            details,
        })
        .await
    {
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    // Make sure the server responded with the proper message
    let (session_id, server_roles) = loop {
        // Receive the response to the HELLO message (either WELCOME or CHALLENGE are expected)
        let resp = match core.recv().await {
            Ok(r) => r,
            Err(e) => {
                let _ = res.send(Err(e));
                return Status::Shutdown;
            }
        };

        match resp {
            Msg::Welcome { session, details } => break (session, details),
            Msg::Challenge {
                authentication_method,
                extra,
            } => {
                if let Some(ref on_challenge_handler) = on_challenge_handler {
                    match on_challenge_handler(authentication_method, extra).await {
                        Ok(AuthenticationChallengeResponse { signature, extra }) => {
                            if let Err(e) = core.send(&Msg::Authenticate { signature, extra }).await
                            {
                                let _ = res.send(Err(e));
                                return Status::Shutdown;
                            }
                        }
                        Err(e) => {
                            let _ = res.send(Err(e));
                            return Status::Shutdown;
                        }
                    }
                } else {
                    let _ = res.send(Err(From::from(
                        "Server requested a CHALLENGE to authenticate, but there was no challenge handler provided".to_string()
                    )));
                    return Status::Shutdown;
                }
            }
            m => {
                let _ = res.send(Err(From::from(format!(
                    "Server did not respond with WELCOME : {:?}",
                    m
                ))));
                return Status::Shutdown;
            }
        }
    };

    // Return the pertinent info to the caller
    core.valid_session = true;
    let _ = res.send(Ok((session_id, server_roles)));

    Status::Ok
}

/// Handler for any leave realm request. This function will send a GOODBYE and wait for a GOODBYE response
pub async fn leave_realm(core: &mut Core<'_>, res: Sender<Result<(), WampError>>) -> Status {
    core.valid_session = false;

    if let Err(e) = core
        .send(&Msg::Goodbye {
            reason: "wamp.close.close_realm".to_string(),
            details: WampDict::new(),
        })
        .await
    {
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    let _ = res.send(Ok(()));

    Status::Ok
}

pub async fn subscribe(core: &mut Core<'_>, topic: WampString, res: PendingSubResult) -> Status {
    let request = core.create_request();

    if let Err(e) = core
        .send(&Msg::Subscribe {
            request,
            topic,
            options: WampDict::new(),
        })
        .await
    {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_sub.insert(request, res);

    Status::Ok
}

pub async fn unsubscribe(
    core: &mut Core<'_>,
    sub_id: WampId,
    res: Sender<Result<Option<WampId>, WampError>>,
) -> Status {
    match core.subscriptions.remove(&sub_id) {
        Some(_v) => { /*drop*/ }
        None => {
            warn!("Tried to unsubscribe using invalid sub_id : {}", sub_id);
            let _ = res.send(Err(From::from(
                "Tried to unsubscribe from unknown sub_id".to_string(),
            )));
            return Status::Ok;
        }
    };

    let request = core.create_request();

    if let Err(e) = core
        .send(&Msg::Unsubscribe {
            request,
            subscription: sub_id,
        })
        .await
    {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_transactions.insert(request, res);

    Status::Ok
}

pub async fn publish(
    core: &mut Core<'_>,
    uri: WampString,
    options: WampDict,
    arguments: Option<WampArgs>,
    arguments_kw: Option<WampKwArgs>,
    res: Sender<Result<Option<WampId>, WampError>>,
) -> Status {
    let request = core.create_request();

    if let Err(e) = core
        .send(&Msg::Publish {
            request,
            topic: uri,
            options,
            arguments,
            arguments_kw,
        })
        .await
    {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_transactions.insert(request, res);

    Status::Ok
}

pub async fn register<'a>(
    core: &mut Core<'a>,
    uri: WampString,
    res: PendingRegisterResult,
    func_ptr: RpcFunc<'a>,
) -> Status {
    let request = core.create_request();

    if let Err(e) = core
        .send(&Msg::Register {
            request,
            procedure: uri,
            options: WampDict::new(),
        })
        .await
    {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_register.insert(request, (func_ptr, res));
    Status::Ok
}

pub async fn unregister(
    core: &mut Core<'_>,
    rpc_id: WampId,
    res: Sender<Result<Option<WampId>, WampError>>,
) -> Status {
    match core.rpc_endpoints.remove(&rpc_id) {
        Some(_v) => { /*drop*/ }
        None => {
            warn!("Tried to unregister RPC using invalid ID : {}", rpc_id);
            let _ = res.send(Err(From::from(
                "Tried to unregister RPC using invalid ID".to_string(),
            )));
            return Status::Ok;
        }
    };

    let request = core.create_request();

    if let Err(e) = core
        .send(&Msg::Unregister {
            request,
            registration: rpc_id,
        })
        .await
    {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_transactions.insert(request, res);

    Status::Ok
}

pub async fn invoke_yield(
    core: &mut Core<'_>,
    request: WampId,
    res: Result<(Option<WampArgs>, Option<WampKwArgs>), WampError>,
) -> Status {
    let msg: Msg = match res {
        Ok((arguments, arguments_kw)) => Msg::Yield {
            request,
            options: WampDict::new(),
            arguments,
            arguments_kw,
        },
        Err(e) => Msg::Error {
            typ: INVOCATION_ID as WampInteger,
            request,
            details: WampDict::new(),
            error: "wamp.async.rs.rpc.failed".to_string(),
            arguments: Some(vec![format!("{:?}", e).into()]),
            arguments_kw: None,
        },
    };
    if core.send(&msg).await.is_err() {
        return Status::Shutdown;
    }

    Status::Ok
}

pub async fn call(
    core: &mut Core<'_>,
    uri: WampString,
    options: WampDict,
    arguments: Option<WampArgs>,
    arguments_kw: Option<WampKwArgs>,
    res: PendingCallResult,
) -> Status {
    let request = core.create_request();

    if let Err(e) = core
        .send(&Msg::Call {
            request,
            procedure: uri,
            options,
            arguments,
            arguments_kw,
        })
        .await
    {
        core.pending_requests.remove(&request);
        let _ = res.send(Err(e));
        return Status::Shutdown;
    }

    core.pending_call.insert(request, res);

    Status::Ok
}
