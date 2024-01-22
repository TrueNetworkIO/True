#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
use frame_support::dispatch::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{dispatch, dispatch::*, pallet_prelude::*};
	use frame_system::pallet_prelude::*;
	use scale_info::prelude;
	use sp_runtime::{FixedI64, FixedPointNumber, Rounding};
	use wasmi::{self, core::F64, Value};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
	}

	// The pallet's runtime storage items.
	// https://docs.substrate.io/main-docs/build/runtime-storage/
	#[pallet::storage]
	#[pallet::getter(fn something)]
	// Learn more about declaring storage items:
	// https://docs.substrate.io/main-docs/build/runtime-storage/#declaring-storage-items
	pub type Something<T> = StorageValue<_, u32>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		SomethingStored {
			something: u32,
			who: T::AccountId,
		},
		AlgoResult {
			result: i64,
		},
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		AlgoError1,
		AlgoError2,
		AlgoError3,
		AlgoError4,
		AlgoError5,
		AlgoError6,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::do_something())]
		pub fn do_something(origin: OriginFor<T>, something: u32) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/main-docs/build/origins/
			let who = ensure_signed(origin)?;

			// Update storage.
			<Something<T>>::put(something);

			// Emit an event.
			Self::deposit_event(Event::SomethingStored { something, who });
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		/// An example dispatchable that may throw a custom error.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::cause_error())]
		pub fn cause_error(origin: OriginFor<T>) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			// Read a value from storage.
			match <Something<T>>::get() {
				// Return an error if the value has not been set.
				None => return Err(Error::<T>::NoneValue.into()),
				Some(old) => {
					// Increment the value read from storage; will error in the event of overflow.
					let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
					// Update the value in storage with the incremented result.
					<Something<T>>::put(new);
					Ok(())
				},
			}
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::cause_error())]
		pub fn run_algo(origin: OriginFor<T>, a: i32, b: i32, wasm: Vec<u8>) -> DispatchResult {
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
	}
}
