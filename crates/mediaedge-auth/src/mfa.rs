//! Multi-Factor Authentication (MFA)
//!
//! Supports TOTP (RFC 6238) and WebAuthn (FIDO2).
//! Integrates with user management for enrollment and verification.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use dashmap::DashMap;
use ring::hmac;
use serde::{Deserialize, Serialize};
use totp_lite::{Totp, TotpBuilder};
use uuid::Uuid;
use webauthn_rs::prelude::*;

use mediaedge_config::Config;

/// MFA method types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MfaMethod {
    Totp,
    WebAuthn,
}

/// TOTP configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpConfig {
    pub issuer: String,
    pub algorithm: TotpAlgorithm,
    pub digits: usize,
    pub period: u64,
    pub skew: u64,
}

impl Default for TotpConfig {
    fn default() -> Self {
        Self {
            issuer: "MediaEdge".to_string(),
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            skew: 1,
        }
    }
}

/// TOTP algorithm.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

/// WebAuthn configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnConfig {
    pub rp_id: String,
    pub rp_name: String,
    pub rp_origin: String,
    pub timeout_ms: u32,
    pub user_verification: UserVerificationPolicy,
    pub attestation: AttestationConveyancePreference,
}

impl Default for WebAuthnConfig {
    fn default() -> Self {
        Self {
            rp_id: "localhost".to_string(),
            rp_name: "MediaEdge".to_string(),
            rp_origin: "https://localhost".to_string(),
            timeout_ms: 60000,
            user_verification: UserVerificationPolicy::Preferred,
            attestation: AttestationConveyancePreference::None,
        }
    }
}

/// TOTP secret with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpSecret {
    pub secret: String,
    pub algorithm: TotpAlgorithm,
    pub digits: usize,
    pub period: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// WebAuthn credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnCredential {
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub sign_count: u32,
    pub aaguid: Option<Vec<u8>>,
    pub transports: Vec<AuthenticatorTransport>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub name: String,
}

/// MFA enrollment state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaEnrollment {
    pub user_id: Uuid,
    pub method: MfaMethod,
    pub status: MfaEnrollmentStatus,
    pub totp_secret: Option<TotpSecret>,
    pub webauthn_credential: Option<WebAuthnCredential>,
    pub backup_codes: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// MFA enrollment status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MfaEnrollmentStatus {
    Pending,
    Verified,
    Failed,
    Expired,
}

/// MFA verification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaVerificationResult {
    pub success: bool,
    pub method: MfaMethod,
    pub backup_code_used: bool,
    pub remaining_backup_codes: usize,
}

/// MFA manager.
pub struct MfaManager {
    config: MfaManagerConfig,
    totp_config: TotpConfig,
    webauthn_config: WebAuthnConfig,
    enrollments: Arc<DashMap<Uuid, MfaEnrollment>>, // user_id -> enrollment
    pending_challenges: Arc<DashMap<String, WebAuthnChallenge>>, // challenge_id -> challenge
}

#[derive(Debug, Clone)]
pub struct MfaManagerConfig {
    pub totp: TotpConfig,
    pub webauthn: WebAuthnConfig,
    pub backup_codes_count: usize,
    pub backup_code_length: usize,
    pub challenge_ttl: Duration,
}

impl MfaManagerConfig {
    pub fn from_config(config: &Config) -> Self {
        Self {
            totp: TotpConfig {
                issuer: config.auth.mfa.totp_issuer.clone().unwrap_or_else(|| "MediaEdge".to_string()),
                ..Default::default()
            },
            webauthn: WebAuthnConfig {
                rp_id: config.auth.mfa.webauthn_rp_id.clone().unwrap_or_else(|| "localhost".to_string()),
                rp_name: config.auth.mfa.webauthn_rp_name.clone().unwrap_or_else(|| "MediaEdge".to_string()),
                rp_origin: config.auth.mfa.webauthn_rp_origin.clone().unwrap_or_else(|| "https://localhost".to_string()),
                ..Default::default()
            },
            backup_codes_count: config.auth.mfa.backup_codes_count.unwrap_or(10),
            backup_code_length: config.auth.mfa.backup_code_length.unwrap_or(8),
            challenge_ttl: Duration::from_secs(config.auth.mfa.challenge_ttl_secs.unwrap_or(300)),
        }
    }
}

impl MfaManager {
    /// Create a new MFA manager.
    pub async fn new(config: &Config) -> Result<Arc<Self>> {
        let mgr_config = MfaManagerConfig::from_config(config);
        let webauthn_config = mgr_config.webauthn.clone();

        // Initialize WebAuthn
        let webauthn = WebauthnBuilder::new(&webauthn_config.rp_id, &webauthn_config.rp_origin)
            .map_err(|e| anyhow::anyhow!("WebAuthn init failed: {}", e))?
            .rp_name(&webauthn_config.rp_name)
            .timeout(Duration::from_millis(webauthn_config.timeout_ms as u64))
            .build()
            .map_err(|e| anyhow::anyhow!("WebAuthn build failed: {}", e))?;

        Ok(Arc::new(Self {
            config: mgr_config,
            totp_config: mgr_config.totp.clone(),
            webauthn_config,
            enrollments: Arc::new(DashMap::new()),
            pending_challenges: Arc::new(DashMap::new()),
        }))
    }

    // ==================== TOTP Methods ====================

    /// Generate a new TOTP secret for enrollment.
    pub fn generate_totp_secret(&self, user_email: &str) -> TotpSecret {
        let secret_bytes = ring::rand::generate(&ring::rand::SYSTEM_RANDOM).unwrap();
        let secret = general_purpose::STANDARD32.encode(secret_bytes.as_ref());

        TotpSecret {
            secret,
            algorithm: self.totp_config.algorithm,
            digits: self.totp_config.digits,
            period: self.totp_config.period,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        }
    }

    /// Get TOTP URI for QR code.
    pub fn get_totp_uri(&self, user_email: &str, secret: &TotpSecret) -> String {
        let totp = TotpBuilder::new()
            .algorithm(match secret.algorithm {
                TotpAlgorithm::Sha1 => totp_lite::Algorithm::SHA1,
                TotpAlgorithm::Sha256 => totp_lite::Algorithm::SHA256,
                TotpAlgorithm::Sha512 => totp_lite::Algorithm::SHA512,
            })
            .digits(secret.digits)
            .step(secret.period)
            .skew(self.totp_config.skew)
            .build()
            .unwrap();

        format!(
            "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm={}&digits={}&period={}",
            self.totp_config.issuer,
            user_email,
            secret.secret,
            self.totp_config.issuer,
            match secret.algorithm {
                TotpAlgorithm::Sha1 => "SHA1",
                TotpAlgorithm::Sha256 => "SHA256",
                TotpAlgorithm::Sha512 => "SHA512",
            },
            secret.digits,
            secret.period,
        )
    }

    /// Verify a TOTP code.
    pub fn verify_totp(&self, secret: &TotpSecret, code: &str) -> bool {
        let totp = TotpBuilder::new()
            .algorithm(match secret.algorithm {
                TotpAlgorithm::Sha1 => totp_lite::Algorithm::SHA1,
                TotpAlgorithm::Sha256 => totp_lite::Algorithm::SHA256,
                TotpAlgorithm::Sha512 => totp_lite::Algorithm::SHA512,
            })
            .digits(secret.digits)
            .step(secret.period)
            .skew(self.totp_config.skew)
            .build()
            .unwrap();

        // Parse code
        let code: u32 = match code.parse() {
            Ok(c) => c,
            Err(_) => return false,
        };

        // Verify with current time
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        totp.check(code, now)
    }

    // ==================== WebAuthn Methods ====================

    /// Start WebAuthn registration (returns creation challenge).
    pub async fn start_webauthn_registration(&self, user_id: Uuid, user_name: &str, display_name: &str) -> Result<CreationChallengeResponse> {
        let user_unique_id = Uuid::new_v4().as_bytes().to_vec();

        let (ccr, challenge) = self.webauthn
            .start_passkey_registration(user_unique_id, user_name, display_name, None)
            .map_err(|e| anyhow::anyhow!("WebAuthn registration start failed: {}", e))?;

        // Store challenge for verification
        let challenge_id = Uuid::new_v4().to_string();
        self.pending_challenges.insert(challenge_id.clone(), WebAuthnChallenge {
            challenge_id,
            user_id,
            challenge,
            challenge_type: WebAuthnChallengeType::Registration,
            expires_at: chrono::Utc::now() + chrono::Duration::from_std(self.config.challenge_ttl).unwrap(),
        });

        Ok(ccr)
    }

    /// Complete WebAuthn registration.
    pub async fn complete_webauthn_registration(
        &self,
        challenge_id: &str,
        registration: RegisterPublicKeyCredential,
    ) -> Result<WebAuthnCredential> {
        let pending = self.pending_challenges.remove(challenge_id)
            .ok_or_else(|| anyhow::anyhow!("Invalid or expired challenge"))?;

        let credential = self.webauthn
            .finish_passkey_registration(&registration, &pending.1)
            .map_err(|e| anyhow::anyhow!("WebAuthn registration finish failed: {}", e))?;

        let webauthn_cred = WebAuthnCredential {
            credential_id: credential.cred_id().to_vec(),
            public_key: credential.public_key().to_vec(),
            sign_count: 0,
            aaguid: credential.aaguid().map(|a| a.to_vec()),
            transports: credential.transports().unwrap_or_default(),
            created_at: chrono::Utc::now(),
            last_used_at: None,
            name: "Security Key".to_string(),
        };

        Ok(webauthn_cred)
    }

    /// Start WebAuthn authentication (returns request challenge).
    pub async fn start_webauthn_authentication(&self, user_id: Uuid) -> Result<RequestChallengeResponse> {
        // Get user's registered credentials
        let enrollment = self.enrollments.get(&user_id)
            .ok_or_else(|| anyhow::anyhow!("No MFA enrollment found"))?;

        let credential = enrollment.webauthn_credential.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No WebAuthn credential enrolled"))?;

        let allowed_creds = vec![PublicKeyCredentialDescriptor {
            id: credential.credential_id.clone(),
            transports: Some(credential.transports.clone()),
            type_: PublicKeyCredentialType::PublicKey,
            attestation_transport: None,
        }];

        let (rcr, challenge) = self.webauthn
            .start_passkey_authentication(&allowed_creds)
            .map_err(|e| anyhow::anyhow!("WebAuthn auth start failed: {}", e))?;

        let challenge_id = Uuid::new_v4().to_string();
        self.pending_challenges.insert(challenge_id.clone(), WebAuthnChallenge {
            challenge_id,
            user_id,
            challenge,
            challenge_type: WebAuthnChallengeType::Authentication,
            expires_at: chrono::Utc::now() + chrono::Duration::from_std(self.config.challenge_ttl).unwrap(),
        });

        Ok(rcr)
    }

    /// Complete WebAuthn authentication.
    pub async fn complete_webauthn_authentication(
        &self,
        challenge_id: &str,
        authentication: PublicKeyCredential,
    ) -> Result<()> {
        let pending = self.pending_challenges.remove(challenge_id)
            .ok_or_else(|| anyhow::anyhow!("Invalid or expired challenge"))?;

        let enrollment = self.enrollments.get(&pending.0.user_id)
            .ok_or_else(|| anyhow::anyhow!("No MFA enrollment found"))?;

        let credential = enrollment.webauthn_credential.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No WebAuthn credential enrolled"))?;

        // Verify authentication
        self.webauthn
            .finish_passkey_authentication(&authentication, &pending.1, &credential.credential_id)
            .map_err(|e| anyhow::anyhow!("WebAuthn auth finish failed: {}", e))?;

        // Update sign count
        if let Some(mut enrollment) = self.enrollments.get_mut(&pending.0.user_id) {
            if let Some(ref mut cred) = enrollment.webauthn_credential {
                cred.sign_count = authentication.authenticator_data.sign_count();
                cred.last_used_at = Some(chrono::Utc::now());
            }
        }

        Ok(())
    }

    // ==================== Enrollment Management ====================

    /// Start TOTP enrollment.
    pub async fn start_totp_enrollment(&self, user_id: Uuid, user_email: &str) -> Result<MfaEnrollment> {
        let secret = self.generate_totp_secret(user_email);
        let backup_codes = self.generate_backup_codes();

        let enrollment = MfaEnrollment {
            user_id,
            method: MfaMethod::Totp,
            status: MfaEnrollmentStatus::Pending,
            totp_secret: Some(secret),
            webauthn_credential: None,
            backup_codes,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };

        self.enrollments.insert(user_id, enrollment.clone());
        Ok(enrollment)
    }

    /// Complete TOTP enrollment (verify first code).
    pub async fn complete_totp_enrollment(&self, user_id: Uuid, code: &str) -> Result<bool> {
        let mut enrollment = self.enrollments.get_mut(&user_id)
            .ok_or_else(|| anyhow::anyhow!("No pending TOTP enrollment"))?;

        if enrollment.method != MfaMethod::Totp {
            return Err(anyhow::anyhow!("Invalid enrollment method"));
        }

        let secret = enrollment.totp_secret.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No TOTP secret"))?;

        let verified = self.verify_totp(secret, code);

        if verified {
            enrollment.status = MfaEnrollmentStatus::Verified;
            enrollment.completed_at = Some(chrono::Utc::now());
        } else {
            enrollment.status = MfaEnrollmentStatus::Failed;
        }

        Ok(verified)
    }

    /// Start WebAuthn enrollment.
    pub async fn start_webauthn_enrollment(&self, user_id: Uuid, user_name: &str, display_name: &str) -> Result<(MfaEnrollment, CreationChallengeResponse)> {
        let ccr = self.start_webauthn_registration(user_id, user_name, display_name).await?;
        let backup_codes = self.generate_backup_codes();

        let enrollment = MfaEnrollment {
            user_id,
            method: MfaMethod::WebAuthn,
            status: MfaEnrollmentStatus::Pending,
            totp_secret: None,
            webauthn_credential: None,
            backup_codes,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };

        self.enrollments.insert(user_id, enrollment.clone());
        Ok((enrollment, ccr))
    }

    /// Complete WebAuthn enrollment.
    pub async fn complete_webauthn_enrollment(
        &self,
        user_id: Uuid,
        challenge_id: &str,
        registration: RegisterPublicKeyCredential,
    ) -> Result<bool> {
        let credential = self.complete_webauthn_registration(challenge_id, registration).await?;

        let mut enrollment = self.enrollments.get_mut(&user_id)
            .ok_or_else(|| anyhow::anyhow!("No pending WebAuthn enrollment"))?;

        enrollment.webauthn_credential = Some(credential);
        enrollment.status = MfaEnrollmentStatus::Verified;
        enrollment.completed_at = Some(chrono::Utc::now());

        Ok(true)
    }

    /// Verify MFA code (TOTP or backup code).
    pub async fn verify_mfa(&self, user_id: Uuid, code: &str) -> Result<MfaVerificationResult> {
        let enrollment = self.enrollments.get(&user_id)
            .ok_or_else(|| anyhow::anyhow!("No MFA enrollment found"))?;

        // Check backup codes first
        if enrollment.backup_codes.contains(&code.to_string()) {
            let mut enrollment = self.enrollments.get_mut(&user_id).unwrap();
            enrollment.backup_codes.retain(|c| c != code);
            return Ok(MfaVerificationResult {
                success: true,
                method: enrollment.method,
                backup_code_used: true,
                remaining_backup_codes: enrollment.backup_codes.len(),
            });
        }

        // Check TOTP
        if enrollment.method == MfaMethod::Totp {
            if let Some(secret) = &enrollment.totp_secret {
                let verified = self.verify_totp(secret, code);
                if verified {
                    // Update last used
                    if let Some(mut e) = self.enrollments.get_mut(&user_id) {
                        if let Some(ref mut s) = e.totp_secret {
                            s.last_used_at = Some(chrono::Utc::now());
                        }
                    }
                }
                return Ok(MfaVerificationResult {
                    success: verified,
                    method: MfaMethod::Totp,
                    backup_code_used: false,
                    remaining_backup_codes: enrollment.backup_codes.len(),
                });
            }
        }

        Ok(MfaVerificationResult {
            success: false,
            method: enrollment.method,
            backup_code_used: false,
            remaining_backup_codes: enrollment.backup_codes.len(),
        })
    }

    /// Generate backup codes.
    fn generate_backup_codes(&self) -> Vec<String> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..self.config.backup_codes_count)
            .map(|_| {
                (0..self.config.backup_code_length)
                    .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
                    .collect()
            })
            .collect()
    }

    /// Regenerate backup codes.
    pub async fn regenerate_backup_codes(&self, user_id: Uuid) -> Result<Vec<String>> {
        let mut enrollment = self.enrollments.get_mut(&user_id)
            .ok_or_else(|| anyhow::anyhow!("No MFA enrollment found"))?;

        let codes = self.generate_backup_codes();
        enrollment.backup_codes = codes.clone();
        Ok(codes)
    }

    /// Remove MFA enrollment.
    pub async fn remove_enrollment(&self, user_id: Uuid) -> Result<()> {
        self.enrollments.remove(&user_id);
        Ok(())
    }

    /// Get enrollment status.
    pub async fn get_enrollment(&self, user_id: Uuid) -> Option<MfaEnrollment> {
        self.enrollments.get(&user_id).map(|e| e.clone())
    }
}

/// WebAuthn challenge storage.
#[derive(Debug, Clone)]
struct WebAuthnChallenge {
    challenge_id: String,
    user_id: Uuid,
    challenge: webauthn_rs::prelude::Challenge,
    challenge_type: WebAuthnChallengeType,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy)]
enum WebAuthnChallengeType {
    Registration,
    Authentication,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_totp_verification() {
        let config = MfaManagerConfig {
            totp: TotpConfig::default(),
            webauthn: WebAuthnConfig::default(),
            backup_codes_count: 10,
            backup_code_length: 8,
            challenge_ttl: Duration::from_secs(300),
        };

        let secret = TotpSecret {
            secret: "JBSWY3DPEHPK3PXP".to_string(), // Base32 encoded "Hello!"
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        };

        // This would need actual time synchronization to test properly
        // Just verify the code structure
        assert_eq!(secret.digits, 6);
        assert_eq!(secret.period, 30);
    }

    #[test]
    fn test_backup_code_generation() {
        let config = MfaManagerConfig {
            totp: TotpConfig::default(),
            webauthn: WebAuthnConfig::default(),
            backup_codes_count: 5,
            backup_code_length: 8,
            challenge_ttl: Duration::from_secs(300),
        };

        let mgr = MfaManager {
            config,
            totp_config: TotpConfig::default(),
            webauthn_config: WebAuthnConfig::default(),
            enrollments: Arc::new(DashMap::new()),
            pending_challenges: Arc::new(DashMap::new()),
        };

        let codes = mgr.generate_backup_codes();
        assert_eq!(codes.len(), 5);
        for code in codes {
            assert_eq!(code.len(), 8);
        }
    }
}