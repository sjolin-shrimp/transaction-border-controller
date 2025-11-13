pub mod state;
pub mod messages;
pub mod validation;
pub mod types;

// Optional: Re-export commonly used items
pub use state::{TGPState, TGPSession, TGPStateError};
pub use messages::{TGPMessage, QueryMessage, OfferMessage, SettleMessage, ErrorMessage};