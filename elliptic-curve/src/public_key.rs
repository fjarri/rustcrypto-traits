//! Elliptic curve public keys.

use crate::{
    consts::U1,
    scalar::NonZeroScalar,
    sec1::{
        EncodedPoint, FromEncodedPoint, ToEncodedPoint, UncompressedPointSize, UntaggedPointSize,
    },
    weierstrass::{point, Curve},
    AffinePoint, Error, FieldBytes, ProjectiveArithmetic, ProjectivePoint, Scalar,
};
use core::{
    convert::{TryFrom, TryInto},
    fmt::Debug,
    ops::Add,
};
use ff::PrimeField;
use generic_array::ArrayLength;
use group::{Curve as _, Group};

#[cfg(feature = "pkcs8")]
use {
    crate::{AlgorithmParameters, ALGORITHM_OID},
    pkcs8::FromPublicKey,
};

#[cfg(feature = "pem")]
use {
    alloc::{
        string::{String, ToString},
        vec::Vec,
    },
    core::str::FromStr,
    pkcs8::ToPublicKey,
};

/// Elliptic curve public keys.
///
/// This is a wrapper type for [`AffinePoint`] which ensures an inner
/// non-identity point and provides a common place to handle encoding/decoding.
///
/// # Parsing "SPKI" Keys
///
/// X.509 `SubjectPublicKeyInfo` (SPKI) is a commonly used format for encoding
/// public keys, notably public keys corresponding to PKCS#8 private keys.
/// (especially ones generated by OpenSSL).
///
/// Keys in SPKI format are either binary (ASN.1 BER/DER), or PEM encoded
/// (ASCII) and begin with the following:
///
/// ```text
/// -----BEGIN PUBLIC KEY-----
/// ```
///
/// To decode an elliptic curve public key from SPKI, enable the `pkcs8`
/// feature of this crate (or the `pkcs8` feature of a specific RustCrypto
/// elliptic curve crate) and use the
/// [`elliptic_curve::pkcs8::FromPublicKey`][`pkcs8::FromPublicKey`]
/// trait to parse it.
///
/// When the `pem` feature of this crate (or a specific RustCrypto elliptic
/// curve crate) is enabled, a [`FromStr`] impl is also available.
#[derive(Clone, Debug)]
pub struct PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug,
{
    point: AffinePoint<C>,
}

impl<C> PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug,
    ProjectivePoint<C>: From<AffinePoint<C>>,
{
    /// Convert an [`AffinePoint`] into a [`PublicKey`]
    pub fn from_affine(point: AffinePoint<C>) -> Result<Self, Error> {
        if ProjectivePoint::<C>::from(point).is_identity().into() {
            Err(Error)
        } else {
            Ok(Self { point })
        }
    }

    /// Compute a [`PublicKey`] from a secret [`NonZeroScalar`] value
    /// (i.e. a secret key represented as a raw scalar value)
    pub fn from_secret_scalar(scalar: &NonZeroScalar<C>) -> Self {
        // `NonZeroScalar` ensures the resulting point is not the identity
        Self {
            point: (C::ProjectivePoint::generator() * scalar.as_ref()).to_affine(),
        }
    }

    /// Decode [`PublicKey`] (compressed or uncompressed) from the
    /// `Elliptic-Curve-Point-to-Octet-String` encoding described in
    /// SEC 1: Elliptic Curve Cryptography (Version 2.0) section
    /// 2.3.3 (page 10).
    ///
    /// <http://www.secg.org/sec1-v2.pdf>
    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, Error>
    where
        Self: TryFrom<EncodedPoint<C>, Error = Error>,
        UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
        UncompressedPointSize<C>: ArrayLength<u8>,
    {
        EncodedPoint::from_bytes(bytes)
            .map_err(|_| Error)
            .and_then(TryInto::try_into)
    }

    /// Borrow the inner [`AffinePoint`] from this [`PublicKey`].
    ///
    /// In ECC, public keys are elliptic curve points.
    pub fn as_affine(&self) -> &AffinePoint<C> {
        &self.point
    }

    /// Convert this [`PublicKey`] to a [`ProjectivePoint`] for the given curve
    pub fn to_projective(&self) -> ProjectivePoint<C> {
        self.point.clone().into()
    }

    /// Encode this public key as an ASN.1 DER bitstring as used in both
    /// PKCS#8 private keys and SPKI public keys.
    #[cfg(feature = "pem")]
    pub(crate) fn to_der_bitstring(&self) -> Vec<u8>
    where
        AffinePoint<C>: Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
        ProjectivePoint<C>: From<AffinePoint<C>>,
        UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
        UncompressedPointSize<C>: ArrayLength<u8>,
    {
        let mut bitstring = Vec::new();
        bitstring.push(0);
        bitstring.extend_from_slice(self.to_encoded_point(false).as_ref());
        bitstring
    }
}

impl<C> AsRef<AffinePoint<C>> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug,
    ProjectivePoint<C>: From<AffinePoint<C>>,
{
    fn as_ref(&self) -> &AffinePoint<C> {
        self.as_affine()
    }
}

impl<C> TryFrom<EncodedPoint<C>> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(encoded_point: EncodedPoint<C>) -> Result<Self, Error> {
        encoded_point.decode()
    }
}

impl<C> TryFrom<&EncodedPoint<C>> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(encoded_point: &EncodedPoint<C>) -> Result<Self, Error> {
        encoded_point.decode()
    }
}

impl<C> From<PublicKey<C>> for EncodedPoint<C>
where
    C: Curve + ProjectiveArithmetic + point::Compression,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn from(public_key: PublicKey<C>) -> EncodedPoint<C> {
        EncodedPoint::<C>::from(&public_key)
    }
}

impl<C> From<&PublicKey<C>> for EncodedPoint<C>
where
    C: Curve + ProjectiveArithmetic + point::Compression,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn from(public_key: &PublicKey<C>) -> EncodedPoint<C> {
        public_key.to_encoded_point(C::COMPRESS_POINTS)
    }
}

impl<C> FromEncodedPoint<C> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    /// Initialize [`PublicKey`] from an [`EncodedPoint`]
    fn from_encoded_point(encoded_point: &EncodedPoint<C>) -> Option<Self> {
        AffinePoint::<C>::from_encoded_point(encoded_point)
            .and_then(|point| PublicKey::from_affine(point).ok())
    }
}

impl<C> ToEncodedPoint<C> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    /// Serialize this [`PublicKey`] as a SEC1 [`EncodedPoint`], optionally applying
    /// point compression
    fn to_encoded_point(&self, compress: bool) -> EncodedPoint<C> {
        self.point.to_encoded_point(compress)
    }
}

impl<C> Copy for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug,
{
}

impl<C> Eq for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
}

impl<C> PartialEq for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn eq(&self, other: &Self) -> bool {
        // TODO(tarcieri): more efficient implementation?
        // This is implemented this way to reduce bounds for `AffinePoint<C>`
        self.to_encoded_point(false) == other.to_encoded_point(false)
    }
}

#[cfg(feature = "pkcs8")]
#[cfg_attr(docsrs, doc(cfg(feature = "pkcs8")))]
impl<C> FromPublicKey for PublicKey<C>
where
    Self: TryFrom<EncodedPoint<C>, Error = Error>,
    C: Curve + AlgorithmParameters + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn from_spki(spki: pkcs8::SubjectPublicKeyInfo<'_>) -> pkcs8::Result<Self> {
        if spki.algorithm.oid != ALGORITHM_OID || spki.algorithm.parameters_oid() != Some(C::OID) {
            return Err(pkcs8::Error::Decode);
        }

        // Look for a leading `0x00` byte in the bitstring
        if spki.subject_public_key.get(0).cloned() != Some(0x00) {
            return Err(pkcs8::Error::Decode);
        }

        Self::from_sec1_bytes(&spki.subject_public_key[1..]).map_err(|_| pkcs8::Error::Decode)
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl<C> ToPublicKey for PublicKey<C>
where
    C: Curve + AlgorithmParameters + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn to_public_key_der(&self) -> pkcs8::PublicKeyDocument {
        let public_key_bytes = self.to_der_bitstring();

        pkcs8::SubjectPublicKeyInfo {
            algorithm: C::algorithm_identifier(),
            subject_public_key: &public_key_bytes,
        }
        .to_der()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl<C> FromStr for PublicKey<C>
where
    Self: TryFrom<EncodedPoint<C>, Error = Error>,
    C: Curve + AlgorithmParameters + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Self::from_public_key_pem(s).map_err(|_| Error)
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl<C> ToString for PublicKey<C>
where
    C: Curve + AlgorithmParameters + ProjectiveArithmetic,
    FieldBytes<C>: From<Scalar<C>> + for<'r> From<&'r Scalar<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn to_string(&self) -> String {
        self.to_public_key_pem()
    }
}

#[cfg(all(feature = "dev", test))]
mod tests {
    use crate::{dev::MockCurve, sec1::FromEncodedPoint};

    type EncodedPoint = crate::sec1::EncodedPoint<MockCurve>;
    type PublicKey = super::PublicKey<MockCurve>;

    #[test]
    fn from_encoded_point_rejects_identity() {
        let identity = EncodedPoint::identity();
        assert_eq!(PublicKey::from_encoded_point(&identity), None);
    }
}