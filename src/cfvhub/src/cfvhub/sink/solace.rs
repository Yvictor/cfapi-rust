use rsolace::solclient::{SessionProps, SolClient};
use rsolace::solmsg::SolMsg;
use rsolace::types::{SolClientLogLevel, SolClientReturnCode};
use serde::Serialize;
use tracing::{error, info};

use super::{Dest, Formated, FormaterExt, SinkError, SinkExt};

// #[derive(Debug)]
#[derive(Serialize)]
pub struct SolaceSink {
    #[serde(skip)]
    solclient: SolClient,
    id: String,
    // props: SessionProps,
}

fn load_session_props_from_dotenv() -> SessionProps {
    SessionProps::default()
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
        )
}

impl Default for SolaceSink {
    fn default() -> Self {
        let props = load_session_props_from_dotenv();
        Self::new(props, "default")
    }
}

impl SolaceSink {
    pub fn new(props: SessionProps, id: &str) -> Self {
        let mut solclient = SolClient::new(SolClientLogLevel::Warning).unwrap();
        // info!("SolaceSink created: {:?}", props);
        let r = solclient.connect(props);
        if r {
            info!("SolaceSink {} connected", id);
        } else {
            error!("SolaceSink {} connect error", id);
        }
        let event_recv = solclient.get_event_receiver();
        let id_th = id.to_string();
        let _th_event = std::thread::spawn(move || loop {
            match event_recv.recv() {
                Ok(event) => {
                    tracing::info!("SolaceSink {} {:?}", id_th, event);
                }
                Err(e) => {
                    tracing::error!("SolaceSink {} recv event error: {:?}", id_th, e);
                    break;
                }
            }
        });
        // TODO event handle

        Self {
            solclient,
            id: id.to_string(),
        }
    }
}

impl<In: Serialize + Dest> SinkExt<In> for SolaceSink {
    fn build(id: &str) -> Self {
        let props = load_session_props_from_dotenv();
        Self::new(props, id)
    }

    fn exec(&mut self, input: &In, formater: &impl FormaterExt<In>) {
        let dest = input.get_dest();
        let content_type = formater.content_type();
        let formated = formater.format(input);
        match formated {
            Ok(formated) => {
                let mut msg = SolMsg::new().unwrap();
                msg.set_topic(dest);
                msg.set_user_prop("ct", content_type, 20);
                let r = match formated {
                    Formated::String(s) => {
                        println!("topic: {} data: {}", dest, s);
                        msg.set_binary_attachment(s.as_bytes());
                        self.solclient.send_msg(&msg)
                    }
                    Formated::Bytes(b) => {
                        msg.set_binary_attachment(&b);
                        self.solclient.send_msg(&msg)
                    }
                };
                match r {
                    SolClientReturnCode::Ok => {
                        // TODO to check queue
                        // info!("SolaceSink send message success");
                    }
                    _ => {
                        error!("SolaceSink send message error: {:?}", r);
                    }
                }
            }
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
