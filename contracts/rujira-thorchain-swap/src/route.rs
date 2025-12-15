use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Uint128, Uint256};
use std::ops::{Div, Mul};

#[cw_serde]
pub enum Route {
    // A -> RUNE (single hop)
    AR {
        a: Uint128,
        r: Uint128,
    },
    // RUNE -> B (single hop)
    RB {
        r: Uint128,
        b: Uint128,
    },
    // A -> R -> B (two hop)
    ARB {
        a: Uint128,
        r1: Uint128,
        r2: Uint128,
        b: Uint128,
    },
}

impl Route {
    pub fn return_balance(&self) -> Uint128 {
        match *self {
            Route::AR { r, .. } => r,
            Route::RB { b, .. } => b,
            Route::ARB { b, .. } => b,
        }
    }
    pub fn swap(&self, x: Uint128) -> Uint128 {
        match *self {
            Route::AR { a, r } => calculate_return(x, a, r),
            Route::RB { r, b } => calculate_return(x, r, b),
            Route::ARB { a, r1, r2, b } => calculate_return(calculate_return(x, a, r1), r2, b),
        }
    }

    pub fn size(&self, s: u32) -> Uint128 {
        match *self {
            Route::AR { a, .. } => size_single(a, s),
            Route::RB { r, .. } => size_single(r, s),
            Route::ARB { a, r1, r2, .. } => size_dual(a, r1, r2, s),
        }
    }
}

fn size_single(xx: Uint128, s_bps: u32) -> Uint128 {
    if s_bps == 0 {
        return Uint128::zero();
    }
    xx.multiply_ratio(s_bps, 10_000u128)
}

// See https://gitlab.com/thorchain/thornode/-/blob/develop/x/thorchain/helpers.go#L212-221
fn size_dual(
    oa: Uint128, // asset balance in offer pool (input side of hop1)
    or: Uint128, // rune balance in offer pool (output side of hop1)
    ar: Uint128, // rune balance in ask pool (input side of hop2)
    s_bps: u32,
) -> Uint128 {
    if s_bps == 0 {
        return Uint128::zero();
    }
    // Find smallest value in rune and convert to offer asset
    size_single(or, s_bps)
        .min(size_single(ar, s_bps))
        .multiply_ratio(oa, or)
}

/// swap_out for CLP: y = (x * X * Y) / (x + X)^2
fn calculate_return(x: Uint128, xx: Uint128, yy: Uint128) -> Uint128 {
    if x.is_zero() || xx.is_zero() || yy.is_zero() {
        return Uint128::zero();
    }
    let x = Uint256::from(x);
    let xx = Uint256::from(xx);
    let yy = Uint256::from(yy);
    x.mul(xx)
        .mul(yy)
        // integer division floors, which is what we want for conservative quoting
        .div((x + xx).pow(2))
        .try_into()
        .unwrap_or(Uint128::zero())
}
