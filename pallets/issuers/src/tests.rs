#[cfg(test)]
mod tests {
	use crate::{ self as pallet_issuers, Error, Issuer, Pallet, Event };
	use frame_support::{
		assert_noop,
		assert_ok,
		traits::{ ConstU16, ConstU32, ConstU64, ConstU128, Everything },
		BoundedVec,
	};
	use sp_core::H256;
	use sp_runtime::{ traits::{ BlakeTwo256, Hash, IdentityLookup }, BuildStorage };

	type Block = frame_system::mocking::MockBlock<Test>;
	type Balance = u128;

	frame_support::construct_runtime!(
        pub enum Test
        where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system,
            Balances: pallet_balances,
            IssuersModule: pallet_issuers,
        }
    );

	impl frame_system::Config for Test {
		type BaseCallFilter = Everything;
		type BlockWeights = ();
		type BlockLength = ();
		type DbWeight = ();
		type RuntimeOrigin = RuntimeOrigin;
		type RuntimeCall = RuntimeCall;
		type Nonce = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Block = Block;
		type RuntimeEvent = RuntimeEvent;
		type BlockHashCount = ConstU64<250>;
		type Version = ();
		type PalletInfo = PalletInfo;
		type AccountData = pallet_balances::AccountData<Balance>;
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type SystemWeightInfo = ();
		type SS58Prefix = ConstU16<7>;
		type OnSetCode = ();
		type MaxConsumers = ConstU32<16>;
		type RuntimeTask = ();
		type SingleBlockMigrations = ();
		type MultiBlockMigrator = ();
		type PreInherents = ();
		type PostInherents = ();
		type PostTransactions = ();
	}

	impl pallet_balances::Config for Test {
		type Balance = Balance;
		type RuntimeEvent = RuntimeEvent;
		type DustRemoval = ();
		type ExistentialDeposit = ConstU128<500>;
		type AccountStore = System;
		type MaxLocks = ConstU32<50>;
		type MaxReserves = ();
		type ReserveIdentifier = [u8; 8];
		type WeightInfo = ();
		type FreezeIdentifier = ();
		type MaxFreezes = ();
		type RuntimeHoldReason = ();
		type RuntimeFreezeReason = RuntimeHoldReason;
	}

	impl pallet_issuers::Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type Hashing = BlakeTwo256;
		type Currency = Balances;
		type MaxNameLength = ConstU32<120>;
		type MaxControllers = ConstU32<20>;
		type WeightInfo = crate::weights::SubstrateWeight<Test>;
		type IssuerRegistryDeposit = ConstU128<1_000_000_000_000>;
	}

	pub fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		(pallet_balances::GenesisConfig::<Test> {
			balances: vec![(1, 2_000_000_000_000), (2, 2_000_000_000_000), (3, 2_000_000_000_000)],
		})
			.assimilate_storage(&mut t)
			.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1)); // This is important!
		ext
	}

	#[test]
	fn create_issuer_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(
				IssuersModule::create_issuer(
					RuntimeOrigin::signed(1),
					b"Test Issuer".to_vec(),
					vec![1, 2]
				)
			);

			let hash = <Test as frame_system::Config>::Hashing::hash(&b"Test Issuer"[..]);
			let issuer = crate::Issuers::<Test>::get(hash).unwrap();
			assert_eq!(&issuer.name[..], b"Test Issuer");
			assert_eq!(issuer.controllers, vec![1, 2]);

			System::assert_last_event(
				(Event::IssuerCreated {
					hash,
					issuer_name: issuer.name,
					controllers_identified: issuer.controllers,
				}).into()
			);
		});
	}

	#[test]
	fn create_issuer_fails_with_insufficient_balance() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				IssuersModule::create_issuer(
					RuntimeOrigin::signed(4),
					b"Test Issuer".to_vec(),
					vec![1, 2]
				),
				Error::<Test>::InsufficientBalance
			);
		});
	}

	#[test]
	fn create_issuer_fails_with_too_long_name() {
		new_test_ext().execute_with(|| {
			let too_long_name = vec![b'a'; 121];
			assert_noop!(
				IssuersModule::create_issuer(RuntimeOrigin::signed(1), too_long_name, vec![1, 2]),
				Error::<Test>::IssuerNameTooLong
			);
		});
	}

	#[test]
	fn create_issuer_fails_with_too_many_controllers() {
		new_test_ext().execute_with(|| {
			let too_many_controllers = (1..22).collect::<Vec<_>>();
			assert_noop!(
				IssuersModule::create_issuer(
					RuntimeOrigin::signed(1),
					b"Test Issuer".to_vec(),
					too_many_controllers
				),
				Error::<Test>::TooManyControllers
			);
		});
	}

	#[test]
	fn edit_controllers_works() {
		new_test_ext().execute_with(|| {
			// First create an issuer
			assert_ok!(
				IssuersModule::create_issuer(
					RuntimeOrigin::signed(1),
					b"Test Issuer".to_vec(),
					vec![1, 2]
				)
			);

			let hash = <Test as frame_system::Config>::Hashing::hash(&b"Test Issuer"[..]);

			// Edit controllers
			assert_ok!(
				IssuersModule::edit_controllers(RuntimeOrigin::signed(1), hash, Some(vec![1, 2, 3]))
			);

			let issuer = crate::Issuers::<Test>::get(hash).unwrap();
			let expected_controllers: BoundedVec<u64, ConstU32<20>> = vec![1, 2, 3]
				.try_into()
				.unwrap();
			assert_eq!(issuer.controllers, expected_controllers);

			// Check event
			System::assert_has_event(
				RuntimeEvent::IssuersModule(Event::IssuerUpdated {
					hash,
					issuer_name: issuer.name,
					controllers_identified: expected_controllers,
				})
			);
		});
	}

	#[test]
	fn edit_controllers_fails_when_not_authorized() {
		new_test_ext().execute_with(|| {
			assert_ok!(
				IssuersModule::create_issuer(
					RuntimeOrigin::signed(1),
					b"Test Issuer".to_vec(),
					vec![1, 2]
				)
			);

			let hash = <Test as frame_system::Config>::Hashing::hash(&b"Test Issuer"[..]);

			assert_noop!(
				IssuersModule::edit_controllers(
					RuntimeOrigin::signed(3),
					hash,
					Some(vec![1, 2, 3])
				),
				Error::<Test>::NotAuthorized
			);
		});
	}

	#[test]
	fn edit_controllers_fails_with_too_many_controllers() {
		new_test_ext().execute_with(|| {
			assert_ok!(
				IssuersModule::create_issuer(
					RuntimeOrigin::signed(1),
					b"Test Issuer".to_vec(),
					vec![1, 2]
				)
			);

			let hash = <Test as frame_system::Config>::Hashing::hash(&b"Test Issuer"[..]);
			let too_many_controllers = (1..22).collect::<Vec<_>>();

			assert_noop!(
				IssuersModule::edit_controllers(
					RuntimeOrigin::signed(1),
					hash,
					Some(too_many_controllers)
				),
				Error::<Test>::TooManyControllers
			);
		});
	}
}
