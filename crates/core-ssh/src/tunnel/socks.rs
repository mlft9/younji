//! Serveur SOCKS5 minimal pour la redirection dynamique `-D` (TUN-01), RFC 1928.
//!
//! Seul le strict nécessaire pour un navigateur est pris en charge : méthode « sans
//! authentification » et commande `CONNECT`. `BIND` et `UDP ASSOCIATE` sont refusés. La
//! protection du proxy est l'écoute sur `127.0.0.1` par défaut (TUN-03).
//!
//! La négociation est bornée en taille par construction (au plus 1 + 1 + 255 octets par
//! message) ; l'appelant la borne en temps.

use std::net::{Ipv4Addr, Ipv6Addr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const VERSION: u8 = 0x05;
const METHOD_NO_AUTH: u8 = 0x00;
const METHOD_NONE_ACCEPTABLE: u8 = 0xFF;
const CMD_CONNECT: u8 = 0x01;
const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;

/// Code de réponse SOCKS5 (RFC 1928 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reply {
    Succeeded = 0x00,
    GeneralFailure = 0x01,
    HostUnreachable = 0x04,
    CommandNotSupported = 0x07,
    AddressTypeNotSupported = 0x08,
}

/// Destination demandée par le client SOCKS. Le nom de domaine est transmis tel quel au
/// serveur SSH, qui le résout (le poste ne fait aucune requête DNS pour la destination).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SocksRequest {
    pub host: String,
    pub port: u16,
}

/// Cause d'échec de la négociation. Une réponse d'erreur a déjà été envoyée quand le protocole
/// le permet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SocksError {
    /// Flux interrompu ou erreur d'E/S.
    Io,
    /// Version autre que 5.
    BadVersion,
    /// Le client ne propose pas la méthode « sans authentification ».
    NoAcceptableMethod,
    /// Commande autre que `CONNECT`.
    UnsupportedCommand,
    /// Type d'adresse inconnu.
    UnsupportedAddress,
    /// Nom de domaine ou port invalide.
    InvalidDestination,
}

/// Négocie une demande `CONNECT`. En cas de succès, l'appelant ouvre le canal puis appelle
/// [`reply`] avec [`Reply::Succeeded`] ou un code d'échec.
pub(crate) async fn handshake<S>(stream: &mut S) -> Result<SocksRequest, SocksError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // Salutation : VER, NMETHODS, METHODS.
    let mut head = [0u8; 2];
    read(stream, &mut head).await?;
    if head[0] != VERSION {
        return Err(SocksError::BadVersion);
    }
    let mut methods = vec![0u8; usize::from(head[1])];
    read(stream, &mut methods).await?;
    if !methods.contains(&METHOD_NO_AUTH) {
        let _ = stream.write_all(&[VERSION, METHOD_NONE_ACCEPTABLE]).await;
        return Err(SocksError::NoAcceptableMethod);
    }
    write(stream, &[VERSION, METHOD_NO_AUTH]).await?;

    // Demande : VER, CMD, RSV, ATYP, DST.ADDR, DST.PORT.
    let mut request = [0u8; 4];
    read(stream, &mut request).await?;
    if request[0] != VERSION {
        return Err(SocksError::BadVersion);
    }
    let host = match request[3] {
        ATYP_IPV4 => {
            let mut ip = [0u8; 4];
            read(stream, &mut ip).await?;
            Ipv4Addr::from(ip).to_string()
        }
        ATYP_IPV6 => {
            let mut ip = [0u8; 16];
            read(stream, &mut ip).await?;
            Ipv6Addr::from(ip).to_string()
        }
        ATYP_DOMAIN => {
            let mut len = [0u8; 1];
            read(stream, &mut len).await?;
            let mut name = vec![0u8; usize::from(len[0])];
            read(stream, &mut name).await?;
            match domain(&name) {
                Some(name) => name,
                None => {
                    // Port lu pour consommer la demande avant de répondre.
                    let _ = read(stream, &mut [0u8; 2]).await;
                    reply(stream, Reply::GeneralFailure).await;
                    return Err(SocksError::InvalidDestination);
                }
            }
        }
        _ => {
            reply(stream, Reply::AddressTypeNotSupported).await;
            return Err(SocksError::UnsupportedAddress);
        }
    };
    let mut port = [0u8; 2];
    read(stream, &mut port).await?;
    let port = u16::from_be_bytes(port);
    if request[1] != CMD_CONNECT {
        reply(stream, Reply::CommandNotSupported).await;
        return Err(SocksError::UnsupportedCommand);
    }
    if port == 0 {
        reply(stream, Reply::HostUnreachable).await;
        return Err(SocksError::InvalidDestination);
    }
    Ok(SocksRequest { host, port })
}

/// Envoie la réponse à une demande `CONNECT`. L'adresse liée n'est pas communiquée (zéros) :
/// elle n'apprendrait rien d'utile au client et révélerait le réseau du serveur.
pub(crate) async fn reply<S>(stream: &mut S, code: Reply)
where
    S: AsyncWrite + Unpin,
{
    let message = [VERSION, code as u8, 0x00, ATYP_IPV4, 0, 0, 0, 0, 0, 0];
    let _ = stream.write_all(&message).await;
}

/// Nom de domaine admis : 1 à 255 octets ASCII imprimables, sans espace.
fn domain(raw: &[u8]) -> Option<String> {
    let valid = !raw.is_empty() && raw.iter().all(|b| b.is_ascii_graphic());
    valid.then(|| String::from_utf8_lossy(raw).into_owned())
}

async fn read<S: AsyncRead + Unpin>(stream: &mut S, buf: &mut [u8]) -> Result<(), SocksError> {
    stream
        .read_exact(buf)
        .await
        .map(|_| ())
        .map_err(|_| SocksError::Io)
}

async fn write<S: AsyncWrite + Unpin>(stream: &mut S, data: &[u8]) -> Result<(), SocksError> {
    stream.write_all(data).await.map_err(|_| SocksError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Joue `input` côté client et renvoie le résultat et ce que le serveur a répondu.
    async fn run(input: &[u8]) -> (Result<SocksRequest, SocksError>, Vec<u8>) {
        let (mut client, mut server) = tokio::io::duplex(1024);
        client.write_all(input).await.unwrap();
        // Fin de l'envoi : une demande tronquée se termine par une erreur, pas une attente.
        client.shutdown().await.unwrap();
        let result = handshake(&mut server).await;
        drop(server);
        let mut answer = Vec::new();
        client.read_to_end(&mut answer).await.unwrap();
        (result, answer)
    }

    #[tokio::test]
    async fn connect_ipv4() {
        let (result, answer) = run(&[5, 1, 0, 5, 1, 0, 1, 127, 0, 0, 1, 0x1F, 0x90]).await;
        assert_eq!(
            result,
            Ok(SocksRequest {
                host: "127.0.0.1".into(),
                port: 8080
            })
        );
        assert_eq!(answer, [5, 0]);
    }

    #[tokio::test]
    async fn connect_domain_and_ipv6() {
        let mut input = vec![5, 2, 2, 0, 5, 1, 0, 3, 11];
        input.extend_from_slice(b"example.org");
        input.extend_from_slice(&443u16.to_be_bytes());
        let (result, _) = run(&input).await;
        assert_eq!(result.unwrap().host, "example.org");

        let mut input = vec![5, 1, 0, 5, 1, 0, 4];
        input.extend_from_slice(&Ipv6Addr::LOCALHOST.octets());
        input.extend_from_slice(&22u16.to_be_bytes());
        let (result, _) = run(&input).await;
        assert_eq!(result.unwrap().host, "::1");
    }

    #[tokio::test]
    async fn authentication_only_clients_are_refused() {
        let (result, answer) = run(&[5, 1, 2]).await;
        assert_eq!(result, Err(SocksError::NoAcceptableMethod));
        assert_eq!(answer, [5, 0xFF]);
    }

    #[tokio::test]
    async fn other_versions_are_refused() {
        assert_eq!(run(&[4, 1, 0]).await.0, Err(SocksError::BadVersion));
        assert_eq!(
            run(&[5, 1, 0, 4, 1, 0, 1, 1, 2, 3, 4, 0, 80]).await.0,
            Err(SocksError::BadVersion)
        );
    }

    #[tokio::test]
    async fn bind_and_udp_are_refused() {
        for command in [2u8, 3] {
            let (result, answer) = run(&[5, 1, 0, 5, command, 0, 1, 1, 2, 3, 4, 0, 80]).await;
            assert_eq!(result, Err(SocksError::UnsupportedCommand));
            assert_eq!(answer[2..4], [5, Reply::CommandNotSupported as u8]);
        }
    }

    #[tokio::test]
    async fn bad_destinations_are_refused() {
        // Type d'adresse inconnu.
        let (result, answer) = run(&[5, 1, 0, 5, 1, 0, 9]).await;
        assert_eq!(result, Err(SocksError::UnsupportedAddress));
        assert_eq!(answer[3], Reply::AddressTypeNotSupported as u8);
        // Domaine vide, avec un caractère de contrôle ou une espace.
        for name in [&b""[..], b"a\x1bb", b"a b"] {
            let mut input = vec![5, 1, 0, 5, 1, 0, 3, u8::try_from(name.len()).unwrap()];
            input.extend_from_slice(name);
            input.extend_from_slice(&80u16.to_be_bytes());
            assert_eq!(run(&input).await.0, Err(SocksError::InvalidDestination));
        }
        // Port 0.
        let (result, _) = run(&[5, 1, 0, 5, 1, 0, 1, 1, 2, 3, 4, 0, 0]).await;
        assert_eq!(result, Err(SocksError::InvalidDestination));
    }

    #[tokio::test]
    async fn truncated_input_is_an_error() {
        assert_eq!(run(&[5, 3, 0]).await.0, Err(SocksError::Io));
        assert_eq!(
            run(&[5, 1, 0, 5, 1, 0, 1, 127]).await.0,
            Err(SocksError::Io)
        );
    }

    #[tokio::test]
    async fn reply_hides_the_bound_address() {
        let (mut client, mut server) = tokio::io::duplex(64);
        reply(&mut server, Reply::Succeeded).await;
        drop(server);
        let mut answer = Vec::new();
        client.read_to_end(&mut answer).await.unwrap();
        assert_eq!(answer, [5, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    }
}
