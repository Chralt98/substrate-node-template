0.

This line is a piece of Rust code that makes use of conditional compilation and attributes.

Let's break it down:

    #![cfg_attr()] is a compile-time configuration attribute that conditionally includes the attribute specified by the second argument, based on whether the condition specified by the first argument is true.

    not(feature = "std") is a condition that checks if the "std" feature is not enabled. In Rust, features are a way of enabling optional functionality in a crate.

    no_std is an attribute that changes the Rust environment to not link the Rust standard library (std) by default.

So, the line #![cfg_attr(not(feature = "std"), no_std)] will include the no_std attribute (i.e., it will not link the Rust standard library) if the "std" feature is not enabled.

Why? For using WebAssembly, since this works only without the standard Rust library.
WebAssembly is a platform-agnostic binary format, meaning it will run the exact instructions across whatever machine it operates on.
Blockchains need determinacy to have reliable state transition updates across all nodes in the peer-to-peer network without forcing every peer to run the same hardware.
Wasm is an excellent fit for reliability among the diverse set of machines. Wasm is both efficient and fast.
The efficiency means that it can be uploaded onto the chain as a blob of code without causing too much state bloat while keeping its ability to execute at near-native speeds.

- uploaded onto the chain is useful for forkless upgrades

1.  

So, pub use pallet::_; is re-exporting all items from the pallet module.
This means that another module that uses the module which contains pub use pallet::_; can access all the public items from pallet directly,
as if they were defined in the current module. This could be useful for organizing your code and managing visibility in a large project.

2.

The mock module, typically present under the #[cfg(test)] attribute,
is used to create simulated versions ('mocks') of your code's dependencies for isolated unit testing.
These mocks provide predictable behavior, enabling controlled and consistent tests.

3.

This code enables a benchmarking module when the runtime-benchmarks feature is active. 
The benchmarking module, typically used in Rust, contains functions and logic to measure the performance 
/ weight of specific code segments under the "runtime-benchmarks" feature flag, aiding in performance tuning and optimization.
We need this to measure processing time to know beforehand how many transaction can be put into a block.

4.

CheckedDiv and Zero are called trait bounds. They restrict the type Balance to types that implement these traits.

5.

CheckedDiv can't be removed because it ensures Balance type implements safe division functionality, returning an option that handles divide-by-zero cases gracefully.

6.

CheckedDiv + Zero means that the Balance type has to support safe division (CheckedDiv) and also be able to represent the value zero (Zero), which is used as a fallback in case of a division by zero.

7.

This code defines a helper function DefaultMarketCounter that provides a default value for a MarketId in the context of a Substrate's pallet. 
The #[pallet::type_value] attribute signifies that the function is a provider for a default value. 
Here, it initializes the MarketId counter to 1u128 in the blockchain's runtime storage when the pallet is first added to the runtime.

8.

It should be benchmarked properly empirically rather than adding reads and writes manually.

9.

When tests are written for `on_initialize` the tests would fail if a market with a state other than `Active` sits inside `MarketIdsPerCloseBlock`. 
This is useful to ensure previous assumptions about the internal state management process.

10.

The weight of `on_finalize_impl` should be added to `on_initialize` weight.

- `on_finalize` exceeds the maximum block limit (is executed in any case)

11.
Why do we check `can_reserve` here? Why not just using `reserve` alone?

The function T::Currency::can_reserve(&who, bond) is used to check if the account identified by who has enough balance to reserve the amount specified by bond. 
It's a common practice to check the operation beforehand to avoid consuming unnecessary computational resources for the operations between `can_reserve` and `reserve`. 

12.

Why do we like to store the bond in the market? We could have just used `T::CreatorBond::get()` for the unreserve call.

Assume a creator bond is reserved.
Say you want to cut the bond in half, because you think it's to high.
You change this by upgrading the chain, but now, 
when you want to unreserve the bond with the configuration value, 
the market creator gets only half of his initial bond back.
So, better to store the bond amounts inside the storage 
than just depending on configuration values.

13.

Why could we want to reserve the bond here?

To incentivise the extrinsic to clear the storage if all operations are done.

14.

What does `Pays::No` mean? Why is it only placed here?

No transaction fees required. 
Because we can rely on the DestroyOrigin (which is root origin) that this transaction will likely not be spammed.
Otherwise a Denial of Service attack would disturb the operation of the blockchain.

15.

What does `DispatchClass::Operational` mean? Why is it only placed here?

Unlike normal dispatches, which represent usage of network capabilities, operational dispatches are those that provide network capabilities. 
Operational dispatches can consume the entire weight limit of a block. 
They are not bound by the AvailableBlockRatio. 
Dispatches in this class are given maximum priority and are exempt from paying the length_fee.

16.

Why didn't I use `ensure_root(origin)?;` here?

A custom origin inside the config trait allows 
a flexible configuration of the origin outside of the pallet, but in the runtime.
So, if another project includes the pallet the inner code doesn't have to be changed,
only the pallet configuration.

17.

What could be done instead of `Pays::Yes` to get the same effect?

Just leave it out. It uses `Pays::Yes` as the default, if it is not written explicitly.

18.

What does `DispatchClass::Normal` mean?

Dispatches in this class represent normal user-triggered transactions. 
These types of dispatches only consume a portion of a block's total weight limit. 
For information about the maximum portion of a block that can be consumed for normal dispatches,
see AvailableBlockRatio. Normal dispatches are sent to the transaction pool.

It it also the default, so it doesn't need to be written.

19.

Why could this `transactional` be useful here? Why is not used in other calls?

In this dispatchable call there are two fallible transfer operations.
If the first transfer operation executes flawlessly, but the second does fail, 
without the `transactional` marker, the first transfer operation modified the storage already.

If we want the behaviour to only execute both transfers or none of them, then we use `transactional`.
It's like an atomic dispatchable call then.

20.

What could the users do, if the oracle is not honest? What is done at Zeitgeist to solve this problem?

The user is unable to do anything for this current implementaion.

Zeitgeist solves this with a dispute system. I personally worked on a decentralized court system.
This court system let jurors vote on the correct outcomes.

21.

Why don't I use a question mark operator here?

I wanted to surpress a possible error to fulfill the underlying reason for this call.
To clear the storage in any case. The debug_assert would also trigger in the tests.

22.

What could be the purpose of this function?

Get the probability / share of one outcome price from the combined total of all outcome prices together.

So the market thinks that the outcome with the highest price has the highest probability to be the winning outcome.
The probability is between zero and one (or 0% and 100%).

23.

Imagine this trait is defined outside of this pallet. Why could this be useful?

Another pallet can include this application programming interface and access the storage of our template pallet.

This is useful to transfer data between two different pallets.