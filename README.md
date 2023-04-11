# cornedbeef

Writing a hashmap in Rust for learning and fun.

Please see the accompanying blog posts:
- [Part 1: Intro and two naive maps](https://kylematsuda.com/blog/writing_a_hashmap_part_1)
- [Part 2: Swiss Table metadata and MaybeUninit](https://kylematsuda.com/blog/writing_a_hashmap_part_2)
- [Part 3a: SIMD probing](https://kylematsuda.com/blog/writing_a_hashmap_part_3a)
- [Part 3b: Exception safety](https://kylematsuda.com/blog/writing_a_hashmap_part_3b)
- [Part 3c: Resizing with SIMD](https://kylematsuda.com/blog/writing_a_hashmap_part_3c)

The design was inspired by Google's [Swiss Tables map](https://abseil.io/about/design/swisstables) and Rust's `std::collections::HashMap` aka [hashbrown](https://crates.io/crates/hashbrown) (based on Swiss Tables).

This repo contains 6 iterations on a hashmap, building from a naive design toward (a simplified) Swiss Tables:
- `first::Map`: separate chaining using `std::collections::LinkedList`
- `second::Map`: open addressing (quadratic probing)
- `third::Map`: open addressing with Swiss tables metadata
- `fourth::Map`: same as `third` but using `std::mem::MaybeUninit` as an optimization
- `fifth::Map`: same as `fourth` but adding SIMD probing
- `sixth::Map` (unfinished): same as `fifth` but putting the metadata and backing storage in the same allocation (with a lot of `unsafe`)

# Speed comparison with `std`

These are done with the benchmarks in `/benches`.

Reported times are for my laptop (Intel i7-10750H, 32 GB RAM, Fedora 37).

| Benchmark name        | `std` runtime (ms) | `fifth::Map` runtime (ms) | Ratio |
| ---                   | ---           | ---                   | ---   |
| insert_grow_seq 1     |	3.85          | 4.58                  |	1.19  |
| insert_grow_seq 8	    | 4.81	        | 5.79                  |	1.20  |
| insert_grow_random 1  |	3.95	        | 4.75	                | 1.20  |
| insert_grow_random 8  |	4.87	        | 5.87	                | 1.21  |
| insert_reserved 1	    | 2.03	        | 2.26	                | 1.11  |
| insert_reserved 8	    | 2.47	        | 2.74	                | 1.11  |
| lookup 1              |	2.14	        | 2.61	                | 1.22  |
| lookup 8	            | 3.11	        | 4.3	                  | 1.38  |
| lookup string 1	      | 4.32	        | 4.61	                | 1.07  |
| lookup string 8	      | 7.07	        | 7.17	                | 1.01  |
| lookup miss 1	        | 1.84	        | 1.99	                | 1.08  |
| lookup miss 8	        | 2.02	        | 2.26	                | 1.12  |
| remove 1	            | 3.07	        | 3.49	                | 1.14  |
| remove 8	            | 5.00	        | 5.60	                | 1.12  |
