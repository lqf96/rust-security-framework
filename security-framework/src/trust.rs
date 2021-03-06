//! Trust evaluation support.

use core_foundation_sys::base::{Boolean, CFIndex};
use core_foundation::base::TCFType;
use core_foundation::array::CFArray;

use security_framework_sys::trust::*;
use std::ptr;

use cvt;
use base::Result;
use certificate::SecCertificate;
use policy::SecPolicy;

/// The result of trust evaluation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TrustResult(SecTrustResultType);

impl TrustResult {
    /// An invalid setting or result.
    pub const INVALID: TrustResult = TrustResult(kSecTrustResultInvalid);

    /// You may proceed.
    pub const PROCEED: TrustResult = TrustResult(kSecTrustResultProceed);

    /// Indicates a denial by the user, do not proceed.
    pub const DENY: TrustResult = TrustResult(kSecTrustResultDeny);

    /// The certificate is implicitly trusted.
    pub const UNSPECIFIED: TrustResult = TrustResult(kSecTrustResultUnspecified);

    /// Indicates a trust policy failure that the user can override.
    pub const RECOVERABLE_TRUST_FAILURE: TrustResult =
        TrustResult(kSecTrustResultRecoverableTrustFailure);

    /// Indicates a trust policy failure that the user cannot override.
    pub const FATAL_TRUST_FAILURE: TrustResult = TrustResult(kSecTrustResultFatalTrustFailure);

    /// An error not related to trust validation.
    pub const OTHER_ERROR: TrustResult = TrustResult(kSecTrustResultOtherError);
}

impl TrustResult {
    /// Returns true if the result is "successful" - specifically `PROCEED` or `UNSPECIFIED`.
    pub fn success(&self) -> bool {
        match *self {
            TrustResult::PROCEED | TrustResult::UNSPECIFIED => true,
            _ => false,
        }
    }
}

declare_TCFType! {
    /// A type representing a trust evaluation for a certificate.
    SecTrust, SecTrustRef
}
impl_TCFType!(SecTrust, SecTrustRef, SecTrustGetTypeID);

unsafe impl Sync for SecTrust {}
unsafe impl Send for SecTrust {}

impl SecTrust {
    /// Creates a SecTrustRef that is configured with a certificate chain, for validating
    /// that chain against a collection of policies.
    pub fn create_with_certificates(
        certs: &[SecCertificate],
        policies: &[SecPolicy],
    ) -> Result<SecTrust> {
        let cert_array = CFArray::from_CFTypes(&certs);
        let policy_array = CFArray::from_CFTypes(&policies);
        let mut trust = ptr::null_mut();
        unsafe {
            cvt(SecTrustCreateWithCertificates(
                cert_array.as_CFTypeRef(),
                policy_array.as_CFTypeRef(),
                &mut trust,
            ))?;
            Ok(SecTrust(trust))
        }
    }

    /// Sets additional anchor certificates used to validate trust.
    pub fn set_anchor_certificates(&mut self, certs: &[SecCertificate]) -> Result<()> {
        let certs = CFArray::from_CFTypes(&certs);

        unsafe {
            cvt(SecTrustSetAnchorCertificates(
                self.0,
                certs.as_concrete_TypeRef(),
            ))
        }
    }

    /// If set to `true`, only the certificates specified by
    /// `set_anchor_certificates` will be trusted, but not globally trusted
    /// certificates.
    pub fn set_trust_anchor_certificates_only(&mut self, only: bool) -> Result<()> {
        unsafe { cvt(SecTrustSetAnchorCertificatesOnly(self.0, only as Boolean)) }
    }

    /// Sets the policy used to evaluate trust.
    pub fn set_policy(&mut self, policy: &SecPolicy) -> Result<()> {
        unsafe { cvt(SecTrustSetPolicies(self.0, policy.as_CFTypeRef())) }
    }

    /// Evaluates trust.
    pub fn evaluate(&self) -> Result<TrustResult> {
        unsafe {
            let mut result = kSecTrustResultInvalid;
            cvt(SecTrustEvaluate(self.0, &mut result))?;
            Ok(TrustResult(result))
        }
    }

    /// Returns the number of certificates in an evaluated certificate chain.
    ///
    /// Note: evaluate must first be called on the SecTrust.
    pub fn certificate_count(&self) -> CFIndex {
        unsafe { SecTrustGetCertificateCount(self.0) }
    }

    /// Returns a specific certificate from the certificate chain used to evaluate trust.
    ///
    /// Note: evaluate must first be called on the SecTrust.
    pub fn certificate_at_index(&self, ix: CFIndex) -> Option<SecCertificate> {
        unsafe {
            if self.certificate_count() <= ix {
                None
            } else {
                let certificate = SecTrustGetCertificateAtIndex(self.0, ix);
                Some(SecCertificate::wrap_under_get_rule(certificate as *mut _))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use test::certificate;
    use trust::SecTrust;
    use policy::SecPolicy;
    use secure_transport::SslProtocolSide;

    #[test]
    fn create_with_certificates() {
        let cert = certificate();
        let ssl_policy = SecPolicy::create_ssl(SslProtocolSide::CLIENT, Some("certifi.io"));
        let trust = SecTrust::create_with_certificates(&[cert], &[ssl_policy]).unwrap();
        assert_eq!(trust.evaluate().unwrap().success(), false)
    }

    #[test]
    fn certificate_count_and_at_index() {
        let cert = certificate();
        let ssl_policy = SecPolicy::create_ssl(SslProtocolSide::CLIENT, Some("certifi.io"));
        let trust = SecTrust::create_with_certificates(&[cert], &[ssl_policy]).unwrap();
        trust.evaluate().unwrap();

        let count = trust.certificate_count();
        assert_eq!(count, 1);

        let cert_bytes = trust.certificate_at_index(0).unwrap().to_der();
        assert_eq!(cert_bytes, certificate().to_der());
    }

    #[test]
    fn certificate_at_index_out_of_bounds() {
        let cert = certificate();
        let ssl_policy = SecPolicy::create_ssl(SslProtocolSide::CLIENT, Some("certifi.io"));
        let trust = SecTrust::create_with_certificates(&[cert], &[ssl_policy]).unwrap();
        trust.evaluate().unwrap();

        assert!(trust.certificate_at_index(1).is_none());
    }

    #[test]
    fn set_policy() {
        let cert = certificate();
        let ssl_policy = SecPolicy::create_ssl(SslProtocolSide::CLIENT, Some("certifi.io.bogus"));
        let mut trust = SecTrust::create_with_certificates(&[cert], &[ssl_policy]).unwrap();
        let ssl_policy = SecPolicy::create_ssl(SslProtocolSide::CLIENT, Some("certifi.io"));
        trust.set_policy(&ssl_policy).unwrap();
        assert_eq!(trust.evaluate().unwrap().success(), false)
    }
}
