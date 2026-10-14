//! `#[tenancy_user]`/`#[tenancy_admin]`'s parameter mode: a method tagged
//! with either, carrying a `SecuredOrg`-typed parameter, is checked against
//! *this call's* own arguments, not a compile-time literal. Needed for
//! multi-tenant applets (e.g. `codensics`) that serve many orgs from a
//! single deployment and decide which on each call.
//!
//! Plain type aliases, not newtype wrappers — this keeps them serde-wire
//! identical to `String`/`Option<String>` with zero custom (de)serialization
//! code. `weil_macros`'s proc-macro looks for the literal type name written
//! in source (`SecuredOrg`/`SecuredSubgroup`), not the resolved/aliased type,
//! so a plain alias is sufficient — no need for a distinct wrapper struct.
//!
//! Neither parameter is trusted on its own. The macro requires the org
//! (and, whenever the caller's own `Runtime::org()` claim names a subgroup,
//! the subgroup too) to match `Runtime::org()` exactly before using it to
//! select which Identity applet and key to check — a caller can only ever
//! act as the org/subgroup they actually signed for, never one they merely
//! pass as an argument.
pub type SecuredOrg = String;
pub type SecuredSubgroup = Option<String>;
