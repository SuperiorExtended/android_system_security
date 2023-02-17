// Copyright 2021, The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Implements safe wrappers around the public API of libopen-dice.
//! ## Example:
//! ```
//! use diced_open_dice_cbor as dice;
//!
//! let context = dice::dice::OpenDiceCborContext::new()
//! let parent_cdi_attest = [1u8, dice::CDI_SIZE];
//! let parent_cdi_seal = [2u8, dice::CDI_SIZE];
//! let input_values = dice::InputValues::new(
//!     [3u8, dice::HASH_SIZE], // code_hash
//!     dice::Config::Descriptor(&"My descriptor".as_bytes().to_vec()),
//!     [0u8, dice::HASH_SIZE], // authority_hash
//!     dice::DiceMode::kDiceModeNormal,
//!     [0u8, dice::HIDDEN_SIZE], // hidden
//! };
//! let (cdi_attest, cdi_seal, cert_chain) = context
//!     .main_flow(&parent_cdi_attest, &parent_cdi_seal, &input_values)?;
//! ```

pub use diced_open_dice::{
    check_result, derive_cdi_private_key_seed, hash, retry_bcc_format_config_descriptor,
    retry_bcc_main_flow, retry_dice_main_flow, sign, Config, DiceError, Hash, Hidden, InputValues,
    OwnedDiceArtifacts, Result, CDI_SIZE, HASH_SIZE, HIDDEN_SIZE, PRIVATE_KEY_SEED_SIZE,
};
use keystore2_crypto::ZVec;
pub use open_dice_cbor_bindgen::DiceMode;
use open_dice_cbor_bindgen::{DiceKeypairFromSeed, DICE_PRIVATE_KEY_SIZE, DICE_PUBLIC_KEY_SIZE};
use std::ffi::c_void;

/// The size of a private key.
pub const PRIVATE_KEY_SIZE: usize = DICE_PRIVATE_KEY_SIZE as usize;
/// The size of a public key.
pub const PUBLIC_KEY_SIZE: usize = DICE_PUBLIC_KEY_SIZE as usize;

/// Some libopen-dice variants use a context. Developers that want to customize these
/// bindings may want to implement their own Context factory that creates a context
/// useable by their preferred backend.
pub trait Context {
    /// # Safety
    /// The return value of get_context is passed to any open dice function.
    /// Implementations must explain why the context pointer returned is safe
    /// to be used by the open dice library.
    unsafe fn get_context(&mut self) -> *mut c_void;
}

impl<T: Context + Send> ContextImpl for T {}

/// This represents a context for the open dice library. The wrapped open dice instance, which
/// is based on boringssl and cbor, does not use a context, so that this type is empty.
#[derive(Default)]
pub struct OpenDiceCborContext();

impl OpenDiceCborContext {
    /// Construct a new instance of OpenDiceCborContext.
    pub fn new() -> Self {
        Default::default()
    }
}

impl Context for OpenDiceCborContext {
    unsafe fn get_context(&mut self) -> *mut c_void {
        // # Safety
        // The open dice cbor implementation does not use a context. It is safe
        // to return NULL.
        std::ptr::null_mut()
    }
}

/// Type alias for ZVec indicating that it holds a CDI_ATTEST secret.
pub type CdiAttest = ZVec;

/// Type alias for ZVec indicating that it holds a CDI_SEAL secret.
pub type CdiSeal = ZVec;

/// Type alias for Vec<u8> indicating that it hold a DICE certificate.
pub type Cert = Vec<u8>;

/// Type alias for Vec<u8> indicating that it holds a BCC certificate chain.
pub type Bcc = Vec<u8>;

/// ContextImpl is a mixin trait that implements the safe wrappers around the open dice
/// library calls. Implementations must implement Context::get_context(). As of
/// this writing, the only implementation is OpenDiceCborContext, which returns NULL.
pub trait ContextImpl: Context + Send {
    /// Safe wrapper around open-dice DiceKeyPairFromSeed, see open dice
    /// documentation for details.
    fn keypair_from_seed(&mut self, seed: &[u8; PRIVATE_KEY_SEED_SIZE]) -> Result<(Vec<u8>, ZVec)> {
        let mut private_key = ZVec::new(PRIVATE_KEY_SIZE)?;
        let mut public_key = vec![0u8; PUBLIC_KEY_SIZE];

        // SAFETY:
        // * The first context argument may be NULL and is unused by the wrapped
        //   implementation.
        // * The second argument is a pointer to a const buffer of size `PRIVATE_KEY_SEED_SIZE`
        //   fulfilled by the definition of the argument.
        // * The third argument and the fourth argument are mutable buffers of size
        //   `PRIVATE_KEY_SIZE` and `PUBLIC_KEY_SIZE` respectively. This is fulfilled by the
        //   allocations above.
        // * All pointers must be valid for the duration of the function call but not beyond.
        check_result(unsafe {
            DiceKeypairFromSeed(
                self.get_context(),
                seed.as_ptr(),
                public_key.as_mut_ptr(),
                private_key.as_mut_ptr(),
            )
        })?;
        Ok((public_key, private_key))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use diced_open_dice::DiceArtifacts;
    use diced_sample_inputs::make_sample_bcc_and_cdis;
    use std::convert::TryInto;

    static SEED_TEST_VECTOR: &[u8] = &[
        0xfa, 0x3c, 0x2f, 0x58, 0x37, 0xf5, 0x8e, 0x96, 0x16, 0x09, 0xf5, 0x22, 0xa1, 0xf1, 0xba,
        0xaa, 0x19, 0x95, 0x01, 0x79, 0x2e, 0x60, 0x56, 0xaf, 0xf6, 0x41, 0xe7, 0xff, 0x48, 0xf5,
        0x3a, 0x08, 0x84, 0x8a, 0x98, 0x85, 0x6d, 0xf5, 0x69, 0x21, 0x03, 0xcd, 0x09, 0xc3, 0x28,
        0xd6, 0x06, 0xa7, 0x57, 0xbd, 0x48, 0x4b, 0x0f, 0x79, 0x0f, 0xf8, 0x2f, 0xf0, 0x0a, 0x41,
        0x94, 0xd8, 0x8c, 0xa8,
    ];

    static CDI_ATTEST_TEST_VECTOR: &[u8] = &[
        0xfa, 0x3c, 0x2f, 0x58, 0x37, 0xf5, 0x8e, 0x96, 0x16, 0x09, 0xf5, 0x22, 0xa1, 0xf1, 0xba,
        0xaa, 0x19, 0x95, 0x01, 0x79, 0x2e, 0x60, 0x56, 0xaf, 0xf6, 0x41, 0xe7, 0xff, 0x48, 0xf5,
        0x3a, 0x08,
    ];
    static CDI_PRIVATE_KEY_SEED_TEST_VECTOR: &[u8] = &[
        0x5f, 0xcc, 0x8e, 0x1a, 0xd1, 0xc2, 0xb3, 0xe9, 0xfb, 0xe1, 0x68, 0xf0, 0xf6, 0x98, 0xfe,
        0x0d, 0xee, 0xd4, 0xb5, 0x18, 0xcb, 0x59, 0x70, 0x2d, 0xee, 0x06, 0xe5, 0x70, 0xf1, 0x72,
        0x02, 0x6e,
    ];

    static PUB_KEY_TEST_VECTOR: &[u8] = &[
        0x47, 0x42, 0x4b, 0xbd, 0xd7, 0x23, 0xb4, 0xcd, 0xca, 0xe2, 0x8e, 0xdc, 0x6b, 0xfc, 0x23,
        0xc9, 0x21, 0x5c, 0x48, 0x21, 0x47, 0xee, 0x5b, 0xfa, 0xaf, 0x88, 0x9a, 0x52, 0xf1, 0x61,
        0x06, 0x37,
    ];
    static PRIV_KEY_TEST_VECTOR: &[u8] = &[
        0x5f, 0xcc, 0x8e, 0x1a, 0xd1, 0xc2, 0xb3, 0xe9, 0xfb, 0xe1, 0x68, 0xf0, 0xf6, 0x98, 0xfe,
        0x0d, 0xee, 0xd4, 0xb5, 0x18, 0xcb, 0x59, 0x70, 0x2d, 0xee, 0x06, 0xe5, 0x70, 0xf1, 0x72,
        0x02, 0x6e, 0x47, 0x42, 0x4b, 0xbd, 0xd7, 0x23, 0xb4, 0xcd, 0xca, 0xe2, 0x8e, 0xdc, 0x6b,
        0xfc, 0x23, 0xc9, 0x21, 0x5c, 0x48, 0x21, 0x47, 0xee, 0x5b, 0xfa, 0xaf, 0x88, 0x9a, 0x52,
        0xf1, 0x61, 0x06, 0x37,
    ];

    static SIGNATURE_TEST_VECTOR: &[u8] = &[
        0x44, 0xae, 0xcc, 0xe2, 0xb9, 0x96, 0x18, 0x39, 0x0e, 0x61, 0x0f, 0x53, 0x07, 0xbf, 0xf2,
        0x32, 0x3d, 0x44, 0xd4, 0xf2, 0x07, 0x23, 0x30, 0x85, 0x32, 0x18, 0xd2, 0x69, 0xb8, 0x29,
        0x3c, 0x26, 0xe6, 0x0d, 0x9c, 0xa5, 0xc2, 0x73, 0xcd, 0x8c, 0xb8, 0x3c, 0x3e, 0x5b, 0xfd,
        0x62, 0x8d, 0xf6, 0xc4, 0x27, 0xa6, 0xe9, 0x11, 0x06, 0x5a, 0xb2, 0x2b, 0x64, 0xf7, 0xfc,
        0xbb, 0xab, 0x4a, 0x0e,
    ];

    #[test]
    fn hash_derive_sign_verify() {
        let mut ctx = OpenDiceCborContext::new();
        let seed = diced_open_dice::hash("MySeedString".as_bytes()).unwrap();
        assert_eq!(seed, SEED_TEST_VECTOR);
        let cdi_attest = &seed[..CDI_SIZE];
        assert_eq!(cdi_attest, CDI_ATTEST_TEST_VECTOR);
        let cdi_private_key_seed =
            derive_cdi_private_key_seed(cdi_attest.try_into().unwrap()).unwrap();
        assert_eq!(&cdi_private_key_seed[..], CDI_PRIVATE_KEY_SEED_TEST_VECTOR);
        let (pub_key, priv_key) =
            ctx.keypair_from_seed(cdi_private_key_seed[..].try_into().unwrap()).unwrap();
        assert_eq!(&pub_key, PUB_KEY_TEST_VECTOR);
        assert_eq!(&priv_key[..], PRIV_KEY_TEST_VECTOR);
        let mut signature =
            diced_open_dice::sign(b"MyMessage", priv_key[..].try_into().unwrap()).unwrap();
        assert_eq!(&signature, SIGNATURE_TEST_VECTOR);
        assert!(diced_open_dice::verify(b"MyMessage", &signature, pub_key[..].try_into().unwrap())
            .is_ok());
        assert!(diced_open_dice::verify(
            b"MyMessage_fail",
            &signature,
            pub_key[..].try_into().unwrap()
        )
        .is_err());
        signature[0] += 1;
        assert!(diced_open_dice::verify(b"MyMessage", &signature, pub_key[..].try_into().unwrap())
            .is_err());
    }

    static SAMPLE_CDI_ATTEST_TEST_VECTOR: &[u8] = &[
        0x3e, 0x57, 0x65, 0x5d, 0x48, 0x02, 0xbd, 0x5c, 0x66, 0xcc, 0x1f, 0x0f, 0xbe, 0x5e, 0x32,
        0xb6, 0x9e, 0x3d, 0x04, 0xaf, 0x00, 0x15, 0xbc, 0xdd, 0x1f, 0xbc, 0x59, 0xe4, 0xc3, 0x87,
        0x95, 0x5e,
    ];

    static SAMPLE_CDI_SEAL_TEST_VECTOR: &[u8] = &[
        0x36, 0x1b, 0xd2, 0xb3, 0xc4, 0xda, 0x77, 0xb2, 0x9c, 0xba, 0x39, 0x53, 0x82, 0x93, 0xd9,
        0xb8, 0x9f, 0x73, 0x2d, 0x27, 0x06, 0x15, 0xa8, 0xcb, 0x6d, 0x1d, 0xf2, 0xb1, 0x54, 0xbb,
        0x62, 0xf1,
    ];

    static SAMPLE_BCC_TEST_VECTOR: &[u8] = &[
        0x84, 0xa5, 0x01, 0x01, 0x03, 0x27, 0x04, 0x81, 0x02, 0x20, 0x06, 0x21, 0x58, 0x20, 0x3e,
        0x85, 0xe5, 0x72, 0x75, 0x55, 0xe5, 0x1e, 0xe7, 0xf3, 0x35, 0x94, 0x8e, 0xbb, 0xbd, 0x74,
        0x1e, 0x1d, 0xca, 0x49, 0x9c, 0x97, 0x39, 0x77, 0x06, 0xd3, 0xc8, 0x6e, 0x8b, 0xd7, 0x33,
        0xf9, 0x84, 0x43, 0xa1, 0x01, 0x27, 0xa0, 0x59, 0x01, 0x8a, 0xa9, 0x01, 0x78, 0x28, 0x34,
        0x32, 0x64, 0x38, 0x38, 0x36, 0x34, 0x66, 0x39, 0x37, 0x62, 0x36, 0x35, 0x34, 0x37, 0x61,
        0x35, 0x30, 0x63, 0x31, 0x65, 0x30, 0x61, 0x37, 0x34, 0x39, 0x66, 0x38, 0x65, 0x66, 0x38,
        0x62, 0x38, 0x31, 0x65, 0x63, 0x36, 0x32, 0x61, 0x66, 0x02, 0x78, 0x28, 0x31, 0x66, 0x36,
        0x39, 0x36, 0x66, 0x30, 0x37, 0x32, 0x35, 0x32, 0x66, 0x32, 0x39, 0x65, 0x39, 0x33, 0x66,
        0x65, 0x34, 0x64, 0x65, 0x31, 0x39, 0x65, 0x65, 0x33, 0x32, 0x63, 0x64, 0x38, 0x31, 0x64,
        0x63, 0x34, 0x30, 0x34, 0x65, 0x37, 0x36, 0x3a, 0x00, 0x47, 0x44, 0x50, 0x58, 0x40, 0x16,
        0x48, 0xf2, 0x55, 0x53, 0x23, 0xdd, 0x15, 0x2e, 0x83, 0x38, 0xc3, 0x64, 0x38, 0x63, 0x26,
        0x0f, 0xcf, 0x5b, 0xd1, 0x3a, 0xd3, 0x40, 0x3e, 0x23, 0xf8, 0x34, 0x4c, 0x6d, 0xa2, 0xbe,
        0x25, 0x1c, 0xb0, 0x29, 0xe8, 0xc3, 0xfb, 0xb8, 0x80, 0xdc, 0xb1, 0xd2, 0xb3, 0x91, 0x4d,
        0xd3, 0xfb, 0x01, 0x0f, 0xe4, 0xe9, 0x46, 0xa2, 0xc0, 0x26, 0x57, 0x5a, 0xba, 0x30, 0xf7,
        0x15, 0x98, 0x14, 0x3a, 0x00, 0x47, 0x44, 0x53, 0x56, 0xa3, 0x3a, 0x00, 0x01, 0x11, 0x71,
        0x63, 0x41, 0x42, 0x4c, 0x3a, 0x00, 0x01, 0x11, 0x72, 0x01, 0x3a, 0x00, 0x01, 0x11, 0x73,
        0xf6, 0x3a, 0x00, 0x47, 0x44, 0x52, 0x58, 0x40, 0x47, 0xae, 0x42, 0x27, 0x4c, 0xcb, 0x65,
        0x4d, 0xee, 0x74, 0x2d, 0x05, 0x78, 0x2a, 0x08, 0x2a, 0xa5, 0xf0, 0xcf, 0xea, 0x3e, 0x60,
        0xee, 0x97, 0x11, 0x4b, 0x5b, 0xe6, 0x05, 0x0c, 0xe8, 0x90, 0xf5, 0x22, 0xc4, 0xc6, 0x67,
        0x7a, 0x22, 0x27, 0x17, 0xb3, 0x79, 0xcc, 0x37, 0x64, 0x5e, 0x19, 0x4f, 0x96, 0x37, 0x67,
        0x3c, 0xd0, 0xc5, 0xed, 0x0f, 0xdd, 0xe7, 0x2e, 0x4f, 0x70, 0x97, 0x30, 0x3a, 0x00, 0x47,
        0x44, 0x54, 0x58, 0x40, 0xf9, 0x00, 0x9d, 0xc2, 0x59, 0x09, 0xe0, 0xb6, 0x98, 0xbd, 0xe3,
        0x97, 0x4a, 0xcb, 0x3c, 0xe7, 0x6b, 0x24, 0xc3, 0xe4, 0x98, 0xdd, 0xa9, 0x6a, 0x41, 0x59,
        0x15, 0xb1, 0x23, 0xe6, 0xc8, 0xdf, 0xfb, 0x52, 0xb4, 0x52, 0xc1, 0xb9, 0x61, 0xdd, 0xbc,
        0x5b, 0x37, 0x0e, 0x12, 0x12, 0xb2, 0xfd, 0xc1, 0x09, 0xb0, 0xcf, 0x33, 0x81, 0x4c, 0xc6,
        0x29, 0x1b, 0x99, 0xea, 0xae, 0xfd, 0xaa, 0x0d, 0x3a, 0x00, 0x47, 0x44, 0x56, 0x41, 0x01,
        0x3a, 0x00, 0x47, 0x44, 0x57, 0x58, 0x2d, 0xa5, 0x01, 0x01, 0x03, 0x27, 0x04, 0x81, 0x02,
        0x20, 0x06, 0x21, 0x58, 0x20, 0xb1, 0x02, 0xcc, 0x2c, 0xb2, 0x6a, 0x3b, 0xe9, 0xc1, 0xd3,
        0x95, 0x10, 0xa0, 0xe1, 0xff, 0x51, 0xde, 0x57, 0xd5, 0x65, 0x28, 0xfd, 0x7f, 0xeb, 0xd4,
        0xca, 0x15, 0xf3, 0xca, 0xdf, 0x37, 0x88, 0x3a, 0x00, 0x47, 0x44, 0x58, 0x41, 0x20, 0x58,
        0x40, 0x58, 0xd8, 0x03, 0x24, 0x53, 0x60, 0x57, 0xa9, 0x09, 0xfa, 0xab, 0xdc, 0x57, 0x1e,
        0xf0, 0xe5, 0x1e, 0x51, 0x6f, 0x9e, 0xa3, 0x42, 0xe6, 0x6a, 0x8c, 0xaa, 0xad, 0x08, 0x48,
        0xde, 0x7f, 0x4f, 0x6e, 0x2f, 0x7f, 0x39, 0x6c, 0xa1, 0xf8, 0x42, 0x71, 0xfe, 0x17, 0x3d,
        0xca, 0x31, 0x83, 0x92, 0xed, 0xbb, 0x40, 0xb8, 0x10, 0xe0, 0xf2, 0x5a, 0x99, 0x53, 0x38,
        0x46, 0x33, 0x97, 0x78, 0x05, 0x84, 0x43, 0xa1, 0x01, 0x27, 0xa0, 0x59, 0x01, 0x8a, 0xa9,
        0x01, 0x78, 0x28, 0x31, 0x66, 0x36, 0x39, 0x36, 0x66, 0x30, 0x37, 0x32, 0x35, 0x32, 0x66,
        0x32, 0x39, 0x65, 0x39, 0x33, 0x66, 0x65, 0x34, 0x64, 0x65, 0x31, 0x39, 0x65, 0x65, 0x33,
        0x32, 0x63, 0x64, 0x38, 0x31, 0x64, 0x63, 0x34, 0x30, 0x34, 0x65, 0x37, 0x36, 0x02, 0x78,
        0x28, 0x32, 0x35, 0x39, 0x34, 0x38, 0x39, 0x65, 0x36, 0x39, 0x37, 0x34, 0x38, 0x37, 0x30,
        0x35, 0x64, 0x65, 0x33, 0x65, 0x32, 0x66, 0x34, 0x34, 0x32, 0x36, 0x37, 0x65, 0x61, 0x34,
        0x39, 0x33, 0x38, 0x66, 0x66, 0x36, 0x61, 0x35, 0x37, 0x32, 0x35, 0x3a, 0x00, 0x47, 0x44,
        0x50, 0x58, 0x40, 0xa4, 0x0c, 0xcb, 0xc1, 0xbf, 0xfa, 0xcc, 0xfd, 0xeb, 0xf4, 0xfc, 0x43,
        0x83, 0x7f, 0x46, 0x8d, 0xd8, 0xd8, 0x14, 0xc1, 0x96, 0x14, 0x1f, 0x6e, 0xb3, 0xa0, 0xd9,
        0x56, 0xb3, 0xbf, 0x2f, 0xfa, 0x88, 0x70, 0x11, 0x07, 0x39, 0xa4, 0xd2, 0xa9, 0x6b, 0x18,
        0x28, 0xe8, 0x29, 0x20, 0x49, 0x0f, 0xbb, 0x8d, 0x08, 0x8c, 0xc6, 0x54, 0xe9, 0x71, 0xd2,
        0x7e, 0xa4, 0xfe, 0x58, 0x7f, 0xd3, 0xc7, 0x3a, 0x00, 0x47, 0x44, 0x53, 0x56, 0xa3, 0x3a,
        0x00, 0x01, 0x11, 0x71, 0x63, 0x41, 0x56, 0x42, 0x3a, 0x00, 0x01, 0x11, 0x72, 0x01, 0x3a,
        0x00, 0x01, 0x11, 0x73, 0xf6, 0x3a, 0x00, 0x47, 0x44, 0x52, 0x58, 0x40, 0x93, 0x17, 0xe1,
        0x11, 0x27, 0x59, 0xd0, 0xef, 0x75, 0x0b, 0x2b, 0x1c, 0x0f, 0x5f, 0x52, 0xc3, 0x29, 0x23,
        0xb5, 0x2a, 0xe6, 0x12, 0x72, 0x6f, 0x39, 0x86, 0x65, 0x2d, 0xf2, 0xe4, 0xe7, 0xd0, 0xaf,
        0x0e, 0xa7, 0x99, 0x16, 0x89, 0x97, 0x21, 0xf7, 0xdc, 0x89, 0xdc, 0xde, 0xbb, 0x94, 0x88,
        0x1f, 0xda, 0xe2, 0xf3, 0xe0, 0x54, 0xf9, 0x0e, 0x29, 0xb1, 0xbd, 0xe1, 0x0c, 0x0b, 0xd7,
        0xf6, 0x3a, 0x00, 0x47, 0x44, 0x54, 0x58, 0x40, 0xb2, 0x69, 0x05, 0x48, 0x56, 0xb5, 0xfa,
        0x55, 0x6f, 0xac, 0x56, 0xd9, 0x02, 0x35, 0x2b, 0xaa, 0x4c, 0xba, 0x28, 0xdd, 0x82, 0x3a,
        0x86, 0xf5, 0xd4, 0xc2, 0xf1, 0xf9, 0x35, 0x7d, 0xe4, 0x43, 0x13, 0xbf, 0xfe, 0xd3, 0x36,
        0xd8, 0x1c, 0x12, 0x78, 0x5c, 0x9c, 0x3e, 0xf6, 0x66, 0xef, 0xab, 0x3d, 0x0f, 0x89, 0xa4,
        0x6f, 0xc9, 0x72, 0xee, 0x73, 0x43, 0x02, 0x8a, 0xef, 0xbc, 0x05, 0x98, 0x3a, 0x00, 0x47,
        0x44, 0x56, 0x41, 0x01, 0x3a, 0x00, 0x47, 0x44, 0x57, 0x58, 0x2d, 0xa5, 0x01, 0x01, 0x03,
        0x27, 0x04, 0x81, 0x02, 0x20, 0x06, 0x21, 0x58, 0x20, 0x96, 0x6d, 0x96, 0x42, 0xda, 0x64,
        0x51, 0xad, 0xfa, 0x00, 0xbc, 0xbc, 0x95, 0x8a, 0xb0, 0xb9, 0x76, 0x01, 0xe6, 0xbd, 0xc0,
        0x26, 0x79, 0x26, 0xfc, 0x0f, 0x1d, 0x87, 0x65, 0xf1, 0xf3, 0x99, 0x3a, 0x00, 0x47, 0x44,
        0x58, 0x41, 0x20, 0x58, 0x40, 0x10, 0x7f, 0x77, 0xad, 0x70, 0xbd, 0x52, 0x81, 0x28, 0x8d,
        0x24, 0x81, 0xb4, 0x3f, 0x21, 0x68, 0x9f, 0xc3, 0x80, 0x68, 0x86, 0x55, 0xfb, 0x2e, 0x6d,
        0x96, 0xe1, 0xe1, 0xb7, 0x28, 0x8d, 0x63, 0x85, 0xba, 0x2a, 0x01, 0x33, 0x87, 0x60, 0x63,
        0xbb, 0x16, 0x3f, 0x2f, 0x3d, 0xf4, 0x2d, 0x48, 0x5b, 0x87, 0xed, 0xda, 0x34, 0xeb, 0x9c,
        0x4d, 0x14, 0xac, 0x65, 0xf4, 0xfa, 0xef, 0x45, 0x0b, 0x84, 0x43, 0xa1, 0x01, 0x27, 0xa0,
        0x59, 0x01, 0x8f, 0xa9, 0x01, 0x78, 0x28, 0x32, 0x35, 0x39, 0x34, 0x38, 0x39, 0x65, 0x36,
        0x39, 0x37, 0x34, 0x38, 0x37, 0x30, 0x35, 0x64, 0x65, 0x33, 0x65, 0x32, 0x66, 0x34, 0x34,
        0x32, 0x36, 0x37, 0x65, 0x61, 0x34, 0x39, 0x33, 0x38, 0x66, 0x66, 0x36, 0x61, 0x35, 0x37,
        0x32, 0x35, 0x02, 0x78, 0x28, 0x35, 0x64, 0x34, 0x65, 0x64, 0x37, 0x66, 0x34, 0x31, 0x37,
        0x61, 0x39, 0x35, 0x34, 0x61, 0x31, 0x38, 0x31, 0x34, 0x30, 0x37, 0x62, 0x35, 0x38, 0x38,
        0x35, 0x61, 0x66, 0x64, 0x37, 0x32, 0x61, 0x35, 0x62, 0x66, 0x34, 0x30, 0x64, 0x61, 0x36,
        0x3a, 0x00, 0x47, 0x44, 0x50, 0x58, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3a, 0x00, 0x47, 0x44,
        0x53, 0x58, 0x1a, 0xa3, 0x3a, 0x00, 0x01, 0x11, 0x71, 0x67, 0x41, 0x6e, 0x64, 0x72, 0x6f,
        0x69, 0x64, 0x3a, 0x00, 0x01, 0x11, 0x72, 0x0c, 0x3a, 0x00, 0x01, 0x11, 0x73, 0xf6, 0x3a,
        0x00, 0x47, 0x44, 0x52, 0x58, 0x40, 0x26, 0x1a, 0xbd, 0x26, 0xd8, 0x37, 0x8f, 0x4a, 0xf2,
        0x9e, 0x49, 0x4d, 0x93, 0x23, 0xc4, 0x6e, 0x02, 0xda, 0xe0, 0x00, 0x02, 0xe7, 0xed, 0x29,
        0xdf, 0x2b, 0xb3, 0x69, 0xf3, 0x55, 0x0e, 0x4c, 0x22, 0xdc, 0xcf, 0xf5, 0x92, 0xc9, 0xfa,
        0x78, 0x98, 0xf1, 0x0e, 0x55, 0x5f, 0xf4, 0x45, 0xed, 0xc0, 0x0a, 0x72, 0x2a, 0x7a, 0x3a,
        0xd2, 0xb1, 0xf7, 0x76, 0xfe, 0x2a, 0x6b, 0x7b, 0x2a, 0x53, 0x3a, 0x00, 0x47, 0x44, 0x54,
        0x58, 0x40, 0x04, 0x25, 0x5d, 0x60, 0x5f, 0x5c, 0x45, 0x0d, 0xf2, 0x9a, 0x6e, 0x99, 0x30,
        0x03, 0xb8, 0xd6, 0xe1, 0x99, 0x71, 0x1b, 0xf8, 0x44, 0xfa, 0xb5, 0x31, 0x79, 0x1c, 0x37,
        0x68, 0x4e, 0x1d, 0xc0, 0x24, 0x74, 0x68, 0xf8, 0x80, 0x20, 0x3e, 0x44, 0xb1, 0x43, 0xd2,
        0x9c, 0xfc, 0x12, 0x9e, 0x77, 0x0a, 0xde, 0x29, 0x24, 0xff, 0x2e, 0xfa, 0xc7, 0x10, 0xd5,
        0x73, 0xd4, 0xc6, 0xdf, 0x62, 0x9f, 0x3a, 0x00, 0x47, 0x44, 0x56, 0x41, 0x01, 0x3a, 0x00,
        0x47, 0x44, 0x57, 0x58, 0x2d, 0xa5, 0x01, 0x01, 0x03, 0x27, 0x04, 0x81, 0x02, 0x20, 0x06,
        0x21, 0x58, 0x20, 0xdb, 0xe7, 0x5b, 0x3f, 0xa3, 0x42, 0xb0, 0x9c, 0xf8, 0x40, 0x8c, 0xb0,
        0x9c, 0xf0, 0x0a, 0xaf, 0xdf, 0x6f, 0xe5, 0x09, 0x21, 0x11, 0x92, 0xe1, 0xf8, 0xc5, 0x09,
        0x02, 0x3d, 0x1f, 0xb7, 0xc5, 0x3a, 0x00, 0x47, 0x44, 0x58, 0x41, 0x20, 0x58, 0x40, 0xc4,
        0xc1, 0xd7, 0x1c, 0x2d, 0x26, 0x89, 0x22, 0xcf, 0xa6, 0x99, 0x77, 0x30, 0x84, 0x86, 0x27,
        0x59, 0x8f, 0xd8, 0x08, 0x75, 0xe0, 0xb2, 0xef, 0xf9, 0xfa, 0xa5, 0x40, 0x8c, 0xd3, 0xeb,
        0xbb, 0xda, 0xf2, 0xc8, 0xae, 0x41, 0x22, 0x50, 0x9c, 0xe8, 0xb2, 0x9c, 0x9b, 0x3f, 0x8a,
        0x78, 0x76, 0xab, 0xd0, 0xbe, 0xfc, 0xe4, 0x79, 0xcb, 0x1b, 0x2b, 0xaa, 0x4d, 0xdd, 0x15,
        0x61, 0x42, 0x06,
    ];

    // This test invokes make_sample_bcc_and_cdis and compares the result bitwise to the target
    // vectors. The function uses main_flow, bcc_main_flow, format_config_descriptor,
    // derive_cdi_private_key_seed, and keypair_from_seed. This test is sensitive to errors
    // and changes in any of those functions.
    #[test]
    fn main_flow_and_bcc_main_flow() {
        let dice_artifacts = make_sample_bcc_and_cdis().unwrap();
        assert_eq!(dice_artifacts.cdi_attest(), SAMPLE_CDI_ATTEST_TEST_VECTOR);
        assert_eq!(dice_artifacts.cdi_seal(), SAMPLE_CDI_SEAL_TEST_VECTOR);
        assert_eq!(dice_artifacts.bcc(), Some(SAMPLE_BCC_TEST_VECTOR));
    }
}
