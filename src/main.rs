// #![feature(generators)]
use std::error::Error;
use std::io::{stdout, Write};
use std::{
    cell::RefCell,
    rc::Rc,
};
use std::collections::VecDeque;
use async_openai::types::ChatCompletionRequestMessage;
use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role, CreateChatCompletionStreamResponse},
    Client,
};
use futures::StreamExt;
use axum::{
    extract::TypedHeader,
    headers,
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures::stream::{self, Stream};
use futures_util::{pin_mut, stream::StreamExt as __};
use std::{convert::Infallible, net::SocketAddr, path::PathBuf, time::Duration};
use tokio_stream::StreamExt as _;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use async_stream::stream as async_stream;
use futures::{channel::mpsc, sink::SinkExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_sse=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    let static_files_service = ServeDir::new(assets_dir).append_index_html_on_directories(true);

    let app = Router::new()
        .fallback_service(static_files_service)
        .route("/sse", get(sse_handler))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>>  {
    let content = "tell me an interesting fact";
    let client = Client::new();
    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .max_tokens(512_u16)
        .messages([
            ChatCompletionRequestMessageArgs::default()
                .content(content)
                .role(Role::User)
                .build()
                .unwrap()
        ])
        .build()
        .unwrap();

    
    let mut stream /* : impl Stream<Item = Result<CreateChatCompletionStreamResponse, async_openapi::error::OpenAIError>> */
        = client.chat().create_stream(request).await.unwrap();

    let stream_responder = Box::pin(async_stream! { 
        while let Some(result) = futures_util::stream::StreamExt::next(&mut stream).await {
            let response = result.unwrap();
            for chat_choice in response.choices.into_iter() {
                if let Some(ref content) = chat_choice.delta.content {
                    let event = Ok(Event::default().data(content));
                    yield event;
                }
                continue;
            }
        }
    }); // as Pin<Box<dyn Stream<Item = Result<Event, _> + Send>;
        
        
    Sse::new(stream_responder).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}

