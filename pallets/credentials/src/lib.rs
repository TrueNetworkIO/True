#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::dispatch::Vec;
    use frame_support::log;
    use frame_support::pallet_prelude::{*, OptionQuery};
    use frame_system::pallet_prelude::*;
    use scale_info::prelude::vec;
    use sp_runtime::traits::Hash;

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
        type Hashing: Hash<Output=Self::Hash>;
    }

    #[pallet::storage]
    pub type Schemas<T: Config> =
    StorageMap<_, Blake2_128Concat, u64, CredSchema, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultNextSchemaId<T: Config>() -> u64 { 100u64 }

    #[pallet::storage]
    pub type NextSchemaId<T: Config> = StorageValue<_, u64, ValueQuery, DefaultNextSchemaId<T>>;

    #[pallet::storage]
    pub type Attestations<T: Config> =
    StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Twox64Concat, u64, CredAttestation, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        SchemaCreated { id: u64, schema: CredSchema },
        AttestationCreated { account_id: T::AccountId, schema_id: u64, attestation: CredAttestation },
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        SchemaNotFound,
        InvalidFormat,
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


            let issuer = Issuers::<T>::get(issuer_hash)
                .ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
            ensure!(issuer.controllers.contains(&who), pallet_issuers::Error::<T>::NotAuthorized);

            let id = NextSchemaId::<T>::get();
            NextSchemaId::<T>::set(id + 1);

            Schemas::<T>::insert(id, schema.clone());

            Self::deposit_event(Event::SchemaCreated { id, schema });


            Ok(())
        }


        #[pallet::call_index(2)]
        #[pallet::weight(100_000)]
        pub fn attest(
            origin: OriginFor<T>,
            issuer_hash: T::Hash,
            schema_id: u64,
            for_account: T::AccountId,
            attestation: CredAttestation,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;


            let issuer = Issuers::<T>::get(issuer_hash)
                .ok_or(pallet_issuers::Error::<T>::IssuerNotFound)?;
            ensure!(issuer.controllers.contains(&who), pallet_issuers::Error::<T>::NotAuthorized);

            let schema = Schemas::<T>::get(schema_id)
                .ok_or(Error::<T>::SchemaNotFound)?;

            let attestation = Pallet::<T>::validate_attestation(&schema, &attestation)
                .ok_or(Error::<T>::InvalidFormat)?;

            log::debug!(target: "algo", "Creds:{:?}", attestation);

            Attestations::<T>::insert(for_account.clone(), schema_id, attestation.clone());

            Self::deposit_event(Event::AttestationCreated { account_id: for_account, schema_id, attestation });

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
        pub fn validate_attestation(schema: &CredSchema, attestation: &CredAttestation) -> Option<CredAttestation> {
            if schema.len() != attestation.len() {
                return None;
            }

            let mut formatted = vec![vec![]; attestation.len()];

            for (((_, cred_type), val), i) in schema.iter().zip(attestation).zip(0..attestation.len()) {
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
    }
}

mod testt {
    #[test]
    pub fn test_a() {
        let bytes = (1i64).to_be_bytes();
        println!("{bytes:#04X?}\n{bytes:?}");

    }
}