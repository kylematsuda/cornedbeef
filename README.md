# cornedbeef

Writing a hashmap in Rust for learning and fun.

Please see the accompanying blog posts:
- [Part 1: Intro and two naive maps](https://kylematsuda.com/blog/writing_a_hashmap_part_1)
- [Part 2: Swiss Table metadata and MaybeUninit](https://kylematsuda.com/blog/writing_a_hashmap_part_2)
- [Part 3a: SIMD probing](https://kylematsuda.com/blog/writing_a_hashmap_part_3a)
- [Part 3b: Exception safety](https://kylematsuda.com/blog/writing_a_hashmap_part_3b)

The design was inspired by Google's [Swiss Tables map](https://abseil.io/about/design/swisstables) and Rust's `std::collections::HashMap` aka [hashbrown](https://crates.io/crates/hashbrown) (based on Swiss Tables).

This repo contains 6 iterations on a hashmap, building from a naive design toward (a simplified) Swiss Tables:
- `first::Map`: separate chaining using `std::collections::LinkedList`
- `second::Map`: open addressing (quadratic probing)
- `third::Map`: open addressing with Swiss tables metadata
- `fourth::Map`: same as `third` but using `std::mem::MaybeUninit` as an optimization
- `fifth::Map`: same as `fourth` but adding SIMD probing
- `sixth::Map` (unfinished): same as `fifth` but putting the metadata and backing storage in the same allocation (with a lot of `unsafe`)
