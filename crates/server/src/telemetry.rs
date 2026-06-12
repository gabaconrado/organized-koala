//! Tracing/OTel initialization. A formatting subscriber is always installed; an OTLP export
//! layer is added only when a collector endpoint is configured. A missing or unreachable
//! collector degrades to log-only — it never prevents the server from booting or serving.

use anyhow::Context as _;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig as _;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer as _;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// Service name reported to the collector.
const SERVICE_NAME: &str = "organized-koalad";

/// Guard that flushes and shuts down the OTLP exporter when dropped. Held for the process
/// lifetime by `run`; `None` when no collector is configured.
#[must_use = "dropping this guard shuts down the tracer provider"]
pub struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

impl std::fmt::Debug for TelemetryGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryGuard")
            .field("otlp_enabled", &self.provider.is_some())
            .finish()
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take()
            && let Err(error) = provider.shutdown()
        {
            tracing::warn!(%error, "failed to shut down the tracer provider cleanly");
        }
    }
}

/// Install the global tracing subscriber. When `otlp_endpoint` is `Some`, also export spans
/// to that OTLP collector; export setup failures are logged and demoted to log-only.
pub fn init(otlp_endpoint: Option<&str>) -> anyhow::Result<TelemetryGuard> {
    let filter = EnvFilter::try_from_env("OK_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer();

    let provider = match otlp_endpoint {
        Some(endpoint) => match build_provider(endpoint) {
            Ok(provider) => Some(provider),
            Err(error) => {
                // Degraded collector must not crash the server: fall back to log-only.
                eprintln!("otlp export disabled: {error:#}");
                None
            }
        },
        None => None,
    };

    // Box the optional OTLP layer so both arms (export / log-only) share one subscriber type.
    let otel_layer = provider.as_ref().map(|provider| {
        let tracer = provider.tracer(SERVICE_NAME);
        tracing_opentelemetry::layer().with_tracer(tracer).boxed()
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    Ok(TelemetryGuard { provider })
}

/// Build an OTLP-backed tracer provider pointed at `endpoint`.
fn build_provider(endpoint: &str) -> anyhow::Result<SdkTracerProvider> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.to_owned())
        .build()
        .context("building the OTLP span exporter")?;

    let resource = Resource::builder().with_service_name(SERVICE_NAME).build();
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();
    Ok(provider)
}
