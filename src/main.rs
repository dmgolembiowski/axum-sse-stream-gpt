use async_openai::types::ChatCompletionRequestFunctionMessageArgs;
use async_openai::{
    types::{CreateChatCompletionRequestArgs, Role},
    Client,
};
use async_stream::stream as async_stream;
use axum::{
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use axum_extra::{headers, TypedHeader};
use futures::stream::Stream;
use std::{convert::Infallible, path::PathBuf, time::Duration};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::debug!("listening on {:?}", listener);
    axum::serve(listener, app).await.unwrap();
}

async fn sse_handler(
    TypedHeader(_user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let content = "tell me an interesting fact";
    let client = Client::new();
    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .max_tokens(512_u16)
        .messages(vec![ChatCompletionRequestFunctionMessageArgs::default()
            .content(content)
            .role(Role::User)
            .build()
            .unwrap()
            .into()])
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
