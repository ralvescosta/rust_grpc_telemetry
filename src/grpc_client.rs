use log;
use opentelemetry::{global, propagation::Injector, sdk::propagation::TraceContextPropagator};
use tracing::{info_span, instrument};
use tracing_futures::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::prelude::*;
pub mod hello_world {
    tonic::include_proto!("helloworld");
}

use hello_world::{greeter_client::GreeterClient, HelloRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("grpc-client")
        .install_simple()?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("INFO"))
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .try_init()?;

    greet().await?;

    global::shutdown_tracer_provider();
    Ok(())
}

struct MetadataMap<'a>(&'a mut tonic::metadata::MetadataMap);
impl<'a> Injector for MetadataMap<'a> {
    /// Set a key and value in the MetadataMap.  Does nothing if the key or value are not valid inputs
    fn set(&mut self, key: &str, value: String) {
        if let Ok(key) = tonic::metadata::MetadataKey::from_bytes(key.as_bytes()) {
            if let Ok(val) = tonic::metadata::MetadataValue::from_str(&value) {
                self.0.insert(key, val);
            }
        }
    }
}

#[instrument]
async fn greet() -> Result<(), Box<dyn std::error::Error>> {
    log::warn!("[greet]");
    let mut client = GreeterClient::connect("http://[::1]:50051")
        .instrument(info_span!("first client connect"))
        .await?;

    let mut request = tonic::Request::new(HelloRequest {
        name: "Tonic".into(),
    });

    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(
            &tracing::Span::current().context(),
            &mut MetadataMap(request.metadata_mut()),
        )
    });

    log::warn!("[greet] do request");

    let response = client
        .say_hello(request)
        .instrument(tracing::Span::current())
        .await?;

    println!("[CLIENT] - Response received: {:?}", response);
    Ok(())
}
