# embed-struct

Rust library for embedding data structures.

[![CI Status][ci-badge]][ci-url]
[![MIT licensed][mit-badge]][mit-url]

[ci-badge]: https://github.com/aegistudio/embed-struct/actions/workflows/rust.yml/badge.svg
[ci-url]: https://github.com/aegistudio/embed-struct/actions/workflows/rust.yml
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/aegistudio/embed-struct/blob/master/LICENSE


## Overview

It's quite common to amalgamate multiple data structures in order to achieve complex functionalities:

- Consider a LRU cache, it can be implemented by a HashMap from the keys to the cache items, plus a queue to track the recent visits to the cache items.
- Consider a snake game, it can be implemented by a 2D grid with a queue of snake body embedded into it.
- ...

Hand-rolling these embedded data structure might be repetitive and error prone, so many frameworks or libraries provide facilities to embed them. For example, in the linux kernel, they have macros like `list_add*`, `list_del*` to embed doubly linked lists, and macros like `rb_link_node`, `rb_erase*` to embed rbtrees.

And if you want to do the same embedding stuffs in rust, this is what the library aims to provide.
