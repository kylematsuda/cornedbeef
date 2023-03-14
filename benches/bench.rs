use cornedbeef::CbHashMap;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::HashMap as StdHashMap;

static SIZE: usize = 100_000;

// A random key iterator.
// Copied from rust-lang/hashbrown
#[derive(Clone, Copy)]
struct RandomKeys {
    state: usize,
}

impl RandomKeys {
    fn new() -> Self {
        RandomKeys { state: 0 }
    }
}

impl Iterator for RandomKeys {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        // Add 1 then multiply by some 32 bit prime.
        self.state = self.state.wrapping_add(1).wrapping_mul(3_787_392_781);
        Some(self.state)
    }
}

macro_rules! bench_new {
    ($map:ident, $size:expr) => {
        |b| {
            b.iter(|| {
                let _ = black_box($map::<usize, usize>::with_capacity($size));
            })
        }
    };
}

pub fn new(c: &mut Criterion) {
    let mut group = c.benchmark_group("new");
    for size in [0, 10000] {
        group.bench_function(BenchmarkId::new("std", size), bench_new!(StdHashMap, size));
        group.bench_function(BenchmarkId::new("cb", size), bench_new!(CbHashMap, size));
    }
    group.finish();
}

macro_rules! bench_grow {
    ($map:ident, $it:expr, $len:expr) => {
        |b| {
            let mut map = $map::new();
            b.iter(|| {
                for i in $it {
                    black_box(map.insert(i, [i; $len]));
                }
            })
        }
    };
}

pub fn insert_grow_seq(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_grow_seq");

    {
        const LEN: usize = 1;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_grow!(StdHashMap, 0..SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_grow!(CbHashMap, 0..SIZE, LEN),
        );
    }

    {
        const LEN: usize = 8;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_grow!(StdHashMap, 0..SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_grow!(CbHashMap, 0..SIZE, LEN),
        );
    }
    group.finish();
}

pub fn insert_grow_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_grow_random");
    let seq = RandomKeys::new();

    {
        const LEN: usize = 1;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_grow!(StdHashMap, seq.take(SIZE), LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_grow!(CbHashMap, seq.take(SIZE), LEN),
        );
    }

    {
        const LEN: usize = 8;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_grow!(StdHashMap, seq.take(SIZE), LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_grow!(CbHashMap, seq.take(SIZE), LEN),
        );
    }
    group.finish();
}

macro_rules! bench_reserved {
    ($map:ident, $it:expr, $size:expr, $len:expr) => {
        |b| {
            let mut map = $map::with_capacity($size);
            b.iter(|| {
                for i in $it {
                    black_box(map.insert(i, [i; $len]));
                }
            })
        }
    };
}

pub fn insert_reserved_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_reserved_random");
    let seq = RandomKeys::new();

    {
        const LEN: usize = 1;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_reserved!(StdHashMap, seq.take(SIZE), SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_reserved!(CbHashMap, seq.take(SIZE), SIZE, LEN),
        );
    }

    {
        const LEN: usize = 8;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_reserved!(StdHashMap, seq.take(SIZE), SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_reserved!(CbHashMap, seq.take(SIZE), SIZE, LEN),
        );
    }
    group.finish();
}

macro_rules! bench_lookup {
    ($map:ident, $size:expr, $len:expr) => {
        |b| {
            let mut map = $map::new();
            let seq = RandomKeys::new();

            for i in seq.take($size) {
                map.insert(i, [i; $len]);
            }
            b.iter(|| {
                for i in seq.take($size) {
                    black_box(map.get(&i));
                }
            })
        }
    };
}

pub fn lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup");

    {
        const LEN: usize = 1;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_lookup!(StdHashMap, SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_lookup!(CbHashMap, SIZE, LEN),
        );
    }

    {
        const LEN: usize = 8;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_lookup!(StdHashMap, SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_lookup!(CbHashMap, SIZE, LEN),
        );
    }

    group.finish();
}

macro_rules! bench_lookup_miss {
    ($map:ident, $size:expr, $len:expr) => {
        |b| {
            let mut map = $map::new();
            let mut seq = RandomKeys::new();

            for i in (&mut seq).take($size) {
                map.insert(i, [i; $len]);
            }
            b.iter(|| {
                for i in (&mut seq).take($size) {
                    black_box(map.get(&i));
                }
            })
        }
    };
}

pub fn lookup_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup_miss");

    {
        const LEN: usize = 1;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_lookup_miss!(StdHashMap, SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_lookup_miss!(CbHashMap, SIZE, LEN),
        );
    }

    {
        const LEN: usize = 8;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_lookup_miss!(StdHashMap, SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_lookup_miss!(CbHashMap, SIZE, LEN),
        );
    }

    group.finish();
}

macro_rules! bench_remove {
    ($map:ident, $size:expr, $len:expr) => {
        |b| {
            let mut map = $map::new();
            let seq = RandomKeys::new();

            for i in seq.take($size) {
                map.insert(i, [i; $len]);
            }
            b.iter(|| {
                for i in seq.take($size) {
                    black_box(map.remove(&i));
                }
            })
        }
    };
}

pub fn remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove");

    {
        const LEN: usize = 1;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_remove!(StdHashMap, SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_remove!(CbHashMap, SIZE, LEN),
        );
    }

    {
        const LEN: usize = 8;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_remove!(StdHashMap, SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_remove!(CbHashMap, SIZE, LEN),
        );
    }

    group.finish();
}

macro_rules! bench_remove_miss {
    ($map:ident, $size:expr, $len:expr) => {
        |b| {
            let mut map = $map::new();
            let mut seq = RandomKeys::new();

            for i in (&mut seq).take($size) {
                map.insert(i, [i; $len]);
            }
            b.iter(|| {
                for i in (&mut seq).take($size) {
                    black_box(map.remove(&i));
                }
            })
        }
    };
}

pub fn remove_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_miss");

    {
        const LEN: usize = 1;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_remove_miss!(StdHashMap, SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_remove_miss!(CbHashMap, SIZE, LEN),
        );
    }

    {
        const LEN: usize = 8;
        group.bench_function(
            BenchmarkId::new("std", LEN),
            bench_remove_miss!(StdHashMap, SIZE, LEN),
        );
        group.bench_function(
            BenchmarkId::new("cb", LEN),
            bench_remove_miss!(CbHashMap, SIZE, LEN),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    new,
    insert_grow_seq,
    insert_grow_random,
    insert_reserved_random,
    lookup,
    lookup_miss,
    remove,
    remove_miss,
);
criterion_main!(benches);
