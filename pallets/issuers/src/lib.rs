#![cfg_attr(not(feature = "std"), no_std)]


pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::{*, OptionQuery};
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Hash;
    use frame_support::dispatch::Vec;

    use super::*;

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct Issuer<AccountId> {
        pub name: Vec<u8>,
        pub controllers: Vec<AccountId>,
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Hashing: Hash<Output=Self::Hash>;
    }

    #[pallet::storage]
    pub type Issuers<T: Config> =
    StorageMap<_, Blake2_128Concat, T::Hash, Issuer<T::AccountId>, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        IssuerCreated { hash: T::Hash, issuer: Issuer<T::AccountId> },
        IssuerUpdated { hash: T::Hash, issuer: Issuer<T::AccountId> },
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        IssuerAlreadyExists,
        IssuerNotFound,
        NotAuthorized,
    }


    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(100_000)]
        pub fn create_issuer(
            origin: OriginFor<T>,
            name: Vec<u8>,
            controllers: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let hash = <T as Config>::Hashing::hash(&name);

            ensure!(!Issuers::<T>::contains_key(hash), Error::<T>::IssuerAlreadyExists);

            let issuer = Issuer::<T::AccountId> { name, controllers };
            Issuers::<T>::insert(hash, issuer.clone());
            Self::deposit_event(Event::IssuerCreated { hash, issuer });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(100_000)]
        pub fn edit_issuer(
            origin: OriginFor<T>,
            hash: T::Hash,
            name: Option<Vec<u8>>,
            controllers: Option<Vec<T::AccountId>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;


            let mut issuer = Issuers::<T>::get(hash)
                .ok_or(Error::<T>::IssuerNotFound)?;
            let mut hash = hash;


            ensure!(!issuer.controllers.contains(&who), Error::<T>::NotAuthorized);

            if let Some(name) = name {
                hash = <T as Config>::Hashing::hash(&name);
                ensure!(!Issuers::<T>::contains_key(hash), Error::<T>::IssuerAlreadyExists);
                issuer.name = name;
            }

            if let Some(controllers) = controllers {
                issuer.controllers = controllers;
            }

            Issuers::<T>::insert(hash, issuer.clone());
            Self::deposit_event(Event::IssuerUpdated { hash, issuer });

            Ok(())
        }
    }
}
