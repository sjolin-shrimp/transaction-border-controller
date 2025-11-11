# Transaction Border Controller (TBC)

A programmable transaction firewall and policy gateway enabling secure and policy aware settlement across blockchains, domains, and agents.

## Overview

TBC provides trustless, privacy-preserving payment settlement for digital and physical goods. The CoreProver integration implements a dual-commitment model where **both buyer and seller must commit** before claims can be processed.

## Key Features

- **Dual-Commitment Security**: No unilateral advantage - both parties must commit
- **Flexible Seller Commitments**: Counter-escrow OR legal signature
- **Privacy-First Receipts**: NFT receipts stored in immutable vault, accessed via ZK proofs
- **Timed Release**: Automatic payment release for service-based transactions
- **Multi-Chain Support**: EVM-compatible (PulseChain, Base)

## Architecture

```
┌─────────────────┐
│   TBC Gateway   │  ← Existing routing & agent coordination
└────────┬────────┘
         │
┌────────▼────────┐
│ CoreProver SDK  │  ← High-level builder API
└────────┬────────┘
         │
┌────────▼────────┐
│ CoreProver      │  ← Contract bindings & event listeners
│    Bridge       │
└────────┬────────┘
         │
┌────────▼────────┐
│   Solidity      │  ← On-chain escrow & receipt vault
│   Contracts     │
└─────────────────┘
```

## Quick Start

```bash
# Install dependencies
./scripts/setup-dev.sh

# Build workspace
cargo build --workspace

# Run tests
cargo test --workspace

# Start local development
docker-compose -f docker/docker-compose.dev.yml up
```

## Repository Structure

- `crates/tbc-core` - Core gateway protocol
- `crates/tbc-gateway` - TGP implementation
- `crates/coreprover-contracts` - Solidity smart contracts (Foundry)
- `crates/coreprover-bridge` - Rust ↔ Solidity bridge
- `crates/coreprover-service` - Settlement service
- `crates/coreprover-zk` - ZK circuits & provers
- `crates/coreprover-cli` - Operator CLI
- `crates/coreprover-sdk` - Developer SDK

## Documentation

See `docs/` for comprehensive guides:
- [CoreProver Specification](docs/specs/coreprover.md)
- [Quick Start Guide](docs/guides/quickstart.md)
- [API Reference](docs/api/README.md)

## License

MIT OR Apache-2.0
