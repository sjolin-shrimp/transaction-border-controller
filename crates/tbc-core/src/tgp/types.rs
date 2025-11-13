# TGP Supporting Types

**Destination Path:** `crates/tbc-core/src/tgp/types.rs`

**Implementation:** M1 - TGP Message Parsing & Basic Routing

-----

```rust
//! TGP supporting types per TGP-00 specification
//!
//! This module contains enumerations and structures that support the core
//! TGP message types but are reusable across multiple contexts.
//!
//! # Types
//!
//! - [`ZkProfile`] - §3.5: Buyer's ZK proof preference
//! - [`EconomicEnvelope`] - §3.6: Economic constraints for offers
//! - [`SettleSource`] - §3.7: Settlement reporter identity
//!
//! # Examples
//!
//! ```rust
//! use tbc_core::tgp::types::{ZkProfile, EconomicEnvelope, SettleSource};
//!
//! let profile = ZkProfile::Required;
//! let envelope = EconomicEnvelope::new(50); // 0.50% max fees
//! let source = SettleSource::BuyerNotify;
//! ```

use serde::{Deserialize, Serialize};

// ============================================================================
// ZkProfile Enumeration (§3.5)
// ============================================================================

/// Buyer's preference for zero-knowledge proof and CoreProver escrow involvement
///
/// Indicates whether the Buyer wants to use CoreProver escrow settlement
/// or prefers direct x402 payment.
///
/// # Specification Reference
/// - TGP-00 §3.5 ZkProfile Enumeration
///
/// # Values
///
/// | Value | Meaning |
/// |-------|---------|
/// | `NONE` | Buyer does not want CoreProver escrow (direct x402 preferred) |
/// | `OPTIONAL` | Buyer is willing to use CoreProver if Controller recommends it |
/// | `REQUIRED` | Buyer demands CoreProver escrow (high-value or untrusted seller) |
///
/// # Examples
///
/// ```rust
/// use tbc_core::tgp::types::ZkProfile;
///
/// let profile = ZkProfile::Required;
/// assert!(profile.requires_escrow());
/// assert!(profile.allows_escrow());
///
/// let json = serde_json::to_string(&profile).unwrap();
/// assert_eq!(json, r#""REQUIRED""#);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ZkProfile {
    /// Buyer does not want CoreProver escrow (direct x402 preferred)
    ///
    /// **Use Case:** Low-value transactions with trusted sellers
    ///
    /// **Spec:** TGP-00 §3.5
    #[serde(rename = "NONE")]
    None,

    /// Buyer is willing to use CoreProver if Controller recommends it
    ///
    /// **Use Case:** Medium-value transactions, defer to Controller policy
    ///
    /// **Spec:** TGP-00 §3.5
    #[serde(rename = "OPTIONAL")]
    Optional,

    /// Buyer demands CoreProver escrow
    ///
    /// **Use Case:** High-value or untrusted counterparties
    ///
    /// **Spec:** TGP-00 §3.5
    #[serde(rename = "REQUIRED")]
    Required,
}

impl ZkProfile {
    /// Check if this profile allows escrow settlement
    ///
    /// Returns `true` for `Optional` and `Required`, `false` for `None`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::types::ZkProfile;
    /// assert!(!ZkProfile::None.allows_escrow());
    /// assert!(ZkProfile::Optional.allows_escrow());
    /// assert!(ZkProfile::Required.allows_escrow());
    /// ```
    pub fn allows_escrow(&self) -> bool {
        matches!(self, ZkProfile::Optional | ZkProfile::Required)
    }

    /// Check if this profile requires escrow settlement
    ///
    /// Returns `true` only for `Required`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::types::ZkProfile;
    /// assert!(!ZkProfile::None.requires_escrow());
    /// assert!(!ZkProfile::Optional.requires_escrow());
    /// assert!(ZkProfile::Required.requires_escrow());
    /// ```
    pub fn requires_escrow(&self) -> bool {
        matches!(self, ZkProfile::Required)
    }

    /// Get a human-readable description of this profile
    pub fn description(&self) -> &'static str {
        match self {
            ZkProfile::None => "No escrow (direct payment preferred)",
            ZkProfile::Optional => "Escrow optional (defer to policy)",
            ZkProfile::Required => "Escrow required (untrusted counterparty)",
        }
    }
}

impl Default for ZkProfile {
    fn default() -> Self {
        ZkProfile::Optional
    }
}

impl std::fmt::Display for ZkProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZkProfile::None => write!(f, "NONE"),
            ZkProfile::Optional => write!(f, "OPTIONAL"),
            ZkProfile::Required => write!(f, "REQUIRED"),
        }
    }
}

// ============================================================================
// EconomicEnvelope Structure (§3.6)
// ============================================================================

/// Economic constraints for an OFFER
///
/// Encodes fee limits and validity constraints that the Buyer must
/// accept when proceeding with the offered settlement path.
///
/// # Specification Reference
/// - TGP-00 §3.6 EconomicEnvelope Structure
///
/// # Fields
///
/// | Field | Type | Required | Description |
/// |-------|------|----------|-------------|
/// | `max_fees_bps` | u32 | ✓ | Max fees in basis points (e.g., 50 = 0.50%) |
/// | `expiry` | string? | optional | RFC3339 timestamp for offer expiry |
///
/// # Examples
///
/// ```rust
/// use tbc_core::tgp::types::EconomicEnvelope;
///
/// let envelope = EconomicEnvelope::new(50); // 0.50% max fees
/// assert!(envelope.validate().is_ok());
///
/// let with_expiry = EconomicEnvelope::with_expiry(50, "2025-11-10T23:59:59Z");
/// assert!(with_expiry.validate().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EconomicEnvelope {
    /// Maximum acceptable total fees in basis points
    ///
    /// **Spec:** TGP-00 §3.6 - Required field
    ///
    /// **Format:** Basis points (1 bps = 0.01%)
    ///
    /// **Example:** 50 = 0.50%, 100 = 1.00%
    ///
    /// **Validation:** Must not exceed 10000 (100%)
    pub max_fees_bps: u32,

    /// RFC3339 timestamp after which the offer is invalid
    ///
    /// **Spec:** TGP-00 §3.6 - Optional field
    ///
    /// **Format:** RFC3339 (e.g., "2025-11-10T23:59:59Z")
    ///
    /// **Validation:** Must be valid RFC3339 and in the future
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>,
}

impl EconomicEnvelope {
    /// Validate the economic envelope
    ///
    /// # Validation Rules (per TGP-00 §3.6)
    ///
    /// - `max_fees_bps` must not exceed 10000 (100%)
    /// - `expiry` must be valid RFC3339 format if present
    ///
    /// # Errors
    ///
    /// Returns a descriptive error string if validation fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::types::EconomicEnvelope;
    /// let valid = EconomicEnvelope::new(50);
    /// assert!(valid.validate().is_ok());
    ///
    /// let invalid = EconomicEnvelope::new(20000);
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        if self.max_fees_bps > 10000 {
            return Err(format!(
                "max_fees_bps cannot exceed 10000 (100%), got: {}",
                self.max_fees_bps
            ));
        }

        // Validate RFC3339 format if expiry is present
        if let Some(ref expiry) = self.expiry {
            // Simple format check (full validation would require chrono)
            if !expiry.contains('T') || (!expiry.ends_with('Z') && !expiry.contains('+') && !expiry.contains('-')) {
                return Err(format!(
                    "expiry must be in RFC3339 format (e.g., 2025-11-10T23:59:59Z): {}",
                    expiry
                ));
            }
        }

        Ok(())
    }

    /// Create a new EconomicEnvelope with required fields
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::types::EconomicEnvelope;
    /// let envelope = EconomicEnvelope::new(50);
    /// assert_eq!(envelope.max_fees_bps, 50);
    /// assert!(envelope.expiry.is_none());
    /// ```
    pub fn new(max_fees_bps: u32) -> Self {
        Self {
            max_fees_bps,
            expiry: None,
        }
    }

    /// Create an EconomicEnvelope with expiry
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::types::EconomicEnvelope;
    /// let envelope = EconomicEnvelope::with_expiry(50, "2025-11-10T23:59:59Z");
    /// assert!(envelope.expiry.is_some());
    /// ```
    pub fn with_expiry(max_fees_bps: u32, expiry: impl Into<String>) -> Self {
        Self {
            max_fees_bps,
            expiry: Some(expiry.into()),
        }
    }

    /// Get the maximum fee as a percentage (0.0 to 100.0)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::types::EconomicEnvelope;
    /// let envelope = EconomicEnvelope::new(50); // 50 bps
    /// assert_eq!(envelope.max_fee_percentage(), 0.5);
    /// ```
    pub fn max_fee_percentage(&self) -> f64 {
        self.max_fees_bps as f64 / 100.0
    }

    /// Calculate the maximum fee for a given amount
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::types::EconomicEnvelope;
    /// let envelope = EconomicEnvelope::new(50); // 0.50%
    /// let max_fee = envelope.calculate_max_fee(1_000_000); // 1 USDC
    /// assert_eq!(max_fee, 5_000); // 0.005 USDC = 5000 base units
    /// ```
    pub fn calculate_max_fee(&self, amount: u64) -> u64 {
        ((amount as u128 * self.max_fees_bps as u128) / 10000) as u64
    }

    /// Check if the envelope has expired (requires current time)
    ///
    /// Note: This is a simple string comparison. For production use,
    /// parse with chrono and compare timestamps.
    pub fn is_expired(&self, current_time_rfc3339: &str) -> bool {
        if let Some(ref expiry) = self.expiry {
            current_time_rfc3339 > expiry
        } else {
            false
        }
    }
}

// ============================================================================
// SettleSource Enumeration (§3.7)
// ============================================================================

/// Indicates who is notifying the Controller about settlement
///
/// Used in SETTLE messages to identify the source of the settlement
/// notification for audit and trust purposes.
///
/// # Specification Reference
/// - TGP-00 §3.7 SettleSource Enumeration
///
/// # Values
///
/// | Value | Meaning |
/// |-------|---------|
/// | `buyer-notify` | Buyer (or Buyer Agent) directly reporting |
/// | `controller-watcher` | Controller's indexer observed transaction |
/// | `coreprover-indexer` | External indexer sent notification |
///
/// # Examples
///
/// ```rust
/// use tbc_core::tgp::types::SettleSource;
///
/// let source = SettleSource::BuyerNotify;
/// assert!(source.requires_verification());
///
/// let json = serde_json::to_string(&source).unwrap();
/// assert_eq!(json, r#""buyer-notify""#);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum SettleSource {
    /// Buyer (or Buyer Agent) directly reporting settlement
    ///
    /// **Trust Level:** Lowest (requires verification)
    ///
    /// **Spec:** TGP-00 §3.7
    BuyerNotify,

    /// Controller's own CoreProver indexer/watcher observed the transaction
    ///
    /// **Trust Level:** Highest (Controller verified)
    ///
    /// **Spec:** TGP-00 §3.7
    ControllerWatcher,

    /// External third-party CoreProver indexer sent notification
    ///
    /// **Trust Level:** Medium (requires verification against Controller's view)
    ///
    /// **Spec:** TGP-00 §3.7
    CoreproverIndexer,
}

impl SettleSource {
    /// Check if this source requires verification
    ///
    /// Returns `true` for all sources except `ControllerWatcher`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::types::SettleSource;
    /// assert!(SettleSource::BuyerNotify.requires_verification());
    /// assert!(!SettleSource::ControllerWatcher.requires_verification());
    /// assert!(SettleSource::CoreproverIndexer.requires_verification());
    /// ```
    pub fn requires_verification(&self) -> bool {
        !matches!(self, SettleSource::ControllerWatcher)
    }

    /// Get the trust level as a numeric score (0-100)
    ///
    /// Higher values indicate higher trust.
    pub fn trust_level(&self) -> u8 {
        match self {
            SettleSource::ControllerWatcher => 100,
            SettleSource::CoreproverIndexer => 60,
            SettleSource::BuyerNotify => 30,
        }
    }

    /// Get a human-readable description of this source
    pub fn description(&self) -> &'static str {
        match self {
            SettleSource::BuyerNotify => "Buyer reported (requires verification)",
            SettleSource::ControllerWatcher => "Controller verified (highest trust)",
            SettleSource::CoreproverIndexer => "External indexer (medium trust)",
        }
    }
}

impl std::fmt::Display for SettleSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettleSource::BuyerNotify => write!(f, "buyer-notify"),
            SettleSource::ControllerWatcher => write!(f, "controller-watcher"),
            SettleSource::CoreproverIndexer => write!(f, "coreprover-indexer"),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zk_profile_serialization() {
        assert_eq!(
            serde_json::to_string(&ZkProfile::None).unwrap(),
            r#""NONE""#
        );
        assert_eq!(
            serde_json::to_string(&ZkProfile::Optional).unwrap(),
            r#""OPTIONAL""#
        );
        assert_eq!(
            serde_json::to_string(&ZkProfile::Required).unwrap(),
            r#""REQUIRED""#
        );
    }

    #[test]
    fn test_zk_profile_methods() {
        assert!(!ZkProfile::None.allows_escrow());
        assert!(ZkProfile::Optional.allows_escrow());
        assert!(ZkProfile::Required.allows_escrow());

        assert!(!ZkProfile::None.requires_escrow());
        assert!(!ZkProfile::Optional.requires_escrow());
        assert!(ZkProfile::Required.requires_escrow());
    }

    #[test]
    fn test_economic_envelope_validation() {
        let valid = EconomicEnvelope::new(50);
        assert!(valid.validate().is_ok());

        let invalid = EconomicEnvelope::new(20000);
        assert!(invalid.validate().is_err());

        let with_expiry = EconomicEnvelope::with_expiry(50, "2025-11-10T23:59:59Z");
        assert!(with_expiry.validate().is_ok());

        let invalid_expiry = EconomicEnvelope::with_expiry(50, "invalid-date");
        assert!(invalid_expiry.validate().is_err());
    }

    #[test]
    fn test_economic_envelope_calculations() {
        let envelope = EconomicEnvelope::new(50); // 0.50%

        assert_eq!(envelope.max_fee_percentage(), 0.5);
        assert_eq!(envelope.calculate_max_fee(1_000_000), 5_000);
        assert_eq!(envelope.calculate_max_fee(100_000_000), 500_000);
    }

    #[test]
    fn test_settle_source_serialization() {
        assert_eq!(
            serde_json::to_string(&SettleSource::BuyerNotify).unwrap(),
            r#""buyer-notify""#
        );
        assert_eq!(
            serde_json::to_string(&SettleSource::ControllerWatcher).unwrap(),
            r#""controller-watcher""#
        );
        assert_eq!(
            serde_json::to_string(&SettleSource::CoreproverIndexer).unwrap(),
            r#""coreprover-indexer""#
        );
    }

    #[test]
    fn test_settle_source_trust_levels() {
        assert_eq!(SettleSource::ControllerWatcher.trust_level(), 100);
        assert_eq!(SettleSource::CoreproverIndexer.trust_level(), 60);
        assert_eq!(SettleSource::BuyerNotify.trust_level(), 30);

        assert!(!SettleSource::ControllerWatcher.requires_verification());
        assert!(SettleSource::BuyerNotify.requires_verification());
    }

    #[test]
    fn test_display_implementations() {
        assert_eq!(ZkProfile::Required.to_string(), "REQUIRED");
        assert_eq!(SettleSource::BuyerNotify.to_string(), "buyer-notify");
    }
}
```