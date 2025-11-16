📘 TBC-00 — Transaction Border Controller Specification

Version: 0.1-draft
Status: Draft (internal)
Author: Ledger of Earth
Scope: Defines the on-network policy and routing engine for TGP.
Audience: Backend developers, protocol implementers, security reviewers.

⸻

0. Overview

The Transaction Border Controller (TBC) is the authoritative policy and routing engine of the Transaction Gateway Protocol (TGP).
It:
	•	receives TGP QUERY messages from TGP Clients
	•	evaluates local + merchant + user policies
	•	determines the correct settlement verb
	•	constructs a fully-formed blockchain transaction specification
	•	returns a TGP ACK response
	•	optionally relays signed transactions to RPC endpoints
	•	manages session-level state and trust boundaries

The TBC is the “firewall + NAT + SBC” layer for economic transactions.

⸻

1. Core Responsibilities

A compliant TBC implementation MUST:
	1.	Accept TGP QUERY JSON requests
	2.	Validate QUERY data
	3.	Perform routing and policy evaluation
	4.	Generate TGP ACK
	5.	Construct transaction calldata for escrow settlement verbs
	6.	Track session states
	7.	Relay signed transactions if in relay mode
	8.	Provide audit logs
	9.	Support multiple chains (EVM now; SVM future)

A TBC MUST NOT:
	•	sign transactions
	•	manage private keys
	•	override user wallets
	•	execute arbitrary transaction modifications outside ACK construction
	•	store private user data

⸻

2. TBC Architecture

The TBC consists of several internal modules:
	•	Parser: Validates QUERY structure
	•	Session Manager: Tracks session state
	•	Policy Engine: Evaluates:
	•	user policy
	•	merchant profile
	•	jurisdiction
	•	limits
	•	trust rules
	•	Escrow Engine: Decides next verb (commit/accept/fulfill/claim/…)
	•	Transaction Constructor: Builds raw tx spec
	•	Router: Determines RPC or relay path
	•	Relay Handler: Submits signed tx to RPC
	•	Audit Log: Writes event logs

⸻

3. TBC API Specification (Authoritative Section)

This is the core of TBC-00.

⸻

3.1 Endpoint Summary

Method	Endpoint	Description
POST	/tgp/query	Process TGP QUERY → return TGP ACK
POST	/tgp/relay	Accept signed tx for relay submission
GET	/tgp/session/:id	Retrieve session status
GET	/tgp/health	Health & reachability indicator
GET	/tgp/version	Return version string

These endpoints MUST use HTTPS only.

⸻

3.2 POST /tgp/query

Purpose:

Primary entry point.
Receives TGP QUERY, performs policy evaluation, returns TGP ACK.

Request Format

(From TGP-CP-00)

{
  "tgp_version": "0.1",
  "session_id": null,
  "buyer_address": "0xabc...",
  "payment_profile": "0xContract",
  "chain_id": 369,
  "amount": "1000000000000000000",
  "intent": {
    "verb": "commit"
  },
  "metadata": {
    "x402": {...}
  }
}

Behavior & Validation

The TBC MUST:
	•	validate buyer_address
	•	validate payment_profile exists
	•	ensure chain_id is recognized
	•	check local + merchant + user policy compliance
	•	look up current session if session_id not null
	•	decide next escrow verb
	•	generate transaction calldata

ACK Response Format

{
  "status": "allow" | "deny" | "revise",
  "session_id": "abcd-1234",
  "next_verb": "commit",
  "tx": {
    "to": "0xPaymentProfile",
    "data": "0x...",
    "value": "1000000000000000000",
    "chain_id": 369,
    "gas_limit": null,
    "gas_price": null
  },
  "routing": {
    "mode": "direct" | "relay",
    "rpc_url": "https://rpc.pulsechain.com",
    "tbc_url": "https://tbc.mydomain.com/tgp/relay"
  },
  "timeouts": {
    "fulfillment_window": 30000,
    "session_expiry": 600000
  },
  "notes": "Commit authorized."
}

Error Cases
	•	400 malformed
	•	403 policy violation
	•	409 conflicting session state
	•	500 internal

⸻

3.3 POST /tgp/relay

Purpose:

Accept signed transactions from clients when ACK routing mode = relay.

Request Format

{
  "session_id": "abcd-1234",
  "signed_tx": "0xDeadBeef..."
}

Behavior

The TBC MUST:
	•	validate session_id
	•	check session verb state
	•	ensure tx matches expected destination & structure
	•	relay via RPC
	•	log tx hash

Response Example

{
  "status": "submitted",
  "tx_hash": "0x1234..."
}


⸻

**3.4 GET `/tgp/session/:id``

Purpose:

Return session information for audit/debug flows.

Response Example

{
  "session_id": "abcd-1234",
  "state": "awaiting_fulfillment",
  "current_verb": "commit",
  "expires_in_ms": 28000
}


⸻

3.5 GET /tgp/health

Purpose:

Allow TGP-EX and wallets to detect TBC reachability.

Return Example

{
  "tbc": "reachable",
  "version": "0.1"
}

(Used by Presence API)

⸻

4. Escrow Verb Determination Logic

TBC MUST implement verb sequencing:

commit → accept → fulfill → verify → claim → complete

Determined based on:
	•	contract ABI
	•	merchant rules
	•	prior session verb
	•	timeouts

⸻

5. Policy Engine Requirements

Policies MAY include:
	•	per-user transaction limits
	•	AI-agent session limits
	•	jurisdiction
	•	merchant risk score
	•	routing restrictions
	•	whitelisted payment profiles
	•	dynamic spend ceilings

Policy engine MUST output:
	•	allow
	•	deny
	•	revise (modified amount or verb)

⸻

6. Internal Session Management

TBC MUST track:
	•	session_id
	•	current verb
	•	timeouts
	•	last ACK
	•	last TX hash
	•	buyer address
	•	payment profile
	•	chain ID

TBC MUST NOT track:
	•	private keys
	•	wallet provider data
	•	identity data beyond what is required

⸻

7. Logging Requirements

The TBC MUST log:
	•	QUERY received
	•	ACK emitted
	•	relay events
	•	session state transitions
	•	RPC responses (scrubbed)

Logs MUST NOT store:
	•	full signed TX unless for debugging (optional, configurable)
	•	personal data

⸻

8. Security Requirements

The TBC MUST:
	•	use TLS
	•	validate requests
	•	defend against replay attacks
	•	ensure ACK → relay coherence
	•	verify signed tx matches expected tx spec
	•	reject mismatched calldata

TBC MUST NOT:
	•	sign transactions
	•	transform values not permitted by policy
	•	weaken chain security

⸻

9. Compliance Tests

A TBC implementation MUST pass:
	1.	QUERY/ACK handshake test
	2.	Verb sequencing correctness test
	3.	Transaction construction test
	4.	Relay correctness test
	5.	Timeout logic test
	6.	Session recovery test
	7.	Negative policy test
	8.	RPC fallback test

