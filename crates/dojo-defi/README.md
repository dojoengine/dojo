# Gradual Dutch Auctions (GDA)

## Introduction

Gradual Dutch Auctions (GDA) enable efficient sales of assets without relying on liquid markets. GDAs offer a novel solution for selling both non-fungible tokens (NFTs) and fungible tokens through discrete and continuous mechanisms.

## Discrete GDA

### Motivation

Discrete GDAs are perfect for selling NFTs in integer quantities. They offer an efficient way to conduct bulk purchases through a sequence of Dutch auctions.

### Mechanism

The process involves holding virtual Dutch auctions for each token, allowing for efficient clearing of batches. Price decay is exponential, controlled by a decay constant, and the starting price increases by a fixed scale factor.

### Calculating Batch Purchase Prices

Calculations can be made efficiently for purchasing a batch of auctions, following a given price function.

## Continuous GDA

### Motivation

Continuous GDAs offer a mechanism for selling fungible tokens, allowing for constant rate emissions over time.

### Mechanism

The process works by incrementally making more assets available for sale, splitting sales into an infinite sequence of auctions. Various price functions, including exponential decay, can be applied.

### Calculating Purchase Prices

It's possible to compute the purchase price for any quantity of tokens gas-efficiently, using specific mathematical expressions.

## How to Use

### Discrete Gradual Dutch Auction

The `DiscreteGDA` structure represents a Gradual Dutch Auction using discrete time steps. Here's how you can use it:

#### Creating a Discrete GDA

```rust
let gda = DiscreteGDA {
    sold: Fixed::new_unscaled(0),
    initial_price: Fixed::new_unscaled(100, false),
    scale_factor: FixedTrait::new_unscaled(11, false) / FixedTrait::new_unscaled(10, false), // 1.1
    decay_constant: FixedTrait::new_unscaled(1, false) / FixedTrait::new_unscaled(2, false), // 0.5,
};
```

#### Calculating the Purchase Price

You can calculate the purchase price for a specific quantity at a given time using the `purchase_price` method.

```rust
let time_since_start = FixedTrait::new(2, false); // 2 days since the start, it must be scaled to avoid overflow.
let quantity = FixedTrait::new_unscaled(5, false); // Quantity to purchase
let price = gda.purchase_price(time_since_start, quantity);
```

### Continuous Gradual Dutch Auction

The `ContinuousGDA` structure represents a Gradual Dutch Auction using continuous time steps.

#### Creating a Continuous GDA

```rust
let gda = ContinuousGDA {
    initial_price: FixedTrait::new_unscaled(1000, false),
    emission_rate: FixedTrait::ONE(),
    decay_constant: FixedTrait::new_unscaled(1, false) / FixedTrait::new_unscaled(2, false),
};
```

#### Calculating the Purchase Price

Just like with the discrete version, you can calculate the purchase price for a specific quantity at a given time using the `purchase_price` method.

```rust
let time_since_last = FixedTrait::new(1, false); // 1 day since the last purchase, it must be scaled to avoid overflow.
let quantity = FixedTrait::new_unscaled(3, false); // Quantity to purchase
let price = gda.purchase_price(time_since_last, quantity);
```

---

These examples demonstrate how to create instances of the `DiscreteGDA` and `ContinuousGDA` structures, and how to utilize their `purchase_price` methods to calculate the price for purchasing specific quantities at given times.

You'll need to include the `cubit` crate in your project to work with the `Fixed` type and mathematical operations like `exp` and `pow`. Make sure to follow the respective documentation for additional details and proper integration into your project.

## Conclusion

GDAs present a powerful tool for selling both fungible and non-fungible tokens in various contexts. They offer efficient, flexible solutions for asset sales, opening doors to innovative applications beyond traditional markets.

# Variable Rate GDAs (VRGDAs)

## Overview

Variable Rate GDAs (VRGDAs) enable the selling of tokens according to a custom schedule, raising or lowering prices based on the sales pace. VRGDA is a generalization of the GDA mechanism.

## How to Use

### Linear Variable Rate Gradual Dutch Auction (LinearVRGDA)

The `LinearVRGDA` struct represents a linear auction where the price decays based on the target price, decay constant, and per-time-unit rate.

#### Creating a LinearVRGDA instance

```rust
const _69_42: u128 = 1280572973596917000000;
const _0_31: u128 = 5718490662849961000;

let auction = LinearVRGDA {
    target_price: FixedTrait::new(_69_42, false),
    decay_constant: FixedTrait::new(_0_31, false),
    per_time_unit: FixedTrait::new_unscaled(2, false),
};
```

#### Calculating Target Sale Time

```rust
let target_sale_time = auction.get_target_sale_time(sold_quantity);
```

#### Calculating VRGDA Price

```rust
let price = auction.get_vrgda_price(time_since_start, sold_quantity);
```

### Logistic Variable Rate Gradual Dutch Auction (LogisticVRGDA)

The `LogisticVRGDA` struct represents an auction where the price decays according to a logistic function, based on the target price, decay constant, max sellable quantity, and time scale.

#### Creating a LogisticVRGDA instance

```rust
const MAX_SELLABLE: u128 = 6392;
const _0_0023: u128 = 42427511369531970;

let auction = LogisticVRGDA {
    target_price: FixedTrait::new(_69_42, false),
    decay_constant: FixedTrait::new(_0_31, false),
    max_sellable: FixedTrait::new_unscaled(MAX_SELLABLE, false),
    time_scale: FixedTrait::new(_0_0023, false),
};
```

#### Calculating Target Sale Time

```rust
let target_sale_time = auction.get_target_sale_time(sold_quantity);
```

#### Calculating VRGDA Price

```rust
let price = auction.get_vrgda_price(time_since_start, sold_quantity);
```

Make sure to import the required dependencies at the beginning of your Cairo file:

```rust
use cubit::f128::types::fixed::{Fixed, FixedTrait};
```

These examples show you how to create instances of both `LinearVRGDA` and `LogisticVRGDA` and how to use their methods to calculate the target sale time and VRGDA price.

## Conclusion

VRGDAs offer a flexible way to issue NFTs on nearly any schedule, enabling seamless purchases at any time.
