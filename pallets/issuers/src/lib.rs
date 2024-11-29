#![cfg_attr(not(feature = "std"), no_std)]


pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::{*, OptionQuery};
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Hash;
    use sp_std::prelude::*;

    use super::*;

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct Issuer<T: Config> {
        pub name: BoundedVec<u8, T::MaxNameLength>,
        pub controllers: BoundedVec<T::AccountId, T::MaxControllers>,
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

        #[pallet::constant]
        type MaxNameLength: Get<u32>;

        #[pallet::constant]
        type MaxControllers: Get<u32>;
    }

    #[pallet::storage]
    pub type Issuers<T: Config> =
    StorageMap<_, Blake2_128Concat, T::Hash, Issuer<T>, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        IssuerCreated { hash: T::Hash, issuer_name: BoundedVec<u8, T::MaxNameLength> , controllers_identified: BoundedVec<T::AccountId, T::MaxControllers>},
        IssuerUpdated { hash: T::Hash, issuer_name: BoundedVec<u8, T::MaxNameLength> , controllers_identified: BoundedVec<T::AccountId, T::MaxControllers>},
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        IssuerAlreadyExists,
        IssuerNotFound,
        NotAuthorized,
        IssuerNameTooLong,
        TooManyControllers
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

            //TODO: trim trailing spaces from spaces from issuer name

            ensure!(name.len() <= T::MaxNameLength::get() as usize, Error::<T>::IssuerNameTooLong);

            let hash = <T as Config>::Hashing::hash(&name);

            ensure!(!Issuers::<T>::contains_key(hash), Error::<T>::IssuerAlreadyExists);

            ensure!(controllers.len() <= T::MaxControllers::get() as usize, Error::<T>::TooManyControllers);

            let issuer_name = BoundedVec::<u8, T::MaxNameLength>::try_from(name)
            .map_err(|_| Error::<T>::IssuerNameTooLong)?;

            let controllers_identified =  BoundedVec::<T::AccountId, T::MaxControllers>::try_from(controllers)
            .map_err(|_| Error::<T>::TooManyControllers)?;
            

            // let issuer = Issuer::<T> { name: issuer_name.clone(), controllers: controllers_identified.clone() };
            Issuers::<T>::insert(hash, Issuer::<T> { name: issuer_name.clone(), controllers: controllers_identified.clone() });
            Self::deposit_event(Event::IssuerCreated { hash, issuer_name, controllers_identified });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(100_000)]
        pub fn edit_controllers(
            origin: OriginFor<T>,
            hash: T::Hash,
            controllers: Option<Vec<T::AccountId>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut issuer = Issuers::<T>::get(hash)
                .ok_or(Error::<T>::IssuerNotFound)?;
            let mut hash = hash;


            ensure!(issuer.controllers.contains(&who), Error::<T>::NotAuthorized);

            // if let Some(name) = name {
            //     ensure!(name.len() <= T::MaxNameLength::get() as usize, Error::<T>::IssuerNameTooLong);
            //     hash = <T as Config>::Hashing::hash(&name);
            //     ensure!(!Issuers::<T>::contains_key(hash), Error::<T>::IssuerAlreadyExists);
            //     issuer.name = BoundedVec::<u8, T::MaxNameLength>::try_from(name)
            //     .map_err(|_| Error::<T>::IssuerNameTooLong)?;
            // }

            if let Some(controllers) = controllers {
                ensure!(controllers.len() <= T::MaxControllers::get() as usize, Error::<T>::TooManyControllers);
                issuer.controllers = BoundedVec::<T::AccountId, T::MaxControllers>::try_from(controllers)
                .map_err(|_| Error::<T>::TooManyControllers)?;
            }

            Issuers::<T>::insert(hash, Issuer::<T> { name: issuer.name.clone(), controllers: issuer.controllers.clone() });

            Self::deposit_event(Event::IssuerUpdated { hash,  issuer_name: issuer.name, controllers_identified: issuer.controllers});

            Ok(())
        }
    }
}
