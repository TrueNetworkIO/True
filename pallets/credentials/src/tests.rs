#![cfg(test)]
// Tests for Credentials Pallet

use super::*;
use crate::{
	self as pallet_credentials
};
use pallet_issuers;
use pallet_balances;

use codec::{Decode, Encode};
use frame_support::{
	assert_err, assert_noop, assert_ok, derive_impl, parameter_types,
	traits::{ConstU32, ConstU64},
	BoundedVec,
};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_io::crypto::{sr25519_generate, sr25519_sign};
use sp_runtime::{
	traits::{BadOrigin, BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature, MultiSigner,
};

use frame_support::traits::{tokens::ExistenceRequirement, Currency};

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;
pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

type Block = frame_system::mocking::MockBlock<Test>;

type Balance = <Test as pallet_balances::Config>::Balance;
type AccountData = pallet_balances::AccountData<Balance>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		CredentialsModule: pallet_credentials,
        Issuers: pallet_issuers,
        Balances: pallet_balances,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = AccountData;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type Block = Block;
    type RuntimeTask = ();
}

parameter_types! {
	pub const MaxSchemaFields: u32 = 20;
	pub const MaxSchemaFieldSize: u32 = 120;
    pub const MaxNameLength: u32 = 120;
	pub const MaxControllers: u32 = 20;
    pub const IssuerRegistryDeposit: u128 = 1_000_000_000_000;
    pub const ExistentialDeposit: u64 = 1;
    pub const MaxFreezes: u32 = 1;
}

impl pallet_balances::Config for Test {
    type Balance = u128;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = frame_system::Pallet<Test>;
    type WeightInfo = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type FreezeIdentifier = ();
    type MaxFreezes = MaxFreezes;
}

impl pallet_issuers::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Hashing = BlakeTwo256;

	type MaxNameLength = MaxNameLength;
	type MaxControllers = MaxControllers;

    type WeightInfo = pallet_issuers::weights::SubstrateWeight<Test>;
    type Currency = Balances;

    type IssuerRegistryDeposit = IssuerRegistryDeposit;
}

impl pallet_credentials::Config  for Test {
	type RuntimeEvent = RuntimeEvent;
	type Hashing = BlakeTwo256;

	type MaxSchemaFields = MaxSchemaFields;
	type MaxSchemaFieldSize = MaxSchemaFieldSize;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}