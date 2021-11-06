use opentelemetry::{global, sdk::propagation::TraceContextPropagator};
use tracing_subscriber::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("grpc-client")
        .install_simple()?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("INFO"))
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .try_init()?;

    global::shutdown_tracer_provider();
    Ok(())
}
