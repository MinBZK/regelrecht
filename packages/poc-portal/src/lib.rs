//! The PoC portal: one hostname, several proof-of-concepts, each behind its own
//! password.
//!
//! `poc.regelrecht.rijks.app` is a single ZAD component. It cannot be several,
//! because the deployment publishes on `component.subdomain` and every
//! component therefore gets its own hostname — so the routing between the PoCs
//! happens here, inside one container, rather than at the ingress.
//!
//! Two kinds of PoC live behind it. A `statisch` one is built into this image
//! and served from disk. A `proxy` one runs as its own (unpublished) component
//! and is forwarded to in-cluster, the way `editor-api` already forwards to
//! `pipelineapi`.

pub mod gate;
pub mod registry;
