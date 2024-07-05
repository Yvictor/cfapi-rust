use rsolace::solclient::{SessionProps, SolClient};
use rsolace::solmsg::SolMsg;
use rsolace::types::SolClientLogLevel;
use serde::Serialize;
use tracing::{error, info};

use super::{Formated, FormaterExt, SinkError, SinkExt};

// #[derive(Debug)]
pub struct SolaceSink {
    solclient: SolClient,
    // props: SessionProps,
}

impl Default for SolaceSink {
    fn default() -> Self {
        let props = SessionProps::default()
            .host(&dotenvy::var("SOLACE_HOST").unwrap_or_else(|_| "localhost".to_string()))
            .vpn(&dotenvy::var("SOLACE_VPN").unwrap_or_else(|_| "default".to_string()))
            .username(&dotenvy::var("SOLACE_USERNAME").unwrap_or_else(|_| "default".to_string()))
            .password(&dotenvy::var("SOLACE_PASSWORD").unwrap_or_else(|_| "default".to_string()))
            // .client_name(
            // &dotenvy::var("SOLACE_CLIENT_NAME").unwrap_or_else(|_| "default".to_string()),
            // ) maybe use ap and thread id
            .reapply_subscriptions(
                dotenvy::var("SOLACE_REAPPLY_SUBSCRIPTIONS")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse::<bool>()
                    .unwrap(),
            )
            .connect_retries(
                dotenvy::var("SOLACE_CONNECT_RETRIES")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse::<u32>()
                    .unwrap(),
            )
            .connect_timeout_ms(
                dotenvy::var("SOLACE_CONNECT_TIMEOUT_MS")
                    .unwrap_or_else(|_| "3000".to_string())
                    .parse::<u32>()
                    .unwrap(),
            )
            .compression_level(
                dotenvy::var("SOLACE_COMPRESSION_LEVEL")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse::<u32>()
                    .unwrap(),
            );
        Self::new(props)
    }
}

impl SolaceSink {
    pub fn new(props: SessionProps) -> Self {
        let mut solclient = SolClient::new(SolClientLogLevel::Warning).unwrap();
        info!("SolaceSink created: {:?}", props);
        let r = solclient.connect(props);
        if r {
            info!("SolaceSink connected");
        } else {
            error!("SolaceSink connect error");
        }
        let event_recv = solclient.get_event_receiver();
        let _th_event = std::thread::spawn(move || loop {
            match event_recv.recv() {
                Ok(event) => {
                    tracing::info!("{:?}", event);
                }
                Err(e) => {
                    tracing::error!("recv event error: {:?}", e);
                    break;
                }
            }
        });
        // TODO event handle

        Self { solclient }
    }
}

impl<In: Serialize> SinkExt<In> for SolaceSink {
    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>) {
        let formated = formater.format(input);
        match formated {
            Ok(formated) => match formated {
                Formated::String(s) => {
                    let mut msg = SolMsg::new().unwrap();
                    msg.set_binary_attachment(s.as_bytes());
                    msg.set_topic("sw/api/v1/tick");
                    msg.set_user_prop("ct", "json", 20);
                    let _r = self.solclient.send_msg(&msg);
                    // let topic = dotenvy::var("SOLACE_TOPIC").unwrap_or_else(|_| "default".to_string());
                    // let r = self.solclient.send_message(&topic, &s);
                    // if r {
                    //     info!("SolaceSink send message success");
                    // } else {
                    //     error!("SolaceSink send message error");
                    // }
                }
                Formated::Bytes(b) => {
                    let mut msg = SolMsg::new().unwrap();
                    msg.set_binary_attachment(&b);
                    msg.set_topic("sw/api/v1/tick");
                    msg.set_user_prop("ct", "msgpack", 20);
                    let _r = self.solclient.send_msg(&msg);
                    // let topic = dotenvy::var("SOLACE_TOPIC").unwrap_or_else(|_| "default".to_string());
                    // let r = self.solclient.send_message(&topic, &b);
                    // if r {
                    //     info!("SolaceSink send message success");
                    // } else {
                    //     error!("SolaceSink send message error");
                    // }
                }
            },
            Err(e) => {
                eprintln!("Format Error: {}", e);
            }
        }
        // let json = serde_json::to_string(input).unwrap();
        // let topic = dotenvy::var("SOLACE_TOPIC").unwrap_or_else(|_| "default".to_string());
        // let r = self.solclient.send_message(&topic, &json);
        // if r {
        //     info!("SolaceSink send message success");
        // } else {
        //     error!("SolaceSink send message error");
        // }
    }
}
