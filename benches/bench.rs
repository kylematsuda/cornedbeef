use cornedbeef::CbHashMap;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
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
    for size in [0, SIZE] {
        group.bench_function(BenchmarkId::new("std", size), bench_new!(StdHashMap, size));
        group.bench_function(BenchmarkId::new("cb", size), bench_new!(CbHashMap, size));
    }
    group.finish();
}

macro_rules! bench_drop {
    ($group:expr, $map:ident, $label:expr, $size:expr) => {
        let mut map = $map::new();

        for i in 0..$size {
            map.insert(i, i.to_string());
        }

        $group.bench_function(BenchmarkId::new($label, $size), |b| {
            b.iter_batched(
                || map.clone(),
                |map| {
                    black_box(map);
                },
                BatchSize::PerIteration,
            )
        });
    };
}

pub fn drop(c: &mut Criterion) {
    let mut group = c.benchmark_group("drop");
    bench_drop!(group, StdHashMap, "std", SIZE);
    bench_drop!(group, CbHashMap, "cb", SIZE);
    group.finish();
}

macro_rules! bench_grow {
    ($group:expr, $map:ident, $label:expr, $it:expr, $len:expr) => {
        $group.bench_function(BenchmarkId::new($label, $len), |b| {
            b.iter_batched_ref(
                || $map::new(),
                |map| {
                    for i in $it {
                        black_box(map.insert(i, [i; $len]));
                    }
                    black_box(map);
                },
                BatchSize::PerIteration,
            )
        });
    };
}

pub fn insert_grow_seq(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_grow_seq");

    {
        const LEN: usize = 1;
        bench_grow!(group, StdHashMap, "std", 0..SIZE, LEN);
        bench_grow!(group, CbHashMap, "cb", 0..SIZE, LEN);
    }

    {
        const LEN: usize = 8;
        bench_grow!(group, StdHashMap, "std", 0..SIZE, LEN);
        bench_grow!(group, CbHashMap, "cb", 0..SIZE, LEN);
    }
    group.finish();
}

pub fn insert_grow_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_grow_random");
    let seq = RandomKeys::new();

    {
        const LEN: usize = 1;
        bench_grow!(group, StdHashMap, "std", seq.take(SIZE), LEN);
        bench_grow!(group, CbHashMap, "cb", seq.take(SIZE), LEN);
    }

    {
        const LEN: usize = 8;
        bench_grow!(group, StdHashMap, "std", seq.take(SIZE), LEN);
        bench_grow!(group, CbHashMap, "cb", seq.take(SIZE), LEN);
    }
    group.finish();
}

macro_rules! bench_reserved {
    ($group:expr, $map:ident, $label:expr, $it:expr, $size:expr, $len:expr) => {
        $group.bench_function(BenchmarkId::new($label, $len), |b| {
            b.iter_batched_ref(
                || $map::with_capacity($size),
                |map| {
                    for i in $it {
                        black_box(map.insert(i, [i; $len]));
                    }
                    black_box(map);
                },
                BatchSize::PerIteration,
            )
        });
    };
}

pub fn insert_reserved(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_reserved_random");
    let seq = RandomKeys::new();

    {
        const LEN: usize = 1;
        bench_reserved!(group, StdHashMap, "std", seq.take(SIZE), SIZE, LEN);
        bench_reserved!(group, CbHashMap, "cb", seq.take(SIZE), SIZE, LEN);
    }

    {
        const LEN: usize = 8;
        bench_reserved!(group, StdHashMap, "std", seq.take(SIZE), SIZE, LEN);
        bench_reserved!(group, CbHashMap, "cb", seq.take(SIZE), SIZE, LEN);
    }
    group.finish();
}

macro_rules! bench_lookup {
    ($group:expr, $map:ident, $label:expr, $size:expr, $len:expr) => {
        let mut map = $map::new();
        let seq = RandomKeys::new();

        for i in seq.take($size) {
            map.insert(i, [i; $len]);
        }

        $group.bench_function(BenchmarkId::new($label, $len), |b| {
            b.iter(|| {
                for i in seq.take($size) {
                    black_box(map.get(&i));
                }
                black_box(&mut map);
            })
        });
    };
}

pub fn lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup");

    {
        const LEN: usize = 1;
        bench_lookup!(group, StdHashMap, "std", SIZE, LEN);
        bench_lookup!(group, CbHashMap, "cb", SIZE, LEN);
    }

    {
        const LEN: usize = 8;
        bench_lookup!(group, StdHashMap, "std", SIZE, LEN);
        bench_lookup!(group, CbHashMap, "cb", SIZE, LEN);
    }

    group.finish();
}

macro_rules! bench_lookup_string {
    ($group:expr, $map:ident, $label:expr, $size:expr, $len:expr) => {
        let mut map = $map::new();
        let seq = RandomKeys::new();
        let keys = seq.take($size).map(|i| i.to_string()).collect::<Vec<_>>();

        for i in &keys {
            map.insert(i.clone(), [i.len(); $len]);
        }

        $group.bench_function(BenchmarkId::new($label, $len), |b| {
            b.iter(|| {
                for i in &keys {
                    black_box(map.get(i));
                }
                black_box(&mut map);
            })
        });
    };
}

pub fn lookup_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup_string");

    {
        const LEN: usize = 1;
        bench_lookup_string!(group, StdHashMap, "std", SIZE, LEN);
        bench_lookup_string!(group, CbHashMap, "cb", SIZE, LEN);
    }

    {
        const LEN: usize = 8;
        bench_lookup_string!(group, StdHashMap, "std", SIZE, LEN);
        bench_lookup_string!(group, CbHashMap, "cb", SIZE, LEN);
    }

    group.finish();
}

macro_rules! bench_lookup_miss {
    ($group:expr, $map:ident, $label:expr, $size:expr, $len:expr) => {
        let mut map = $map::new();
        let mut seq = RandomKeys::new();

        for i in (&mut seq).take($size) {
            map.insert(i, [i; $len]);
        }

        let misses: Vec<_> = (&mut seq).take($size).collect();

        $group.bench_function(BenchmarkId::new($label, $len), |b| {
            b.iter(|| {
                for i in &misses {
                    black_box(map.get(i));
                }
                black_box(&mut map);
            })
        });
    };
}

pub fn lookup_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup_miss");

    {
        const LEN: usize = 1;
        bench_lookup_miss!(group, StdHashMap, "std", SIZE, LEN);
        bench_lookup_miss!(group, CbHashMap, "cb", SIZE, LEN);
    }

    {
        const LEN: usize = 8;
        bench_lookup_miss!(group, StdHashMap, "std", SIZE, LEN);
        bench_lookup_miss!(group, CbHashMap, "cb", SIZE, LEN);
    }

    group.finish();
}

macro_rules! bench_remove {
    ($group:expr, $map:ident, $label:expr, $size:expr, $len:expr) => {
        let seq: Vec<_> = RandomKeys::new().take($size).collect();
        let mut map = $map::new();

        for i in &seq {
            map.insert(i, [i; $len]);
        }

        $group.bench_function(BenchmarkId::new($label, $len), |b| {
            b.iter_batched_ref(
                || map.clone(),
                |map| {
                    for i in &seq {
                        black_box(map.remove(&i));
                    }
                    assert!(map.len() == 0);
                    black_box(map);
                },
                BatchSize::PerIteration,
            )
        });
    };
}

pub fn remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove");

    {
        const LEN: usize = 1;
        bench_remove!(group, StdHashMap, "std", SIZE, LEN);
        bench_remove!(group, CbHashMap, "cb", SIZE, LEN);
    }

    {
        const LEN: usize = 8;
        bench_remove!(group, StdHashMap, "std", SIZE, LEN);
        bench_remove!(group, CbHashMap, "cb", SIZE, LEN);
    }

    group.finish();
}

criterion_group!(
    benches,
    new,
    drop,
    insert_grow_seq,
    insert_grow_random,
    insert_reserved,
    lookup,
    lookup_string,
    lookup_miss,
    remove,
);
criterion_main!(benches);
