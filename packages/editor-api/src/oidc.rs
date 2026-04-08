use openidconnect::core::CoreClient;
use openidconnect::{
    ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet, EndpointSet, IssuerUrl,
    ProviderMetadataWithLogout,
};

use crate::config::OidcConfig;

pub type ConfiguredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
    EndpointMaybeSet,
>;

pub struct DiscoveryResult {
    pub client: ConfiguredClient,
    pub end_session_url: Option<String>,
}

pub async fn discover_client(oidc_config: &OidcConfig) -> Result<DiscoveryResult, String> {
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    tracing::info!("discovering OIDC provider at {}", oidc_config.issuer_url);

    let issuer = IssuerUrl::new(oidc_config.issuer_url.clone())
        .map_err(|e| format!("invalid issuer URL: {e}"))?;

    let provider_metadata = ProviderMetadataWithLogout::discover_async(issuer, &http_client)
        .await
        .map_err(|e| format!("OIDC discovery failed: {e}"))?;

    let token_url = provider_metadata
        .token_endpoint()
        .ok_or("provider metadata missing token_endpoint")?
        .clone();

    let end_session_url = provider_metadata
        .additional_metadata()
        .end_session_endpoint
        .as_ref()
        .map(|url| url.url().to_string());

    if let Some(ref url) = end_session_url {
        tracing::info!("end_session_endpoint: {url}");
    } else {
        tracing::warn!("provider metadata does not include end_session_endpoint");
    }

    let client = CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(oidc_config.client_id.clone()),
        Some(ClientSecret::new(oidc_config.client_secret.clone())),
    )
    .set_token_uri(token_url);

    tracing::info!("OIDC client configured successfully");

    Ok(DiscoveryResult {
        client,
        end_session_url,
    })
}
