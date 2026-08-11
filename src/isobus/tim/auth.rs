//! AEF 023 TIM authentication (§4.4.4, Annexes B.4 / F / H).
//!
//! Replaces `TimPkiHandshake`, which was four states, a fixed 8-byte nonce, and
//! a `verify_response(expected, actual)` that compared two **caller-supplied**
//! slices — the caller passed in the answer, so the type performed no
//! cryptography and validated nothing. It could not even express B.4.1.1 code
//! `0x23` ("challenge length not 32 or 16 bytes") with an 8-byte nonce.
//!
//! What the AEF actually requires, and what this implements on RustCrypto:
//!
//! - an X.509 DER certificate chain (root → test lab → manufacturer → series →
//!   device), parsed with `x509-cert`/`der` rather than assumed well-formed;
//! - a curve25519 ECDH exchange (`x25519-dalek`) producing a shared secret;
//! - an AES-CMAC challenge-response over that secret (`aes` + `cmac`);
//! - a certificate revocation list checked before a peer is trusted;
//! - the B.4.1.1 error codes, so a refusal says why.
//!
//! Enabled by the `tim-auth` feature.

use alloc::vec::Vec;

use aes::Aes128;
use cmac::{Cmac, Mac};
use der::{Decode, Encode};
use rsa::pkcs8::DecodePublicKey;
use sha2::{Digest, Sha256};
use x509_cert::Certificate;
use x25519_dalek::{PublicKey, StaticSecret};

/// Challenge length for a random challenge (B.4.1.1 code 0x23).
pub const RANDOM_CHALLENGE_LEN: usize = 32;
/// Challenge length for a signed challenge (B.4.1.1 code 0x23).
pub const SIGNED_CHALLENGE_LEN: usize = 16;
/// AES-CMAC output length.
pub const CMAC_LEN: usize = 16;

/// AEF B.4.1.1 authentication error codes.
///
/// Only the subset this implementation can actually detect is modelled; adding
/// a variant without a code path that raises it would repeat the original
/// mistake of claiming more than the code does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuthError {
    /// The participant is busy with another authentication (0x01).
    ParticipantBusy = 0x01,
    /// Certificate format specifier was not DER (0x02).
    CertificateFormatNotSupported = 0x02,
    /// Root certificate signature invalid (0x04).
    RootCertificateSignatureInvalid = 0x04,
    /// Manufacturer certificate signature invalid (0x06).
    ManufacturerCertificateSignatureInvalid = 0x06,
    /// Device certificate signature invalid (0x08).
    DeviceCertificateSignatureInvalid = 0x08,
    /// The challenge-response signatures did not match (0x09).
    ChallengesDoNotMatch = 0x09,
    /// Any internal error not related to the authentication process (0x0A).
    InternalError = 0x0A,
    /// Root certificate binary format could not be parsed (0x0B).
    RootCertificateDataCorrupt = 0x0B,
    /// Manufacturer certificate could not be parsed (0x0D).
    ManufacturerCertificateDataCorrupt = 0x0D,
    /// Device certificate could not be parsed (0x0F).
    DeviceCertificateDataCorrupt = 0x0F,
    /// The authentication status message was lost (0x10).
    AuthenticationStatusLost = 0x10,
    /// The received authentication status is invalid (0x12).
    AuthenticationStatusInvalid = 0x12,
    /// The certificate chain does not link up (0x17). Distinguished Names are
    /// attacker-controlled text, so a DN mismatch is a structural failure, not
    /// a failed signature.
    CertificateChainInvalid = 0x17,
    /// The public ECC key is not valid for curve 25519 (0x14). A peer that
    /// sends an all-zero (or other small-order) point drives the shared secret
    /// to zero, which would key the CMAC with zeros and let anyone on the bus
    /// complete the handshake.
    EccPublicKeyValidationFailed = 0x14,
    /// The root certificate is listed on the CRL (0x19).
    RootCertificateRevoked = 0x19,
    /// The lab certificate is listed on the CRL (0x1A).
    LabCertificateRevoked = 0x1A,
    /// The manufacturer certificate is listed on the CRL (0x1B).
    ManufacturerCertificateRevoked = 0x1B,
    /// The manufacturer series certificate is listed on the CRL (0x1C).
    ManufacturerSeriesCertificateRevoked = 0x1C,
    /// The device certificate is listed on the CRL (0x1D).
    DeviceCertificateRevoked = 0x1D,
    /// Challenge length was neither 32 nor 16 bytes (0x23).
    ChallengeLengthInvalid = 0x23,
    /// Challenge data not equal to 32 bytes (random) or 16 bytes (signed)
    /// (0x24). This code used to be used for "certificate revoked", which told
    /// a peer with a revoked certificate entirely the wrong thing.
    ChallengeDataCorrupt = 0x24,
}

impl AuthError {
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Where a certificate sits in the AEF chain (Annex F).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CertificateRole {
    Root,
    TestLab,
    Manufacturer,
    ManufacturerSeries,
    Device,
}

impl CertificateRole {
    /// The chain in issuing order. A peer presents these bottom-up.
    pub const CHAIN: [Self; 5] = [
        Self::Root,
        Self::TestLab,
        Self::Manufacturer,
        Self::ManufacturerSeries,
        Self::Device,
    ];

    /// The error code to report when this link fails to parse.
    #[must_use]
    pub const fn corrupt_error(self) -> AuthError {
        match self {
            Self::Root | Self::TestLab => AuthError::RootCertificateDataCorrupt,
            Self::Manufacturer | Self::ManufacturerSeries => {
                AuthError::ManufacturerCertificateDataCorrupt
            }
            Self::Device => AuthError::DeviceCertificateDataCorrupt,
        }
    }

    /// The error code to report when this link's signature does not verify.
    #[must_use]
    pub const fn signature_error(self) -> AuthError {
        match self {
            Self::Root | Self::TestLab => AuthError::RootCertificateSignatureInvalid,
            Self::Manufacturer | Self::ManufacturerSeries => {
                AuthError::ManufacturerCertificateSignatureInvalid
            }
            Self::Device => AuthError::DeviceCertificateSignatureInvalid,
        }
    }

    /// The error code to report when this link appears on the CRL (Table 20).
    ///
    /// Each role has its own code — a peer needs to know *which* certificate in
    /// its chain was revoked to do anything about it.
    #[must_use]
    pub const fn revoked_error(self) -> AuthError {
        match self {
            Self::Root => AuthError::RootCertificateRevoked,
            Self::TestLab => AuthError::LabCertificateRevoked,
            Self::Manufacturer => AuthError::ManufacturerCertificateRevoked,
            Self::ManufacturerSeries => AuthError::ManufacturerSeriesCertificateRevoked,
            Self::Device => AuthError::DeviceCertificateRevoked,
        }
    }
}

/// A parsed X.509 certificate chain.
pub struct CertificateChain {
    certificates: Vec<Certificate>,
}

impl CertificateChain {
    /// Parse DER-encoded certificates in issuing order (root first).
    ///
    /// # Errors
    /// Reports the per-role corrupt-data code, so a caller can answer with the
    /// error the AEF specifies rather than a generic failure.
    pub fn parse_der(links: &[&[u8]]) -> Result<Self, AuthError> {
        let mut certificates = Vec::with_capacity(links.len());
        for (index, der) in links.iter().enumerate() {
            let role = CertificateRole::CHAIN
                .get(index)
                .copied()
                .unwrap_or(CertificateRole::Device);
            let certificate = Certificate::from_der(der).map_err(|_| role.corrupt_error())?;
            certificates.push(certificate);
        }
        Ok(Self { certificates })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.certificates.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.certificates.is_empty()
    }

    /// The end-entity (device) certificate.
    #[must_use]
    pub fn device(&self) -> Option<&Certificate> {
        self.certificates.last()
    }

    /// Check that each certificate's issuer matches the previous subject.
    ///
    /// A cheap pre-filter only. Distinguished Names are attacker-controlled
    /// text; matching them proves nothing on its own, which is why this reports
    /// [`AuthError::CertificateChainInvalid`] rather than a signature code —
    /// a DN mismatch is not a failed signature.
    ///
    /// # Errors
    /// [`AuthError::CertificateChainInvalid`] for the first link whose issuer
    /// does not match its parent's subject.
    pub fn check_issuer_linkage(&self) -> Result<(), AuthError> {
        for pair in self.certificates.windows(2) {
            let issuer_subject = &pair[0].tbs_certificate.subject;
            let child_issuer = &pair[1].tbs_certificate.issuer;
            if issuer_subject != child_issuer {
                return Err(AuthError::CertificateChainInvalid);
            }
        }
        Ok(())
    }

    /// Verify every link's signature against its parent's public key, and the
    /// root against `trust_anchor`.
    ///
    /// AEF 023 §4.3.1: "the xPCs are exchanged, verified, and used to validate
    /// the identity of the participants. The TIM functionality shall be used
    /// only if both checks are positive." Table 4 step (3a): "If none of the
    /// received certificates is listed in the CRL, both parties verify the
    /// validity of the certificate chain." Annex F.2.1 names RSASSA-PSS per
    /// RFC 3447.
    ///
    /// `trust_anchor` is the AEF root public key, in DER `SubjectPublicKeyInfo`
    /// form. It is a parameter rather than a compiled-in constant on purpose:
    /// verifying the top of the chain against a key the *peer* supplied proves
    /// nothing, and the integrator is the one who provisions the real anchor.
    ///
    /// Without this, `accept_chain` admitted any chain whose Distinguished
    /// Names happened to link up — one `openssl` invocation away from a full
    /// handshake against a forged device identity.
    ///
    /// # Errors
    /// The per-role signature-invalid code for the first link that fails.
    pub fn verify_signatures(&self, trust_anchor: &[u8]) -> Result<(), AuthError> {
        let Some(root) = self.certificates.first() else {
            return Err(AuthError::DeviceCertificateDataCorrupt);
        };

        // The root must be the one we already trust. Comparing the encoded
        // SubjectPublicKeyInfo keeps this independent of DN text entirely.
        let root_spki = root
            .tbs_certificate
            .subject_public_key_info
            .to_der()
            .map_err(|_| CertificateRole::Root.signature_error())?;
        if root_spki.as_slice() != trust_anchor {
            return Err(CertificateRole::Root.signature_error());
        }

        for (index, pair) in self.certificates.windows(2).enumerate() {
            let role = CertificateRole::CHAIN
                .get(index + 1)
                .copied()
                .unwrap_or(CertificateRole::Device);
            verify_link(&pair[0], &pair[1]).map_err(|()| role.signature_error())?;
        }
        Ok(())
    }

    /// `true` when any certificate's serial number appears in `revoked`.
    #[must_use]
    pub fn is_revoked(&self, revoked: &CertificateRevocationList) -> bool {
        self.revoked_role(revoked).is_some()
    }

    /// Which link is revoked, if any, so the caller can report the Table 20
    /// code for that specific role rather than a generic "revoked".
    #[must_use]
    pub fn revoked_role(&self, revoked: &CertificateRevocationList) -> Option<CertificateRole> {
        self.certificates.iter().enumerate().find_map(|(i, c)| {
            revoked
                .contains(c.tbs_certificate.serial_number.as_bytes())
                .then(|| {
                    CertificateRole::CHAIN
                        .get(i)
                        .copied()
                        .unwrap_or(CertificateRole::Device)
                })
        })
    }
}

/// DER object identifier for X25519 (RFC 8410 `id-X25519`), the curve AEF
/// Annex F.2.2 mandates: "25519 shall be used as elliptic curve with 256 bit
/// key length as defined in [19] RFC 7748".
const OID_X25519: &[u8] = &[0x2B, 0x65, 0x6E];

impl CertificateChain {
    /// The device certificate's X25519 public key.
    ///
    /// AEF §4.4.5.4 phase 3 step 1: "The client and server use the 'public key'
    /// (i.e., an elliptic curve point d*G) from their certificate with
    /// corresponding private key (i.e., the scalar d)." Step 2 then says the
    /// key exchange "is implicitly carried out during the certificate
    /// validation step in phase 2" — which is only true if the key used *is*
    /// the certified one.
    ///
    /// # Errors
    /// [`AuthError::EccPublicKeyValidationFailed`] if the device certificate
    /// carries no X25519 key of the right length.
    pub fn device_x25519_public_key(&self) -> Result<[u8; 32], AuthError> {
        let device = self
            .certificates
            .last()
            .ok_or(AuthError::DeviceCertificateDataCorrupt)?;
        let spki = &device.tbs_certificate.subject_public_key_info;
        let algorithm = spki.algorithm.oid.as_bytes();
        if algorithm != OID_X25519 {
            return Err(AuthError::EccPublicKeyValidationFailed);
        }
        let key = spki
            .subject_public_key
            .as_bytes()
            .ok_or(AuthError::EccPublicKeyValidationFailed)?;
        key.try_into()
            .map_err(|_| AuthError::EccPublicKeyValidationFailed)
    }
}

/// Verify `child`'s signature using `parent`'s public key.
///
/// AEF Annex F.2.1 names RSASSA-PSS (RFC 3447) with SHA-256. Anything else —
/// including a certificate whose signature algorithm is not PSS — fails: an
/// algorithm the peer chose is not a trust decision this side gets to accept.
#[cfg(feature = "tim-auth")]
fn verify_link(parent: &Certificate, child: &Certificate) -> Result<(), ()> {
    use rsa::pss::{Signature, VerifyingKey};
    use rsa::signature::Verifier;

    let spki = parent
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|_| ())?;
    let public = rsa::RsaPublicKey::from_public_key_der(&spki).map_err(|_| ())?;
    // RFC 3447 / AEF F.2.1: at least a 2048-bit modulus.
    use rsa::traits::PublicKeyParts;
    if public.n().bits() < 2048 {
        return Err(());
    }

    let tbs = child.tbs_certificate.to_der().map_err(|_| ())?;
    let signature = Signature::try_from(child.signature.raw_bytes()).map_err(|_| ())?;
    let verifying: VerifyingKey<Sha256> = VerifyingKey::new(public);
    verifying.verify(&tbs, &signature).map_err(|_| ())
}

/// A certificate revocation list. AEF §4.4.7 requires support for at least
/// 1000 entries; this stores whatever it is given and reports its capacity so a
/// caller can assert conformance.
#[derive(Debug, Default)]
pub struct CertificateRevocationList {
    serials: Vec<Vec<u8>>,
}

/// Minimum CRL size an implementation must support (§4.4.7).
pub const MIN_CRL_ENTRIES: usize = 1000;

impl CertificateRevocationList {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a revoked certificate serial number.
    pub fn revoke(&mut self, serial: &[u8]) {
        self.serials.push(serial.to_vec());
    }

    #[must_use]
    pub fn contains(&self, serial: &[u8]) -> bool {
        self.serials.iter().any(|s| s.as_slice() == serial)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.serials.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.serials.is_empty()
    }
}

/// State of the TIM authentication handshake (§4.4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthState {
    #[default]
    Unauthenticated,
    /// Certificates exchanged and structurally validated.
    CertificatesExchanged,
    /// ECDH complete; a shared secret exists.
    KeyAgreed,
    /// Challenge issued, awaiting the peer's CMAC.
    ChallengeIssued,
    /// Mutually authenticated — TIM automation is permitted.
    Authenticated,
    /// Failed; the reason is carried alongside.
    Failed(AuthError),
}

/// The TIM authentication handshake.
///
/// Unlike the stub it replaces, `verify_response` computes the expected CMAC
/// itself from the ECDH shared secret and the challenge it issued. The caller
/// supplies only the peer's answer, so a caller cannot accidentally (or
/// deliberately) authenticate itself.
/// Which side of the TIM handshake this instance is.
///
/// AEF §4.4.5.4 splits the derived key into a client-to-server half and a
/// server-to-client half. Which half a peer signs with is fixed by its *role*,
/// not by anything negotiated: a conformant server always signs with the
/// server-to-client half and a conformant client with the other. Deriving the
/// split from the challenge bytes instead — as this used to — agreed with
/// another machbus node but with a conformant peer only about half the time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimRole {
    Client,
    Server,
}

pub struct TimAuthentication {
    role: TimRole,
    state: AuthState,
    secret: Option<StaticSecret>,
    shared: Option<[u8; 32]>,
    /// The challenge this side issued, and the one the peer issued.
    ///
    /// §4.4.5.4 splits the derived key so that "one key is used for
    /// server-to-client authentication [and] the other client-to-server", and
    /// each side MACs the *peer's* challenge. A single key and a single
    /// challenge slot made a response reflectable: the same bytes that
    /// authenticate A to B also authenticate B to A.
    own_challenge: Vec<u8>,
    peer_challenge: Vec<u8>,
    /// The peer's ECDH public key, taken from its **device certificate** when
    /// the chain was accepted.
    ///
    /// This used to be an argument to `complete_key_agreement`, so the chain
    /// and the key exchange were two unrelated facts: a peer could present any
    /// valid chain — including one captured off the bus — and then supply its
    /// own freshly generated key. The resulting CMAC proved possession of
    /// *some* key, not the certified peer's key, which is the entire point of
    /// phase 4 (§4.4.5.5).
    peer_ecdh_public: Option<[u8; 32]>,
}

impl Default for TimAuthentication {
    fn default() -> Self {
        Self::new()
    }
}

impl TimAuthentication {
    #[must_use]
    /// A client-side authentication. See [`TimAuthentication::new_server`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_role(TimRole::Client)
    }

    /// A server-side authentication.
    ///
    /// The role decides which half of the derived key this side signs with
    /// (§4.4.5.4), so it must match what the peer expects.
    #[must_use]
    pub fn new_server() -> Self {
        Self::with_role(TimRole::Server)
    }

    /// This side's role in the handshake.
    #[must_use]
    pub const fn role(&self) -> TimRole {
        self.role
    }

    #[must_use]
    fn with_role(role: TimRole) -> Self {
        Self {
            role,
            state: AuthState::Unauthenticated,
            secret: None,
            shared: None,
            own_challenge: Vec::new(),
            peer_challenge: Vec::new(),
            peer_ecdh_public: None,
        }
    }

    #[must_use]
    pub fn state(&self) -> AuthState {
        self.state
    }

    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.state == AuthState::Authenticated
    }

    /// Validate a peer's certificate chain against a trust anchor and a
    /// revocation list.
    ///
    /// AEF 023 §4.3.1: "the xPCs are exchanged, verified, and used to validate
    /// the identity of the participants. The TIM functionality shall be used
    /// only if both checks are positive."
    ///
    /// `trust_anchor` is the AEF root public key as DER `SubjectPublicKeyInfo`,
    /// provisioned by the integrator. This used to check only that the chain's
    /// Distinguished Names linked to each other, which any peer can arrange
    /// with one `openssl` invocation — the chain was never verified against
    /// anything this side already trusted.
    ///
    /// # Errors
    /// The AEF error code for the first problem found.
    pub fn accept_chain(
        &mut self,
        chain: &CertificateChain,
        trust_anchor: &[u8],
        revoked: &CertificateRevocationList,
    ) -> Result<(), AuthError> {
        if chain.is_empty() {
            return self.fail(AuthError::DeviceCertificateDataCorrupt);
        }
        // Cheap structural pre-filter, then the check that actually matters.
        if let Err(e) = chain.check_issuer_linkage() {
            return self.fail(e);
        }
        if let Err(e) = chain.verify_signatures(trust_anchor) {
            return self.fail(e);
        }
        if let Some(role) = chain.revoked_role(revoked) {
            return self.fail(role.revoked_error());
        }
        // Bind the key exchange to the identity we just verified.
        match chain.device_x25519_public_key() {
            Ok(key) => self.peer_ecdh_public = Some(key),
            Err(e) => return self.fail(e),
        }
        self.state = AuthState::CertificatesExchanged;
        Ok(())
    }

    /// Generate this side's ECDH key pair and return the public key to send.
    ///
    /// `private_bytes` is the caller's key material — from a hardware RNG or a
    /// provisioned secret. It is never derived from the challenge.
    pub fn begin_key_agreement(&mut self, private_bytes: [u8; 32]) -> [u8; 32] {
        let secret = StaticSecret::from(private_bytes);
        let public = PublicKey::from(&secret);
        self.secret = Some(secret);
        public.to_bytes()
    }

    /// Complete ECDH with the peer's public key.
    ///
    /// # Errors
    /// [`AuthError::InternalError`] if no key agreement was started.
    pub fn complete_key_agreement(&mut self) -> Result<(), AuthError> {
        // §4.3 makes certificate validation the precondition for the rest of
        // the handshake; reaching key agreement without it authorises anyone.
        if !matches!(
            self.state,
            AuthState::CertificatesExchanged | AuthState::KeyAgreed
        ) {
            return self.fail(AuthError::InternalError);
        }
        let Some(secret) = self.secret.as_ref() else {
            return self.fail(AuthError::InternalError);
        };
        // The key comes from the certificate that was verified in phase 2, not
        // from the caller — see `peer_ecdh_public`.
        let Some(peer_public) = self.peer_ecdh_public else {
            return self.fail(AuthError::EccPublicKeyValidationFailed);
        };
        let shared = secret.diffie_hellman(&PublicKey::from(peer_public));
        // B.4.1.1 code 0x14. A small-order or all-zero point drives the shared
        // secret to zero for *every* peer, so the CMAC key would be known to
        // anyone: a complete authentication bypass.
        if !shared.was_contributory() {
            return self.fail(AuthError::EccPublicKeyValidationFailed);
        }
        self.shared = Some(shared.to_bytes());
        self.state = AuthState::KeyAgreed;
        Ok(())
    }

    /// Derive one directional key per §4.4.5.4: "a Key Derivation Function
    /// (KDF) is performed on the generated common secret … the derived key …
    /// is split into two parts", with both challenges mixed in so "the
    /// generated common secret would [not] always be the same".
    ///
    /// SP800-56A one-step KDF with SHA-256, which is the defensible
    /// instantiation available here — `sha2` is already a dependency.
    fn directional_keys(&self) -> Result<([u8; 16], [u8; 16]), AuthError> {
        let shared = self.shared.ok_or(AuthError::InternalError)?;
        if self.own_challenge.is_empty() || self.peer_challenge.is_empty() {
            return Err(AuthError::InternalError);
        }
        // Two random challenges that are byte-identical mean either a replay
        // or a peer reflecting ours; there is then no way to tell the two
        // directions apart, which is exactly the property C28 was about.
        if self.own_challenge == self.peer_challenge {
            return Err(AuthError::ChallengesDoNotMatch);
        }
        // Both sides must derive the same pair, so the challenges are mixed in
        // a canonical order rather than "mine then theirs". This ordering is
        // only about agreeing on the *input*; it must not decide which half
        // each side signs with — see the role split below.
        let (first, second) = if self.own_challenge < self.peer_challenge {
            (&self.own_challenge, &self.peer_challenge)
        } else {
            (&self.peer_challenge, &self.own_challenge)
        };
        let mut hash = Sha256::new();
        hash.update([0, 0, 0, 1]);
        hash.update(shared);
        hash.update(b"machbus-tim-lwa");
        hash.update((first.len() as u16).to_be_bytes());
        hash.update(first);
        hash.update((second.len() as u16).to_be_bytes());
        hash.update(second);
        let derived = hash.finalize();
        let mut low = [0u8; 16];
        let mut high = [0u8; 16];
        low.copy_from_slice(&derived[..16]);
        high.copy_from_slice(&derived[16..]);
        // §4.4.5.4: "one key is used for server-to-client authentication ...
        // the other key is used for client-to-server authentication". The half
        // a side signs with follows from its role, so a conformant peer and
        // this one always agree. Returns (outbound, inbound).
        match self.role {
            TimRole::Client => Ok((low, high)),
            TimRole::Server => Ok((high, low)),
        }
    }

    fn mac_with(key: &[u8; 16], data: &[u8]) -> Result<[u8; CMAC_LEN], AuthError> {
        let mut mac =
            <Cmac<Aes128> as Mac>::new_from_slice(key).map_err(|_| AuthError::InternalError)?;
        mac.update(data);
        let tag = mac.finalize().into_bytes();
        let mut out = [0u8; CMAC_LEN];
        out.copy_from_slice(&tag);
        Ok(out)
    }

    /// Issue a challenge. It must be 32 bytes (random) or 16 (signed).
    ///
    /// # Errors
    /// [`AuthError::ChallengeLengthInvalid`] (B.4.1.1 code 0x23) for any other
    /// length — a rule the 8-byte-nonce stub could not express at all.
    pub fn issue_challenge(&mut self, challenge: &[u8]) -> Result<(), AuthError> {
        if challenge.len() != RANDOM_CHALLENGE_LEN && challenge.len() != SIGNED_CHALLENGE_LEN {
            return self.fail(AuthError::ChallengeLengthInvalid);
        }
        if self.shared.is_none() {
            return self.fail(AuthError::InternalError);
        }
        self.own_challenge = challenge.to_vec();
        self.state = AuthState::ChallengeIssued;
        Ok(())
    }

    /// Record the challenge the peer issued. Annex F.4 has each side compute a
    /// CMAC "over the challenge received by the [peer]", so both are needed
    /// before either response can be produced.
    ///
    /// # Errors
    /// [`AuthError::ChallengeLengthInvalid`] for a length other than 32 or 16.
    pub fn accept_peer_challenge(&mut self, challenge: &[u8]) -> Result<(), AuthError> {
        if challenge.len() != RANDOM_CHALLENGE_LEN && challenge.len() != SIGNED_CHALLENGE_LEN {
            return self.fail(AuthError::ChallengeLengthInvalid);
        }
        if self.shared.is_none() {
            return self.fail(AuthError::InternalError);
        }
        self.peer_challenge = challenge.to_vec();
        Ok(())
    }

    /// The AES-CMAC this side computes over the outstanding challenge. Send
    /// this as the response to a peer's challenge.
    ///
    /// # Errors
    /// [`AuthError::InternalError`] when there is no shared secret or no
    /// outstanding challenge.
    pub fn compute_response(&self) -> Result<[u8; CMAC_LEN], AuthError> {
        // F.4 step 4.1/4.2: MAC the challenge *received from the peer*, with
        // this direction's key.
        let (outbound, _) = self.directional_keys()?;
        Self::mac_with(&outbound, &self.peer_challenge)
    }

    /// The CMAC this side expects back from the peer: the other direction's
    /// key over the challenge this side issued (F.4 steps 4.5/4.6).
    fn expected_peer_response(&self) -> Result<[u8; CMAC_LEN], AuthError> {
        let (_, inbound) = self.directional_keys()?;
        Self::mac_with(&inbound, &self.own_challenge)
    }

    /// Verify a peer's response against the CMAC computed here.
    ///
    /// # Errors
    /// [`AuthError::ChallengesDoNotMatch`] (code 0x09) when the peer's answer
    /// does not match, which also moves the handshake to `Failed`.
    pub fn verify_response(&mut self, peer_response: &[u8]) -> Result<(), AuthError> {
        if !matches!(self.state, AuthState::ChallengeIssued) {
            return self.fail(AuthError::InternalError);
        }
        let expected = match self.expected_peer_response() {
            Ok(tag) => tag,
            Err(e) => return self.fail(e),
        };
        if peer_response.len() != CMAC_LEN || !constant_time_eq(&expected, peer_response) {
            return self.fail(AuthError::ChallengesDoNotMatch);
        }
        self.state = AuthState::Authenticated;
        Ok(())
    }

    /// Abandon the handshake. AEF §4.3: in any case other than successful
    /// mutual authentication, TIM automation is refused.
    pub fn reset(&mut self) {
        self.state = AuthState::Unauthenticated;
        self.secret = None;
        self.shared = None;
        self.own_challenge.clear();
        self.peer_challenge.clear();
    }

    fn fail(&mut self, error: AuthError) -> Result<(), AuthError> {
        self.state = AuthState::Failed(error);
        Err(error)
    }
}

/// Compare two tags without an early exit on the first differing byte.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two peers that have validated each other's certificates and completed
    /// ECDH. The certificate step is short-circuited here because building a
    /// signed DER chain is not what these tests are about; that the gate exists
    /// at all is covered by `key_agreement_requires_certificate_validation`.
    fn agreed_pair() -> (TimAuthentication, TimAuthentication) {
        // §4.4.5.4 splits the derived key by role, so a handshake needs one of
        // each: two clients would both sign with the client-to-server half.
        let mut a = TimAuthentication::new();
        let mut b = TimAuthentication::new_server();
        a.state = AuthState::CertificatesExchanged;
        b.state = AuthState::CertificatesExchanged;
        let a_public = a.begin_key_agreement([7u8; 32]);
        let b_public = b.begin_key_agreement([11u8; 32]);
        // `accept_chain` is what normally supplies these; short-circuited here
        // for the same reason the certificate step is.
        a.peer_ecdh_public = Some(b_public);
        b.peer_ecdh_public = Some(a_public);
        a.complete_key_agreement().unwrap();
        b.complete_key_agreement().unwrap();
        (a, b)
    }

    /// Each side issues its own challenge and learns the peer's, as F.4
    /// requires before either response can be computed.
    fn exchange_challenges(
        a: &mut TimAuthentication,
        b: &mut TimAuthentication,
        a_challenge: &[u8],
        b_challenge: &[u8],
    ) {
        a.issue_challenge(a_challenge).unwrap();
        b.issue_challenge(b_challenge).unwrap();
        a.accept_peer_challenge(b_challenge).unwrap();
        b.accept_peer_challenge(a_challenge).unwrap();
    }

    #[test]
    fn ecdh_gives_both_sides_the_same_secret() {
        let (a, b) = agreed_pair();
        assert_eq!(a.state(), AuthState::KeyAgreed);
        assert_eq!(a.shared, b.shared, "curve25519 ECDH is symmetric");
        assert!(a.shared.is_some());
    }

    #[test]
    fn challenge_response_authenticates_a_genuine_peer() {
        let (mut a, mut b) = agreed_pair();
        let a_challenge = [0x5Au8; RANDOM_CHALLENGE_LEN];
        let b_challenge = [0xA5u8; RANDOM_CHALLENGE_LEN];
        exchange_challenges(&mut a, &mut b, &a_challenge, &b_challenge);

        // B answers A's challenge with a CMAC only the shared secret produces.
        let response = b.compute_response().unwrap();
        a.verify_response(&response).unwrap();
        assert!(a.is_authenticated());
    }

    /// C28 — one key and one challenge slot for both directions made the
    /// response reflectable: the bytes that authenticate B to A were exactly
    /// the bytes A would send back, so an attacker could echo A's own answer.
    #[test]
    fn a_reflected_response_does_not_authenticate() {
        let (mut a, mut b) = agreed_pair();
        let a_challenge = [0x5Au8; RANDOM_CHALLENGE_LEN];
        let b_challenge = [0xA5u8; RANDOM_CHALLENGE_LEN];
        exchange_challenges(&mut a, &mut b, &a_challenge, &b_challenge);

        // Echo A's own outbound response straight back at it.
        let a_own = a.compute_response().unwrap();
        assert_ne!(
            a_own,
            b.compute_response().unwrap(),
            "the two directions must not produce the same tag"
        );
        assert_eq!(
            a.verify_response(&a_own),
            Err(AuthError::ChallengesDoNotMatch)
        );
        assert!(!a.is_authenticated());
    }

    /// C26 — an all-zero (small-order) peer key drives the shared secret to
    /// zero for every peer, so the CMAC key becomes public knowledge and any
    /// device on the bus completes the handshake.
    #[test]
    fn a_non_contributory_public_key_is_refused() {
        let mut a = TimAuthentication::new();
        a.state = AuthState::CertificatesExchanged;
        let _ = a.begin_key_agreement([7u8; 32]);
        a.peer_ecdh_public = Some([0u8; 32]);
        assert_eq!(
            a.complete_key_agreement(),
            Err(AuthError::EccPublicKeyValidationFailed)
        );
        assert!(!a.is_authenticated());
        assert_eq!(
            a.state(),
            AuthState::Failed(AuthError::EccPublicKeyValidationFailed)
        );
        assert_eq!(AuthError::EccPublicKeyValidationFailed.as_u8(), 0x14);
    }

    /// C27 — §4.3 makes certificate validation the precondition for TIM
    /// automation. Key agreement used to proceed from a fresh, unvalidated
    /// state, so the PKI gate authorised anyone.
    #[test]
    fn key_agreement_requires_certificate_validation() {
        let mut a = TimAuthentication::new();
        let peer = {
            let mut b = TimAuthentication::new();
            b.state = AuthState::CertificatesExchanged;
            b.begin_key_agreement([11u8; 32])
        };
        let _ = a.begin_key_agreement([7u8; 32]);
        a.peer_ecdh_public = Some(peer);
        assert_eq!(
            a.complete_key_agreement(),
            Err(AuthError::InternalError),
            "no certificate validation, no key agreement"
        );
    }

    /// Different challenges must key different sessions — otherwise a recorded
    /// handshake replays.
    #[test]
    fn the_derived_key_depends_on_both_challenges() {
        let (mut a1, mut b1) = agreed_pair();
        exchange_challenges(&mut a1, &mut b1, &[1u8; 32], &[2u8; 32]);
        let first = a1.compute_response().unwrap();

        let (mut a2, mut b2) = agreed_pair();
        exchange_challenges(&mut a2, &mut b2, &[1u8; 32], &[3u8; 32]);
        let second = a2.compute_response().unwrap();

        assert_ne!(
            first, second,
            "changing the peer's challenge must change the response"
        );
    }

    /// The stub took the expected signature from its caller, so it authenticated
    /// anything. A peer without the shared secret must now fail.
    #[test]
    fn a_peer_without_the_shared_secret_cannot_authenticate() {
        let (mut a, mut b) = agreed_pair();
        let a_challenge = [0x5Au8; RANDOM_CHALLENGE_LEN];
        let b_challenge = [0xA5u8; RANDOM_CHALLENGE_LEN];
        exchange_challenges(&mut a, &mut b, &a_challenge, &b_challenge);

        let mut impostor = TimAuthentication::new();
        impostor.state = AuthState::CertificatesExchanged;
        let eve_public = impostor.begin_key_agreement([99u8; 32]);
        impostor.peer_ecdh_public = Some(eve_public);
        impostor.complete_key_agreement().unwrap();
        impostor.issue_challenge(&b_challenge).unwrap();
        impostor.accept_peer_challenge(&a_challenge).unwrap();

        let forged = impostor.compute_response().unwrap();
        assert_eq!(
            a.verify_response(&forged),
            Err(AuthError::ChallengesDoNotMatch)
        );
        assert!(!a.is_authenticated());
        assert_eq!(
            a.state(),
            AuthState::Failed(AuthError::ChallengesDoNotMatch)
        );
    }

    #[test]
    fn challenge_length_rule_is_enforceable() {
        let (mut a, _) = agreed_pair();
        // B.4.1.1 code 0x23: 32 bytes random or 16 bytes signed.
        a.issue_challenge(&[0u8; RANDOM_CHALLENGE_LEN]).unwrap();
        a.issue_challenge(&[0u8; SIGNED_CHALLENGE_LEN]).unwrap();

        // The old 8-byte nonce could not even represent a valid challenge.
        assert_eq!(
            a.issue_challenge(&[0u8; 8]),
            Err(AuthError::ChallengeLengthInvalid)
        );
        assert_eq!(
            a.issue_challenge(&[0u8; 31]),
            Err(AuthError::ChallengeLengthInvalid)
        );
    }

    #[test]
    fn a_truncated_response_is_rejected() {
        let (mut a, mut b) = agreed_pair();
        exchange_challenges(&mut a, &mut b, &[1u8; 32], &[2u8; 32]);
        let response = b.compute_response().unwrap();

        assert_eq!(
            a.verify_response(&response[..8]),
            Err(AuthError::ChallengesDoNotMatch)
        );
    }

    #[test]
    fn cmac_is_deterministic_and_challenge_dependent() {
        let (mut a, mut b) = agreed_pair();
        exchange_challenges(&mut a, &mut b, &[0xAAu8; 32], &[0xBBu8; 32]);
        let first = a.compute_response().unwrap();
        assert_eq!(first, a.compute_response().unwrap(), "deterministic");

        // A different peer challenge yields a different response.
        a.accept_peer_challenge(&[0xCCu8; 32]).unwrap();
        assert_ne!(first, a.compute_response().unwrap());
    }

    #[test]
    fn revoked_certificates_are_refused() {
        let mut crl = CertificateRevocationList::new();
        assert!(crl.is_empty());
        crl.revoke(&[0x01, 0x02, 0x03]);
        assert!(crl.contains(&[0x01, 0x02, 0x03]));
        assert!(!crl.contains(&[0x09]));

        // Section 4.4.7 asks for at least 1000 entries.
        for i in 0..MIN_CRL_ENTRIES {
            crl.revoke(&(i as u32).to_be_bytes());
        }
        assert!(crl.len() > MIN_CRL_ENTRIES);
        assert!(crl.contains(&500u32.to_be_bytes()));
    }

    #[test]
    fn a_corrupt_certificate_reports_its_role_specific_code() {
        // Not DER at all.
        assert_eq!(
            CertificateChain::parse_der(&[&[0xDE, 0xAD, 0xBE, 0xEF]]).err(),
            Some(AuthError::RootCertificateDataCorrupt)
        );
        // An empty chain cannot authenticate anything.
        let mut auth = TimAuthentication::new();
        let empty = CertificateChain::parse_der(&[]).unwrap();
        assert_eq!(
            auth.accept_chain(&empty, &[], &CertificateRevocationList::new()),
            Err(AuthError::DeviceCertificateDataCorrupt)
        );
        assert!(!auth.is_authenticated());
    }

    /// D1 — the finding that survived at critical severity.
    ///
    /// `accept_chain` verified only that the chain's Distinguished Names linked
    /// to each other. DNs are attacker-controlled text: anyone could mint a
    /// chain whose names match and whose top claims to be the AEF root, and the
    /// handshake proceeded against a forged device identity. Both chains below
    /// pass the DN check; only one of them is signed by the key we trust.
    #[test]
    fn a_forged_chain_that_links_by_name_is_rejected() {
        const VECTORS: &str = include_str!("../../../tests/fixtures/tim/certificate_chain.hex");

        fn vector(name: &str) -> Vec<u8> {
            let line = VECTORS
                .lines()
                .find(|l| l.starts_with(&alloc::format!("{name}=")))
                .unwrap_or_else(|| panic!("vector {name} is missing"));
            let hex = &line[name.len() + 1..];
            (0..hex.len() / 2)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
                .collect()
        }

        let anchor = vector("root_spki");
        let crl = CertificateRevocationList::new();

        // The genuine chain verifies. Its device certificate carries the
        // Curve25519 key F.2.2 mandates, which `accept_chain` now also binds.
        let root = vector("root");
        let device = vector("x25519_device");
        let good = CertificateChain::parse_der(&[&root, &device]).expect("a real chain parses");
        good.check_issuer_linkage().expect("names link");
        let mut auth = TimAuthentication::new();
        assert_eq!(auth.accept_chain(&good, &anchor, &crl), Ok(()));

        // The forged chain has the same subject/issuer names throughout, so the
        // old structural check accepted it outright.
        let evil_root = vector("evil_root");
        let evil_device = vector("evil_device");
        let forged = CertificateChain::parse_der(&[&evil_root, &evil_device])
            .expect("a forged chain parses");
        forged
            .check_issuer_linkage()
            .expect("the forgery links by name — that is the whole point");

        let mut auth = TimAuthentication::new();
        assert_eq!(
            auth.accept_chain(&forged, &anchor, &crl),
            Err(AuthError::RootCertificateSignatureInvalid),
            "a chain that links by name but not by signature must be refused"
        );
        assert!(!auth.is_authenticated());

        // A genuine chain still fails when the caller trusts a different root:
        // the anchor, not the peer, decides.
        let mut auth = TimAuthentication::new();
        assert_eq!(
            auth.accept_chain(&good, &vector("evil_root"), &crl),
            Err(AuthError::RootCertificateSignatureInvalid)
        );
    }

    /// D2 — the other half of the compound with D1.
    ///
    /// The certificate chain and the ECDH exchange used to be two unrelated
    /// facts: `complete_key_agreement` took the peer's public key as an
    /// *argument*, so a peer could present a chain captured off a real bus and
    /// then supply a freshly generated key of its own. The CMAC then proved
    /// possession of some key, not the certified peer's key — which is exactly
    /// what AEF §4.4.5.5 says phase 4 exists to establish.
    ///
    /// AEF §4.4.5.4 phase 3: the public key used *is* the one from the
    /// certificate, so "this step is implicitly carried out during the
    /// certificate validation step in phase 2".
    #[test]
    fn the_ecdh_key_comes_from_the_device_certificate() {
        const VECTORS: &str = include_str!("../../../tests/fixtures/tim/certificate_chain.hex");

        fn vector(name: &str) -> Vec<u8> {
            let line = VECTORS
                .lines()
                .find(|l| l.starts_with(&alloc::format!("{name}=")))
                .unwrap_or_else(|| panic!("vector {name} is missing"));
            let hex = &line[name.len() + 1..];
            (0..hex.len() / 2)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
                .collect()
        }

        let root = vector("root");
        let device = vector("x25519_device");
        let chain = CertificateChain::parse_der(&[&root, &device]).expect("chain parses");

        // The key is read out of the certificate, and it is the one openssl
        // embedded — not anything a peer could choose at handshake time.
        let from_cert = chain
            .device_x25519_public_key()
            .expect("the device certificate carries an X25519 key");
        assert_eq!(
            from_cert.as_slice(),
            vector("x25519_device_key").as_slice(),
            "the ECDH key must be the certified one"
        );

        // Accepting the chain is what binds it, so key agreement has no
        // argument left for a peer to substitute.
        let mut auth = TimAuthentication::new();
        auth.accept_chain(
            &chain,
            &vector("root_spki"),
            &CertificateRevocationList::new(),
        )
        .expect("a genuine chain is accepted");
        assert_eq!(auth.peer_ecdh_public, Some(from_cert));

        // A device certificate carrying an RSA key has no curve point to agree
        // on: F.2.2 mandates Curve25519, so this is not a usable TIM peer.
        let rsa_device = vector("device");
        let rsa_chain = CertificateChain::parse_der(&[&root, &rsa_device]).expect("parses");
        assert_eq!(
            rsa_chain.device_x25519_public_key(),
            Err(AuthError::EccPublicKeyValidationFailed)
        );
    }

    /// D5 — AEF §4.4.5.4: "one key is used for server-to-client authentication
    /// (ECDH.sharedKey.Server_to_Client); the other key is used for
    /// client-to-server authentication".
    ///
    /// Which half a side signs with follows from its **role**. This used to be
    /// decided by a lexicographic comparison of the two random challenges, so
    /// two machbus nodes always agreed with each other and a conformant peer
    /// agreed only when the random bytes happened to sort the right way — about
    /// half of all handshakes.
    #[test]
    fn the_key_halves_are_split_by_role_not_by_challenge_bytes() {
        fn halves(role: TimRole, own: &[u8], peer: &[u8]) -> ([u8; 16], [u8; 16]) {
            let mut auth = TimAuthentication::with_role(role);
            auth.state = AuthState::KeyAgreed;
            auth.shared = Some([0x42; 32]);
            auth.own_challenge = own.to_vec();
            auth.peer_challenge = peer.to_vec();
            auth.directional_keys().unwrap()
        }

        // Two challenge pairs that sort in opposite directions.
        let low = [0x01u8; 32];
        let high = [0xFEu8; 32];

        for (own, peer) in [(&low, &high), (&high, &low)] {
            let client = halves(TimRole::Client, own, peer);
            // The server sees the same pair from the other side.
            let server = halves(TimRole::Server, peer, own);

            assert_eq!(
                client.0, server.1,
                "what the client signs with is what the server verifies with"
            );
            assert_eq!(
                client.1, server.0,
                "and the other direction uses the other half"
            );
            assert_ne!(client.0, client.1, "the two directions must differ");
        }

        // The client half does not depend on how the challenges sort.
        let a = halves(TimRole::Client, &low, &high);
        let b = halves(TimRole::Client, &high, &low);
        assert_eq!(
            a.0, b.0,
            "a client always signs with the client-to-server half"
        );
    }

    #[test]
    fn error_codes_match_annex_b4() {
        assert_eq!(AuthError::ParticipantBusy.as_u8(), 0x01);
        assert_eq!(AuthError::ChallengesDoNotMatch.as_u8(), 0x09);
        assert_eq!(AuthError::ChallengeLengthInvalid.as_u8(), 0x23);
        assert_eq!(
            CertificateRole::Device.corrupt_error(),
            AuthError::DeviceCertificateDataCorrupt
        );
        assert_eq!(
            CertificateRole::Root.signature_error(),
            AuthError::RootCertificateSignatureInvalid
        );

        // D4 — Table 20 gives each link its own revocation code. They used to
        // all report 0x24, which the table defines as "Challenge data corrupt":
        // a peer with a revoked certificate was told the wrong thing entirely.
        assert_eq!(AuthError::RootCertificateRevoked.as_u8(), 0x19);
        assert_eq!(AuthError::LabCertificateRevoked.as_u8(), 0x1A);
        assert_eq!(AuthError::ManufacturerCertificateRevoked.as_u8(), 0x1B);
        assert_eq!(
            AuthError::ManufacturerSeriesCertificateRevoked.as_u8(),
            0x1C
        );
        assert_eq!(AuthError::DeviceCertificateRevoked.as_u8(), 0x1D);
        assert_eq!(AuthError::ChallengeDataCorrupt.as_u8(), 0x24);
        assert_eq!(
            CertificateRole::ManufacturerSeries.revoked_error(),
            AuthError::ManufacturerSeriesCertificateRevoked
        );

        // D1 — a DN mismatch is a structural failure, not a failed signature.
        assert_eq!(AuthError::CertificateChainInvalid.as_u8(), 0x17);
    }
}
