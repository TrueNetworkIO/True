#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::{OptionQuery, BoundedVec, *}
    };
    use log;
    use sp_runtime::Vec;
    use sp_std::prelude::ToOwned;
    use frame_system::pallet_prelude::*;
    use scale_info::prelude::{format, string::String, vec};
    use sp_core::{
        crypto::{Ss58Codec},
        sr25519::Public,
        H256, U256,
    };
    use sp_core::{H160};
    use sp_runtime::AccountId32;
    use sp_runtime::traits::{Hash, Keccak256};

    use ed25519_dalek::VerifyingKey;

    use pallet_issuers::Issuers;

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub enum CredType {
        Char,
        U8,
        I8,
        U16,
        I16,
        U32,
        I32,
        U64,
        I64,
        F32,
        F64,
        Hash,
        Boolean,
        Text
    }

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub enum AcquirerAddress {
        Substrate(AccountId32),
        Ethereum(H160),
        Solana(String),
    }

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    pub enum SizeInBytes {
        Limited(u8),
    }

    impl CredType {
        pub fn size_in_bytes(&self) -> SizeInBytes {
            match self {
                CredType::Char => SizeInBytes::Limited(1),
                CredType::U8 => SizeInBytes::Limited(1),
                CredType::I8 => SizeInBytes::Limited(1),
                CredType::U16 => SizeInBytes::Limited(2),
                CredType::I16 => SizeInBytes::Limited(2),
                CredType::U32 => SizeInBytes::Limited(4),
                CredType::I32 => SizeInBytes::Limited(4),
                CredType::U64 => SizeInBytes::Limited(8),
                CredType::I64 => SizeInBytes::Limited(8),
                CredType::F32 => SizeInBytes::Limited(4),
                CredType::F64 => SizeInBytes::Limited(8),
                CredType::Hash => SizeInBytes::Limited(32),
                CredType::Boolean => SizeInBytes::Limited(1),
                CredType::Text => SizeInBytes::Limited(128),
            }
        }
    }

    pub type CredVal<T: Config> = (BoundedVec<u8, T::MaxSchemaFieldSize>, CredType);
    pub type CredSchema<T: Config> = BoundedVec<CredVal<T>, T::MaxSchemaFields>;


    pub type CredAttestation<T: Config> = BoundedVec<BoundedVec<u8, T::MaxSchemaFieldSize>, T::MaxSchemaFields>;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_issuers::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Hashing: Hash<Output = Self::Hash>;

        #[pallet::constant]
		type MaxSchemaFields: Get<u32>;

        #[pallet::constant]
        type MaxSchemaFieldSize: Get<u32>;
    }

    #[pallet::storage]
    pub type Schemas<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, CredSchema<T>, OptionQuery>;

    #[pallet::storage]
    pub type Attestations<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, AcquirerAddress>, // Reciever account.
            NMapKey<Twox64Concat, T::Hash>,  // Issuer hash.
            NMapKey<Twox64Concat, T::Hash>,      // Schema hash.
        ),
        Vec<CredAttestation<T>>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        SchemaCreated {
            schema_hash: T::Hash,
            schema: CredSchema<T>,
        },
        AttestationCreated {
            issuer_hash: T::Hash,
            account_id: AcquirerAddress,
            schema_hash: T::Hash,
            attestation: CredAttestation<T>,
        },
        AttestationUpdated {
          issuer_hash: T::Hash,
          account_id: AcquirerAddress,
          schema_hash: T::Hash,
          attestation_index: u32,
          attestation: CredAttestation<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        SchemaNotFound,
        InvalidFormat,
        SchemaAlreadyExists,
        TooManySchemaFields,
        SchemaFieldTooLarge,
        InvalidAddress,
        AttestationNotFound,
        InvalidAttestationIndex
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(100_000)]
        pub fn create_schema(
            origin: OriginFor<T>,
            issuer_hash: T::Hash,
            schema: Vec<(Vec<u8>, CredType)>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(schema.len() <= T::MaxSchemaFields::get() as usize, Error::<T>::TooManySchemaFields);
            
            let mut bounded_schema = CredSchema::<T>::default();

            for (vec, cred_type) in schema {
                ensure!(vec.len() <= T::MaxSchemaFieldSize::get() as usize, Error::<T>::SchemaFieldTooLarge);
                let bounded_vec = BoundedVec::<u8, T::MaxSchemaFieldSize>::try_from(vec)
                    .map_err(|_| Error::<T>::SchemaFieldTooLarge)?;
                bounded_schema.try_push((bounded_vec, cred_type)).map_err(|_| Error::<T>::TooManySchemaFields)?;
            }

            let bytes: Vec<u8> = bounded_schema
                .iter()
                .flat_map(|(vec, cred_type)| {
                    let mut bytes = vec.to_vec();
                    bytes.extend_from_slice(&cred_type.encode());
                    bytes
                })
                .collect();

            //TODO: multiple fields in schema with the same name

            let schema_hash = <T as Config>::Hashing::hash(&bytes);

            ensure!(
                !Schemas::<T>::contains_key(schema_hash),
                Error::<T>::SchemaAlreadyExists
            );

            let issuer = Issuers::<T>::get(issuer_hash)
                .ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
            ensure!(
                issuer.controllers.contains(&who),
                pallet_issuers::Error::<T>::NotAuthorized
            );

            
            let cred_schema = CredSchema::<T>::try_from(bounded_schema).map_err(|_| Error::<T>::TooManySchemaFields)?;

            Schemas::<T>::insert(schema_hash, cred_schema.clone());

            Self::deposit_event(Event::SchemaCreated { schema_hash, schema: cred_schema });

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10000_000)]
        pub fn attest(
            origin: OriginFor<T>,
            issuer_hash: T::Hash,
            schema_hash: T::Hash,
            for_account: Vec<u8>,
            attestation: Vec<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let acquirer_address = Self::parse_acquirer_address(for_account)?;

            let issuer = Issuers::<T>::get(issuer_hash)
                .ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
            ensure!(
                issuer.controllers.contains(&who),
                pallet_issuers::Error::<T>::NotAuthorized
            );

            let schema = Schemas::<T>::get(schema_hash).ok_or(Error::<T>::SchemaNotFound)?;

            let attestation =
                Self::validate_attestation(&schema, &attestation).ok_or(Error::<T>::InvalidFormat)?;

            log::debug!(target: "algo", "Creds:{:?}", attestation);

            let mut existing_attestations = Attestations::<T>::get(
                (acquirer_address.clone(), issuer_hash, schema_hash)
            ).unwrap_or_default();
  
            // Add new attestation
            existing_attestations.push(attestation.clone());

            Attestations::<T>::insert(
              (acquirer_address.clone(), issuer_hash, schema_hash),
              existing_attestations,
            );

            Self::deposit_event(Event::AttestationCreated {
                issuer_hash,
                account_id: acquirer_address,
                schema_hash,
                attestation,
            });

            Ok(())
        }

         // New extrinsic to update a specific attestation
        #[pallet::call_index(3)]
        #[pallet::weight(10000_000)]
        pub fn update_attestation(
            origin: OriginFor<T>,
            issuer_hash: T::Hash,
            schema_hash: T::Hash,
            for_account: Vec<u8>,
            attestation_index: u32,
            new_attestation: Vec<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            let acquirer_address = Self::parse_acquirer_address(for_account)?;

            // Check if the caller is authorized
            let issuer = Issuers::<T>::get(issuer_hash)
                .ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
            ensure!(
                issuer.controllers.contains(&who),
                pallet_issuers::Error::<T>::NotAuthorized
            );

            // Get and validate schema
            let schema = Schemas::<T>::get(schema_hash).ok_or(Error::<T>::SchemaNotFound)?;

            // Validate the new attestation
            let validated_attestation = 
                Self::validate_attestation(&schema, &new_attestation)
                    .ok_or(Error::<T>::InvalidFormat)?;

            // Get existing attestations
            let mut attestations = Attestations::<T>::get(
                (acquirer_address.clone(), issuer_hash, schema_hash)
            ).ok_or(Error::<T>::AttestationNotFound)?;

            ensure!(
                attestation_index < attestations.len() as u32,
                Error::<T>::InvalidAttestationIndex
            );

            // Update the specific attestation
            attestations[attestation_index as usize] = validated_attestation.clone();

            // Store updated attestations
            Attestations::<T>::insert(
                (acquirer_address.clone(), issuer_hash, schema_hash),
                attestations,
            );

            Self::deposit_event(Event::AttestationUpdated {
                issuer_hash,
                account_id: acquirer_address,
                schema_hash,
                attestation_index,
                attestation: validated_attestation,
            });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn validate_attestation(
            schema: &CredSchema<T>,
            attestation: &Vec<Vec<u8>>,
        ) -> Option<CredAttestation<T>> {
            if schema.len() != attestation.len() {
                return None;
            }

            let mut formatted = Vec::with_capacity(attestation.len());

            for ((_, cred_type), val) in
                schema.iter().zip(attestation)
            {
                let SizeInBytes::Limited(expected_len) = cred_type.size_in_bytes();
                if val.is_empty() || val.len() > expected_len as usize {
                    return None;
                }

                let mut formatted_val = val.clone();
                if *cred_type != CredType::Text && val.len() != expected_len as usize {
                    formatted_val.resize(expected_len as usize, 0);
                }

                formatted.push(
                    BoundedVec::try_from(formatted_val).map_err(|_| Error::<T>::SchemaFieldTooLarge).ok()?
                );
            }

            CredAttestation::<T>::try_from(formatted).ok()
        }

        pub fn is_valid_solana_address(address: Vec<u8>) -> bool {
          match address.try_into() {
              Ok(address_array) => {
                  VerifyingKey::from_bytes(&address_array).is_ok()
              }
              Err(_) => false,
          }
        }

        pub fn check_valid_substrate_address(address: &[u8]) -> Option<AccountId32> {
            let address_array: [u8; 32] = address.try_into().ok()?;
            let account_id = AccountId32::new(address_array);
            AccountId32::from_ss58check(&account_id.to_ss58check()).ok().map(|_| account_id)
        }

        pub fn check_solana_address(input: Vec<u8>) -> Option<String> {
          let base58_str = core::str::from_utf8(&input).ok()?;
      
          let mut decoded_output = [0u8; 32];
          match bs58::decode(base58_str).onto(&mut decoded_output) {
              Ok(32) => {
                  if Self::is_valid_solana_address(decoded_output.to_vec()) {
                      Some(String::from(base58_str))
                  } else {
                      None
                  }
              },
              Ok(decoded_length) => {
                  log::error!("Decoded length is not 32 bytes: {}", decoded_length);
                  None
              }
              Err(e) => {
                  log::error!("Failed to decode Base58: {:?}", e);
                  None
              }
          }
      }
      pub fn parse_acquirer_address(address: Vec<u8>) -> Result<AcquirerAddress, DispatchError> {
        // First check if it's an Ethereum address by length
        if address.len() == 20 {
            let mut array = [0u8; 20];
            array.copy_from_slice(&address);
            let wallet_address = H160::from(array);
            return Ok(AcquirerAddress::Ethereum(wallet_address));
        }
    
        // Then check Solana address
        if let Some(solana_address) = Self::check_solana_address(address.clone()) {
            return Ok(AcquirerAddress::Solana(solana_address));
        }
    
        // Try Substrate address with more robust handling
        if let Ok(address_str) = core::str::from_utf8(&address) {
            // Try direct conversion first
            if let Ok(account_id32) = AccountId32::from_ss58check(address_str) {
                return Ok(AcquirerAddress::Substrate(account_id32));
            }
        }
        
        // If that fails, try as raw bytes if length is correct
        if address.len() == 32 {
            let mut array = [0u8; 32];
            array.copy_from_slice(&address);
            return Ok(AcquirerAddress::Substrate(AccountId32::new(array)));
        }
    
        // If everything fails, it's an invalid address
        Err(Error::<T>::InvalidAddress.into())
    }
  }
}
