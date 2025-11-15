# TBC-MGMT-00: Transaction Border Controller Management Plane Specification
## Version 0.1 • Draft • November 2025

---

# Table of Contents
1. Introduction  
2. Personas & Responsibilities  
3. System Architecture  
4. Operator Console (NMI)  
5. Merchant Console (TMC)  
6. Network Management API (NMI API)  
7. Merchant Layer API  
8. SNMPv3 & MIB Stubs  
9. CLI Specification  
10. Security & Threat Model  
11. Appendices  

---

# 0. Introduction

The **Transaction Border Controller (TBC)** is the control-plane and routing system that enables secure, cross-domain economic transactions using the TGP-00 protocol and the CoreProver escrow engine.

**TBC-MGMT-00** defines the **management plane** for:

- **Operators** (infrastructure owners deploying TBC nodes)  
- **Merchants** (commercial users configuring payment flows)  
- **Device Owners** (single-node deployers)  

Its purpose is to provide a unified, coherent management framework for:

- Device provisioning  
- Multi-chain routing configuration  
- Smart-contract-based payment profile deployment  
- Telemetry, policy, and identity controls  
- Integration with websites, apps, and commerce systems  

The spec is intentionally modular and includes stubs for future sub-specifications: NMI, CLI, MIB, SNMPv3.

---

# 1. Personas & Responsibilities

## 1.1 Operators (Network Owners / Infrastructure Providers)

Operators deploy, run, and maintain TBC hardware or cloud nodes.

**Operator Responsibilities Include:**

- Provisioning and onboarding new TBC nodes  
- Configuring RPC endpoints, chain selection, routing rules  
- Monitoring TBC health, throughput, error rates  
- Managing upgrades and contract versioning  
- Maintaining redundancy, scaling clusters  
- Enforcing operational policy and compliance

**Operator Examples:**

- ISPs  
- Datacenters  
- Enterprises  
- Hosting providers  
- Carriers  
- Cloud operators  
- Anyone deploying multiple TBCs  

---

## 1.2 Merchants (Commercial Users)

Merchants use TBC to power commerce using the CoreProver dual-commitment engine.

**Merchant Responsibilities Include:**

- Creating and deploying payment profiles  
- Defining fulfillment windows and discount rules  
- Integrating TGP into website checkout flows  
- Managing settlement wallets and webhooks  
- Viewing receipts and ZK-discount status  

**Merchant Examples:**

- eCommerce shops  
- Delivery businesses  
- Subscription providers  
- Service vendors  
- Digital goods sellers  

---

# 2. System Architecture

TBC-MGMT is divided into two parallel consoles:

```
       +-----------------------------------------------+
       |              TBC-MGMT Platform                |
       +----------------------+------------------------+
                              |
    +-------------------------+---------------------------------------+
    |                                                             |
+-----------+                                             +----------------+
| Operator  |                                             |   Merchant    |
| Console   |                                             |   Console     |
| (NMI)     |                                             |    (TMC)      |
+-----------+                                             +----------------+
```

## 2.1 Node Types

- **Standalone TBC Node**
  - Single hardware device  
  - Local GUI optional  
  - SNMPv3-capable  

- **Clustered Deployment**
  - Multiple TBCs sharing routing state  
  - Automated failover  
  - Central NMI  

- **Cloud-Hosted TBC**
  - Managed service  
  - Merchant access only  
  - No device ownership required  

---

## 2.2 Components

- **TBC Routing Engine**  
- **CoreProver Escrow Engine**  
- **TGP Router**  
- **Receipt Vault**  
- **ZK-Verifier (optional)**  
- **Management Plane (TBC-MGMT)**  
- **SNMP Agent & MIB**  
- **CLI Utility (tbc-mgmt)**  

---

## 2.3 High-Level Architecture Diagram

```
                           +------------------------+
                           |       Merchant         |
                           |    Website / App       |
                           +------------+-----------+
                                        |
                                        | TGP Messages (QUERY/OFFER/SETTLE)
                                        |
                          +-------------v-------------+
                          |        TBC Device        |
                          |  - TGP Router            |
                          |  - CoreProver Engine     |
                          |  - ZK Proofs (optional)  |
                          +-------------+-------------+
                                        |
                      +-----------------v------------------+
                      |        TBC-MGMT Platform          |
                      | - NMI (Operator)                   |
                      | - TMC (Merchant)                   |
                      +-----------------+------------------+
                                        |
                           +------------v-------------+
                           |  Blockchain(s)/RPC Layer |
                           +--------------------------+
```

---

# 3. Operator Console (NMI)

The Network Management Interface (NMI) is for Operators.

## 3.1 TBC Provisioning

Operators can:

- Onboard new devices  
- Assign device ID, name, and metadata  
- Register hardware serial numbers  
- Distribute authentication certificates  
- Attach device to cluster  

## 3.2 Multi-Chain Configuration

Operators configure:

- RPC endpoints  
- Chain IDs  
- Signing keys  
- Fee settings  
- Routing policy  
- L8/L9/L10 compliance constraints  

## 3.3 Telemetry Dashboard

Metrics include:

- TPS per chain  
- CPU/GPU usage  
- Escrow counts  
- Acceptance → fulfillment → claim timing  
- Discount issuance rate  
- Late-fulfillment KPIs  

## 3.4 Device-Level Settings

- Upgrade firmware  
- Upgrade CoreProver contracts  
- Manage failover  
- Access logs  

---

# 4. Merchant Console (TMC)

The TMC abstracts away all blockchain complexity.

## 4.1 Payment Profiles (Smart-Contract Abstraction Layer)

Merchants configure:

- Acceptance window  
- Fulfillment window  
- Claim window  
- Discount rules  
- Counter-escrow requirements  
- Seller commitment type  
- Settlement wallets  
- Whether late-fulfillment discounts apply  

**These become:**

- CoreProver `PaymentProfile` structs  
- Smart contract deployments  
- OFFER templates  
- TGP routing hints  

Merchants do **not** interact with Solidity or ABIs.

---

## 4.2 Smart Contract Deployment Flow

For each profile:

1. Generate Payment Profile  
2. Validate profile  
3. Deploy to selected chain(s)  
4. Store deployment metadata  
5. Register route with TGP controller  

---

## 4.3 TGP Integration With Merchant Website

Merchant console generates:

- “PAY NOW” buttons  
- Checkout JS snippets  
- API keys  
- Webhooks for SETTLE  

Example checkout embed:

```html
<script src="tgp.js"></script>
<button onclick="tgp.pay({profileId:'pp_382', amount:3000})">
  Pay with Web3
</button>
```

---

# 5. Network Management API (NMI API)

REST-based interface for Operators.

## 5.1 Authentication

- JWT  
- API Keys  
- RBAC  
- Optional multi-signature approval for upgrades  

## 5.2 Key Endpoints

```
POST /nmi/device/register
POST /nmi/device/update
GET  /nmi/device/telemetry
POST /nmi/network/chain/add
POST /nmi/network/routing/update
POST /nmi/device/upgrade
GET  /nmi/cluster/status
```

---

# 6. Merchant API

## 6.1 Payment Profile Endpoints

```
POST /merchant/profile
GET  /merchant/profile/{id}
POST /merchant/profile/{id}/deploy
```

## 6.2 Webhooks

```
POST /webhook/settle
POST /webhook/receipt
```

## 6.3 JS SDK

Key functions:

```
tgp.query()
tgp.offer()
tgp.checkout(profile, amount)
tgp.onSettle(callback)
```

---

# 7. SNMPv3 & MIB (Stubs)

**tbcHealth**

- CPU/GPU load  
- Memory  
- Uptime  
- RPC connectivity  

**tbcSessions**

- Active escrows  
- Settlements per minute  

**tbcChainStatus**

- Per-chain configuration state  
- Height monitoring  

**tbcUpgradeStatus**

- Firmware version  
- Pending upgrade flags  

---

# 8. CLI Specification (tbc-mgmt)

```
tbc-mgmt init
tbc-mgmt device add
tbc-mgmt configure --chain 369 --rpc https://...
tbc-mgmt deploy-profile ./pizza.json
tbc-mgmt list profiles
tbc-mgmt logs --follow
```

---

# 9. Security & Threat Model

## 9.1 Cross-Chain Risks

- Mismatched RPC responses  
- Chain reorg handling  
- Withdrawal lock enforcement  

## 9.2 Operator-Key Hardening

- HSM integration  
- Role separation  
- Multi-approval for config changes  

## 9.3 Merchant Key Mistakes

- Wrong settlement wallet  
- Unclaimed discounts  
- Improper profile configuration  

## 9.4 ZK-Based Receipt Privacy

- Proofs never reveal buyer addresses  
- Discount redemption via nullifier  
- On-chain vault retains receipts  

---

# 10. Appendices

- JSON schemas  
- Example UX mockups  
- API error codes  
- Deployment recipes  

---

**End of TBC-MGMT-00 Version 0.1**