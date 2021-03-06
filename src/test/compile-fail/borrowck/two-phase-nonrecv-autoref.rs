// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// revisions: lxl nll g2p
//[lxl]compile-flags: -Z borrowck=mir -Z two-phase-borrows
//[nll]compile-flags: -Z borrowck=mir -Z two-phase-borrows -Z nll
//[g2p]compile-flags: -Z borrowck=mir -Z two-phase-borrows -Z nll -Z two-phase-beyond-autoref

// This is a test checking that when we limit two-phase borrows to
// method receivers, we do not let other kinds of auto-ref to leak
// through.
//
// The g2p revision illustrates the "undesirable" behavior you would
// otherwise observe without limiting the phasing to autoref on method
// receivers (namely, in many cases demonstrated below, the error
// would not arise).

// (If we revise the compiler or this test so that the g2p revision
// passes, turn the `rustc_attrs` feature back on and tag the `fn
// main` with `#[rustc_error]` so that this remains a valid
// compile-fail test.)
//
// #![feature(rustc_attrs)]

use std::ops::{Index, IndexMut};
use std::ops::{AddAssign, SubAssign, MulAssign, DivAssign, RemAssign};
use std::ops::{BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign};

// This is case outlined by Niko that we want to ensure we reject
// (at least initially).

fn foo(x: &mut u32, y: u32) {
    *x += y;
}

fn deref_coercion(x: &mut u32) {
    foo(x, *x);
    //[lxl]~^ ERROR cannot use `*x` because it was mutably borrowed [E0503]
    //[nll]~^^ ERROR cannot use `*x` because it was mutably borrowed [E0503]
}

// While adding a flag to adjustments (indicating whether they
// should support two-phase borrows, here are the cases I
// encountered:
//
// - [x] Resolving overloaded_call_traits (call, call_mut, call_once)
// - [x] deref_coercion (shown above)
// - [x] coerce_unsized e.g. `&[T; n]`, `&mut [T; n] -> &[T]`,
//                      `&mut [T; n] -> &mut [T]`, `&Concrete -> &Trait`
// - [x] Method Call Receivers (the case we want to support!)
// - [x] ExprIndex and ExprUnary Deref; only need to handle coerce_index_op
// - [x] overloaded_binops

fn overloaded_call_traits() {
    // Regarding overloaded call traits, note that there is no
    // scenario where adding two-phase borrows should "fix" these
    // cases, because either we will resolve both invocations to
    // `call_mut` (in which case the inner call requires a mutable
    // borrow which will conflict with the outer reservation), or we
    // will resolve both to `call` (which will just work, regardless
    // of two-phase borrow support), or we will resolve both to
    // `call_once` (in which case the inner call requires moving the
    // receiver, invalidating the outer call).

    fn twice_ten_sm<F: FnMut(i32) -> i32>(f: &mut F) {
        f(f(10));
        //[lxl]~^     ERROR cannot borrow `*f` as mutable more than once at a time
        //[lxl]~|     ERROR cannot borrow `*f` as mutable more than once at a time
        //[nll]~^^^   ERROR cannot borrow `*f` as mutable more than once at a time
        //[nll]~|     ERROR cannot borrow `*f` as mutable more than once at a time
        //[g2p]~^^^^^ ERROR cannot borrow `*f` as mutable more than once at a time
    }
    fn twice_ten_si<F: Fn(i32) -> i32>(f: &mut F) {
        f(f(10));
    }
    fn twice_ten_so<F: FnOnce(i32) -> i32>(f: Box<F>) {
        f(f(10));
        //[lxl]~^   ERROR use of moved value: `*f`
        //[nll]~^^  ERROR use of moved value: `*f`
        //[g2p]~^^^ ERROR use of moved value: `*f`
    }

    fn twice_ten_om(f: &mut FnMut(i32) -> i32) {
        f(f(10));
        //[lxl]~^     ERROR cannot borrow `*f` as mutable more than once at a time
        //[lxl]~|     ERROR cannot borrow `*f` as mutable more than once at a time
        //[nll]~^^^   ERROR cannot borrow `*f` as mutable more than once at a time
        //[nll]~|     ERROR cannot borrow `*f` as mutable more than once at a time
        //[g2p]~^^^^^ ERROR cannot borrow `*f` as mutable more than once at a time
    }
    fn twice_ten_oi(f: &mut Fn(i32) -> i32) {
        f(f(10));
    }
    fn twice_ten_oo(f: Box<FnOnce(i32) -> i32>) {
        f(f(10));
        //[lxl]~^             ERROR cannot move a value of type
        //[lxl]~^^            ERROR cannot move a value of type
        //[lxl]~^^^           ERROR use of moved value: `*f`
        //[nll]~^^^^          ERROR cannot move a value of type
        //[nll]~^^^^^         ERROR cannot move a value of type
        //[nll]~^^^^^^        ERROR cannot move a value of type
        //[nll]~^^^^^^^       ERROR cannot move a value of type
        //[nll]~^^^^^^^^      ERROR use of moved value: `*f`
        //[g2p]~^^^^^^^^^     ERROR cannot move a value of type
        //[g2p]~^^^^^^^^^^    ERROR cannot move a value of type
        //[g2p]~^^^^^^^^^^^   ERROR cannot move a value of type
        //[g2p]~^^^^^^^^^^^^  ERROR cannot move a value of type
        //[g2p]~^^^^^^^^^^^^^ ERROR use of moved value: `*f`
    }

    twice_ten_sm(&mut |x| x + 1);
    twice_ten_si(&mut |x| x + 1);
    twice_ten_so(Box::new(|x| x + 1));
    twice_ten_om(&mut |x| x + 1);
    twice_ten_oi(&mut |x| x + 1);
    twice_ten_oo(Box::new(|x| x + 1));
}

trait TwoMethods {
    fn m(&mut self, x: i32) -> i32 { x + 1 }
    fn i(&self, x: i32) -> i32 { x + 1 }
}

struct T;

impl TwoMethods for T { }

struct S;

impl S {
    fn m(&mut self, x: i32) -> i32 { x + 1 }
    fn i(&self, x: i32) -> i32 { x + 1 }
}

impl TwoMethods for [i32; 3] { }

fn double_access<X: Copy>(m: &mut [X], s: &[X]) {
    m[0] = s[1];
}

fn coerce_unsized() {
    let mut a = [1, 2, 3];

    // This is not okay.
    double_access(&mut a, &a);
    //[lxl]~^   ERROR cannot borrow `a` as immutable because it is also borrowed as mutable [E0502]
    //[nll]~^^  ERROR cannot borrow `a` as immutable because it is also borrowed as mutable [E0502]
    //[g2p]~^^^ ERROR cannot borrow `a` as immutable because it is also borrowed as mutable [E0502]

    // But this is okay.
    a.m(a.i(10));
}

struct I(i32);

impl Index<i32> for I {
    type Output = i32;
    fn index(&self, _: i32) -> &i32 {
        &self.0
    }
}

impl IndexMut<i32> for I {
    fn index_mut(&mut self, _: i32) -> &mut i32 {
        &mut self.0
    }
}

fn coerce_index_op() {
    let mut i = I(10);
    i[i[3]] = 4;
    //[lxl]~^  ERROR cannot borrow `i` as immutable because it is also borrowed as mutable [E0502]
    //[nll]~^^ ERROR cannot borrow `i` as immutable because it is also borrowed as mutable [E0502]

    i[3] = i[4];

    i[i[3]] = i[4];
    //[lxl]~^  ERROR cannot borrow `i` as immutable because it is also borrowed as mutable [E0502]
    //[nll]~^^ ERROR cannot borrow `i` as immutable because it is also borrowed as mutable [E0502]
}

struct A(i32);

macro_rules! trivial_binop {
    ($Trait:ident, $m:ident) => {
        impl $Trait<i32> for A { fn $m(&mut self, rhs: i32) { self.0 = rhs; } }
    }
}

trivial_binop!(AddAssign, add_assign);
trivial_binop!(SubAssign, sub_assign);
trivial_binop!(MulAssign, mul_assign);
trivial_binop!(DivAssign, div_assign);
trivial_binop!(RemAssign, rem_assign);
trivial_binop!(BitAndAssign, bitand_assign);
trivial_binop!(BitOrAssign, bitor_assign);
trivial_binop!(BitXorAssign, bitxor_assign);
trivial_binop!(ShlAssign, shl_assign);
trivial_binop!(ShrAssign, shr_assign);

fn overloaded_binops() {
    let mut a = A(10);
    a += a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
    a -= a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
    a *= a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
    a /= a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
    a &= a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
    a |= a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
    a ^= a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
    a <<= a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
    a >>= a.0;
    //[lxl]~^   ERROR cannot use `a.0` because it was mutably borrowed
    //[nll]~^^  ERROR cannot use `a.0` because it was mutably borrowed
}

fn main() {

    // As a reminder, this is the basic case we want to ensure we handle.
    let mut v = vec![1, 2, 3];
    v.push(v.len());

    // (as a rule, pnkfelix does not like to write tests with dead code.)

    deref_coercion(&mut 5);
    overloaded_call_traits();


    let mut s = S;
    s.m(s.i(10));

    let mut t = T;
    t.m(t.i(10));

    coerce_unsized();
    coerce_index_op();
    overloaded_binops();
}
