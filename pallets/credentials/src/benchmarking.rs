#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{v2::*, whitelisted_caller};
use frame_support::{ensure, traits::Get};
use frame_system::RawOrigin;
use sp_std::{vec, iter};
use sp_std::vec::Vec;
use sp_core::Hasher;
use sp_runtime::{BoundedVec, format};
use codec::Encode;

#[benchmarks]
mod benchmarks {
    use super::*;

    fn generate_field_value(cred_type: &CredType, size: usize) -> Vec<u8> {
        match cred_type {
            CredType::Text => vec![b'x'; size],
            CredType::Hash => vec![1u8; 32],
            CredType::U64 => vec![1u8; 8],
            CredType::Boolean => vec![1u8],
            _ => vec![0u8; size]
        }
    }

    fn generate_schema_fields<T: Config>(
        num_fields: usize,
        field_size: usize,
    ) -> Vec<(Vec<u8>, CredType)> {
        let mut schema = Vec::new();
        let field_types = [
            CredType::Text,
            CredType::Hash,
            CredType::U64,
            CredType::Boolean
        ];

        for i in 0..num_fields {
            let field_name = format!("field_{}", i).into_bytes();
            let field_type = field_types[i % field_types.len()].clone();
            schema.push((field_name, field_type));
        }
        schema
    }

    fn generate_attestation<T: Config>(
        schema: &Vec<(Vec<u8>, CredType)>,
        field_size: usize
    ) -> Vec<Vec<u8>> {
        schema.iter()
            .map(|(_, cred_type)| generate_field_value(cred_type, field_size))
            .collect()
    }

    fn create_test_issuer<T: Config>(caller: T::AccountId) -> T::Hash {
        let name = vec![1u8; T::MaxNameLength::get() as usize];
        let issuer_hash = <T as Config>::Hashing::hash(&name);
        let controllers = vec![caller];
        
        let bounded_name = BoundedVec::try_from(name).expect("name too long");
        let bounded_controllers = BoundedVec::try_from(controllers).expect("too many controllers");
        
        pallet_issuers::Issuers::<T>::insert(
            issuer_hash,
            pallet_issuers::Issuer { name: bounded_name, controllers: bounded_controllers }
        );
        
        issuer_hash
    }

    fn calculate_schema_hash<T: Config>(schema: &Vec<(Vec<u8>, CredType)>) -> T::Hash {
        let bytes: Vec<u8> = schema.iter()
            .flat_map(|(vec, cred_type)| {
                let mut bytes = vec.clone();
                bytes.extend_from_slice(&cred_type.encode());
                bytes
            })
            .collect();
        <T as Config>::Hashing::hash(&bytes)
    }

    fn generate_test_address(address_type: usize) -> Vec<u8> {
        match address_type % 3 {
            0 => "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".as_bytes().to_vec(),
            1 => "7FP6jcgXCh3D2gJAaV8Ewk3yVXwrVRmEcD38mSe4fWey".as_bytes().to_vec(),
            _ => "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".as_bytes().to_vec()
        }
    }

    #[benchmark]
    fn create_schema(
        f: Linear<1, { T::MaxSchemaFields::get() }>,    // Number of fields
        s: Linear<1, { T::MaxSchemaFieldSize::get() }>  // Field name size
    ) -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let issuer_hash = create_test_issuer::<T>(caller.clone());
        
        let schema = generate_schema_fields::<T>(f as usize, s as usize);

        #[extrinsic_call]
        create_schema(RawOrigin::Signed(caller), issuer_hash, schema);

        Ok(())
    }

    #[benchmark]
    fn attest(
        f: Linear<1, { T::MaxSchemaFields::get() }>,    // Number of fields
        s: Linear<1, { T::MaxSchemaFieldSize::get() }>, // Field value size
        a: Linear<0, 2>                                 // Address type (ETH/SOL/SUB)
    ) -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let issuer_hash = create_test_issuer::<T>(caller.clone());
        
        let schema = generate_schema_fields::<T>(f as usize, s as usize);
        let schema_hash = calculate_schema_hash::<T>(&schema);
        
        Pallet::<T>::create_schema(
            RawOrigin::Signed(caller.clone()).into(),
            issuer_hash,
            schema.clone()
        )?;

        let attestation = generate_attestation::<T>(&schema, s as usize);
        let for_account = generate_test_address(a as usize);

        #[extrinsic_call]
        attest(
            RawOrigin::Signed(caller),
            issuer_hash,
            schema_hash,
            for_account,
            attestation
        );

        Ok(())
    }

    #[benchmark]
    fn update_attestation(
        f: Linear<1, { T::MaxSchemaFields::get() }>,    // Number of fields
        s: Linear<1, { T::MaxSchemaFieldSize::get() }>, // Field value size
        n: Linear<1, 100>                               // Number of existing attestations
    ) -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let issuer_hash = create_test_issuer::<T>(caller.clone());
        
        let schema = generate_schema_fields::<T>(f as usize, s as usize);
        let schema_hash = calculate_schema_hash::<T>(&schema);
        
        // Create schema
        Pallet::<T>::create_schema(
            RawOrigin::Signed(caller.clone()).into(),
            issuer_hash,
            schema.clone()
        )?;

        // Create initial attestations
        let for_account = generate_test_address(0);
        let attestation = generate_attestation::<T>(&schema, s as usize);
        
        for _ in 0..n {
            Pallet::<T>::attest(
                RawOrigin::Signed(caller.clone()).into(),
                issuer_hash,
                schema_hash,
                for_account.clone(),
                attestation.clone()
            )?;
        }

        let new_attestation = generate_attestation::<T>(&schema, s as usize);

        #[extrinsic_call]
        update_attestation(
            RawOrigin::Signed(caller),
            issuer_hash,
            schema_hash,
            for_account,
            n - 1,
            new_attestation
        );

        Ok(())
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::tests::new_test_ext(),
        crate::tests::Test,
    );
}