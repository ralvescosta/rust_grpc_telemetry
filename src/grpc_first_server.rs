use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry::{global, sdk::propagation::TraceContextPropagator};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::*;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::prelude::*;

pub mod hello_world {
    tonic::include_proto!("helloworld"); // The string specified here must match the proto package name
}

use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{greeter_client::GreeterClient, HelloReply, HelloRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("grpc-first-server")
        .install_batch(opentelemetry::runtime::Tokio)?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("INFO"))
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .try_init()?;

    let addr = "[::1]:50051".parse()?;
    let greeter = MyGreeter::default();

    Server::builder()
        .add_service(GreeterServer::new(greeter))
        .serve(addr)
        .await?;

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

struct ExMetadataMap<'a>(&'a tonic::metadata::MetadataMap);
impl<'a> Extractor for ExMetadataMap<'a> {
    /// Get a value for a key from the MetadataMap.  If the value can't be converted to &str, returns None
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|metadata| metadata.to_str().ok())
    }

    /// Collect all the keys from the MetadataMap.
    fn keys(&self) -> Vec<&str> {
        self.0
            .keys()
            .map(|key| match key {
                tonic::metadata::KeyRef::Ascii(v) => v.as_str(),
                tonic::metadata::KeyRef::Binary(v) => v.as_str(),
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, Default)]
pub struct MyGreeter;

#[tonic::async_trait]
impl Greeter for MyGreeter {
    #[instrument]
    async fn say_hello(
        &self,
        request: Request<HelloRequest>, // Accept request of type HelloRequest
    ) -> Result<Response<HelloReply>, Status> {
        let parent_cx = global::get_text_map_propagator(|prop| {
            prop.extract(&ExMetadataMap(request.metadata()))
        });
        tracing::Span::current().set_parent(parent_cx);

        let mut client = GreeterClient::connect("http://[::1]:50052")
            .instrument(info_span!("second client connect"))
            .await
            .unwrap();

        let mut request = tonic::Request::new(HelloRequest {
            name: "Tonic".into(),
        });

        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(
                &tracing::Span::current().context(),
                &mut MetadataMap(request.metadata_mut()),
            )
        });

        let response = client
            .say_hello(request)
            .instrument(tracing::Span::current())
            .await?;

        println!("[FIRST SERVER] - Response received: {:?}", response);
        Ok(response)
    }
}
