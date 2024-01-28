#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{dispatch, dispatch::*, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::prelude;
    use sp_runtime::{FixedI64, FixedPointNumber, Rounding};
    use wasmi::{self, core::F64, Value};
    use frame_support::dispatch::Vec;

    use pallet_credentials::{Attestations, CredAttestation, CredSchema};

    use super::*;

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct Algorithm {
        pub schema_ids: Vec<u64>,
        pub code: Vec<u8>,
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_issuers::Config + pallet_credentials::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    pub type Algorithms<T: Config> =
    StorageMap<_, Blake2_128Concat, u64 /*algoId*/, Algorithm, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultNextAlgoId<T: Config>() -> u64 { 100u64 }

    #[pallet::storage]
    pub type NextAlgoId<T: Config> = StorageValue<_, u64, ValueQuery, DefaultNextAlgoId<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        AlgorithmAdded {
            algorithm_id: u64,
        },
        AlgoResult {
            result: i64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        AlgoNotFound,
        AttestationNotFound,
        AlgoError1,
        AlgoError2,
        AlgoError3,
        AlgoError4,
        AlgoError5,
        AlgoError6,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(100_000)]
        pub fn run_algo(origin: OriginFor<T>, a: i32, b: i32, wasm: Vec<u8>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let engine = wasmi::Engine::default();

            let module =
                wasmi::Module::new(&engine, wasm.as_slice()).map_err(|_| Error::<T>::AlgoError1)?;

            type HostState = u32;
            let mut store = wasmi::Store::new(&engine, 42);
            let host_print = wasmi::Func::wrap(
                &mut store,
                |caller: wasmi::Caller<'_, HostState>, param: i32| {
                    log::debug!(target: "algo", "Message:{:?}", param);
                },
            );
            let memory = wasmi::Memory::new(
                &mut store,
                wasmi::MemoryType::new(8, None).map_err(|_| Error::<T>::AlgoError2)?,
            )
                .map_err(|_| Error::<T>::AlgoError2)?;

            memory.write(&mut store, 0, &a.to_ne_bytes()).map_err(|e| {
                log::error!(target: "algo", "Algo1 {:?}", e);
                Error::<T>::AlgoError1
            })?;
            memory.write(&mut store, 4, &b.to_ne_bytes()).map_err(|e| {
                log::error!(target: "algo", "Algo1 {:?}", e);
                Error::<T>::AlgoError1
            })?;
            // memory.write(&mut store, 0, 5);

            let mut linker = <wasmi::Linker<HostState>>::new(&engine);
            linker.define("host", "print", host_print).map_err(|_| Error::<T>::AlgoError2)?;
            linker.define("env", "memory", memory).map_err(|_| Error::<T>::AlgoError2)?;

            let instance = linker
                .instantiate(&mut store, &module)
                .map_err(|e| {
                    log::error!(target: "algo", "Algo3 {:?}", e);
                    Error::<T>::AlgoError3
                })?
                .start(&mut store)
                .map_err(|_| Error::<T>::AlgoError4)?;

            let hello = instance
                .get_typed_func::<(), i64>(&store, "calc")
                .map_err(|_| Error::<T>::AlgoError5)?;

            // And finally we can call the wasm!
            let a = hello.call(&mut store, ()).map_err(|e| {
                log::error!(target: "algo", "Algo6 {:?}", e);
                Error::<T>::AlgoError6
            })?;
            Self::deposit_event(Event::AlgoResult {
                result: a,
            });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(100_000)]
        pub fn save_algo(origin: OriginFor<T>, schema_ids: Vec<u64>, code: Vec<u8>) -> DispatchResult {
            let who = ensure_signed(origin)?;


            let id = NextAlgoId::<T>::get();
            NextAlgoId::<T>::set(id + 1);

            Algorithms::<T>::insert(id, Algorithm {
                schema_ids,
                code,
            });

            Self::deposit_event(Event::AlgorithmAdded {
                algorithm_id: id,
            });

            Ok(())
        }


        #[pallet::call_index(2)]
        #[pallet::weight(100_000)]
        pub fn run_algo_for(origin: OriginFor<T>, account_id: T::AccountId, algorithm_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let algorithm = Algorithms::<T>::get(algorithm_id).ok_or(Error::<T>::AlgoNotFound)?;

            let mut attestations: Vec<pallet_credentials::CredAttestation> = Vec::<>::with_capacity(algorithm.schema_ids.len());

            for schema_id in algorithm.schema_ids {
                let attestation = Attestations::<T>::get(account_id.clone(), schema_id).ok_or(crate::pallet::Error::<T>::AttestationNotFound)?;
                attestations.push(attestation);
            }


            return Pallet::<T>::run_code(algorithm.code, attestations);
        }
    }


    impl<T: Config> Pallet<T> {
        pub fn run_code(code: Vec<u8>, attestations: Vec<CredAttestation>) -> DispatchResult {
            let engine = wasmi::Engine::default();

            let module =
                wasmi::Module::new(&engine, code.as_slice()).map_err(|_| Error::<T>::AlgoError1)?;

            type HostState = u32;
            let mut store = wasmi::Store::new(&engine, 42);
            let host_print = wasmi::Func::wrap(
                &mut store,
                |caller: wasmi::Caller<'_, HostState>, param: i32| {
                    log::debug!(target: "algo", "Message:{:?}", param);
                },
            );
            let memory = wasmi::Memory::new(
                &mut store,
                wasmi::MemoryType::new(8, None).map_err(|_| Error::<T>::AlgoError2)?,
            )
                .map_err(|_| Error::<T>::AlgoError2)?;


            let bytes = attestations.into_iter().flatten().flatten().collect::<Vec<u8>>();

            memory.write(&mut store, 0, &bytes).map_err(|e| {
                log::error!(target: "algo", "Algo1 {:?}", e);
                Error::<T>::AlgoError1
            })?;
            // memory.write(&mut store, 0, 5);

            let mut linker = <wasmi::Linker<HostState>>::new(&engine);
            linker.define("host", "print", host_print).map_err(|_| Error::<T>::AlgoError2)?;
            linker.define("env", "memory", memory).map_err(|_| Error::<T>::AlgoError2)?;

            let instance = linker
                .instantiate(&mut store, &module)
                .map_err(|e| {
                    log::error!(target: "algo", "Algo3 {:?}", e);
                    Error::<T>::AlgoError3
                })?
                .start(&mut store)
                .map_err(|_| Error::<T>::AlgoError4)?;

            let calc = instance
                .get_typed_func::<(), i64>(&store, "calc")
                .map_err(|_| Error::<T>::AlgoError5)?;

            // And finally we can call the wasm!
            let result = calc.call(&mut store, ()).map_err(|e| {
                log::error!(target: "algo", "Algo6 {:?}", e);
                Error::<T>::AlgoError6
            })?;
            Self::deposit_event(Event::AlgoResult {
                result,
            });

            Ok(())
        }
    }
}
