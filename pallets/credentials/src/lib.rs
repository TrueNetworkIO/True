#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::Vec,
		log,
		pallet_prelude::{OptionQuery, *},
	};
	use frame_system::pallet_prelude::*;
	use scale_info::prelude::vec;
	use sp_runtime::traits::Hash;
	use sp_core::{H160, crypto::Ss58Codec, crypto::AccountId32};
	use sp_core::sr25519::Public;

	use pallet_issuers::Issuers;

	use super::*;

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub enum CredType {
		// String,
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
    Hash
	}

	///wallet addresss attestation can be being issued to
	
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub enum AcquirerAddress{
		Substrate([u8; 32]),
		Ethereum([u8; 20]),
		Solana([u8; 32]),
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum SizeInBytes {
		Limited(u8),
		// Unlimited,
	}

	impl CredType {
		pub fn size_in_bytes(&self) -> SizeInBytes {
			match self {
				// CredType::String => SizeInBytes::Unlimited,
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
        CredType::Hash => SizeInBytes::Limited(32)
			}
		}
	}

	pub type CredVal = (Vec<u8>, CredType);
	pub type CredSchema = Vec<CredVal>;
	pub type CredAttestation = Vec<Vec<u8>>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_issuers::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Hashing: Hash<Output = Self::Hash>;
	}

	#[pallet::storage]
	pub type Schemas<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, CredSchema, OptionQuery>;

	#[pallet::storage]
	pub type Attestations<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		AcquirerAddress,
		Twox64Concat,
		T::Hash,
		CredAttestation,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		SchemaCreated {
			schema_hash: T::Hash,
			schema: CredSchema,
		},
		AttestationCreated {
			account_id: AcquirerAddress,
			schema_hash: T::Hash,
			attestation: CredAttestation,
		},
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		SchemaNotFound,
		InvalidFormat,
		SchemaAlreadyExists,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(100_000)]
		pub fn create_schema(
			origin: OriginFor<T>,
			issuer_hash: T::Hash,
			schema: CredSchema,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let bytes: Vec<u8> = schema
				.iter()
				.flat_map(|(vec, cred_type)| {
					let mut bytes = vec.clone();
					bytes.extend_from_slice(&cred_type.encode());
					bytes
				})
				.collect();

			let schema_hash = <T as Config>::Hashing::hash(&bytes);

			let schema_option = Schemas::<T>::get(schema_hash);
			if let Some(_) = schema_option {
				// Revert the transaction with an error indicating that the schema already exists
				return Err(Error::<T>::SchemaAlreadyExists.into());
			}

			let issuer =
				Issuers::<T>::get(issuer_hash).ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
			ensure!(issuer.controllers.contains(&who), pallet_issuers::Error::<T>::NotAuthorized);

			Schemas::<T>::insert(schema_hash, schema.clone());

			Self::deposit_event(Event::SchemaCreated { schema_hash, schema });

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(100_000)]
		pub fn attest(
			origin: OriginFor<T>,
			issuer_hash: T::Hash,
			schema_hash: T::Hash,
			for_account: Vec<u8>,
			attestation: CredAttestation,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let acquirer_address = Pallet::<T>::parse_acquirer_address(for_account)?;

			let issuer =
				Issuers::<T>::get(issuer_hash).ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
			ensure!(issuer.controllers.contains(&who), pallet_issuers::Error::<T>::NotAuthorized);

			let schema = Schemas::<T>::get(schema_hash).ok_or(Error::<T>::SchemaNotFound)?;

			let attestation = Pallet::<T>::validate_attestation(&schema, &attestation)
				.ok_or(Error::<T>::InvalidFormat)?;

			log::debug!(target: "algo", "Creds:{:?}", attestation);

			Attestations::<T>::insert(acquirer_address.clone(), schema_hash, attestation.clone());

			Self::deposit_event(Event::AttestationCreated {
				account_id: acquirer_address,
				schema_hash,
				attestation,
			});

			Ok(())
		}

		// #[pallet::call_index(1)]
		// #[pallet::weight(100_000)]
		// pub fn edit_issuer(
		//     origin: OriginFor<T>,
		//     hash: T::Hash,
		//     name: Option<Vec<u8>>,
		//     controllers: Option<Vec<T::AccountId>>,
		// ) -> DispatchResult {
		//     let who = ensure_signed(origin)?;
		//
		//
		//     let mut issuer = Issuers::<T>::get(hash)
		//         .ok_or(Error::<T>::IssuerNotFound)?;
		//
		//
		//     ensure!(!issuer.controllers.contains(&who), Error::<T>::NotAuthorized);
		//
		//     if let Some(name) = name {
		//         issuer.name = name;
		//     }
		//
		//     if let Some(controllers) = controllers {
		//         issuer.controllers = controllers;
		//     }
		//
		//     Issuers::<T>::insert(hash, issuer.clone());
		//     Self::deposit_event(Event::IssuerUpdated { hash, issuer });
		//
		//     Ok(())
		// }
	}

	impl<T: Config> Pallet<T> {
		pub fn validate_attestation(
			schema: &CredSchema,
			attestation: &CredAttestation,
		) -> Option<CredAttestation> {
			if schema.len() != attestation.len() {
				return None;
			}

			let mut formatted = vec![vec![]; attestation.len()];

			for (((_, cred_type), val), i) in
				schema.iter().zip(attestation).zip(0..attestation.len())
			{
				let SizeInBytes::Limited(expected_len) = cred_type.size_in_bytes();
				if val.is_empty() || val.len() > expected_len as usize {
					return None;
				}
				formatted[i] = val.clone();
				if val.len() != expected_len as usize {
					for _ in 0..(expected_len as usize - val.len()) {
						formatted[i].push(0);
					}
				}
			}

			Some(formatted)
		}

	pub fn is_valid_ethereum_address(address: &[u8]) -> bool {
        address.len() == 20 && H160::from_slice(address).to_fixed_bytes() == *address
    }

	pub fn is_valid_solana_address(address: &[u8]) -> bool {
		// Check if the address is exactly 32 bytes long

		let address_array: [u8; 32] = match address.try_into() {
			Ok(arr) => arr,
			Err(_) => return false, 
		};

		let public_key = Public(address_array);

		// Attempt to encode the public key to Base58
		public_key.to_ss58check().len() == 44
	}

	// need to make this generic over the chain configurations
	fn is_valid_substrate_address(address: &[u8]) -> bool {
		// Check if the address is exactly 32 bytes long
		let address_array: [u8; 32] = match address.try_into() {
			Ok(arr) => arr,
			Err(_) => return false, 
		};
	
		// Create an AccountId32 from the address bytes
		let account_id = AccountId32::new(address_array);
	
		// Encode the AccountId32 to SS58 and then try to decode it
		// This checks if the address is valid in any SS58 format
		match AccountId32::from_ss58check(&account_id.to_ss58check()) {
			Ok(_) => true,
			Err(_) => false,
		}
	}

	pub fn parse_acquirer_address(address: Vec<u8>) -> Result<AcquirerAddress, DispatchError> {
		match address.len() {
			20 => {
				if Pallet::<T>::is_valid_ethereum_address(&address) {
					Ok(AcquirerAddress::Ethereum(address.try_into().unwrap()))
				} else {
					Err(DispatchError::Other("Invalid Ethereum address format"))
				}
			},
			32 => {
				if Self::is_valid_solana_address(&address) {
					Ok(AcquirerAddress::Solana(address.try_into().unwrap()))
				} else {
					Err(DispatchError::Other("Invalid Solana address format"))
				}
			},
			_ if address.len() == T::AccountId::max_encoded_len() => {
				if Self::is_valid_substrate_address(&address) {
					Ok(AcquirerAddress::Substrate(address.try_into().unwrap()))
				} else {
					Err(DispatchError::Other("Invalid Substrate address format"))
				}
			},			
			_ => Err(DispatchError::Other("Invalid address length"))
		}
	}
	}
}

mod testt {
	#[test]
	pub fn test_a() {
		let bytes = (1i64).to_be_bytes();
		println!("{bytes:#04X?}\n{bytes:?}");
	}
}
