mod common;

use common::*;
use futures_util::SinkExt;
use server::twilio::signature;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

async fn connect(signature: Option<String>) -> Result<(), u16> {
    let addr = spawn_server(app()).await;
    let mut request = format!("ws://{addr}/media").into_client_request().unwrap();
    if let Some(sig) = signature {
        request
            .headers_mut()
            .insert("x-twilio-signature", sig.parse().unwrap());
    }
    match tokio_tungstenite::connect_async(request).await {
        Ok((mut ws, _)) => {
            let start = r#"{"event":"start","start":{"callSid":"CA1","streamSid":"MZ1","customParameters":{"from":"+16175550123"}}}"#;
            ws.send(Message::text(start)).await.unwrap();
            ws.send(Message::text(
                r#"{"event":"media","media":{"track":"inbound","timestamp":"0","payload":"//8="}}"#,
            ))
            .await
            .unwrap();
            ws.send(Message::text(r#"{"event":"stop"}"#)).await.unwrap();
            Ok(())
        }
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            Err(response.status().as_u16())
        }
        Err(other) => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn signed_upgrade_is_accepted() {
    let sig = signature::compute(TOKEN, "wss://cg.example.com/media", &[]);
    assert_eq!(connect(Some(sig)).await, Ok(()));
}

#[tokio::test]
async fn unsigned_or_wrongly_signed_upgrade_is_rejected() {
    assert_eq!(connect(None).await, Err(403));
    // Signed for the https:// URL instead of the wss:// one Twilio uses.
    let wrong = signature::compute(TOKEN, "https://cg.example.com/media", &[]);
    assert_eq!(connect(Some(wrong)).await, Err(403));
}
