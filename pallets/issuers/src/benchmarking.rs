#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{v2::*, whitelisted_caller, BenchmarkError};
use frame_support::{BoundedVec, ensure, traits::Get};
use frame_system::RawOrigin;
use sp_std::vec;
use sp_std::vec::Vec;
use sp_runtime::traits::Hash;


#[benchmarks]
mod benchmarks {
    use super::*;

    fn generate_controllers<T: Config>(n: u32) -> Vec<T::AccountId> {
        let caller: T::AccountId = whitelisted_caller();
        vec![caller.clone(); n as usize]
    }

    // Benchmark `create_issuer` extrinsic
    #[benchmark]
    fn create_issuer(n: Linear<1, { T::MaxNameLength::get() - 1 }>, c: Linear<1, { T::MaxControllers::get() - 1}>) -> Result<(), BenchmarkError> {
        // Setup: Create test data
        let caller: T::AccountId = whitelisted_caller();
        let name: Vec<u8> = vec![1; n as usize];
        let controllers = generate_controllers::<T>(c);

        #[extrinsic_call]
        create_issuer(RawOrigin::Signed(caller), name.clone(), controllers);

        // Verify the issuer was created
        let hash = <T as Config>::Hashing::hash(&name);
        ensure!(Issuers::<T>::contains_key(hash), "Issuer did not get created.");

        Ok(())
    }

    // Benchmark `edit_controllers` extrinsic
    #[benchmark]
    fn edit_controllers(c: Linear<1, { T::MaxControllers::get() - 1}>) -> Result<(), BenchmarkError> {
        
        // Setup: create an issuer
        let caller: T::AccountId = whitelisted_caller();
        let name: Vec<u8> = vec![1; T::MaxNameLength::get() as usize];
        let initial_controllers: Vec<T::AccountId> = vec![caller.clone(); T::MaxControllers::get() as usize];
        
        let hash = <T as Config>::Hashing::hash(&name);
        
        // Create initial issuer
        let issuer_name = BoundedVec::<u8, T::MaxNameLength>::try_from(name).unwrap();
        let controllers_identified = BoundedVec::<T::AccountId, T::MaxControllers>::try_from(initial_controllers).unwrap();
        let issuer = Issuer::<T> { 
            name: issuer_name,
            controllers: controllers_identified,
        };
        Issuers::<T>::insert(hash, issuer);

        // Create new controllers for the update
        let new_controllers: Vec<T::AccountId> = generate_controllers::<T>(c);
        let new_controllers_clone = new_controllers.clone();

        #[extrinsic_call]
        edit_controllers(
            RawOrigin::Signed(caller),
            hash,
            Some(new_controllers),
        );

        // Verify the issuer was updated
        ensure!(Issuers::<T>::contains_key(hash), "issuer got removed");

        let stored_issuer = Issuers::<T>::get(hash);

        ensure!(
            stored_issuer.unwrap().controllers ==
            BoundedVec::<T::AccountId, T::MaxControllers>::try_from(new_controllers_clone).unwrap(),
            "Controllers were not updated correctly"
        );

        Ok(())

    }

    impl_benchmark_test_suite!(
        IssuersModule,
        crate::tests::new_test_ext(),
        crate::tests::Test,
    );
}