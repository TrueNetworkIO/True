use frame_support::pallet_prelude::*;
use sp_std::marker::PhantomData;
use frame_system;
use super::*;
use sp_std::vec::Vec;
use sp_std::boxed::Box;

/// Types of extensions available for schemas
#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
pub enum ExtensionType {
    Expiry,
    // Add future extension types here
}

/// Data structure for Expiry extension
#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
pub struct ExpiryData {
    pub expiry_block: u64,
}

/// All possible extension data types
#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
pub enum ExtensionData {
    Expiry(ExpiryData),
    // Add future extension data types here
}

// Convert ExtensionData to its corresponding ExtensionType
impl ExtensionData {
    pub fn get_type(&self) -> ExtensionType {
        match self {
            ExtensionData::Expiry(_) => ExtensionType::Expiry,
        }
    }

    pub fn get_extension_data(&self) -> Option<ExpiryData> {
        match self {
            ExtensionData::Expiry(data) => Some(data.clone()),
            _ => None,
        }
    }
}

/// Trait that all extensions must implement
pub trait SchemaExtension<T: Config> {
    /// Validates extension data when creating or updating attestation
    fn validate_attestation(
        &self,
        attestation: &CredAttestation<T>,
        extension_data: ExtensionData,
    ) -> Result<(), DispatchError>;
    
    /// Filters attestations when retrieving them
    fn filter_attestation(
        &self,
        attestation: &CredAttestation<T>,
        extension_data: ExtensionData,
    ) -> bool;
}

/// Implementation for Expiry extension
pub struct ExpiryExtension<T>(pub PhantomData<T>);

impl<T: Config> SchemaExtension<T> for ExpiryExtension<T> {
    fn validate_attestation(
        &self,
        _attestation: &CredAttestation<T>,
        extension_data: ExtensionData,
    ) -> Result<(), DispatchError> {
        // Extract expiry data
        let expiry_data = extension_data.get_extension_data()
            .ok_or(Error::<T>::InvalidExtensionData)?;

        let current_block = TryInto::<u64>::try_into(
            frame_system::Pallet::<T>::block_number()
        ).unwrap_or(0);

        ensure!(
            expiry_data.expiry_block >= current_block,
            Error::<T>::ExpiryInPast
        );

        Ok(())
    }

    fn filter_attestation(
        &self,
        _attestation: &CredAttestation<T>,
        extension_data: ExtensionData,
    ) -> bool {
        if let Some(expiry_data) = extension_data.get_extension_data() {
            let current_block = TryInto::<u64>::try_into(
                frame_system::Pallet::<T>::block_number()
            ).unwrap_or(0);

            return current_block <= expiry_data.expiry_block;
        }
        false
    }
}

/// Helper functions to work with extensions
impl ExtensionType {
    /// Get extension implementation for the given type
    pub fn get_implementation<T: Config>(&self) -> Box<dyn SchemaExtension<T>> {
        match self {
            ExtensionType::Expiry => Box::new(ExpiryExtension::<T>(PhantomData)),
        }
    }
}
