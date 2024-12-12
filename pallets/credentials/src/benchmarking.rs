#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{v2::*, whitelisted_caller, BenchmarkError};
use frame_support::{BoundedVec, ensure, traits::Get};
use frame_system::RawOrigin;
use sp_std::{vec, iter};
use sp_std::vec::Vec;
use sp_runtime::traits::Hash;
use codec::Encode;
use sp_core::{crypto::{Ss58Codec}};
use sp_runtime::AccountId32;



#[benchmarks]
mod benchmarks {
    use super::*;

    fn generate_issuer<T: Config>(controllers: Vec<T::AccountId>) -> T::Hash{
        let name_length = T::MaxNameLength::get();
        let issuer_name = BoundedVec::<u8, T::MaxNameLength>::try_from(vec![1; name_length as usize]).expect("name too long");
        let issuer_hash = <T as Config>::Hashing::hash(&issuer_name);
        let bounded_controllers = BoundedVec::<T::AccountId, T::MaxControllers>::try_from(controllers).expect("too many controllers");
        pallet_issuers::Issuers::<T>::insert(issuer_hash, pallet_issuers::Issuer::<T> { name: issuer_name, controllers: bounded_controllers });
        issuer_hash
    }

    fn generate_schema<T: Config>(max_num_fields: usize, max_field_size: usize) -> T::Hash {

        let max_num_fields = max_num_fields.min(T::MaxSchemaFields::get() as usize);

        let field_name: BoundedVec<u8, T::MaxSchemaFieldSize> = BoundedVec::try_from(vec![b'a'; max_field_size]).expect("schema field name too long");

        let schema: BoundedVec<(BoundedVec<u8, T::MaxSchemaFieldSize>, CredType), T::MaxSchemaFields> = BoundedVec::try_from(vec![(field_name, CredType::Hash); max_num_fields]).expect("too many schema fields");

        let bytes: Vec<u8> = schema.iter()
        .flat_map(|(vec, cred_type)| {
            let mut bytes = vec.to_vec();
            bytes.extend_from_slice(&cred_type.encode());
            bytes
        })
        .collect();

        let schema_hash = <T as Config>::Hashing::hash(&bytes);

        Schemas::<T>::insert(schema_hash, schema.clone());

        schema_hash
    }

    fn get_hash_attestation<T: Config>(num_fields: usize) -> Vec<Vec<u8>>{
        let max_fields = num_fields.min(T::MaxSchemaFields::get() as usize);
        let hash_value = <T as Config>::Hashing::hash(&[42u8]).as_ref().to_vec();

        vec![hash_value; max_fields]
    }

    fn generate_test_address<T: Config>(address_type: usize) -> Vec<u8> {
        match address_type % 3 {
            0 => {
                // Ethereum address (20 bytes)
                let mut eth_addr = [0u8; 20];
                for i in 0..20 {
                    eth_addr[i] = i as u8;
                }
                eth_addr.to_vec()
            },
            1 => {
                // Solana address (base58 encoded public key, typically 32 or 44 bytes)
                // Using a valid base58 encoded Solana pubkey
                "7QX6LJUz7LyEm7fRNMpLhRk2ye5h3k6JDJ3VXZsqnFan".as_bytes().to_vec()
            },
            _ => {
                // Substrate address 
                // let account: T::AccountId = whitelisted_caller();
                // account.encode()
                "15LvsPtVtXB3Xr3yk3WGLczSk7jdzhvPf22C1e7ceiCHkkjU".to_string().into_bytes()
            }
        }
    }


    // Benchmark `create_schema` extrinsic
    #[benchmark]
    fn create_schema(x: Linear<1, { T::MaxSchemaFields::get() - 1 }>) -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();

        let issuer_hash = generate_issuer::<T>(vec![caller.clone()]);

        ensure!(pallet_issuers::Issuers::<T>::contains_key(issuer_hash.clone()), "Issuer did not get created.");
        
       // Prepare a schema with maximum field name size
       let max_field_size = T::MaxSchemaFieldSize::get() as usize;
       let num_fields = x as usize;

       let field_name: Vec<u8> = vec![b'a'; max_field_size];

       let schema: Vec<(Vec<u8>, CredType)> = vec![(field_name, CredType::Hash); num_fields];

        #[extrinsic_call]
        create_schema(RawOrigin::Signed(caller.clone()), issuer_hash, schema.clone());

        let bytes: Vec<u8> = schema.iter()
        .flat_map(|(vec, cred_type)| {
            let mut bytes = vec.to_vec();
            bytes.extend_from_slice(&cred_type.encode());
            bytes
        })
        .collect();

        let schema_hash = <T as Config>::Hashing::hash(&bytes);
        
        ensure!(Schemas::<T>::contains_key(schema_hash), "Schema did not get registered");

        Ok(())
    }

    #[benchmark]
    fn attest(
        x: Linear<1, { T::MaxSchemaFields::get() - 1 }>,       // Number of hash fields (1 to 100)
        y: Linear<0, 3> 
    ) -> Result<(), BenchmarkError>{

        // create issuer
        let caller: T::AccountId = whitelisted_caller();

        let max_field_size = T::MaxSchemaFieldSize::get() as usize;

        let issuer_hash = generate_issuer::<T>(vec![caller.clone()]);

        // create schema
        let schema_hash = generate_schema::<T>(x as usize, max_field_size);

        // create attestation
        let attestation = get_hash_attestation::<T>(x as usize);

        // create for account

        let for_account: Vec<u8> = generate_test_address::<T>(y as usize);

        ensure!(!for_account.is_empty(), "Invalid account generation");


        #[extrinsic_call]
        attest(RawOrigin::Signed(caller), issuer_hash, schema_hash, for_account, attestation);

        Ok(())

    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::tests::new_test_ext(),
        crate::tests::Test,
    );
}