// LNP/BP Core Library implementing LNPBP specifications & standards
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use core::convert::TryFrom;
#[cfg(feature = "url")]
use core::convert::TryInto;
use core::fmt::Debug;
#[cfg(feature = "url")]
use core::fmt::{Display, Formatter};
#[cfg(feature = "url")]
use core::str::FromStr;
use std::hash::{Hash, Hasher};
use std::net::{AddrParseError, IpAddr, SocketAddr};
#[cfg(feature = "url")]
use url::Url;

use amplify::internet::InetAddr;
use bitcoin::secp256k1;

use crate::lnp::transport::{zmqsocket, LocalAddr};
use crate::lnp::UrlScheme;

/// Universal Node Locator for LNP protocol
/// (from [LNPBP-19](https://github.com/LNP-BP/LNPBPs/blob/master/lnpbp-0019.md))
///
/// Type is used for visual node and specific protocol representation or parsing
/// It is different from [`NodeAddr`](super::NodeAddr) by the fact that it may
/// not contain port information for LNP-based protocols having known default
/// port, while `NodeAddr` must always contain complete information with the
/// explicit porn number. To convert [`NodeLocator`] to [`NodeAddr`] use
/// [`ToNodeAddr`](super::ToNodeAddr) trait.
///
/// NB: DNS addressing is not used since it is considered insecure in terms of
///     censorship resistance.
#[derive(Clone)]
#[non_exhaustive]
pub enum NodeLocator {
    /// Native Lightning network connection: uses end-to-end encryption and
    /// runs on top of either TCP socket (which may be backed by Tor
    /// connection)
    ///
    /// # URL Scheme
    /// lnp://<node-id>@<ip>|<onion>:<port>
    Native(secp256k1::PublicKey, InetAddr, Option<u16>),

    /// NB: Unfinished!
    ///
    /// UDP-based connection that uses UDP packets instead of TCP. Can't work
    /// with Tor, but may use UDP hole punching in a secure way, since the
    /// connection is still required to be encrypted.
    ///
    /// # URL Scheme
    /// lnpu://<node-id>@<ip>:<port>
    Udp(secp256k1::PublicKey, IpAddr, Option<u16>),

    /// Connection through POSIX (UNIX-type) socket. Does not use encryption.
    ///
    /// # URL Scheme
    /// lnp:<file-path>
    Posix(String),

    /// Local (for inter-process communication based on POSIX sockets)
    /// connection without encryption. Relies on ZMQ IPC sockets internally;
    /// specific socket pair for ZMQ is provided via query parameter
    ///
    /// # URL Schema
    /// lnpz:<file-path>?api=<p2p|rpc|sub>
    #[cfg(feature = "zmq")]
    ZmqIpc(String, zmqsocket::ApiType),

    /// LNP protocol supports in-process communications (between threads of the
    /// same process using Mutex'es and other sync managing routines) without
    /// encryption. It relies on ZMQ IPC sockets internally. However, such
    /// connection can be done only withing the same process, and can't be
    /// represented in the form of URL: it requires presence of ZMQ context
    /// object, which can't be encoded as a string (context object is taken
    /// from a global variable).
    #[cfg(feature = "zmq")]
    ZmqInproc(String, zmqsocket::ApiType),

    /// SHOULD be used only for DMZ area connections; otherwise
    /// [`NodeLocator::Native`] or [`NodeLocator::Websocket`] connection
    /// MUST be used
    ///
    /// # URL Scheme
    /// lnpz://<node-id>@<ip>[:<port>]/?api=<p2p|rpc|sub>
    #[cfg(feature = "zmq")]
    ZmqTcpEncrypted(
        secp256k1::PublicKey,
        zmqsocket::ApiType,
        IpAddr,
        Option<u16>,
    ),

    /// SHOULD be used only for DMZ area connections; otherwise
    /// [`NodeLocator::Native`] or [`NodeLocator::Websocket`] connection
    /// MUST be used
    ///
    /// # URL Schema
    /// lnpz://<ip>[:<port>]/?api=<p2p|rpc|sub>
    #[cfg(feature = "zmq")]
    ZmqTcpUnencrypted(zmqsocket::ApiType, IpAddr, Option<u16>),

    /// # URL Scheme
    /// lnph://<node-id>@<ip>|<onion>[:<port>]
    Http(secp256k1::PublicKey, InetAddr, Option<u16>),

    /// # URL Scheme
    /// lnpws://<node-id>@<ip>|<onion>[:<port>]
    #[cfg(feature = "websockets")]
    Websocket(secp256k1::PublicKey, InetAddr, Option<u16>),

    /// Text (Bech32-based) connection for high latency or non-interactive
    /// protocols. Can work with SMPT, for mesh and satellite networks – or
    /// with other mediums of communications (chat messages, QR codes etc).
    ///
    /// # URL Scheme
    /// lnpt://<node-id>@
    Text(secp256k1::PublicKey),
}

impl PartialEq for NodeLocator {
    fn eq(&self, other: &Self) -> bool {
        use NodeLocator::*;

        fn api_eq(a: &zmqsocket::ApiType, b: &zmqsocket::ApiType) -> bool {
            a == b
                || (*a == zmqsocket::ApiType::PeerListening
                    && *b == zmqsocket::ApiType::PeerConnecting)
                || (*b == zmqsocket::ApiType::PeerListening
                    && *a == zmqsocket::ApiType::PeerConnecting)
        }

        match (self, other) {
            (Native(a1, a2, a3), Native(b1, b2, b3)) => {
                a1 == b1 && a2 == b2 && a3 == b3
            }
            (Udp(a1, a2, a3), Udp(b1, b2, b3)) => {
                a1 == b1 && a2 == b2 && a3 == b3
            }
            #[cfg(feature = "websockets")]
            (Websocket(a1, a2, a3), Websocket(b1, b2, b3)) => {
                a1 == b1 && a2 == b2 && a3 == b3
            }
            #[cfg(feature = "zmq")]
            (ZmqIpc(a1, a2), ZmqIpc(b1, b2)) => a1 == b1 && api_eq(a2, b2),
            #[cfg(feature = "zmq")]
            (ZmqInproc(a1, a2), ZmqInproc(b1, b2)) => {
                a1 == b1 && api_eq(a2, b2)
            }
            #[cfg(feature = "zmq")]
            (ZmqTcpUnencrypted(a1, a2, a3), ZmqTcpUnencrypted(b1, b2, b3)) => {
                api_eq(a1, b1) && a2 == b2 && a3 == b3
            }
            #[cfg(feature = "zmq")]
            (
                ZmqTcpEncrypted(a1, a2, a3, a4),
                ZmqTcpEncrypted(b1, b2, b3, b4),
            ) => a1 == b1 && api_eq(a2, b2) && a3 == b3 && a4 == b4,
            (Http(a1, a2, a3), Http(b1, b2, b3)) => {
                a1 == b1 && a2 == b2 && a3 == b3
            }
            (Text(pubkey1), Text(pubkey2)) => pubkey1 == pubkey2,
            (_, _) => false,
        }
    }
}

impl Eq for NodeLocator {}

impl Hash for NodeLocator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.to_url_string().as_bytes());
    }
}

impl NodeLocator {
    /// Adds port information to the node locator, if it can contain port.
    /// In case if it does not, performs no action. Returns cloned `Self` with
    /// the updated data.
    pub fn with_port(&self, port: u16) -> Self {
        match self.clone() {
            NodeLocator::Native(a, b, _) => {
                NodeLocator::Native(a, b, Some(port))
            }
            NodeLocator::Udp(a, b, _) => NodeLocator::Udp(a, b, Some(port)),
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpEncrypted(a, b, c, _) => {
                NodeLocator::ZmqTcpEncrypted(a, b, c, Some(port))
            }
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpUnencrypted(a, b, _) => {
                NodeLocator::ZmqTcpUnencrypted(a, b, Some(port))
            }
            NodeLocator::Http(a, b, _) => NodeLocator::Http(a, b, Some(port)),
            #[cfg(feature = "websockets")]
            NodeLocator::Websocket(a, b, _) => {
                NodeLocator::Websocket(a, b, Some(port))
            }
            me => me,
        }
    }

    /// Returns URL string representation for a given node locator. If you need
    /// full URL address, plsease use [`Url::from()`] instead (this will require
    /// `url` feature for LNP/BP Core Library).
    pub fn to_url_string(&self) -> String {
        match self {
            NodeLocator::Native(pubkey, inet, port) => {
                let p = port.map(|x| format!(":{}", x)).unwrap_or_default();
                format!("{}://{}@{}{}", self.url_scheme(), pubkey, inet, p)
            }
            NodeLocator::Udp(pubkey, ip, port) => {
                let p = port.map(|x| format!(":{}", x)).unwrap_or_default();
                format!("{}://{}@{}{}", self.url_scheme(), pubkey, ip, p)
            }
            NodeLocator::Posix(path) => {
                format!("{}:{}", self.url_scheme(), path)
            }
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqIpc(path, zmq_type) => format!(
                "{}:{}?api={}",
                self.url_scheme(),
                path,
                zmq_type.api_name()
            ),
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqInproc(name, zmq_type) => format!(
                "{}:?api={}#{}",
                self.url_scheme(),
                zmq_type.api_name(),
                name
            ),
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpEncrypted(pubkey, zmq_type, ip, port) => {
                let p = port.map(|x| format!(":{}", x)).unwrap_or_default();
                format!(
                    "{}://{}@{}{}/?api={}",
                    self.url_scheme(),
                    pubkey,
                    ip,
                    p,
                    zmq_type.api_name()
                )
            }
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpUnencrypted(zmq_type, ip, port) => {
                let p = port.map(|x| format!(":{}", x)).unwrap_or_default();
                format!(
                    "{}://{}{}/?api={}",
                    self.url_scheme(),
                    ip,
                    p,
                    zmq_type.api_name()
                )
            }
            NodeLocator::Http(pubkey, inet, port) => {
                let p = port.map(|x| format!(":{}", x)).unwrap_or_default();
                format!("{}://{}@{}{}", self.url_scheme(), pubkey, inet, p)
            }
            #[cfg(feature = "websockets")]
            NodeLocator::Websocket(pubkey, inet, port) => {
                let p = port.map(|x| format!(":{}", x)).unwrap_or_default();
                format!("{}://{}@{}{}", self.url_scheme(), pubkey, inet, p)
            }
            NodeLocator::Text(pubkey) => {
                format!("{}://{}", self.url_scheme(), pubkey)
            }
        }
    }

    /// Parses [`NodeLocator`] into it's optional components, returned as a
    /// single tuple of optionals:
    /// 1) node public key,
    /// 2) [`InetAddr`] of the node,
    /// 3) port
    /// 4) file path or POSIX socket name
    /// 5) [`zmqsocket::ApiType`] parameter for ZMQ based locators
    pub fn components(
        &self,
    ) -> (
        Option<secp256k1::PublicKey>,
        Option<InetAddr>,
        Option<u16>,
        Option<String>, /* file or named socket */
        Option<zmqsocket::ApiType>,
    ) {
        match self {
            NodeLocator::Native(pubkey, inet, port) => {
                (Some(*pubkey), Some(*inet), *port, None, None)
            }
            NodeLocator::Udp(pubkey, ip, port) => {
                (Some(*pubkey), Some(InetAddr::from(*ip)), *port, None, None)
            }
            NodeLocator::Posix(path) => {
                (None, None, None, Some(path.clone()), None)
            }
            NodeLocator::ZmqIpc(path, api) => {
                (None, None, None, Some(path.clone()), Some(*api))
            }
            NodeLocator::ZmqInproc(name, api) => {
                (None, None, None, Some(name.clone()), Some(*api))
            }
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpEncrypted(pubkey, api, ip, port) => (
                Some(*pubkey),
                Some(InetAddr::from(*ip)),
                *port,
                None,
                Some(*api),
            ),
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpUnencrypted(api, ip, port) => {
                (None, Some(InetAddr::from(*ip)), *port, None, Some(*api))
            }
            NodeLocator::Http(pubkey, inet, port) => {
                (Some(*pubkey), Some(*inet), *port, None, None)
            }
            #[cfg(feature = "websockets")]
            NodeLocator::Websocket(pubkey, inet, port) => {
                (Some(*pubkey), Some(*inet), *port, None, None)
            }
            NodeLocator::Text(pubkey) => {
                (Some(*pubkey), None, None, None, None)
            }
        }
    }

    /// Returns node id for the given locator, if any, or [`Option::None`]
    /// otherwise
    #[inline]
    pub fn node_id(&self) -> Option<secp256k1::PublicKey> {
        self.components().0
    }

    /// Returns [`InetAddr`] for the given locator, if any, or [`Option::None`]
    /// otherwise
    #[inline]
    pub fn inet_addr(&self) -> Option<InetAddr> {
        self.components().1
    }

    /// Returns port number for the given locator, if any, or [`Option::None`]
    /// otherwise
    #[inline]
    pub fn port(&self) -> Option<u16> {
        self.components().2
    }

    /// Returns socket name if for the given locator, if any, or
    /// [`Option::None`] otherwise
    #[inline]
    pub fn socket_name(&self) -> Option<String> {
        self.components().3
    }

    /// Returns [`zmqsocket::ApiType`] for the given locator, if any, or
    /// [`Option::None`] otherwise
    #[inline]
    pub fn api_type(&self) -> Option<zmqsocket::ApiType> {
        self.components().4
    }
}

impl UrlScheme for NodeLocator {
    fn url_scheme(&self) -> &'static str {
        match self {
            NodeLocator::Native(..) => "lnp",
            NodeLocator::Udp(..) => "lnpu",
            NodeLocator::Posix(..) => "lnp",
            NodeLocator::ZmqIpc(..) | NodeLocator::ZmqInproc(..) => "lnpz",
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpEncrypted(..)
            | NodeLocator::ZmqTcpUnencrypted(..) => "lnpz",
            NodeLocator::Http(..) => "lnph",
            #[cfg(feature = "websockets")]
            NodeLocator::Websocket(..) => "lnpws",
            NodeLocator::Text(..) => "lnpt",
        }
    }
}

/// Errors from parting string data into [`NodeLocator`] type
#[derive(Clone, PartialEq, Eq, Hash, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ParseError {
    /// The provided protocol can't be used for a [`LocalAddr`]
    UnsupportedForLocalAddr,

    /// Can't parse URL from the given string
    MalformedUrl,

    /// The provided URL scheme {_0} was not recognized
    UnknownUrlScheme(String),

    /// No host information found in URL, while it is required for the given
    /// schema
    HostRequired,

    /// Invalid public key data representing node id
    #[from(secp256k1::Error)]
    InvalidPubkey,

    /// Unrecognized host information ({_0}).
    /// NB: DNS addressing is not used since it is considered insecure in terms
    ///     of censorship resistance, so you need to provide it in a form of
    ///     either IPv4, IPv6 address or Tor v2, v3 address (no `.onion`
    /// suffix)
    #[from]
    InvalidHost(String),

    /// Used schema must not contain information about host
    HostPresent,

    /// Used schema must not contain information about port
    PortPresent,

    /// Invalid IP information
    #[from(AddrParseError)]
    InvalidIp,

    /// Unsupported ZMQ API type ({_0}). List of supported APIs:
    /// - `rpc`
    /// - `p2p`
    /// - `sub`
    /// - `esb`
    InvalidZmqType(String),

    /// No ZMQ API type information for URL scheme that requires one.
    ApiTypeRequired,

    /// Creation of `Inproc` ZMQ locator requires ZMQ context, while no context
    /// is provided.
    InprocRequireZmqContext,
}

impl Display for NodeLocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> ::core::fmt::Result {
        if f.alternate() {
            self.node_id()
                .map(|id| write!(f, "{}", id))
                .unwrap_or(Ok(()))?;
            if let Some(addr) = self.inet_addr() {
                write!(f, "@{}", addr)?;
                self.port()
                    .map(|port| write!(f, ":{}", port))
                    .unwrap_or(Ok(()))?;
            } else {
                f.write_str(&self.socket_name().expect("Socket name is always known if internet address is not given"))?;
            }
            if let Some(api) = self.api_type() {
                write!(f, "?api={}", api.api_name())?;
            }
            Ok(())
        } else {
            #[cfg(feature = "url")]
            {
                write!(f, "{}", Url::from(self))
            }
            #[cfg(not(feature = "url"))]
            {
                f.write_str(&self.to_url_string())
            }
        }
    }
}

impl Debug for NodeLocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> ::core::fmt::Result {
        match self {
            NodeLocator::Native(pubkey, inet, port) => writeln!(
                f,
                "NodeLocator::Native({:?}, {:?}, {:?})",
                pubkey, inet, port
            ),
            NodeLocator::Udp(pubkey, ip, port) => writeln!(
                f,
                "NodeLocator::Udp({:?}, {:?}, {:?})",
                pubkey, ip, port
            ),
            NodeLocator::Posix(file) => {
                writeln!(f, "NodeLocator::Posix({:?})", file)
            }
            NodeLocator::ZmqIpc(file, api) => {
                writeln!(f, "NodeLocator::Ipc({:?}, {:?})", file, api)
            }
            NodeLocator::ZmqInproc(name, api) => writeln!(
                f,
                "NodeLocator::Inproc({:?}, <zmq::Context>, {:?})",
                name, api
            ),
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpEncrypted(pubkey, api, ip, port) => writeln!(
                f,
                "NodeLocator::ZmqEncrypted({:?}, {:?}, {:?}, {:?})",
                pubkey, api, ip, port
            ),
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpUnencrypted(api, ip, port) => writeln!(
                f,
                "NodeLocator::ZmqUnencrypted({:?}, {:?}, {:?})",
                api, ip, port
            ),
            NodeLocator::Http(pubkey, inet, port) => writeln!(
                f,
                "NodeLocator::Http({:?}, {:?}, {:?})",
                pubkey, inet, port
            ),
            #[cfg(feature = "websockets")]
            NodeLocator::Websocket(pubkey, ip, port) => writeln!(
                f,
                "NodeLocator::Websocket({:?}, {:?}, {:?})",
                pubkey, ip, port
            ),
            NodeLocator::Text(pubkey) => {
                writeln!(f, "NodeLocator::Text({:?})", pubkey)
            }
        }
    }
}

#[cfg(feature = "url")]
impl FromStr for NodeLocator {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.to_string();
        if vec!["lnp:", "lnpu:", "lnpz:", "lnpws:", "lnpt:", "lnph:"]
            .into_iter()
            .find(|p| s.starts_with(*p))
            .is_none()
        {
            s = format!("lnp://{}", s);
        }
        Url::from_str(&s)
            .map_err(|_| ParseError::MalformedUrl)?
            .try_into()
    }
}

#[cfg(feature = "url")]
impl TryFrom<Url> for NodeLocator {
    type Error = ParseError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        let pubkey = secp256k1::PublicKey::from_str(url.username());
        let host = url.host_str();
        let ip = host.map(|host| {
            host.parse::<IpAddr>()
                .map_err(|_| ParseError::InvalidHost(host.to_string()))
        });
        let port = url.port();
        match url.scheme() {
            "lnp" => Ok(NodeLocator::Native(
                pubkey?,
                host.ok_or(ParseError::HostRequired)?.parse::<InetAddr>()?,
                port,
            )),
            "lnpu" => Ok(NodeLocator::Udp(
                pubkey?,
                ip.ok_or(ParseError::HostRequired)??,
                port,
            )),
            "lnph" => Ok(NodeLocator::Http(
                pubkey?,
                host.ok_or(ParseError::HostRequired)?.parse::<InetAddr>()?,
                port,
            )),
            #[cfg(feature = "websockets")]
            "lnpws" => Ok(NodeLocator::Websocket(
                pubkey?,
                host.ok_or(ParseError::HostRequired)?.parse::<InetAddr>()?,
                port,
            )),
            #[cfg(feature = "zmq")]
            "lnpz" => {
                let zmq_type = match url
                    .query_pairs()
                    .find_map(
                        |(key, val)| {
                            if key == "api" {
                                Some(val)
                            } else {
                                None
                            }
                        },
                    )
                    .ok_or(ParseError::ApiTypeRequired)?
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "p2p" => Ok(zmqsocket::ApiType::PeerConnecting),
                    "rpc" => Ok(zmqsocket::ApiType::Client),
                    "sub" => Ok(zmqsocket::ApiType::Subscribe),
                    "esb" => Ok(zmqsocket::ApiType::EsbService),
                    unknown => {
                        Err(ParseError::InvalidZmqType(unknown.to_string()))
                    }
                }?;
                Ok(match (ip, pubkey) {
                    (Some(Err(_)), _) => Err(ParseError::InvalidIp)?,
                    (_, Err(_)) if !url.username().is_empty() => {
                        Err(ParseError::InvalidIp)?
                    }
                    (Some(Ok(ip)), Ok(pubkey)) => {
                        NodeLocator::ZmqTcpEncrypted(pubkey, zmq_type, ip, port)
                    }
                    (Some(Ok(ip)), _) => {
                        NodeLocator::ZmqTcpUnencrypted(zmq_type, ip, port)
                    }
                    (None, _) => {
                        if url.path().is_empty() {
                            Err(ParseError::InprocRequireZmqContext)?
                        }
                        // TODO: Check path data validity
                        NodeLocator::ZmqIpc(url.path().to_string(), zmq_type)
                    }
                })
            }
            "lnpt" => {
                // In this URL scheme we must not use IP address
                if let Ok(pubkey) = pubkey {
                    Err(ParseError::HostPresent)?
                }
                // In this URL scheme we must not use IP address
                if let Some(port) = port {
                    Err(ParseError::PortPresent)?
                }
                if let Some(host) = host {
                    Ok(NodeLocator::Text(secp256k1::PublicKey::from_str(host)?))
                } else {
                    Err(ParseError::InvalidPubkey)?
                }
            }
            unknown => Err(ParseError::UnknownUrlScheme(unknown.to_string())),
        }
    }
}

#[cfg(feature = "url")]
impl From<&NodeLocator> for Url {
    fn from(locator: &NodeLocator) -> Self {
        Url::parse(&locator.to_url_string())
            .expect("Internal URL construction error")
    }
}

impl TryFrom<NodeLocator> for LocalAddr {
    type Error = ParseError;

    fn try_from(value: NodeLocator) -> Result<Self, Self::Error> {
        Ok(match value {
            NodeLocator::Posix(path) => LocalAddr::Posix(path),
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqIpc(path, ..) => {
                LocalAddr::Zmq(zmqsocket::SocketLocator::Ipc(path))
            }
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqInproc(name, ..) => {
                LocalAddr::Zmq(zmqsocket::SocketLocator::Inproc(name))
            }
            #[cfg(feature = "zmq")]
            NodeLocator::ZmqTcpUnencrypted(_, ip, Some(port)) => {
                LocalAddr::Zmq(zmqsocket::SocketLocator::Tcp(SocketAddr::new(
                    ip, port,
                )))
            }
            _ => Err(ParseError::UnsupportedForLocalAddr)?,
        })
    }
}

#[cfg(test)]
mod test {
    use amplify::internet::InetSocketAddr;

    use super::*;
    use crate::lnp::session::{node_addr, NodeAddr};
    use crate::lnp::transport::RemoteAddr;
    use std::net::SocketAddr;

    #[test]
    fn test_native() {
        let pubkey1 = secp256k1::PublicKey::from_str(
            "022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let pubkey2 = secp256k1::PublicKey::from_str(
            "032e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let inet1 = InetAddr::from_str("127.0.0.1").unwrap();
        let inet2 = InetAddr::from_str("127.0.0.2").unwrap();
        let locator1 = NodeLocator::Native(pubkey1, inet1, None);
        let locator2 = NodeLocator::Native(pubkey2, inet2, None);

        assert_ne!(locator1, locator2);
        assert_eq!(locator1, locator1.clone());
        assert_eq!(locator2, locator2.clone());

        assert_eq!(locator1.url_scheme(), "lnp");
        assert_eq!(locator1.node_id(), Some(pubkey1));
        assert_eq!(locator1.port(), None);
        assert_eq!(locator1.api_type(), None);
        assert_eq!(locator1.inet_addr(), Some(inet1));
        assert_eq!(locator1.socket_name(), None);
        let locator_with_port = locator1.with_port(24);
        assert_eq!(locator_with_port.port(), Some(24));

        let socket_addr = InetSocketAddr {
            address: inet1,
            port: 24,
        };
        let node_addr = NodeAddr {
            node_id: pubkey1,
            remote_addr: RemoteAddr::Ftcp(socket_addr),
        };
        let l = NodeLocator::from(node_addr.clone());
        assert_eq!(l, locator_with_port);
        assert_ne!(l, locator1);
        assert_eq!(
            NodeAddr::try_from(locator1.clone()),
            Err(node_addr::Error::NoPort)
        );
        assert_eq!(
            NodeAddr::try_from(locator_with_port.clone()),
            Ok(node_addr)
        );

        assert_eq!(
            locator1.to_url_string(),
            "lnp://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1"
        );
        assert_eq!(
            locator_with_port.to_url_string(),
            "lnp://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24"
        );
        assert_eq!(
            l.to_url_string(),
            "lnp://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24"
        );

        #[cfg(feature = "url")]
        {
            assert_eq!(
                NodeLocator::from_str(
                    "lnp://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1"
                ).unwrap(),
                locator1
            );
            assert_eq!(
                NodeLocator::from_str(
                    "022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24"
                ).unwrap(),
                locator_with_port
            );

            #[cfg(feature = "tor")]
            {
                use torut::onion::{OnionAddressV2, OnionAddressV3};

                assert_eq!(
                    NodeLocator::from_str(
                        "lnp://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af\
                        @32zzibxmqi2ybxpqyggwwuwz7a3lbvtzoloti7cxoevyvijexvgsfeid"
                    ).unwrap().inet_addr().unwrap().to_onion().unwrap(),
                    OnionAddressV3::from_str(
                        "32zzibxmqi2ybxpqyggwwuwz7a3lbvtzoloti7cxoevyvijexvgsfeid"
                    ).unwrap()
                );

                assert_eq!(
                    NodeLocator::from_str(
                        "lnp://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af\
                        @6zdgh5a5e6zpchdz"
                    ).unwrap().inet_addr().unwrap().to_onion_v2().unwrap(),
                    OnionAddressV2::from_str(
                        "6zdgh5a5e6zpchdz"
                    ).unwrap()
                );
            }
        }
    }

    #[test]
    fn test_udp() {
        let pubkey1 = secp256k1::PublicKey::from_str(
            "022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let pubkey2 = secp256k1::PublicKey::from_str(
            "032e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let inet1 = IpAddr::from_str("127.0.0.1").unwrap();
        let inet2 = IpAddr::from_str("127.0.0.2").unwrap();
        let locator1 = NodeLocator::Udp(pubkey1, inet1, None);
        let locator2 = NodeLocator::Udp(pubkey2, inet2, None);

        assert_ne!(locator1, locator2);
        assert_eq!(locator1, locator1.clone());
        assert_eq!(locator2, locator2.clone());

        assert_eq!(locator1.url_scheme(), "lnpu");
        assert_eq!(locator1.node_id(), Some(pubkey1));
        assert_eq!(locator1.port(), None);
        assert_eq!(locator1.api_type(), None);
        assert_eq!(locator1.inet_addr(), Some(InetAddr::from(inet1)));
        assert_eq!(locator1.socket_name(), None);
        let locator_with_port = locator1.with_port(24);
        assert_eq!(locator_with_port.port(), Some(24));

        assert_eq!(
            NodeAddr::try_from(locator1.clone()),
            Err(node_addr::Error::UnsupportedType)
        );
        assert_eq!(
            NodeAddr::try_from(locator_with_port.clone()),
            Err(node_addr::Error::UnsupportedType)
        );

        assert_eq!(
            locator1.to_url_string(),
            "lnpu://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1"
        );
        assert_eq!(
            locator_with_port.to_url_string(),
            "lnpu://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24"
        );

        #[cfg(feature = "url")]
        {
            assert_eq!(
                NodeLocator::from_str(
                    "lnpu://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1"
                ).unwrap(),
                locator1
            );
            assert_eq!(
                NodeLocator::from_str(
                    "lnpu://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24"
                ).unwrap(),
                locator_with_port
            );
        }
    }

    #[cfg(feature = "websockets")]
    #[test]
    fn test_websocket() {
        let pubkey1 = secp256k1::PublicKey::from_str(
            "022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let pubkey2 = secp256k1::PublicKey::from_str(
            "032e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let inet1 = InetAddr::from_str("127.0.0.1").unwrap();
        let inet2 = InetAddr::from_str("127.0.0.2").unwrap();
        let locator1 = NodeLocator::Websocket(pubkey1, inet1, None);
        let locator2 = NodeLocator::Websocket(pubkey2, inet2, None);

        assert_ne!(locator1, locator2);
        assert_eq!(locator1, locator1.clone());
        assert_eq!(locator2, locator2.clone());

        assert_eq!(locator1.url_scheme(), "lnpws");
        assert_eq!(locator1.node_id(), Some(pubkey1));
        assert_eq!(locator1.port(), None);
        assert_eq!(locator1.api_type(), None);
        assert_eq!(locator1.inet_addr(), Some(InetAddr::from(inet1)));
        assert_eq!(locator1.socket_name(), None);
        let locator_with_port = locator1.with_port(24);
        assert_eq!(locator_with_port.port(), Some(24));

        assert_eq!(
            NodeAddr::try_from(locator1.clone()),
            Err(node_addr::Error::UnsupportedType)
        );
        assert_eq!(
            NodeAddr::try_from(locator_with_port.clone()),
            Err(node_addr::Error::UnsupportedType)
        );

        assert_eq!(
            locator1.to_url_string(),
            "lnpws://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1"
        );
        assert_eq!(
            locator_with_port.to_url_string(),
            "lnpws://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24"
        );

        #[cfg(feature = "url")]
        {
            assert_eq!(
                NodeLocator::from_str(
                    "lnpws://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1"
                ).unwrap(),
                locator1
            );
            assert_eq!(
                NodeLocator::from_str(
                    "lnpws://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24"
                ).unwrap(),
                locator_with_port
            );
        }
    }

    #[cfg(feature = "zmq")]
    #[test]
    fn test_zmq_encrypted() {
        let pubkey1 = secp256k1::PublicKey::from_str(
            "022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let pubkey2 = secp256k1::PublicKey::from_str(
            "032e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let inet1 = IpAddr::from_str("127.0.0.1").unwrap();
        let inet2 = IpAddr::from_str("127.0.0.2").unwrap();
        let locator1 = NodeLocator::ZmqTcpEncrypted(
            pubkey1,
            zmqsocket::ApiType::PeerListening,
            inet1,
            None,
        );
        let locator2 = NodeLocator::ZmqTcpEncrypted(
            pubkey2,
            zmqsocket::ApiType::Client,
            inet2,
            None,
        );
        let locator3 = NodeLocator::ZmqTcpEncrypted(
            pubkey1,
            zmqsocket::ApiType::PeerConnecting,
            inet1,
            None,
        );
        let locator4 = NodeLocator::ZmqTcpEncrypted(
            pubkey2,
            zmqsocket::ApiType::Server,
            inet2,
            None,
        );

        assert_ne!(locator1, locator2);
        assert_ne!(locator2, locator4);
        assert_eq!(locator1, locator3);
        assert_eq!(locator1, locator1.clone());
        assert_eq!(locator2, locator2.clone());

        assert_eq!(locator1.url_scheme(), "lnpz");
        assert_eq!(locator1.node_id(), Some(pubkey1));
        assert_eq!(locator1.port(), None);
        assert_eq!(
            locator1.api_type(),
            Some(zmqsocket::ApiType::PeerListening)
        );
        assert_eq!(locator1.inet_addr(), Some(InetAddr::from(inet1)));
        assert_eq!(locator1.socket_name(), None);
        let locator_with_port = locator1.with_port(24);
        assert_eq!(locator_with_port.port(), Some(24));

        assert_eq!(
            NodeAddr::try_from(locator1.clone()),
            Err(node_addr::Error::NoPort)
        );
        assert_eq!(
            NodeAddr::try_from(locator_with_port.clone()),
            Ok(NodeAddr {
                node_id: pubkey1,
                remote_addr: RemoteAddr::Zmq(SocketAddr::new(inet1, 24))
            })
        );

        assert_eq!(
            locator1.to_url_string(),
            "lnpz://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1/?api=p2p"
        );
        assert_eq!(
            locator2.to_url_string(),
            "lnpz://032e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.2/?api=rpc"
        );
        assert_eq!(
            locator_with_port.to_url_string(),
            "lnpz://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24/?api=p2p"
        );

        #[cfg(feature = "url")]
        {
            assert_eq!(
                NodeLocator::from_str(
                    "lnpz://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1/?api=p2p"
                ).unwrap(),
                locator1
            );
            assert_eq!(
                NodeLocator::from_str(
                    "lnpz://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.0.1:24/?api=p2p"
                ).unwrap(),
                locator_with_port
            );
        }
    }

    #[cfg(feature = "zmq")]
    #[test]
    fn test_zmq_unencrypted() {
        let inet1 = IpAddr::from_str("127.0.0.1").unwrap();
        let inet2 = IpAddr::from_str("127.0.0.2").unwrap();
        let locator1 = NodeLocator::ZmqTcpUnencrypted(
            zmqsocket::ApiType::PeerListening,
            inet1,
            None,
        );
        let locator2 = NodeLocator::ZmqTcpUnencrypted(
            zmqsocket::ApiType::Client,
            inet2,
            None,
        );
        let locator3 = NodeLocator::ZmqTcpUnencrypted(
            zmqsocket::ApiType::PeerConnecting,
            inet1,
            None,
        );
        let locator4 = NodeLocator::ZmqTcpUnencrypted(
            zmqsocket::ApiType::Server,
            inet2,
            None,
        );

        assert_ne!(locator1, locator2);
        assert_ne!(locator2, locator4);
        assert_eq!(locator1, locator3);
        assert_eq!(locator1, locator1.clone());
        assert_eq!(locator2, locator2.clone());

        assert_eq!(locator1.url_scheme(), "lnpz");
        assert_eq!(locator1.node_id(), None);
        assert_eq!(locator1.port(), None);
        assert_eq!(
            locator1.api_type(),
            Some(zmqsocket::ApiType::PeerListening)
        );
        assert_eq!(locator1.inet_addr(), Some(InetAddr::from(inet1)));
        let locator_with_port = locator1.with_port(24);
        assert_eq!(locator_with_port.port(), Some(24));

        assert_eq!(
            NodeAddr::try_from(locator1.clone()),
            Err(node_addr::Error::UnsupportedType)
        );
        assert_eq!(
            NodeAddr::try_from(locator_with_port.clone()),
            Err(node_addr::Error::UnsupportedType)
        );

        assert_eq!(locator1.to_url_string(), "lnpz://127.0.0.1/?api=p2p");
        assert_eq!(locator2.to_url_string(), "lnpz://127.0.0.2/?api=rpc");
        assert_eq!(
            locator_with_port.to_url_string(),
            "lnpz://127.0.0.1:24/?api=p2p"
        );

        #[cfg(feature = "url")]
        {
            assert_eq!(
                NodeLocator::from_str("lnpz://127.0.0.1/?api=p2p").unwrap(),
                locator1
            );
        }
    }

    #[cfg(feature = "zmq")]
    #[test]
    fn test_zmq_inproc() {
        let locator1 = NodeLocator::ZmqInproc(
            s!("socket1"),
            zmqsocket::ApiType::PeerListening,
        );
        let locator1_1 = NodeLocator::ZmqInproc(
            s!("socket1"),
            zmqsocket::ApiType::PeerListening,
        );
        let locator2 =
            NodeLocator::ZmqInproc(s!("socket2"), zmqsocket::ApiType::Client);
        let locator3 = NodeLocator::ZmqInproc(
            s!("socket1"),
            zmqsocket::ApiType::PeerConnecting,
        );
        let locator4 =
            NodeLocator::ZmqInproc(s!("socket2"), zmqsocket::ApiType::Server);

        assert_eq!(locator1, locator1_1);
        assert_ne!(locator1, locator2);
        assert_ne!(locator2, locator4);
        assert_eq!(locator1, locator3);
        assert_eq!(locator1, locator1.clone());
        assert_eq!(locator2, locator2.clone());

        assert_eq!(locator1.url_scheme(), "lnpz");
        assert_eq!(locator1.node_id(), None);
        assert_eq!(locator1.port(), None);
        assert_eq!(
            locator1.api_type(),
            Some(zmqsocket::ApiType::PeerListening)
        );
        assert_eq!(locator1.inet_addr(), None);
        assert_eq!(locator1.socket_name(), Some(s!("socket1")));
        let locator_with_port = locator1.with_port(24);
        assert_eq!(locator_with_port.port(), None);

        assert_eq!(
            NodeAddr::try_from(locator1.clone()),
            Err(node_addr::Error::UnsupportedType)
        );
        assert_eq!(
            NodeAddr::try_from(locator_with_port.clone()),
            Err(node_addr::Error::UnsupportedType)
        );

        assert_eq!(locator1.to_url_string(), "lnpz:?api=p2p#socket1");
        assert_eq!(locator2.to_url_string(), "lnpz:?api=rpc#socket2");
        assert_eq!(locator_with_port.to_url_string(), "lnpz:?api=p2p#socket1");

        #[cfg(feature = "url")]
        {
            assert_eq!(
                NodeLocator::from_str("lnpz:?api=p2p#socket1").unwrap_err(),
                ParseError::InprocRequireZmqContext
            );
        }
    }

    #[cfg(feature = "zmq")]
    #[test]
    fn test_zmq_ipc() {
        let locator1 = NodeLocator::ZmqIpc(
            s!("./socket1"),
            zmqsocket::ApiType::PeerListening,
        );
        let locator2 =
            NodeLocator::ZmqIpc(s!("./socket2"), zmqsocket::ApiType::Client);
        let locator3 = NodeLocator::ZmqIpc(
            s!("./socket1"),
            zmqsocket::ApiType::PeerConnecting,
        );
        let locator4 =
            NodeLocator::ZmqIpc(s!("./socket2"), zmqsocket::ApiType::Server);

        assert_ne!(locator1, locator2);
        assert_ne!(locator2, locator4);
        assert_eq!(locator1, locator3);
        assert_eq!(locator1, locator1.clone());
        assert_eq!(locator2, locator2.clone());

        assert_eq!(locator1.url_scheme(), "lnpz");
        assert_eq!(locator1.node_id(), None);
        assert_eq!(locator1.port(), None);
        assert_eq!(
            locator1.api_type(),
            Some(zmqsocket::ApiType::PeerListening)
        );
        assert_eq!(locator1.inet_addr(), None);
        assert_eq!(locator1.socket_name(), Some(s!("./socket1")));
        let locator_with_port = locator1.with_port(24);
        assert_eq!(locator_with_port.port(), None);

        assert_eq!(
            NodeAddr::try_from(locator1.clone()),
            Err(node_addr::Error::UnsupportedType)
        );
        assert_eq!(
            NodeAddr::try_from(locator_with_port.clone()),
            Err(node_addr::Error::UnsupportedType)
        );

        assert_eq!(locator1.to_url_string(), "lnpz:./socket1?api=p2p");
        assert_eq!(locator2.to_url_string(), "lnpz:./socket2?api=rpc");
        assert_eq!(locator_with_port.to_url_string(), "lnpz:./socket1?api=p2p");

        #[cfg(feature = "url")]
        {
            assert_eq!(
                NodeLocator::from_str("lnpz:./socket1?api=p2p").unwrap(),
                locator1
            );
        }
    }

    #[test]
    fn test_text() {
        let pubkey1 = secp256k1::PublicKey::from_str(
            "022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let pubkey2 = secp256k1::PublicKey::from_str(
            "032e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        ).unwrap();
        let locator1 = NodeLocator::Text(pubkey1);
        let locator2 = NodeLocator::Text(pubkey2);

        assert_ne!(locator1, locator2);
        assert_eq!(locator1, locator1.clone());
        assert_eq!(locator2, locator2.clone());

        assert_eq!(locator1.url_scheme(), "lnpt");
        assert_eq!(locator1.node_id(), Some(pubkey1));
        assert_eq!(locator1.port(), None);
        assert_eq!(locator1.api_type(), None);
        assert_eq!(locator1.inet_addr(), None);
        assert_eq!(locator1.socket_name(), None);
        let locator_with_port = locator1.with_port(24);
        assert_eq!(locator_with_port.port(), None);

        assert_eq!(
            NodeAddr::try_from(locator1.clone()),
            Err(node_addr::Error::UnsupportedType)
        );
        assert_eq!(
            NodeAddr::try_from(locator_with_port.clone()),
            Err(node_addr::Error::UnsupportedType)
        );

        assert_eq!(
            locator1.to_url_string(),
            "lnpt://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        );
        assert_eq!(
            locator_with_port.to_url_string(),
            "lnpt://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
        );

        #[cfg(feature = "url")]
        {
            assert_eq!(
                NodeLocator::from_str(
                    "lnpt://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af"
                ).unwrap(),
                locator1
            );
            assert_eq!(
                NodeLocator::from_str(
                    "lnpt://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af@127.0.01"
                ).unwrap_err(),
                ParseError::HostPresent
            );
            assert_eq!(
                NodeLocator::from_str(
                    "lnpt://022e58afe51f9ed8ad3cc7897f634d881fdbe49a81564629ded8156bebd2ffd1af:1323"
                ).unwrap_err(),
                ParseError::PortPresent
            );
        }
    }
}
