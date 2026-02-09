//! Application layer for osiris-marketplace
//!
//! This layer orchestrates the domain logic and coordinates between ports and adapters.
//! It contains the business logic for handling marketplace procurement events.

pub mod event_handler;
pub mod service;

pub use event_handler::MarketplaceEventHandler;
pub use service::MarketplaceService;
