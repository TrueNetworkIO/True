#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::*;
pub mod weights;
pub use weights::WeightInfo as CredentialsWeightInfo;

pub mod tests;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{ pallet_prelude::{ OptionQuery, BoundedVec, * } };
	use frame_support::traits::Time;
	use log;
	use hex;
	use sp_runtime::Vec;
	use frame_system::pallet_prelude::*;
	use scale_info::prelude::{ string::String, vec };
	use sp_core::{ crypto::{ Ss58Codec } };
	use sp_core::{ H160 };
	use sp_runtime::AccountId32;
	use sp_runtime::traits::{ Hash };

	use ed25519_dalek::VerifyingKey;

	use super::CredentialsWeightInfo;
	use pallet_issuers::Issuers;
	use pallet_timestamp;

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
		Text,
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

	#[derive(Clone, Encode, Decode, PartialEq, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct BlockTime<T: Config> {
		pub time: T::Moment,
		pub block_number: BlockNumberFor<T>,
	}

	// Manual Debug implementation for BlockTime
	impl<T: Config> core::fmt::Debug for BlockTime<T> {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
			f.debug_struct("BlockTime")
				.field("time", &"<Moment>")
				.field("block_number", &"<BlockNumber>")
				.finish()
		}
	}

	#[derive(Clone, Encode, Decode, PartialEq, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct AttestationMetadata<T: Config> {
		pub created_at: BlockTime<T>,
    pub updated_at: BlockTime<T>, 
		pub version: u32,
		pub expiration: Option<BlockTime<T>>,

		pub attestation_index: u32, // Index of the attestation in the schema
	}

	// Manual Debug implementation for AttestationMetadata
	impl<T: Config> core::fmt::Debug for AttestationMetadata<T> {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
			f.debug_struct("AttestationMetadata")
				.field("created_at", &self.created_at)
				.field("version", &self.version)
				.field("expiration", &self.expiration)
				.field("attestation_index", &self.attestation_index)
				.finish()
		}
	}

	#[derive(Clone, Encode, Decode, PartialEq, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Attestation<T: Config> {
		// WHO issued it
		pub issuer_hash: IssuerHash<T>,
		// WHAT type of credential
		pub schema_hash: SchemaHash<T>,
		// TO WHOM
		pub account_id: AcquirerAddress,
		// WHEN/VERSION info
		pub metadata: AttestationMetadata<T>,
		// THE ACTUAL DATA
		pub data: CredAttestation<T>,
	}

	// Manual Debug implementation for Attestation
	impl<T: Config> core::fmt::Debug for Attestation<T> {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
			f.debug_struct("Attestation")
				.field("issuer_hash", &"<IssuerHash>")
				.field("schema_hash", &"<SchemaHash>")
				.field("account_id", &self.account_id)
				.field("metadata", &self.metadata)
				.field("data", &self.data)
				.finish()
		}
	}

	impl<T: Config> Attestation<T> {
		pub fn is_valid_at(&self, current_time: &BlockTime<T>) -> bool {
			match &self.metadata.expiration {
				Some(exp) => {
					// Primary: timestamp expiration
					if current_time.time > exp.time {
						return false;
					}
					// Fallback: block number protection during normal operation
					current_time.block_number <= exp.block_number
				}
				None => true,
			}
		}
	}

	pub type SchemaFieldName<T: Config> = BoundedVec<u8, T::MaxSchemaFieldSize>;
	pub type SchemaField<T: Config> = (SchemaFieldName<T>, CredType);

	pub type Schema<T: Config> = BoundedVec<SchemaField<T>, T::MaxSchemaFields>;

	pub type AttestationDataPoint<T: Config> = BoundedVec<u8, T::MaxAttestationValueSize>;

	pub type CredAttestation<T: Config> = BoundedVec<AttestationDataPoint<T>, T::MaxSchemaFields>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_issuers::Config + pallet_timestamp::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Hashing: Hash<Output = Self::Hash>;

		#[pallet::constant]
		type MaxSchemaFields: Get<u32>;

		#[pallet::constant]
		type MaxSchemaFieldSize: Get<u32>;

		#[pallet::constant]
		type MaxAttestationValueSize: Get<u32>;

		type CredentialsWeightInfo: CredentialsWeightInfo;
	}

	pub type SchemaHash<T: Config> = T::Hash;
	pub type IssuerHash<T: Config> = T::Hash;
	pub type AttestationId<T: Config> = T::Hash;

	#[pallet::storage]
	pub type Schemas<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		SchemaHash<T>,
		Schema<T>,
		OptionQuery
	>;

	#[pallet::storage]
	pub type AttestationNextId<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, AcquirerAddress>, // User.
			NMapKey<Twox64Concat, IssuerHash<T>>, // Issuer hash.
			NMapKey<Twox64Concat, SchemaHash<T>>, // Schema hash.
		),
		u32, // NextId (index) to be used for attestation. Latest would be `next_id - 1`.
		ValueQuery
	>;

	#[pallet::storage]
	pub type Attestations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		AttestationId<T>,
		Attestation<T>,
		OptionQuery
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SchemaCreated {
			schema_hash: SchemaHash<T>,
			schema: Schema<T>,
			issuer_hash: IssuerHash<T>,
		},
		AttestationCreated {
			issuer_hash: IssuerHash<T>,
			account_id: AcquirerAddress,
			schema_hash: SchemaHash<T>,
			attestation_id: AttestationId<T>,
			attestation: Attestation<T>,
		},
		AttestationUpdated {
			issuer_hash: IssuerHash<T>,
			account_id: AcquirerAddress,
			schema_hash: SchemaHash<T>,
			attestation_id: AttestationId<T>,
			attestation: CredAttestation<T>,
		},
		AttestationRevoked {
			issuer_hash: IssuerHash<T>,
			account_id: AcquirerAddress,
			schema_hash: SchemaHash<T>,
			attestation_id: AttestationId<T>,
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
		InvalidAttestationIndex,
		InvalidHashFormat,
		AttestationExpired,
		AttestationAlreadyRevoked,
		UnauthorizedUpdate,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight({
			let field_count = schema.len() as u32;
			let max_name_size = schema
				.iter()
				.map(|(name, _)| name.len())
				.max()
				.unwrap_or(0) as u32;
			T::CredentialsWeightInfo::create_schema(field_count, max_name_size)
		})]
		pub fn create_schema(
			origin: OriginFor<T>,
			issuer_hash: T::Hash,
			schema: Vec<(Vec<u8>, CredType)>
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				schema.len() <= (T::MaxSchemaFields::get() as usize),
				Error::<T>::TooManySchemaFields
			);

			let mut bounded_schema = Schema::<T>::default();

			for (vec, cred_type) in schema.clone() {
				ensure!(
					vec.len() <= (T::MaxSchemaFieldSize::get() as usize),
					Error::<T>::SchemaFieldTooLarge
				);
				let bounded_vec = BoundedVec::<u8, T::MaxSchemaFieldSize>
					::try_from(vec)
					.map_err(|_| Error::<T>::SchemaFieldTooLarge)?;
				bounded_schema
					.try_push((bounded_vec, cred_type))
					.map_err(|_| Error::<T>::TooManySchemaFields)?;
			}

			let bytes: Vec<u8> = bounded_schema
				.iter()
				.flat_map(|(vec, cred_type)| {
					let mut bytes = vec.to_vec();
					bytes.extend_from_slice(&cred_type.encode());
					bytes
				})
				.collect();

			let schema_hash = <T as Config>::Hashing::hash(&bytes);

			ensure!(!Schemas::<T>::contains_key(schema_hash), Error::<T>::SchemaAlreadyExists);

			let issuer = Issuers::<T>
				::get(issuer_hash)
				.ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
			ensure!(issuer.controllers.contains(&who), pallet_issuers::Error::<T>::NotAuthorized);

			let cred_schema = Schema::<T>
				::try_from(bounded_schema)
				.map_err(|_| Error::<T>::TooManySchemaFields)?;

			Schemas::<T>::insert(schema_hash, cred_schema.clone());

			Self::deposit_event(Event::SchemaCreated {
				schema_hash,
				schema: cred_schema,
				issuer_hash,
			});

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight({
			let schema = Schemas::<T>::get(schema_hash).unwrap_or_default();
			let field_count = schema.len() as u32;
			let max_value_size = raw_attestation
				.iter()
				.map(|v| v.len())
				.max()
				.unwrap_or(0) as u32;
			let address_type = 1u32; // Default to most expensive case
			T::CredentialsWeightInfo::attest(field_count, max_value_size, address_type)
		})]
		pub fn attest(
			origin: OriginFor<T>,
			issuer_hash: IssuerHash<T>,
			schema_hash: SchemaHash<T>,
			for_account: Vec<u8>,
			raw_attestation: Vec<Vec<u8>>,
			expiry: Option<BlockTime<T>>
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let acquirer_address = Self::parse_acquirer_address(for_account)?;

			let issuer = Issuers::<T>
				::get(issuer_hash)
				.ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
			ensure!(issuer.controllers.contains(&who), pallet_issuers::Error::<T>::NotAuthorized);

			let schema = Schemas::<T>::get(schema_hash).ok_or(Error::<T>::SchemaNotFound)?;

			let attestation_data = Self::validate_attestation(&schema, &raw_attestation).ok_or(
				Error::<T>::InvalidFormat
			)?;

			log::debug!(target: "algo", "Creds:{:?}", attestation_data);

			let attestation_index = AttestationNextId::<T>::get((
				acquirer_address.clone(),
				issuer_hash,
				schema_hash,
			));

			let mut bytes = Vec::new();

			bytes.extend_from_slice(&acquirer_address.encode());
			bytes.extend_from_slice(&issuer_hash.encode());
			bytes.extend_from_slice(&schema_hash.encode());
			bytes.extend_from_slice(&attestation_index.encode());

			let attestation_id = <T as Config>::Hashing::hash(&bytes);

			let attestation = Attestation::<T> {
				issuer_hash,
				schema_hash,
				account_id: acquirer_address.clone(),
				metadata: AttestationMetadata {
					created_at: BlockTime {
						time: pallet_timestamp::Pallet::<T>::now(),
						block_number: frame_system::Pallet::<T>::block_number(),
					},
					updated_at: BlockTime {
						time: pallet_timestamp::Pallet::<T>::now(),
						block_number: frame_system::Pallet::<T>::block_number(),
					},
					version: 0,
					expiration: expiry,
					attestation_index,
				},
				data: attestation_data,
			};

			Attestations::<T>::insert(attestation_id.clone(), attestation.clone());

			// Update the next attestation index
			AttestationNextId::<T>::mutate(
				(acquirer_address.clone(), issuer_hash, schema_hash),
				|next_id| {
					*next_id += 1;
				}
			);

			Self::deposit_event(Event::AttestationCreated {
				issuer_hash,
				account_id: acquirer_address,
				schema_hash,
				attestation_id,
				attestation,
			});

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight({
			match Attestations::<T>::get(&attestation_id) {
				Some(attestation) => {
					let schema = Schemas::<T>::get(attestation.schema_hash).unwrap_or_default();
					let field_count = schema.len() as u32;
					let max_value_size = raw_attestation
						.iter()
						.map(|v| v.len())
						.max()
						.unwrap_or(0) as u32;
					let address_type = 1u32; // Default to most expensive case
					T::CredentialsWeightInfo::attest(field_count, max_value_size, address_type)
				}
				None => T::CredentialsWeightInfo::attest(0, 0, 1),
			}
		})]
		pub fn update_attestation(
			origin: OriginFor<T>,
			attestation_id: AttestationId<T>,
			raw_attestation: Vec<Vec<u8>>,
			new_expiry: Option<BlockTime<T>>
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Get the existing attestation
			let mut attestation = Attestations::<T>
				::get(&attestation_id)
				.ok_or(Error::<T>::AttestationNotFound)?;

			// Verify the caller is authorized (issuer controller)
			let issuer = Issuers::<T>
				::get(attestation.issuer_hash)
				.ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
			ensure!(issuer.controllers.contains(&who), pallet_issuers::Error::<T>::NotAuthorized);

			// Get the schema to validate the new attestation data
			let schema = Schemas::<T>
				::get(attestation.schema_hash)
				.ok_or(Error::<T>::SchemaNotFound)?;

			// Validate the new attestation data
			let new_attestation_data = Self::validate_attestation(&schema, &raw_attestation).ok_or(
				Error::<T>::InvalidFormat
			)?;

			// Update the attestation
			attestation.data = new_attestation_data.clone();
			attestation.metadata.version += 1;
      attestation.metadata.updated_at = BlockTime {
        time: pallet_timestamp::Pallet::<T>::now(),
        block_number: frame_system::Pallet::<T>::block_number(),
      };

			// Update expiry if provided
			if let Some(expiry) = new_expiry {
				attestation.metadata.expiration = Some(expiry);
			}

			// Save the updated attestation
			Attestations::<T>::insert(&attestation_id, &attestation);

			// Emit event
			Self::deposit_event(Event::AttestationUpdated {
				issuer_hash: attestation.issuer_hash,
				account_id: attestation.account_id,
				schema_hash: attestation.schema_hash,
				attestation_id,
				attestation: new_attestation_data,
			});

			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(1_000_000_000)]
		pub fn update_attestation_by_details(
			origin: OriginFor<T>,
			for_account: Vec<u8>,
			issuer_hash: IssuerHash<T>,
			schema_hash: SchemaHash<T>,
			attestation_index: u32,
			raw_attestation: Vec<Vec<u8>>,
			new_expiry: Option<BlockTime<T>>
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			let acquirer_address = Self::parse_acquirer_address(for_account)?;

			let mut bytes = Vec::new();

			bytes.extend_from_slice(&acquirer_address.encode());
			bytes.extend_from_slice(&issuer_hash.encode());
			bytes.extend_from_slice(&schema_hash.encode());
			bytes.extend_from_slice(&attestation_index.encode());

			let attestation_id = <T as Config>::Hashing::hash(&bytes);

			// Get the existing attestation
			Self::update_attestation(origin, attestation_id, raw_attestation, new_expiry)?;

			Ok(())
		}

		// Optional: Add a function to revoke/invalidate an attestation
		#[pallet::call_index(5)]
		#[pallet::weight(T::CredentialsWeightInfo::create_schema(1, 32))] // Simple weight
		pub fn revoke_attestation(
			origin: OriginFor<T>,
			attestation_id: AttestationId<T>
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Get the existing attestation
			let mut attestation = Attestations::<T>
				::get(&attestation_id)
				.ok_or(Error::<T>::AttestationNotFound)?;

			// Verify the caller is authorized (issuer controller)
			let issuer = Issuers::<T>
				::get(attestation.issuer_hash)
				.ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
			ensure!(issuer.controllers.contains(&who), pallet_issuers::Error::<T>::NotAuthorized);

			// Current block time used for checks and revocation
			let now = BlockTime {
				time: pallet_timestamp::Pallet::<T>::now(),
				block_number: frame_system::Pallet::<T>::block_number(),
			};

			// Ensure the attestation hasn't already expired
			ensure!(attestation.is_valid_at(&now), Error::<T>::AttestationAlreadyRevoked);

			// Set expiration to current time (effectively revoking it)
      attestation.metadata.updated_at = now.clone();
			attestation.metadata.expiration = Some(now);

			// Save the updated attestation
			Attestations::<T>::insert(&attestation_id, &attestation);

			// Emit event (you might want to add a new event type for revocation)
			Self::deposit_event(Event::AttestationRevoked {
				issuer_hash: attestation.issuer_hash,
				account_id: attestation.account_id,
				schema_hash: attestation.schema_hash,
				attestation_id,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn validate_attestation(
			schema: &Schema<T>,
			attestation: &Vec<Vec<u8>>
		) -> Option<CredAttestation<T>> {
			if schema.len() != attestation.len() {
				return None;
			}

			let mut formatted = Vec::with_capacity(attestation.len());

			for ((_, cred_type), val) in schema.iter().zip(attestation) {
				let SizeInBytes::Limited(expected_len) = cred_type.size_in_bytes();
				if val.is_empty() || val.len() > (expected_len as usize) {
					return None;
				}

				let mut formatted_val = val.clone();
				if *cred_type != CredType::Text && val.len() != (expected_len as usize) {
					formatted_val.resize(expected_len as usize, 0);
				}

				if *cred_type == CredType::Hash {
					// For Hash type, ensure it's exactly 32 bytes or can be parsed as a valid hex string
					let is_valid_hash = match val.len() {
						32 => true, // Raw 32-byte hash, valid as is
						33..=64 => {
							// Might be a hex string without 0x prefix
							if let Ok(hex_str) = core::str::from_utf8(val) {
								hex::decode(hex_str).map_or(false, |decoded| decoded.len() == 32)
							} else {
								false
							}
						}
						65..=66 if val.starts_with(b"0x") => {
							// Might be a hex string with 0x prefix
							if let Ok(hex_str) = core::str::from_utf8(&val[2..]) {
								hex::decode(hex_str).map_or(false, |decoded| decoded.len() == 32)
							} else {
								false
							}
						}
						_ => false,
					};

					if !is_valid_hash {
						return None; // Invalid hash format
					}
				}

				formatted.push(
					BoundedVec::try_from(formatted_val)
						.map_err(|_| Error::<T>::SchemaFieldTooLarge)
						.ok()?
				);
			}

			CredAttestation::<T>::try_from(formatted).ok()
		}

		pub fn is_valid_solana_address(address: Vec<u8>) -> bool {
			match address.try_into() {
				Ok(address_array) => { VerifyingKey::from_bytes(&address_array).is_ok() }
				Err(_) => false,
			}
		}

		pub fn check_valid_substrate_address(address: &[u8]) -> Option<AccountId32> {
			let address_array: [u8; 32] = address.try_into().ok()?;
			let account_id = AccountId32::new(address_array);
			AccountId32::from_ss58check(&account_id.to_ss58check())
				.ok()
				.map(|_| account_id)
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
				}
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
			// Try to parse as Substrate SS58 string first
			if let Ok(address_str) = core::str::from_utf8(&address) {
				if let Ok(account_id32) = AccountId32::from_ss58check(address_str) {
					return Ok(AcquirerAddress::Substrate(account_id32));
				}
			}

			// Try to handle Ethereum address (if it starts with 0x)
			if address.starts_with(b"0x") {
				let hex_str = core::str
					::from_utf8(&address[2..])
					.map_err(|_| Error::<T>::InvalidAddress)?;
				let bytes = hex::decode(hex_str).map_err(|_| Error::<T>::InvalidAddress)?;
				if bytes.len() == 20 {
					let mut array = [0u8; 20];
					array.copy_from_slice(&bytes);
					return Ok(AcquirerAddress::Ethereum(H160::from(array)));
				}
			}

			// Try to parse as raw Ethereum address (20 bytes)
			if address.len() == 20 {
				let mut array = [0u8; 20];
				array.copy_from_slice(&address);
				return Ok(AcquirerAddress::Ethereum(H160::from(array)));
			}

			// Try to parse as Solana address
			if let Some(solana_addr) = Self::check_solana_address(address.clone()) {
				return Ok(AcquirerAddress::Solana(solana_addr));
			}

			// Try as raw Substrate address (32 bytes) as last resort
			if let Some(account_id) = Self::check_valid_substrate_address(&address) {
				return Ok(AcquirerAddress::Substrate(account_id));
			}

			Err(Error::<T>::InvalidAddress.into())
		}

		pub fn get_attestation(attestation_id: &AttestationId<T>) -> Option<Attestation<T>> {
			Attestations::<T>::get(attestation_id)
		}

		pub fn is_attestation_valid(
			attestation_id: &AttestationId<T>
		) -> Result<bool, DispatchError> {
			let attestation = Self::get_attestation(attestation_id).ok_or(
				Error::<T>::AttestationNotFound
			)?;

			let current_time = BlockTime {
				time: pallet_timestamp::Pallet::<T>::now(),
				block_number: frame_system::Pallet::<T>::block_number(),
			};

			Ok(attestation.is_valid_at(&current_time))
		}

		// Get all attestations for a specific user, issuer, and schema
		pub fn get_user_attestations(
			account: &AcquirerAddress,
			issuer_hash: &IssuerHash<T>,
			schema_hash: &SchemaHash<T>
		) -> Vec<(AttestationId<T>, Attestation<T>)> {
			let next_id = AttestationNextId::<T>::get((
				account.clone(),
				*issuer_hash,
				*schema_hash,
			));
			let mut attestations = Vec::new();

			for index in 0..next_id {
				let mut bytes = Vec::new();
				bytes.extend_from_slice(&account.encode());
				bytes.extend_from_slice(&issuer_hash.encode());
				bytes.extend_from_slice(&schema_hash.encode());
				bytes.extend_from_slice(&index.encode());

				let attestation_id = <T as Config>::Hashing::hash(&bytes);

				if let Some(attestation) = Self::get_attestation(&attestation_id) {
					attestations.push((attestation_id, attestation));
				}
			}

			attestations
		}
	}
}
