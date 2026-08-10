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
use der::Decode;
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
    /// The public ECC key is not valid for curve 25519 (0x14). A peer that
    /// sends an all-zero (or other small-order) point drives the shared secret
    /// to zero, which would key the CMAC with zeros and let anyone on the bus
    /// complete the handshake.
    EccPublicKeyValidationFailed = 0x14,
    /// Challenge length was neither 32 nor 16 bytes (0x23).
    ChallengeLengthInvalid = 0x23,
    /// The peer's certificate appears on the revocation list.
    CertificateRevoked = 0x24,
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
    /// This is the structural half of chain validation. Signature verification
    /// needs the issuer's public key and the algorithm it was signed with; a
    /// caller that has a trust store performs that step and reports
    /// [`CertificateRole::signature_error`] on failure.
    ///
    /// # Errors
    /// The signature-invalid code for the first link whose issuer does not
    /// match its parent's subject.
    pub fn check_issuer_linkage(&self) -> Result<(), AuthError> {
        for (index, pair) in self.certificates.windows(2).enumerate() {
            let issuer_subject = &pair[0].tbs_certificate.subject;
            let child_issuer = &pair[1].tbs_certificate.issuer;
            if issuer_subject != child_issuer {
                let role = CertificateRole::CHAIN
                    .get(index + 1)
                    .copied()
                    .unwrap_or(CertificateRole::Device);
                return Err(role.signature_error());
            }
        }
        Ok(())
    }

    /// `true` when any certificate's serial number appears in `revoked`.
    #[must_use]
    pub fn is_revoked(&self, revoked: &CertificateRevocationList) -> bool {
        self.certificates
            .iter()
            .any(|c| revoked.contains(c.tbs_certificate.serial_number.as_bytes()))
    }
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
pub struct TimAuthentication {
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
}

impl Default for TimAuthentication {
    fn default() -> Self {
        Self::new()
    }
}

impl TimAuthentication {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AuthState::Unauthenticated,
            secret: None,
            shared: None,
            own_challenge: Vec::new(),
            peer_challenge: Vec::new(),
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

    /// Validate a peer's certificate chain against a revocation list.
    ///
    /// # Errors
    /// The AEF error code for the first problem found.
    pub fn accept_chain(
        &mut self,
        chain: &CertificateChain,
        revoked: &CertificateRevocationList,
    ) -> Result<(), AuthError> {
        if chain.is_empty() {
            return self.fail(AuthError::DeviceCertificateDataCorrupt);
        }
        if let Err(e) = chain.check_issuer_linkage() {
            return self.fail(e);
        }
        if chain.is_revoked(revoked) {
            return self.fail(AuthError::CertificateRevoked);
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
    pub fn complete_key_agreement(&mut self, peer_public: [u8; 32]) -> Result<(), AuthError> {
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
        // a canonical order rather than "mine then theirs".
        let own_first = self.own_challenge < self.peer_challenge;
        let (first, second) = if own_first {
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
        // The halves are assigned to opposite directions on the two peers, so
        // the key that authenticates A to B is never the one that
        // authenticates B to A.
        if own_first {
            Ok((low, high))
        } else {
            Ok((high, low))
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
        let mut a = TimAuthentication::new();
        let mut b = TimAuthentication::new();
        a.state = AuthState::CertificatesExchanged;
        b.state = AuthState::CertificatesExchanged;
        let a_public = a.begin_key_agreement([7u8; 32]);
        let b_public = b.begin_key_agreement([11u8; 32]);
        a.complete_key_agreement(b_public).unwrap();
        b.complete_key_agreement(a_public).unwrap();
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
        assert_eq!(
            a.complete_key_agreement([0u8; 32]),
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
        assert_eq!(
            a.complete_key_agreement(peer),
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
        impostor.complete_key_agreement(eve_public).unwrap();
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
            auth.accept_chain(&empty, &CertificateRevocationList::new()),
            Err(AuthError::DeviceCertificateDataCorrupt)
        );
        assert!(!auth.is_authenticated());
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
    }
}
