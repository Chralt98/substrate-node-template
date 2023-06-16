// TODO: Why do we need the following line?
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// TODO: Why is it useful to write `pub` here?
pub use pallet::*;

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_runtime::traits::{CheckedDiv, CheckedSub, One, Zero};
use frame_support::sp_runtime::Saturating;
use frame_support::traits::{
	BalanceStatus, Currency, ExistenceRequirement, ReservableCurrency, WithdrawReasons,
};
use frame_support::PalletId;
use scale_info::TypeInfo;

// TODO: Why do we typically have a `mock` module?
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// TODO: What is this and what does it do?
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::*;

pub type MarketId = u128;

#[derive(Decode, Encode, MaxEncodedLen, TypeInfo, Clone, Debug, PartialEq, Eq)]
pub enum MarketStatus {
	Active,
	Closed,
	Reported,
	Redeemed,
}

#[derive(Decode, Encode, MaxEncodedLen, TypeInfo, Clone, Debug, PartialEq, Eq)]
pub struct Market<AccountId, BlockNumber, Balance> {
	pub creator: AccountId,
	pub bond: Balance,
	pub data: [u8; 32],
	pub end: BlockNumber,
	pub oracle: AccountId,
	pub oracle_outcome_report: Option<u8>,
	pub status: MarketStatus,
}

#[derive(Decode, Encode, MaxEncodedLen, TypeInfo, Clone, Debug, PartialEq, Eq)]
pub struct Outcome<AccountId, Balance> {
	pub owner: AccountId,
	pub data: [u8; 32],
	pub price: Balance,
}

// TODO: What are `CheckedDiv + Zero` called?
// TODO: Why can't we just remove `CheckedDiv`?
// TODO: What does `CheckedDiv + Zero` mean for `Balance`?
impl<AccountId, Balance: CheckedDiv + Zero> Outcome<AccountId, Balance> {
	pub fn p(&self, t: Balance) -> Balance {
		self.price.checked_div(&t).unwrap_or(Zero::zero())
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	pub type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;
	pub type MarketOf<T> = Market<AccountIdOf<T>, BlockNumberFor<T>, BalanceOf<T>>;
	pub type OutcomesOf<T> =
		BoundedVec<Outcome<AccountIdOf<T>, BalanceOf<T>>, <T as Config>::MaxOutcomes>;

	pub type CacheSize = frame_support::pallet_prelude::ConstU32<64>;

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: ReservableCurrency<Self::AccountId>;

		#[pallet::constant]
		type CreatorBond: Get<BalanceOf<Self>>;

		#[pallet::constant]
		type MarketCreatorClearStorageTime: Get<Self::BlockNumber>;

		#[pallet::constant]
		type MaxOutcomes: Get<u32>;

		#[pallet::constant]
		type MinMarketPeriod: Get<Self::BlockNumber>;

		type PalletId: Get<PalletId>;

		type WeightInfo: WeightInfo;
	}

	// TODO: What does this do?
	#[pallet::type_value]
	pub fn DefaultMarketCounter<T: Config>() -> MarketId {
		1u128
	}

	#[pallet::storage]
	#[pallet::getter(fn market_counter)]
	pub type MarketCounter<T: Config> =
		StorageValue<_, MarketId, ValueQuery, DefaultMarketCounter<T>>;

	#[pallet::storage]
	pub type Markets<T: Config> =
		StorageMap<_, Blake2_128Concat, MarketId, MarketOf<T>, OptionQuery>;

	#[pallet::storage]
	pub type Outcomes<T: Config> =
		StorageMap<_, Blake2_128Concat, MarketId, OutcomesOf<T>, ValueQuery>;

	#[pallet::storage]
	pub type MarketIdsPerCloseBlock<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::BlockNumber,
		BoundedVec<MarketId, CacheSize>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		MarketCreated { market_id: MarketId, creator: T::AccountId },
		MarketDestroyed { market_id: MarketId },
		OutcomeBought { market_id: MarketId, outcome_index: u8, buyer: T::AccountId },
		MarketsToCloseNextBlock { market_ids: Vec<MarketId> },
		MarketClosed { market_id: MarketId },
		MarketReported { market_id: MarketId, oracle_report_outcome: u8 },
		MarketRedeemed { market_id: MarketId, winner_outcome: u8, winner: T::AccountId },
		HighestOutcome { market_id: MarketId, highest_outcome: Option<u8> },
	}

	#[pallet::error]
	pub enum Error<T> {
		OutcomesStorageOverflow,
		MarketCounterStorageOverflow,
		MarketIdsPerCloseBlockStorageOverflow,
		InvalidOutcomeIndex,
		MarketNotFound,
		PriceTooLow,
		OutcomeAmountTooLow,
		InsufficientBuyerBalance,
		BelowMinMarketPeriod,
		MarketNotActive,
		CallerNotOracle,
		OutcomeAlreadyReported,
		OutcomeNotReportedYet,
		InvalidMarketStatus,
		InsufficientCreatorBalance,
		OnlyMarketCreatorAllowedYet,
		Invalid,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			let mut total_weight = Weight::zero();

			// TODO What comes to your mind when you see the `total_weight` calculation?
			total_weight = total_weight.saturating_add(T::DbWeight::get().reads(1));
			let market_ids = <MarketIdsPerCloseBlock<T>>::get(n);
			for market_id in market_ids {
				total_weight = total_weight.saturating_add(T::DbWeight::get().reads(1));
				if let Some(mut market) = <Markets<T>>::get(market_id) {
					// TODO Why could this `debug_assert!` be useful here?
					debug_assert!(market.status == MarketStatus::Active, "MarketIdsPerCloseBlock should only contain active markets! Invalid market id: {:?}", market_id);
					market.status = MarketStatus::Closed;
					total_weight = total_weight.saturating_add(T::DbWeight::get().writes(1));
					<Markets<T>>::insert(market_id, market);
					Self::deposit_event(Event::MarketClosed { market_id });
				};
			}
			total_weight = total_weight.saturating_add(T::DbWeight::get().writes(1));
			<MarketIdsPerCloseBlock<T>>::remove(n);

			total_weight
		}

		fn on_finalize(n: T::BlockNumber) {
			// TODO What should be kept in mind, when using `on_finalize`?
			Self::on_finalize_impl(n);
		}

		fn on_idle(_n: T::BlockNumber, mut remaining_weight: Weight) -> Weight {
			if let Some(count) =
				remaining_weight.checked_div_per_component(&T::WeightInfo::do_something())
			{
				// assume this `emit_highest_outcomes` has `do_something` weight
				let consumed_weight = Self::emit_highest_outcomes(count as usize);
				remaining_weight = remaining_weight.saturating_sub(consumed_weight);
			}

			remaining_weight
		}

		fn integrity_test() {
			assert!(
				T::MaxOutcomes::get() <= u8::MAX as u32,
				"The maximum of outcomes should be less than 255!"
			);
			assert!(
				!T::MinMarketPeriod::get().is_zero(),
				"The minimum market period should not be zero!"
			);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::do_something())]
		pub fn create_market(
			origin: OriginFor<T>,
			#[pallet::compact] outcome_amount: u8,
			end: T::BlockNumber,
			oracle: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let bond = T::CreatorBond::get();
			// TODO: Why do we check `can_reserve` here? Why not just using `reserve` alone?
			ensure!(T::Currency::can_reserve(&who, bond), Error::<T>::InsufficientCreatorBalance);

			ensure!(!outcome_amount.is_zero(), Error::<T>::OutcomeAmountTooLow);

			let now = <frame_system::Pallet<T>>::block_number();
			use frame_support::sp_runtime::Saturating;
			ensure!(
				end.saturating_sub(now) >= T::MinMarketPeriod::get(),
				Error::<T>::BelowMinMarketPeriod
			);

			let market_id = Self::market_counter();
			let new_counter =
				market_id.checked_add(1).ok_or(Error::<T>::MarketCounterStorageOverflow)?;

			debug_assert!(!Markets::<T>::contains_key(market_id));

			let mut outcomes = Outcomes::<T>::get(market_id);
			for i in 0..outcome_amount {
				let outcome = Outcome { owner: who.clone(), data: [i; 32], price: Zero::zero() };
				outcomes.try_push(outcome).map_err(|_| Error::<T>::OutcomesStorageOverflow)?;
			}

			let market = Market {
				creator: who.clone(),
				bond,
				data: Default::default(),
				end,
				oracle,
				oracle_outcome_report: None,
				status: MarketStatus::Active,
			};

			MarketIdsPerCloseBlock::<T>::try_mutate(end, |prev_market_ids| -> DispatchResult {
				prev_market_ids
					.try_push(market_id)
					.map_err(|_| <Error<T>>::MarketIdsPerCloseBlockStorageOverflow)?;
				Ok(())
			})?;

			// TODO Why could we want to reserve the bond here?
			T::Currency::reserve(&who, bond)?;

			<Outcomes<T>>::insert(market_id, outcomes);
			<Markets<T>>::insert(market_id, market);
			<MarketCounter<T>>::put(new_counter);

			Self::deposit_event(Event::MarketCreated { market_id, creator: who });

			Ok(())
		}

		// TODO What does `Pays::No` mean? Why is it only placed here?
		// TODO What does `DispatchClass::Operational` mean? Why is it only placed here?
		#[pallet::call_index(1)]
		#[pallet::weight((T::WeightInfo::do_something(), DispatchClass::Operational, Pays::No))]
		pub fn destroy_market(
			origin: OriginFor<T>,
			#[pallet::compact] market_id: MarketId,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(Markets::<T>::contains_key(market_id), Error::<T>::MarketNotFound);

			Markets::<T>::remove(market_id);
			Outcomes::<T>::remove(market_id);

			Self::deposit_event(Event::MarketDestroyed { market_id });

			Ok(())
		}

		// TODO: What could be done instead of `Pays::Yes` to get the same effect? 
		// TODO: What does `DispatchClass::Normal` mean?
		// TODO: Why could this `transactional` be useful here? Why is not used in other calls?
		#[pallet::call_index(2)]
		#[pallet::weight((T::WeightInfo::do_something(), DispatchClass::Normal, Pays::Yes))]
		#[frame_support::transactional]
		pub fn buy_outcome(
			origin: OriginFor<T>,
			#[pallet::compact] market_id: MarketId,
			#[pallet::compact] outcome_index: u8,
			#[pallet::compact] price: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let buyer_balance = T::Currency::free_balance(&who);
			let new_buyer_balance =
				buyer_balance.checked_sub(&price).ok_or(Error::<T>::InsufficientBuyerBalance)?;
			T::Currency::ensure_can_withdraw(
				&who,
				price,
				WithdrawReasons::TRANSFER,
				new_buyer_balance,
			)?;

			let market = <Markets<T>>::get(market_id).ok_or(Error::<T>::MarketNotFound)?;
			ensure!(market.status == MarketStatus::Active, Error::<T>::MarketNotActive);

			let mut outcomes = Outcomes::<T>::get(market_id);
			let mut outcome = outcomes
				.get_mut(outcome_index as usize)
				.ok_or(Error::<T>::InvalidOutcomeIndex)?;
			ensure!(outcome.price < price, Error::<T>::PriceTooLow);

			let market_account = Self::market_account(market_id);

			let refund_previous_buyer = || -> DispatchResult {
				let previous_buyer = &outcome.owner;
				T::Currency::transfer(
					&market_account,
					&previous_buyer,
					outcome.price,
					ExistenceRequirement::AllowDeath,
				)?;
				Ok(())
			};

			if !outcome.price.is_zero() {
				refund_previous_buyer()?;
			}

			T::Currency::transfer(&who, &market_account, price, ExistenceRequirement::AllowDeath)?;

			outcome.owner = who.clone();

			Self::deposit_event(Event::OutcomeBought { market_id, outcome_index, buyer: who });

			Ok(())
		}

		// TODO: What could the users do, if the oracle is not honest?
		// TODO: What is done at Zeitgeist to solve this problem?
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::do_something())]
		pub fn report_as_oracle(
			origin: OriginFor<T>,
			#[pallet::compact] market_id: MarketId,
			#[pallet::compact] outcome_index: u8,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut market = <Markets<T>>::get(market_id).ok_or(Error::<T>::MarketNotFound)?;

			ensure!(market.oracle_outcome_report.is_none(), Error::<T>::OutcomeAlreadyReported);
			ensure!(market.status == MarketStatus::Closed, Error::<T>::InvalidMarketStatus);
			ensure!(market.oracle == who, Error::<T>::CallerNotOracle);

			market.oracle_outcome_report = Some(outcome_index);
			market.status = MarketStatus::Reported;
			<Markets<T>>::insert(market_id, market);

			Self::deposit_event(Event::MarketReported {
				market_id,
				oracle_report_outcome: outcome_index,
			});

			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::do_something())]
		pub fn redeem(
			origin: OriginFor<T>,
			#[pallet::compact] market_id: MarketId,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let mut market = <Markets<T>>::get(market_id).ok_or(Error::<T>::MarketNotFound)?;

			let reported_index =
				market.oracle_outcome_report.ok_or(Error::<T>::OutcomeNotReportedYet)?;

			let outcomes = <Outcomes<T>>::get(market_id);
			let outcome =
				outcomes.get(reported_index as usize).ok_or(Error::<T>::InvalidOutcomeIndex)?;

			let winner = &outcome.owner;

			let market_account = Self::market_account(market_id);
			let reward = T::Currency::free_balance(&market_account);
			T::Currency::transfer(
				&market_account,
				winner,
				reward,
				ExistenceRequirement::AllowDeath,
			)?;

			market.status = MarketStatus::Redeemed;
			<Markets<T>>::insert(market_id, market);

			Self::deposit_event(Event::MarketRedeemed {
				market_id,
				winner_outcome: reported_index,
				winner: winner.clone(),
			});

			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::do_something())]
		pub fn clear_storage(
			origin: OriginFor<T>,
			#[pallet::compact] market_id: MarketId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let market = <Markets<T>>::get(market_id).ok_or(Error::<T>::MarketNotFound)?;
			ensure!(market.status == MarketStatus::Redeemed, Error::<T>::InvalidMarketStatus);

			let now = <frame_system::Pallet<T>>::block_number();
			let end = market.end;
			if now.saturating_sub(end) <= T::MarketCreatorClearStorageTime::get() {
				ensure!(market.creator == who, Error::<T>::OnlyMarketCreatorAllowedYet);
			}

			if who != market.creator {
				// TODO Why don't I use a question mark operator here?
				let res = T::Currency::repatriate_reserved(
					&market.creator,
					&who,
					market.bond,
					BalanceStatus::Free,
				);
				debug_assert!(res.is_ok());
			} else {
				T::Currency::unreserve(&market.creator, market.bond);
			}

			<Markets<T>>::remove(market_id);
			<Outcomes<T>>::remove(market_id);

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn on_finalize_impl(n: T::BlockNumber) {
			let next_block = n.saturating_add(One::one());
			let market_ids_to_close_next_block = <MarketIdsPerCloseBlock<T>>::get(next_block);
			if market_ids_to_close_next_block.is_empty() {
				return;
			}
			Self::deposit_event(Event::MarketsToCloseNextBlock {
				market_ids: market_ids_to_close_next_block.into_inner(),
			});
		}

		// TODO What could be the purpose of this function?
		pub fn g(o: OutcomesOf<T>, i: u8) -> Result<BalanceOf<T>, DispatchError> {
			use frame_support::sp_runtime::SaturatedConversion;
			let t = o
				.iter()
				.map(|j| j.price.saturated_into::<u128>())
				.sum::<u128>()
				.saturated_into::<BalanceOf<T>>();
			let u = o.get(i as usize).ok_or(Error::<T>::Invalid)?;
			Ok(u.p(t))
		}

		pub fn emit_highest_outcomes(count: usize) -> Weight {
			let total_weight = Weight::zero();
			for (market_id, outcomes) in <Outcomes<T>>::iter().take(count) {
				let highest_outcome = outcomes
					.iter()
					.enumerate()
					.max_by_key(|(_, outcome)| outcome.price)
					.map(|(index, _)| index as u8);
				Self::deposit_event(Event::HighestOutcome { market_id, highest_outcome });

				total_weight.saturating_add(T::WeightInfo::do_something());
			}
			total_weight
		}

		pub fn market_account(market_id: MarketId) -> AccountIdOf<T> {
			use frame_support::sp_runtime::traits::AccountIdConversion;
			T::PalletId::get().into_sub_account_truncating(market_id)
		}
	}

	impl<T> MarketApi for Pallet<T>
	where
		T: Config,
	{
		type MarketId = MarketId;
		type AccountId = T::AccountId;
		type Balance = BalanceOf<T>;
		type BlockNumber = T::BlockNumber;

		fn get_market(market_id: &Self::MarketId) -> Result<(Weight, MarketOf<T>), DispatchError> {
			let weight = T::DbWeight::get().reads(1);
			let market = <Markets<T>>::get(market_id).ok_or(Error::<T>::MarketNotFound)?;
			Ok((weight, market))
		}
	}
}

// TODO: Imagine this trait is defined outside of this pallet. Why could this be useful?
trait MarketApi {
	type MarketId;
	type AccountId;
	type Balance;
	type BlockNumber;

	fn get_market(
		market_id: &Self::MarketId,
	) -> Result<
		(
			frame_support::pallet_prelude::Weight,
			Market<Self::AccountId, Self::BlockNumber, Self::Balance>,
		),
		frame_support::pallet_prelude::DispatchError,
	>;
}
