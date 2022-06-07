// Internet2 addresses with support for Tor v3
//
// Written in 2019-2022 by
//     Dr. Maxim Orlovsky <orlovsky@lnp-bp.org>
//     Martin Habovstiak <martin.habovstiak@gmail.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

//! Universal internet addresses that support IPv4, IPv6 and Tor

#![recursion_limit = "256"]
// Coding conventions
#![deny(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    unused_mut,
    unused_imports,
    dead_code,
    missing_docs
)]

#[allow(unused_imports)]
#[macro_use]
extern crate amplify;
#[cfg(feature = "stringly_conversions")]
#[macro_use]
extern crate stringly_conversions_crate as stringly_conversions;
#[cfg(feature = "strict_encoding")]
extern crate strict_encoding;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde_crate as serde;

#[cfg(feature = "strict_encoding")]
mod encoding;
mod inet;

pub use inet::{
    AddrParseError, InetAddr, InetSocketAddr, InetSocketAddrExt,
    NoOnionSupportError, Transport,
};
