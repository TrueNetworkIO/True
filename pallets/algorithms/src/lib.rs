#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use log;
    use frame_support::{dispatch, dispatch::*, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use scale_info::prelude;
    use sp_runtime::{FixedI64, FixedPointNumber, Rounding};
    use wasmi::{self, core::F64, Value};
    use sp_runtime::Vec;
    use sp_runtime::traits::Hash;
    use wasmi::{Func, Caller};
    use pallet_credentials::Schemas;
    use wasmi::core::Trap;

    use pallet_credentials::{self as credentials, Attestations, CredAttestation, CredSchema, AcquirerAddress};

    use super::*;

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct GasMeter {
        pub consumed: u64,
        pub limit: u64,
    }

    impl GasMeter {
        pub fn new(limit: u64) -> Self {
            Self { 
                consumed: 0, 
                limit 
            }
        }

        pub fn charge(&mut self, amount: u64) -> Result<(), DispatchError> {
            self.consumed = self.consumed.checked_add(amount)
                .ok_or(DispatchError::Other("Gas Overflow"))?;

            if self.consumed > self.limit {
                return Err(DispatchError::Other("Out of Gas"));
            }
            Ok(())
        }
    }

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct Algorithm<T: Config> {
        pub schema_hashes: BoundedVec<T::Hash, T::MaxSchemas>,
        pub code: BoundedVec<u8, T::MaxCodeSize>,
        pub gas_limit: u64,
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_issuers::Config + pallet_credentials::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Hashing: Hash<Output = Self::Hash>;

        #[pallet::constant]
        type MaxSchemas: Get<u32>;

        #[pallet::constant]
        type MaxCodeSize: Get<u32>;

        #[pallet::constant]
        type MaxMemoryPages: Get<u32>;

        #[pallet::constant]
        type DefaultGasLimit: Get<u64>;

        #[pallet::constant]
        type GasCost: Get<GasCosts>;
    }

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct GasCosts {
        pub basic_op: u64,
        pub memory_op: u64,
        pub call_op: u64,
    }

    #[pallet::storage]
    pub type Algorithms<T: Config> =
    StorageMap<_, Blake2_128Concat, u64 /*algoId*/, Algorithm<T>, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultNextAlgoId<T: Config>() -> u64 { 100u64 }

    #[pallet::storage]
    pub type NextAlgoId<T: Config> = StorageValue<_, u64, ValueQuery, DefaultNextAlgoId<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        AlgorithmAdded {
            algorithm_id: u64,
            schema_hashes: Vec<T::Hash>,
        },
        AlgoResult {
            result: i64,
            issuer_hash: T::Hash, 
            account_id: Vec<u8>
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        AlgoNotFound,
        AttestationNotFound,
        AcmMemoryWriteError,
        AcmSetupFailed,
        AcmLinkerFailed,
        AcmFailedToStart,
        AcmFailedToFindCalcFunction,
        AcmFailedToCalculate,
        InvalidWasmProvided,
        TooManySchemas,
        CodeTooHeavy,
        SchemaNotFound,

        AlgoExecutionFailed,
        TooComplexModule,
        OutOfGas,
        GasOverflow,
        GasMeteringNotSupported,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(1)]
        #[pallet::weight(100_000)]
        pub fn save_algo(origin: OriginFor<T>, schema_hashes: Vec<T::Hash>, code: Vec<u8>, gas_limit: Option<u64>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(schema_hashes.len() <= T::MaxSchemas::get() as usize, Error::<T>::TooManySchemas);

            ensure!(code.len() <= T::MaxCodeSize::get() as usize, Error::<T>::CodeTooHeavy);

            let engine = wasmi::Engine::default();
    
            // Just validate without storing the module
            wasmi::Module::new(&engine, code.as_slice())
                .map_err(|_| Error::<T>::InvalidWasmProvided)?;

            let id = NextAlgoId::<T>::get();
            NextAlgoId::<T>::set(id + 1);



            Algorithms::<T>::insert(id, Algorithm {
                schema_hashes: BoundedVec::try_from(schema_hashes.clone()).map_err(|_| Error::<T>::TooManySchemas)?,
                code: BoundedVec::try_from(code).map_err(|_| Error::<T>::CodeTooHeavy)?,
                gas_limit: gas_limit.unwrap_or_else(|| T::DefaultGasLimit::get()),
            });

            Self::deposit_event(Event::AlgorithmAdded {
                algorithm_id: id,
                schema_hashes: schema_hashes,
            });

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(100_000)]
        pub fn run_algo_for(origin: OriginFor<T>, issuer_hash: T::Hash, account_id: Vec<u8>, algorithm_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let acquirer_address = credentials::Pallet::<T>::parse_acquirer_address(account_id.clone())?;

            let algorithm = Algorithms::<T>::get(algorithm_id).ok_or(Error::<T>::AlgoNotFound)?;

            let mut attestations: Vec<pallet_credentials::CredAttestation<T>> = Vec::<>::with_capacity(algorithm.schema_hashes.len());
            
            // For each schema, get the latest attestation
            for schema_hash in &algorithm.schema_hashes {
              let attestations_for_schema = Attestations::<T>::get(
                  (acquirer_address.clone(), issuer_hash, *schema_hash)
              ).ok_or(Error::<T>::AttestationNotFound)?;

              // Check if there are any attestations
              ensure!(!attestations_for_schema.is_empty(), Error::<T>::AttestationNotFound);

              // Get the latest attestation (last element in the vector)

              let schema = Schemas::<T>::get(schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
            
              // Get text field indices
              let text_indices: Vec<usize> = schema.iter()
                  .enumerate()
                  .filter_map(|(idx, (_, cred_type))| {
                      if *cred_type == credentials::CredType::Text {
                          Some(idx)
                      } else {
                          None
                      }
                  })
                  .collect();

              // Get the latest attestation and remove text fields
              let mut latest_attestation = attestations_for_schema.last().unwrap().clone();
              
              // Remove text fields from highest index to lowest to maintain index validity
              for &index in text_indices.iter().rev() {
                  latest_attestation.remove(index);
              } 

              attestations.push(latest_attestation.clone());
            }

            match Self::run_code(algorithm.code.to_vec(), attestations, algorithm.gas_limit) {
              Ok(value) => {
                  Self::deposit_event(Event::AlgoResult {
                      result: value,
                      issuer_hash,
                      account_id,
                  });
                  Ok(())
              },
              Err(e) => {
                  log::error!(target: "algo", "Algo execution failed {:?}", e);
                  Err(Error::<T>::AlgoExecutionFailed.into())
              }
          }
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn run_code(code: Vec<u8>, attestations: Vec<CredAttestation<T>>, gas_limit: u64) -> Result<i64, Error<T>> {
            let engine = wasmi::Engine::default();

            let gas_meter = GasMeter::new(gas_limit);

            let module =
                wasmi::Module::new(&engine, code.as_slice()).map_err(|_| Error::<T>::InvalidWasmProvided)?;

            let mut store = wasmi::Store::new(&engine, gas_meter);
            
            let host_print = wasmi::Func::wrap(
                &mut store,
                |mut caller: wasmi::Caller<'_, GasMeter>, param: i32| {
                    caller.data_mut().charge(T::GasCost::get().basic_op).map_err(|_| Trap::new("Gas charge failed"))?;
                    log::debug!(target: "algo", "Message:{:?}", param);
                    Ok(())
                },
            );

            let abort_func = wasmi::Func::wrap(
              &mut store,
              |mut caller: Caller<'_, GasMeter>, msg_id: i32, filename: i32, line: i32, col: i32| -> Result<(), Trap> {
                  caller.data_mut().charge(T::GasCost::get().call_op).map_err(|_| Trap::new("Gas charge failed"))?;
                  log::error!(
                      target: "algo",
                      "Abort called: msg_id={}, file={}, line={}, col={}",
                      msg_id, filename, line, col
                  );
                  Err(Trap::new("Gas charge failed"))
              },
            );

            let memory = wasmi::Memory::new(
                &mut store,
                wasmi::MemoryType::new(T::MaxMemoryPages::get(), Some(T::MaxMemoryPages::get())).map_err(|_| Error::<T>::AcmSetupFailed)?,
            )
                .map_err(|_| Error::<T>::AcmSetupFailed)?;

                // TODO (IMP)
             // get schema indexes for text (CredType::Text) property
             // remove the attestation indexes at schema indexes.    

            let bytes = attestations.into_iter().flatten().flatten().collect::<Vec<u8>>();

            memory.write(&mut store, 0, &bytes).map_err(|e| {
                log::error!(target: "algo", "Memory write error {:?}", e);
                Error::<T>::AcmMemoryWriteError
            })?;

            store.data_mut().charge(
                T::GasCost::get().memory_op * (bytes.len() as u64 / 32 + 1))
                .map_err(|_| Error::<T>::OutOfGas)?;

            // memory.write(&mut store, 0, 5);

            let mut linker = <wasmi::Linker<GasMeter>>::new(&engine);
            linker.define("host", "print", host_print).map_err(|_| Error::<T>::AcmSetupFailed)?;
            linker.define("env", "memory", memory).map_err(|_| Error::<T>::AcmSetupFailed)?;
      
            // Define the abort function in the linker
            linker.define("env", "abort", abort_func).map_err(|_| Error::<T>::AcmSetupFailed)?;

            log::error!(target: "algo", "Algo3 {:?}", bytes.clone());
            log::error!(target: "algo", "Algo3 {:?}", bytes.len());

            let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| {
                log::error!(target: "algo", "Instantiation error {:?}", e);
                Error::<T>::AcmLinkerFailed
            })?
            .start(&mut store)
            .map_err(|_| Error::<T>::AcmFailedToStart)?;

            let calc = instance
                .get_typed_func::<(), i64>(&store, "calc")
                .map_err(|_| Error::<T>::AcmFailedToFindCalcFunction)?;

            // And finally we can call the wasm!
            let result = calc.call(&mut store, ()).map_err(|e| {
                log::error!(target: "algo", "Execution error {:?}", e);
                Error::<T>::AcmFailedToCalculate
            })?;

            Ok(result)
        }
    }
}
